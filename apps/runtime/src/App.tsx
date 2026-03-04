import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { motion, AnimatePresence } from "framer-motion";
import { Sidebar } from "./components/Sidebar";
import { ChatView } from "./components/ChatView";
import { InstallDialog } from "./components/InstallDialog";
import { SettingsView } from "./components/SettingsView";
import { PackagingView } from "./components/packaging/PackagingView";
import { NewSessionLanding } from "./components/NewSessionLanding";
import { ExpertsView } from "./components/experts/ExpertsView";
import { EmployeeHubView } from "./components/employees/EmployeeHubView";
import {
  ExpertCreatePayload,
  ExpertCreateView,
  ExpertPreviewPayload,
  ExpertPreviewResult,
} from "./components/experts/ExpertCreateView";
import { SkillManifest, ModelConfig, SessionInfo, ImRoleDispatchRequest, Message, AgentEmployee, UpsertAgentEmployeeInput } from "./types";

type MainView = "start-task" | "experts" | "experts-new" | "packaging" | "employees";
type SkillAction = "refresh" | "delete" | "check-update" | "update";
const BUILTIN_GENERAL_SKILL_ID = "builtin-general";
const BUILTIN_EMPLOYEE_CREATOR_SKILL_ID = "builtin-employee-creator";
const MODEL_SETUP_HINT_DISMISSED_KEY = "workclaw:model-setup-hint-dismissed";
const INITIAL_MODEL_SETUP_COMPLETED_KEY = "workclaw:initial-model-setup-completed";
const EMPLOYEE_CREATOR_STARTER_PROMPT =
  "请帮我创建一个新的智能体员工。先问我 1-2 个关键问题，再给出配置草案，确认后再执行创建。";

const QUICK_MODEL_PRESETS: Array<{
  key: string;
  label: string;
  name: string;
  api_format: string;
  base_url: string;
  model_name: string;
}> = [
  {
    key: "openai",
    label: "OpenAI",
    name: "OpenAI",
    api_format: "openai",
    base_url: "https://api.openai.com/v1",
    model_name: "gpt-4o-mini",
  },
  {
    key: "anthropic",
    label: "Claude (Anthropic)",
    name: "Claude",
    api_format: "anthropic",
    base_url: "https://api.anthropic.com/v1",
    model_name: "claude-3-5-haiku-20241022",
  },
  {
    key: "deepseek",
    label: "DeepSeek",
    name: "DeepSeek",
    api_format: "openai",
    base_url: "https://api.deepseek.com/v1",
    model_name: "deepseek-chat",
  },
];

type ImBridgeSessionContext = {
  threadId: string;
  roleName: string;
  streamBuffer: string;
  streamSentCount: number;
  waitingForAnswer: boolean;
  streamFlushTimer: ReturnType<typeof setTimeout> | null;
  lastStreamFlushAt: number;
  streamFlushInFlight: boolean;
};

function extractErrorMessage(error: unknown, fallback: string): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message || fallback;
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message?: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }
  return fallback;
}

function extractDuplicateSkillName(error: unknown): string | null {
  const message = extractErrorMessage(error, "");
  const prefix = "DUPLICATE_SKILL_NAME:";
  if (!message.includes(prefix)) {
    return null;
  }
  return message.split(prefix)[1]?.trim() || null;
}

function getDefaultSkillId(skillList: SkillManifest[]): string | null {
  const builtin = skillList.find((item) => item.id === BUILTIN_GENERAL_SKILL_ID);
  if (builtin) {
    return builtin.id;
  }
  return skillList[0]?.id ?? null;
}

export default function App() {
  const [skills, setSkills] = useState<SkillManifest[]>([]);
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [selectedSkillId, setSelectedSkillId] = useState<string | null>(null);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [showInstall, setShowInstall] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [activeMainView, setActiveMainView] = useState<MainView>("start-task");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [newSessionPermissionMode, setNewSessionPermissionMode] = useState<"default" | "accept_edits" | "unrestricted">("accept_edits");
  const [creatingSession, setCreatingSession] = useState(false);
  const [createSessionError, setCreateSessionError] = useState<string | null>(null);
  const [creatingExpertSkill, setCreatingExpertSkill] = useState(false);
  const [expertCreateError, setExpertCreateError] = useState<string | null>(null);
  const [expertSavedPath, setExpertSavedPath] = useState<string | null>(null);
  const [pendingImportDir, setPendingImportDir] = useState<string | null>(null);
  const [retryingExpertImport, setRetryingExpertImport] = useState(false);
  const [skillActionState, setSkillActionState] = useState<{ skillId: string; action: SkillAction } | null>(null);
  const [clawhubUpdateStatus, setClawhubUpdateStatus] = useState<Record<string, { hasUpdate: boolean; message: string }>>({});
  const [employees, setEmployees] = useState<AgentEmployee[]>([]);
  const [selectedEmployeeId, setSelectedEmployeeId] = useState<string | null>(null);
  const [imManagedSessionIds, setImManagedSessionIds] = useState<string[]>([]);
  const [dismissedModelSetupHint, setDismissedModelSetupHint] = useState(() => {
    if (typeof window === "undefined") {
      return false;
    }
    try {
      return window.localStorage.getItem(MODEL_SETUP_HINT_DISMISSED_KEY) === "1";
    } catch {
      return false;
    }
  });
  const [hasCompletedInitialModelSetup, setHasCompletedInitialModelSetup] = useState(() => {
    if (typeof window === "undefined") {
      return false;
    }
    try {
      return window.localStorage.getItem(INITIAL_MODEL_SETUP_COMPLETED_KEY) === "1";
    } catch {
      return false;
    }
  });
  const [showQuickModelSetup, setShowQuickModelSetup] = useState(false);
  const [quickModelPresetKey, setQuickModelPresetKey] = useState(QUICK_MODEL_PRESETS[0].key);
  const [quickModelForm, setQuickModelForm] = useState(() => ({
    name: QUICK_MODEL_PRESETS[0].name,
    api_format: QUICK_MODEL_PRESETS[0].api_format,
    base_url: QUICK_MODEL_PRESETS[0].base_url,
    model_name: QUICK_MODEL_PRESETS[0].model_name,
    api_key: "",
  }));
  const [quickModelSaving, setQuickModelSaving] = useState(false);
  const [quickModelTesting, setQuickModelTesting] = useState(false);
  const [quickModelTestResult, setQuickModelTestResult] = useState<boolean | null>(null);
  const [quickModelError, setQuickModelError] = useState("");
  const [pendingInitialMessage, setPendingInitialMessage] = useState<{
    sessionId: string;
    message: string;
  } | null>(null);
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  function navigate(view: MainView) {
    setActiveMainView(view);
    if (typeof window !== "undefined") {
      window.location.hash = `/${view}`;
    }
  }

  useEffect(() => {
    loadSkills();
    loadModels();
    loadEmployees();
    if (typeof window !== "undefined" && window.location.hash) {
      const raw = window.location.hash.replace(/^#\//, "");
      if (raw === "experts" || raw === "experts-new" || raw === "packaging" || raw === "start-task" || raw === "employees") {
        setActiveMainView(raw);
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (models.length === 0) {
      return;
    }
    setHasCompletedInitialModelSetup(true);
    setDismissedModelSetupHint(false);
    if (typeof window === "undefined") {
      return;
    }
    try {
      window.localStorage.setItem(INITIAL_MODEL_SETUP_COMPLETED_KEY, "1");
      window.localStorage.removeItem(MODEL_SETUP_HINT_DISMISSED_KEY);
    } catch {
      // ignore
    }
  }, [models.length]);

  useEffect(() => {
    if (
      typeof window === "undefined" ||
      !(window as unknown as { __TAURI_INTERNALS__?: { transformCallback?: unknown } })
        .__TAURI_INTERNALS__?.transformCallback
    ) {
      return;
    }
    const seen = new Set<string>();
    const sessionContexts = new Map<string, ImBridgeSessionContext>();
    const STREAM_CHUNK_SIZE = 120;
    const STREAM_FLUSH_INTERVAL_MS = 1200;

    const markImManagedSession = (sessionId: string) => {
      setImManagedSessionIds((prev) => {
        if (prev.includes(sessionId)) return prev;
        return [...prev, sessionId];
      });
    };

    const scheduleImStreamFlush = (sessionId: string, delayMs: number) => {
      const ctx = sessionContexts.get(sessionId);
      if (!ctx || ctx.streamFlushTimer) return;
      const safeDelay = Math.max(20, delayMs);
      ctx.streamFlushTimer = setTimeout(() => {
        const current = sessionContexts.get(sessionId);
        if (!current) return;
        current.streamFlushTimer = null;
        void flushImStream(sessionId);
      }, safeDelay);
    };

    const sendTextToFeishu = async (threadId: string, text: string) => {
      if (!threadId.trim() || !text.trim()) return;
      await invoke("send_feishu_text_message", {
        chatId: threadId,
        text: text.slice(0, 1800),
        appId: null,
        appSecret: null,
        sidecarBaseUrl: null,
      });
    };

    const flushImStream = async (
      sessionId: string,
      options?: { force?: boolean }
    ) => {
      const ctx = sessionContexts.get(sessionId);
      if (!ctx) return;
      if (ctx.streamFlushInFlight) return;
      const force = Boolean(options?.force);
      const chunk = ctx.streamBuffer.trim();
      if (!chunk) return;
      if (!force) {
        const elapsed = Date.now() - ctx.lastStreamFlushAt;
        if (elapsed < STREAM_FLUSH_INTERVAL_MS) {
          scheduleImStreamFlush(sessionId, STREAM_FLUSH_INTERVAL_MS - elapsed);
          return;
        }
      }
      if (ctx.streamFlushTimer) {
        clearTimeout(ctx.streamFlushTimer);
        ctx.streamFlushTimer = null;
      }
      ctx.streamBuffer = "";
      ctx.streamFlushInFlight = true;
      ctx.lastStreamFlushAt = Date.now();
      try {
        if (chunk.length <= 1800) {
          await sendTextToFeishu(ctx.threadId, `${ctx.roleName}: ${chunk}`);
          ctx.streamSentCount += 1;
          return;
        }
        let start = 0;
        while (start < chunk.length) {
          const part = chunk.slice(start, start + 1800);
          await sendTextToFeishu(ctx.threadId, `${ctx.roleName}: ${part}`);
          ctx.streamSentCount += 1;
          start += 1800;
        }
      } finally {
        const latest = sessionContexts.get(sessionId);
        if (!latest) return;
        latest.streamFlushInFlight = false;
        if (latest.streamBuffer.trim().length > 0) {
          const elapsed = Date.now() - latest.lastStreamFlushAt;
          const delayMs = Math.max(0, STREAM_FLUSH_INTERVAL_MS - elapsed);
          scheduleImStreamFlush(sessionId, delayMs);
        }
      }
    };

    const unlistenDispatchPromise = listen<ImRoleDispatchRequest>("im-role-dispatch-request", async ({ payload }) => {
      const key = `${payload.session_id}|${payload.role_id}|${payload.prompt}`;
      if (seen.has(key)) return;
      seen.add(key);

      const existing = sessionContexts.get(payload.session_id);
      const ctx: ImBridgeSessionContext = {
        threadId: payload.thread_id,
        roleName: payload.role_name || payload.role_id,
        streamBuffer: existing?.streamBuffer ?? "",
        streamSentCount: 0,
        waitingForAnswer: existing?.waitingForAnswer ?? false,
        streamFlushTimer: existing?.streamFlushTimer ?? null,
        lastStreamFlushAt: existing?.lastStreamFlushAt ?? 0,
        streamFlushInFlight: existing?.streamFlushInFlight ?? false,
      };
      sessionContexts.set(payload.session_id, ctx);
      markImManagedSession(payload.session_id);

      try {
        if (ctx.waitingForAnswer) {
          ctx.waitingForAnswer = false;
          await invoke("answer_user_question", { answer: payload.prompt });
        } else {
          await invoke("send_message", {
            sessionId: payload.session_id,
            userMessage: payload.prompt,
          });
        }

        await flushImStream(payload.session_id, { force: true });
        if (ctx.streamSentCount === 0) {
          const messages = await invoke<Message[]>("get_messages", {
            sessionId: payload.session_id,
          });
          const latestAssistant = [...messages]
            .reverse()
            .find((m) => m.role === "assistant" && m.content?.trim().length > 0);
          if (latestAssistant) {
            await sendTextToFeishu(
              ctx.threadId,
              `${ctx.roleName}: ${latestAssistant.content.slice(0, 1800)}`
            );
          }
        }
      } catch (e) {
        console.error("IM 分发执行失败:", e);
      } finally {
        setTimeout(() => seen.delete(key), 30_000);
      }
    });

    const unlistenStreamPromise = listen<{
      session_id: string;
      token: string;
      done: boolean;
      sub_agent?: boolean;
    }>("stream-token", ({ payload }) => {
      const ctx = sessionContexts.get(payload.session_id);
      if (!ctx) return;
      if (payload.done) {
        void flushImStream(payload.session_id, { force: true });
        return;
      }
      ctx.streamBuffer += payload.token || "";
      if (ctx.streamBuffer.length >= STREAM_CHUNK_SIZE) {
        void flushImStream(payload.session_id);
      } else {
        scheduleImStreamFlush(payload.session_id, STREAM_FLUSH_INTERVAL_MS);
      }
    });

    const unlistenAskUserPromise = listen<{
      session_id: string;
      question: string;
      options: string[];
    }>("ask-user-event", ({ payload }) => {
      const ctx = sessionContexts.get(payload.session_id);
      if (!ctx) return;
      ctx.waitingForAnswer = true;
      const optionsText = payload.options?.length ? `\n可选项：${payload.options.join(" / ")}` : "";
      void (async () => {
        await flushImStream(payload.session_id, { force: true });
        await sendTextToFeishu(
          ctx.threadId,
          `${ctx.roleName}: ${payload.question}${optionsText}\n请直接回复你的选择或补充信息。`
        );
      })();
    });

    return () => {
      sessionContexts.forEach((ctx) => {
        if (ctx.streamFlushTimer) {
          clearTimeout(ctx.streamFlushTimer);
          ctx.streamFlushTimer = null;
        }
      });
      unlistenDispatchPromise.then((fn) => fn());
      unlistenStreamPromise.then((fn) => fn());
      unlistenAskUserPromise.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (selectedSkillId) {
      loadSessions(selectedSkillId);
    } else {
      setSessions([]);
      setSelectedSessionId(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedSkillId]);

  async function loadSkills(): Promise<SkillManifest[]> {
    const list = await invoke<SkillManifest[]>("list_skills");
    setSkills(list);
    setSelectedSkillId((prev) => {
      if (prev && list.some((item) => item.id === prev)) {
        return prev;
      }
      return getDefaultSkillId(list);
    });
    return list;
  }

  async function loadModels() {
    const list = await invoke<ModelConfig[]>("list_model_configs");
    setModels(list);
  }

  async function loadEmployees(): Promise<AgentEmployee[]> {
    try {
      const raw = await invoke<AgentEmployee[] | null>("list_agent_employees");
      const list = Array.isArray(raw) ? raw : [];
      setEmployees(list);
      setSelectedEmployeeId((prev) => {
        if (prev && list.some((e) => e.id === prev)) return prev;
        return list.find((e) => e.is_default)?.id ?? list[0]?.id ?? null;
      });
      return list;
    } catch {
      setEmployees([]);
      return [];
    }
  }

  async function loadSessions(skillId: string) {
    try {
      const list = await invoke<SessionInfo[]>("get_sessions", { skillId });
      setSessions(list);
    } catch (e) {
      console.error("加载会话列表失败:", e);
      setSessions([]);
    }
  }

  async function handleCreateSession(initialMessage = "") {
    const modelId = models[0]?.id;
    if (!selectedSkillId || !modelId || creatingSession) return;

    setCreatingSession(true);
    setCreateSessionError(null);
    try {
      const selectedEmployee = employees.find((e) => e.id === selectedEmployeeId);
      const chosenSkill = selectedSkillId || selectedEmployee?.primary_skill_id || BUILTIN_GENERAL_SKILL_ID;
      const id = await invoke<string>("create_session", {
        skillId: chosenSkill,
        modelId,
        workDir: selectedEmployee?.default_work_dir || "",
        employeeId: selectedEmployee?.employee_id || selectedEmployee?.role_id || "",
        permissionMode: newSessionPermissionMode,
      });
      const firstMessage = initialMessage.trim();
      if (selectedSkillId) await loadSessions(selectedSkillId);
      setSelectedSessionId(id);

      if (firstMessage) {
        // 由 ChatView 挂载后再自动发送，避免事件监听竞态导致“无响应”。
        setPendingInitialMessage({ sessionId: id, message: firstMessage });
      }
    } catch (e) {
      console.error("创建会话失败:", e);
      setCreateSessionError("创建会话失败，请稍后重试");
    } finally {
      setCreatingSession(false);
    }
  }

  async function handleDeleteSession(sessionId: string) {
    try {
      await invoke("delete_session", { sessionId });
      if (selectedSessionId === sessionId) setSelectedSessionId(null);
      if (selectedSkillId) await loadSessions(selectedSkillId);
    } catch (e) {
      console.error("删除会话失败:", e);
    }
  }

  // 搜索会话（300ms debounce）
  function handleSearchSessions(query: string) {
    if (searchTimerRef.current) {
      clearTimeout(searchTimerRef.current);
    }
    if (!selectedSkillId) return;

    if (!query.trim()) {
      // 搜索词为空时恢复完整会话列表
      searchTimerRef.current = setTimeout(() => {
        loadSessions(selectedSkillId!);
      }, 100);
      return;
    }

    searchTimerRef.current = setTimeout(async () => {
      try {
        const results = await invoke<SessionInfo[]>("search_sessions", {
          skillId: selectedSkillId,
          query: query.trim(),
        });
        setSessions(results);
      } catch (e) {
        console.error("搜索会话失败:", e);
      }
    }, 300);
  }

  // 导出会话为 Markdown 文件
  async function handleExportSession(sessionId: string) {
    try {
      const md = await invoke<string>("export_session", { sessionId });
      const filePath = await save({
        defaultPath: "session-export.md",
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (filePath) {
        await invoke("write_export_file", { path: filePath, content: md });
      }
    } catch (e) {
      console.error("导出会话失败:", e);
    }
  }

  // 安装 Skill 后自动切换并创建新会话
  async function handleInstalled(skillId: string, options?: { createSession?: boolean }) {
    await loadSkills();
    setSelectedSkillId(skillId);
    if (options?.createSession === false) {
      return;
    }
    const modelId = models[0]?.id;
    if (modelId) {
      try {
        const sessionId = await invoke<string>("create_session", {
          skillId,
          modelId,
          workDir: "",
          employeeId: "",
          permissionMode: newSessionPermissionMode,
        });
        const sessions = await invoke<SessionInfo[]>("get_sessions", { skillId });
        setSessions(sessions);
        setSelectedSessionId(sessionId);
      } catch (e) {
        console.error("自动创建会话失败:", e);
      }
    }
  }

  async function handlePickSkillDirectory() {
    const dir = await open({ directory: true, title: "选择技能保存目录" });
    if (!dir || typeof dir !== "string") return null;
    return dir;
  }

  async function handleCreateExpertSkill(payload: ExpertCreatePayload) {
    setCreatingExpertSkill(true);
    setExpertCreateError(null);
    setExpertSavedPath(null);
    setPendingImportDir(null);
    try {
      const skillDir = await invoke<string>("create_local_skill", {
        name: payload.name,
        description: payload.description,
        whenToUse: payload.whenToUse,
        targetDir: payload.targetDir ?? null,
      });
      setExpertSavedPath(skillDir);
      setPendingImportDir(skillDir);

      try {
        const importResult = await invoke<{ manifest: SkillManifest }>("import_local_skill", {
          dirPath: skillDir,
        });
        await loadSkills();
        if (importResult?.manifest?.id) {
          setSelectedSkillId(importResult.manifest.id);
        }
        setExpertSavedPath(null);
        setPendingImportDir(null);
        navigate("experts");
      } catch (importError) {
        const duplicateName = extractDuplicateSkillName(importError);
        if (duplicateName) {
          setExpertCreateError(`技能名称冲突：已存在「${duplicateName}」（文件已保存到：${skillDir}）`);
          return;
        }
        const message = extractErrorMessage(importError, "导入失败，请稍后重试。");
        setExpertCreateError(`${message}（文件已保存到：${skillDir}）`);
        return;
      }
    } catch (e) {
      console.error("创建专家技能失败:", e);
      setExpertCreateError(extractErrorMessage(e, "创建失败，请检查目录权限后重试。"));
    } finally {
      setCreatingExpertSkill(false);
    }
  }

  async function handleRetryExpertImport() {
    if (!pendingImportDir || retryingExpertImport) return;
    setRetryingExpertImport(true);
    setExpertCreateError(null);
    try {
      const importResult = await invoke<{ manifest: SkillManifest }>("import_local_skill", {
        dirPath: pendingImportDir,
      });
      await loadSkills();
      if (importResult?.manifest?.id) {
        setSelectedSkillId(importResult.manifest.id);
      }
      setPendingImportDir(null);
      setExpertSavedPath(null);
      navigate("experts");
    } catch (e) {
      const duplicateName = extractDuplicateSkillName(e);
      if (duplicateName) {
        setExpertCreateError(`技能名称冲突：已存在「${duplicateName}」（文件已保存到：${pendingImportDir}）`);
        return;
      }
      const message = extractErrorMessage(e, "导入失败，请稍后重试。");
      setExpertCreateError(`${message}（文件已保存到：${pendingImportDir}）`);
    } finally {
      setRetryingExpertImport(false);
    }
  }

  async function handleRefreshLocalSkill(skillId: string) {
    if (skillActionState) return;
    setSkillActionState({ skillId, action: "refresh" });
    try {
      await invoke("refresh_local_skill", { skillId });
      await loadSkills();
    } catch (e) {
      console.error("刷新本地技能失败:", e);
    } finally {
      setSkillActionState(null);
    }
  }

  async function handleDeleteSkill(skillId: string) {
    if (skillActionState) return;
    setSkillActionState({ skillId, action: "delete" });
    try {
      await invoke("delete_skill", { skillId });
      if (selectedSkillId === skillId) {
        setSelectedSessionId(null);
      }
      await loadSkills();
    } catch (e) {
      console.error("移除技能失败:", e);
    } finally {
      setSkillActionState(null);
    }
  }

  async function handleCheckClawhubUpdate(skillId: string) {
    if (skillActionState) return;
    setSkillActionState({ skillId, action: "check-update" });
    try {
      const result = await invoke<{ has_update: boolean; message: string }>("check_clawhub_skill_update", {
        skillId,
      });
      setClawhubUpdateStatus((prev) => ({
        ...prev,
        [skillId]: {
          hasUpdate: result.has_update,
          message: result.message,
        },
      }));
    } catch (e) {
      console.error("检查 ClawHub 更新失败:", e);
      setClawhubUpdateStatus((prev) => ({
        ...prev,
        [skillId]: {
          hasUpdate: false,
          message: "检查失败，请稍后重试",
        },
      }));
    } finally {
      setSkillActionState(null);
    }
  }

  async function handleUpdateClawhubSkill(skillId: string) {
    if (skillActionState) return;
    setSkillActionState({ skillId, action: "update" });
    try {
      const result = await invoke<{ manifest: SkillManifest }>("update_clawhub_skill", { skillId });
      await loadSkills();
      if (result?.manifest?.id) {
        setSelectedSkillId(result.manifest.id);
      }
      setClawhubUpdateStatus((prev) => ({
        ...prev,
        [skillId]: {
          hasUpdate: false,
          message: "已更新到最新版本",
        },
      }));
    } catch (e) {
      console.error("更新 ClawHub 技能失败:", e);
      setClawhubUpdateStatus((prev) => ({
        ...prev,
        [skillId]: {
          hasUpdate: true,
          message: "更新失败，请稍后重试",
        },
      }));
    } finally {
      setSkillActionState(null);
    }
  }

  async function handleInstallFromLibrary(slug: string) {
    try {
      const result = await invoke<{ manifest: SkillManifest; missing_mcp: string[] }>("install_clawhub_skill", {
        slug,
        githubUrl: null,
      });
      await loadSkills();
      if (result?.manifest?.id) {
        setSelectedSkillId(result.manifest.id);
      }
    } catch (e) {
      const duplicateName = extractDuplicateSkillName(e);
      if (duplicateName) {
        throw new Error(`技能名称冲突：已存在「${duplicateName}」，请先重命名后再安装。`);
      }
      throw e;
    }
  }

  const handleRenderExpertPreview = useCallback(
    async (payload: ExpertPreviewPayload): Promise<ExpertPreviewResult> => {
      const result = await invoke<{ markdown: string; save_path: string }>(
        "render_local_skill_preview",
        {
          name: payload.name,
          description: payload.description,
          whenToUse: payload.whenToUse,
          targetDir: payload.targetDir ?? null,
        }
      );

      return {
        markdown: result.markdown,
        savePath: result.save_path,
      };
    },
    []
  );

  const handleSessionRefresh = useCallback(() => {
    if (selectedSkillId) {
      loadSessions(selectedSkillId);
      if (selectedSkillId === BUILTIN_EMPLOYEE_CREATOR_SKILL_ID) {
        void loadEmployees();
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedSkillId]);

  const handleSkillInstalledFromChat = useCallback(async (_skillId: string) => {
    await loadSkills();
  }, []);

  function handleOpenStartTask() {
    const mainEmployee = employees.find((e) => e.is_default) ?? employees[0];
    if (mainEmployee) {
      setSelectedEmployeeId(mainEmployee.id);
      if (mainEmployee.primary_skill_id) {
        setSelectedSkillId(mainEmployee.primary_skill_id);
      }
    }
    setSelectedSkillId((prev) => {
      if (prev && skills.some((item) => item.id === prev)) {
        return prev;
      }
      return getDefaultSkillId(skills);
    });
    navigate("start-task");
  }

  async function handleSaveEmployee(input: UpsertAgentEmployeeInput) {
    await invoke<string>("upsert_agent_employee", { input });
    const latest = await loadEmployees();
    const targetEmployeeId = (input.employee_id || input.role_id || "").trim().toLowerCase();
    const target = input.id
      ? latest.find((e) => e.id === input.id)
      : latest.find((e) =>
        e.name === input.name &&
        (e.employee_id || e.role_id || "").trim().toLowerCase() === targetEmployeeId,
      );
    if (target) {
      setSelectedEmployeeId(target.id);
      if (target.is_default && target.primary_skill_id) {
        setSelectedSkillId(target.primary_skill_id);
      }
    }
  }

  async function handleDeleteEmployee(employeeId: string) {
    await invoke("delete_agent_employee", { employeeId });
    await loadEmployees();
  }

  async function handleSetAsMainAndEnter(employeeId: string) {
    const employee = employees.find((e) => e.id === employeeId);
    if (!employee) return;
    await invoke<string>("upsert_agent_employee", {
      input: {
        id: employee.id,
        employee_id: employee.employee_id || employee.role_id,
        name: employee.name,
        role_id: employee.employee_id || employee.role_id,
        persona: employee.persona,
        feishu_open_id: employee.feishu_open_id,
        feishu_app_id: employee.feishu_app_id,
        feishu_app_secret: employee.feishu_app_secret,
        primary_skill_id: employee.primary_skill_id,
        default_work_dir: employee.default_work_dir,
        openclaw_agent_id: employee.employee_id || employee.openclaw_agent_id || employee.role_id,
        routing_priority: employee.routing_priority ?? 100,
        enabled_scopes: employee.enabled_scopes?.length ? employee.enabled_scopes : ["feishu"],
        enabled: employee.enabled,
        is_default: true,
        skill_ids: employee.skill_ids,
      } as UpsertAgentEmployeeInput,
    });
    await loadEmployees();
    setSelectedEmployeeId(employeeId);
    if (employee.primary_skill_id) {
      setSelectedSkillId(employee.primary_skill_id);
    }
    navigate("start-task");
  }

  function dismissModelSetupHint() {
    setDismissedModelSetupHint(true);
    if (typeof window === "undefined") {
      return;
    }
    try {
      window.localStorage.setItem(MODEL_SETUP_HINT_DISMISSED_KEY, "1");
    } catch {
      // ignore
    }
  }

  function openSettingsForModelSetup() {
    setShowQuickModelSetup(false);
    setQuickModelError("");
    setQuickModelTestResult(null);
    setShowSettings(true);
  }

  function openQuickModelSetup() {
    setShowQuickModelSetup(true);
    setQuickModelError("");
    setQuickModelTestResult(null);
  }

  function closeQuickModelSetup() {
    if (
      quickModelSaving ||
      quickModelTesting ||
      (models.length === 0 && !showSettings && !hasCompletedInitialModelSetup)
    ) {
      return;
    }
    setShowQuickModelSetup(false);
    setQuickModelError("");
    setQuickModelTestResult(null);
  }

  function applyQuickModelPreset(presetKey: string) {
    const preset = QUICK_MODEL_PRESETS.find((item) => item.key === presetKey);
    if (!preset) return;
    setQuickModelPresetKey(preset.key);
    setQuickModelForm((prev) => ({
      ...prev,
      name: preset.name,
      api_format: preset.api_format,
      base_url: preset.base_url,
      model_name: preset.model_name,
    }));
    setQuickModelTestResult(null);
    setQuickModelError("");
  }

  function getQuickModelConfig(isDefault: boolean) {
    return {
      id: "",
      name: quickModelForm.name.trim() || "快速配置模型",
      api_format: quickModelForm.api_format,
      base_url: quickModelForm.base_url,
      model_name: quickModelForm.model_name,
      is_default: isDefault,
    };
  }

  async function testQuickModelSetupConnection() {
    if (quickModelSaving || quickModelTesting) return;
    const apiKey = quickModelForm.api_key.trim();
    if (!apiKey) {
      setQuickModelError("请输入 API Key");
      setQuickModelTestResult(null);
      return;
    }
    setQuickModelTesting(true);
    setQuickModelError("");
    setQuickModelTestResult(null);
    try {
      const ok = await invoke<boolean>("test_connection_cmd", {
        config: getQuickModelConfig(false),
        apiKey,
      });
      setQuickModelTestResult(ok);
      if (!ok) {
        setQuickModelError("连接失败，请检查配置后重试");
      }
    } catch (e) {
      setQuickModelError(String(e));
      setQuickModelTestResult(false);
    } finally {
      setQuickModelTesting(false);
    }
  }

  async function saveQuickModelSetup() {
    if (quickModelSaving || quickModelTesting) return;
    const apiKey = quickModelForm.api_key.trim();
    if (!apiKey) {
      setQuickModelError("请输入 API Key");
      return;
    }
    setQuickModelSaving(true);
    setQuickModelError("");
    try {
      await invoke("save_model_config", {
        config: getQuickModelConfig(models.length === 0),
        apiKey,
      });
      await loadModels();
      setShowQuickModelSetup(false);
      setQuickModelForm((prev) => ({ ...prev, api_key: "" }));
      setQuickModelTestResult(null);
    } catch (e) {
      setQuickModelError(String(e));
    } finally {
      setQuickModelSaving(false);
    }
  }

  async function handleStartTaskWithEmployee(employeeId: string) {
    if (creatingSession) return;
    const employee = employees.find((e) => e.id === employeeId);
    if (!employee) return;

    const skillId = employee.primary_skill_id || getDefaultSkillId(skills);
    const modelId = models[0]?.id;

    setSelectedEmployeeId(employee.id);
    if (skillId) {
      setSelectedSkillId(skillId);
    }
    setSelectedSessionId(null);
    setCreateSessionError(null);
    navigate("start-task");

    if (!skillId || !modelId) {
      return;
    }

    setCreatingSession(true);
    try {
      const sessionId = await invoke<string>("create_session", {
        skillId,
        modelId,
        workDir: employee.default_work_dir || "",
        employeeId: employee.employee_id || employee.role_id || "",
        permissionMode: newSessionPermissionMode,
      });
      await loadSessions(skillId);
      setSelectedSessionId(sessionId);
    } catch (e) {
      console.error("从员工页创建会话失败:", e);
      setCreateSessionError("创建会话失败，请稍后重试");
    } finally {
      setCreatingSession(false);
    }
  }

  async function handleOpenEmployeeCreatorSkill() {
    if (creatingSession) return;
    let nextSkills = skills;
    if (!nextSkills.some((item) => item.id === BUILTIN_EMPLOYEE_CREATOR_SKILL_ID)) {
      try {
        nextSkills = await loadSkills();
      } catch (e) {
        console.error("加载创建员工内置技能失败:", e);
      }
    }

    if (!nextSkills.some((item) => item.id === BUILTIN_EMPLOYEE_CREATOR_SKILL_ID)) {
      setCreateSessionError("创建员工助手暂未就绪，请稍后重试");
      navigate("experts");
      return;
    }

    const skillId = BUILTIN_EMPLOYEE_CREATOR_SKILL_ID;
    const modelId = models[0]?.id;

    setSelectedEmployeeId(null);
    setSelectedSkillId(skillId);
    setSelectedSessionId(null);
    setCreateSessionError(null);
    navigate("start-task");

    if (!modelId) {
      return;
    }

    setCreatingSession(true);
    try {
      const sessionId = await invoke<string>("create_session", {
        skillId,
        modelId,
        workDir: "",
        employeeId: "",
        permissionMode: newSessionPermissionMode,
      });
      await loadSessions(skillId);
      setSelectedSessionId(sessionId);
      setPendingInitialMessage({
        sessionId,
        message: EMPLOYEE_CREATOR_STARTER_PROMPT,
      });
    } catch (e) {
      console.error("打开创建员工助手失败:", e);
      setCreateSessionError("创建会话失败，请稍后重试");
    } finally {
      setCreatingSession(false);
    }
  }

  function handleStartTaskWithSkill(skillId: string) {
    setSelectedSkillId(skillId);
    setSelectedSessionId(null);
    setCreateSessionError(null);
    navigate("start-task");
  }

  const selectedSkill = skills.find((s) => s.id === selectedSkillId) ?? null;
  const selectedSession = sessions.find((s) => s.id === selectedSessionId);
  const selectedSessionImManaged = selectedSessionId ? imManagedSessionIds.includes(selectedSessionId) : false;
  const shouldShowModelSetupGate =
    !showSettings && models.length === 0 && !hasCompletedInitialModelSetup;
  const shouldShowModelSetupHint =
    !showSettings &&
    models.length === 0 &&
    hasCompletedInitialModelSetup &&
    !dismissedModelSetupHint;

  return (
    <div className="sm-app flex h-screen overflow-hidden">
      <Sidebar
        activeMainView={activeMainView}
        onOpenStartTask={handleOpenStartTask}
        onOpenExperts={() => navigate("experts")}
        onOpenEmployees={() => navigate("employees")}
        selectedSkillId={selectedSkillId}
        sessions={sessions}
        selectedSessionId={selectedSessionId}
        onSelectSession={setSelectedSessionId}
        newSessionPermissionMode={newSessionPermissionMode}
        onChangeNewSessionPermissionMode={setNewSessionPermissionMode}
        onDeleteSession={handleDeleteSession}
        onSettings={() => {
          navigate("start-task");
          setShowSettings(true);
        }}
        onSearchSessions={handleSearchSessions}
        onExportSession={handleExportSession}
        onCollapse={() => setSidebarCollapsed((prev) => !prev)}
        collapsed={sidebarCollapsed}
      />
      <div className="flex-1 overflow-hidden flex flex-col">
        {shouldShowModelSetupHint && (
          <div className="px-4 pt-4">
            <div
              data-testid="model-setup-hint"
              className="rounded-xl border border-blue-100 bg-blue-50 px-4 py-3 flex flex-col md:flex-row md:items-center md:justify-between gap-3"
            >
              <div className="min-w-0">
                <div className="text-sm font-medium text-blue-900">先连接一个大模型，智能体才能开始工作</div>
                <div className="text-xs text-blue-700 mt-1">
                  只需 1 分钟完成配置。配置后就能创建会话、执行技能和驱动智能体员工协作。
                </div>
              </div>
              <div className="flex items-center gap-2">
                <button
                  data-testid="model-setup-hint-open-quick-setup"
                  onClick={openQuickModelSetup}
                  className="h-8 px-3 rounded bg-blue-500 hover:bg-blue-600 text-white text-xs"
                >
                  快速配置（1分钟）
                </button>
                <button
                  data-testid="model-setup-hint-open-settings"
                  onClick={openSettingsForModelSetup}
                  className="h-8 px-3 rounded border border-blue-200 hover:bg-blue-100 text-blue-700 text-xs"
                >
                  打开设置
                </button>
                <button
                  data-testid="model-setup-hint-dismiss"
                  onClick={dismissModelSetupHint}
                  className="h-8 px-3 rounded border border-blue-200 hover:bg-blue-100 text-blue-700 text-xs"
                >
                  稍后再说
                </button>
              </div>
            </div>
          </div>
        )}
        {showQuickModelSetup && (
          <div
            data-testid="quick-model-setup-dialog"
            className="fixed inset-0 z-40 bg-black/20 flex items-center justify-center p-4"
          >
            <div className="w-full max-w-lg rounded-xl bg-white border border-gray-200 shadow-lg p-4 space-y-3">
              <div>
                <div className="text-sm font-semibold text-gray-900">快速配置模型</div>
                <div className="text-xs text-gray-500 mt-1">填好 API Key 即可完成首次配置，后续可在设置里细调。</div>
              </div>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                <div>
                  <div className="text-xs text-gray-600 mb-1">服务商</div>
                  <select
                    data-testid="quick-model-setup-preset"
                    value={quickModelPresetKey}
                    onChange={(e) => applyQuickModelPreset(e.target.value)}
                    className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm bg-white"
                  >
                    {QUICK_MODEL_PRESETS.map((preset) => (
                      <option key={preset.key} value={preset.key}>
                        {preset.label}
                      </option>
                    ))}
                  </select>
                </div>
                <div>
                  <div className="text-xs text-gray-600 mb-1">连接名称</div>
                  <input
                    value={quickModelForm.name}
                    onChange={(e) => {
                      setQuickModelForm((s) => ({ ...s, name: e.target.value }));
                      setQuickModelTestResult(null);
                    }}
                    className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                  />
                </div>
              </div>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                <div>
                  <div className="text-xs text-gray-600 mb-1">Base URL</div>
                  <input
                    value={quickModelForm.base_url}
                    onChange={(e) => {
                      setQuickModelForm((s) => ({ ...s, base_url: e.target.value }));
                      setQuickModelTestResult(null);
                    }}
                    className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                  />
                </div>
                <div>
                  <div className="text-xs text-gray-600 mb-1">模型名</div>
                  <input
                    value={quickModelForm.model_name}
                    onChange={(e) => {
                      setQuickModelForm((s) => ({ ...s, model_name: e.target.value }));
                      setQuickModelTestResult(null);
                    }}
                    className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                  />
                </div>
              </div>
              <div>
                <div className="text-xs text-gray-600 mb-1">API Key</div>
                <input
                  data-testid="quick-model-setup-api-key"
                  type="password"
                  value={quickModelForm.api_key}
                  onChange={(e) => {
                    setQuickModelForm((s) => ({ ...s, api_key: e.target.value }));
                    setQuickModelTestResult(null);
                  }}
                  className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                  placeholder="请输入 API Key"
                />
              </div>
              {quickModelTestResult !== null && (
                <div
                  data-testid="quick-model-setup-test-result"
                  className={`text-xs rounded px-2 py-1 border ${
                    quickModelTestResult
                      ? "text-green-700 bg-green-50 border-green-100"
                      : "text-orange-700 bg-orange-50 border-orange-100"
                  }`}
                >
                  {quickModelTestResult ? "连接成功，可直接保存并开始" : "连接失败，请检查后重试"}
                </div>
              )}
              {quickModelError && (
                <div className="text-xs text-red-600 bg-red-50 border border-red-100 rounded px-2 py-1">
                  {quickModelError}
                </div>
              )}
              <div className="flex items-center justify-between gap-2">
                <button
                  data-testid="quick-model-setup-test-connection"
                  onClick={testQuickModelSetupConnection}
                  disabled={quickModelSaving || quickModelTesting}
                  className="h-8 px-3 rounded border border-blue-200 hover:bg-blue-50 disabled:bg-gray-100 text-blue-700 text-xs"
                >
                  {quickModelTesting ? "测试中..." : "测试连接"}
                </button>
                <div className="flex items-center gap-2">
                  <button
                    data-testid="quick-model-setup-cancel"
                    onClick={closeQuickModelSetup}
                    disabled={quickModelSaving || quickModelTesting}
                    className="h-8 px-3 rounded border border-gray-200 hover:bg-gray-50 disabled:bg-gray-100 text-gray-600 text-xs"
                  >
                    取消
                  </button>
                  <button
                    data-testid="quick-model-setup-save"
                    onClick={saveQuickModelSetup}
                    disabled={quickModelSaving || quickModelTesting}
                    className="h-8 px-3 rounded bg-blue-500 hover:bg-blue-600 disabled:bg-blue-300 text-white text-xs"
                  >
                    {quickModelSaving ? "保存中..." : "保存并开始"}
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}
        {shouldShowModelSetupGate && (
          <div
            data-testid="model-setup-gate"
            className="fixed inset-0 z-30 bg-white/70 backdrop-blur-[1px] flex items-center justify-center p-4"
          >
            <div className="w-full max-w-lg rounded-xl border border-blue-100 bg-white shadow-sm p-5 space-y-3">
              <div className="text-base font-semibold text-blue-900">首次使用需要先连接一个大模型</div>
              <div className="text-sm text-blue-700">
                完成模型配置后，才能开始任务、创建会话并驱动智能体员工执行技能。现在只需 1 分钟。
              </div>
              <div className="flex items-center gap-2">
                <button
                  data-testid="model-setup-gate-open-quick-setup"
                  onClick={openQuickModelSetup}
                  className="h-9 px-4 rounded bg-blue-500 hover:bg-blue-600 text-white text-sm"
                >
                  快速配置（1分钟）
                </button>
                <button
                  data-testid="model-setup-gate-open-settings"
                  onClick={openSettingsForModelSetup}
                  className="h-9 px-4 rounded border border-blue-200 hover:bg-blue-50 text-blue-700 text-sm"
                >
                  打开设置
                </button>
              </div>
            </div>
          </div>
        )}
        <div className="flex-1 overflow-hidden">
          <AnimatePresence mode="wait">
            {showSettings ? (
            <motion.div
              key="settings"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="h-full"
            >
              <SettingsView
                onClose={async () => {
                  await loadModels();
                  setShowSettings(false);
                }}
              />
            </motion.div>
          ) : activeMainView === "packaging" ? (
            <motion.div
              key="packaging"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="h-full"
            >
              <PackagingView />
            </motion.div>
          ) : activeMainView === "experts-new" ? (
            <motion.div
              key="experts-new"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="h-full"
            >
              <ExpertCreateView
                saving={creatingExpertSkill}
                error={expertCreateError}
                savedPath={expertSavedPath}
                canRetryImport={Boolean(pendingImportDir)}
                retryingImport={retryingExpertImport}
                onBack={() => {
                  setExpertCreateError(null);
                  setExpertSavedPath(null);
                  setPendingImportDir(null);
                  navigate("experts");
                }}
                onOpenPackaging={() => navigate("packaging")}
                onPickDirectory={handlePickSkillDirectory}
                onSave={handleCreateExpertSkill}
                onRetryImport={handleRetryExpertImport}
                onRenderPreview={handleRenderExpertPreview}
              />
            </motion.div>
          ) : activeMainView === "experts" ? (
            <motion.div
              key="experts"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="h-full"
            >
              <ExpertsView
                skills={skills}
                onInstallSkill={() => setShowInstall(true)}
                onCreate={() => {
                  setExpertCreateError(null);
                  setExpertSavedPath(null);
                  setPendingImportDir(null);
                  navigate("experts-new");
                }}
                onOpenPackaging={() => navigate("packaging")}
                onInstallFromLibrary={handleInstallFromLibrary}
                onStartTaskWithSkill={handleStartTaskWithSkill}
                onRefreshLocalSkill={handleRefreshLocalSkill}
                onCheckClawhubUpdate={handleCheckClawhubUpdate}
                onUpdateClawhubSkill={handleUpdateClawhubSkill}
                onDeleteSkill={handleDeleteSkill}
                clawhubUpdateStatus={clawhubUpdateStatus}
                busySkillId={skillActionState?.skillId}
                busyAction={skillActionState?.action ?? null}
              />
            </motion.div>
          ) : activeMainView === "employees" ? (
            <motion.div
              key="employees"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="h-full"
            >
              <EmployeeHubView
                employees={employees}
                skills={skills}
                selectedEmployeeId={selectedEmployeeId}
                onSelectEmployee={setSelectedEmployeeId}
                onSaveEmployee={handleSaveEmployee}
                onDeleteEmployee={handleDeleteEmployee}
                onSetAsMainAndEnter={handleSetAsMainAndEnter}
                onStartTaskWithEmployee={handleStartTaskWithEmployee}
                onOpenEmployeeCreatorSkill={handleOpenEmployeeCreatorSkill}
              />
            </motion.div>
          ) : selectedSkill && models.length > 0 && selectedSessionId ? (
            <motion.div
              key="chat"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="h-full"
            >
              <ChatView
                skill={selectedSkill}
                models={models}
                sessionId={selectedSessionId}
                workDir={selectedSession?.work_dir}
                onSessionUpdate={handleSessionRefresh}
                installedSkillIds={skills.map((s) => s.id)}
                onSkillInstalled={handleSkillInstalledFromChat}
                suppressAskUserPrompt={selectedSessionImManaged}
                initialMessage={
                  pendingInitialMessage && pendingInitialMessage.sessionId === selectedSessionId
                    ? pendingInitialMessage.message
                    : undefined
                }
                onInitialMessageConsumed={() => {
                  setPendingInitialMessage((prev) =>
                    prev && prev.sessionId === selectedSessionId ? null : prev
                  );
                }}
              />
            </motion.div>
          ) : selectedSkill && models.length > 0 ? (
            <motion.div
              key="new-session"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="h-full"
            >
              <NewSessionLanding
                sessions={sessions}
                creating={creatingSession}
                error={createSessionError}
                onSelectSession={setSelectedSessionId}
                onCreateSessionWithInitialMessage={handleCreateSession}
              />
            </motion.div>
          ) : selectedSkill && models.length === 0 ? (
            <div className="flex items-center justify-center h-full sm-text-muted text-sm">
              请先在设置中配置模型和 API Key
            </div>
            ) : (
              <div className="flex items-center justify-center h-full sm-text-muted text-sm">
                从左侧选择一个技能，开始任务
              </div>
            )}
          </AnimatePresence>
        </div>
      </div>
      {showInstall && (
        <InstallDialog onInstalled={handleInstalled} onClose={() => setShowInstall(false)} />
      )}
    </div>
  );
}
