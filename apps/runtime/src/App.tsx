import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { motion, AnimatePresence } from "framer-motion";
import {
  BadgeCheck,
  Bot,
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  Eye,
  EyeOff,
  KeyRound,
  Settings2,
  Sparkles,
  Wand2,
  X,
} from "lucide-react";
import { Sidebar } from "./components/Sidebar";
import { ChatView } from "./components/ChatView";
import { InstallDialog } from "./components/InstallDialog";
import { SettingsView } from "./components/SettingsView";
import { PackagingView } from "./components/packaging/PackagingView";
import { NewSessionLanding } from "./components/NewSessionLanding";
import { ExpertsView } from "./components/experts/ExpertsView";
import { EmployeeHubView } from "./components/employees/EmployeeHubView";
import { SearchConfigForm } from "./components/SearchConfigForm";
import {
  DEFAULT_MODEL_PROVIDER_ID,
  MODEL_PROVIDER_CATALOG,
  buildModelFormFromCatalogItem,
  getModelProviderCatalogItem,
} from "./model-provider-catalog";
import {
  applySearchPresetToForm,
  EMPTY_SEARCH_CONFIG_FORM,
  validateSearchConfigForm,
} from "./lib/search-config";
import { getDefaultModelId } from "./lib/default-model";
import { openExternalUrl } from "./utils/openExternalUrl";
import {
  ExpertCreatePayload,
  ExpertCreateView,
  ExpertPreviewPayload,
  ExpertPreviewResult,
} from "./components/experts/ExpertCreateView";
import { SkillManifest, ModelConfig, SessionInfo, ImRoleDispatchRequest, Message, AgentEmployee, EmployeeGroup, UpsertAgentEmployeeInput, RuntimePreferences } from "./types";

type MainView = "start-task" | "experts" | "experts-new" | "packaging" | "employees";
type SkillAction = "refresh" | "delete" | "check-update" | "update";
type EmployeeAssistantMode = "create" | "update";
type SessionLaunchMode = "general" | "employee_direct" | "team_entry";
type EmployeeAssistantLaunchOptions = {
  mode?: EmployeeAssistantMode;
  employeeId?: string;
};
type EmployeeAssistantSessionContext = {
  mode: EmployeeAssistantMode;
  employeeName?: string;
  employeeCode?: string;
};
const BUILTIN_GENERAL_SKILL_ID = "builtin-general";
const BUILTIN_EMPLOYEE_CREATOR_SKILL_ID = "builtin-employee-creator";
const MODEL_SETUP_HINT_DISMISSED_KEY = "workclaw:model-setup-hint-dismissed";
const INITIAL_MODEL_SETUP_COMPLETED_KEY = "workclaw:initial-model-setup-completed";
const DEFAULT_OPERATION_PERMISSION_MODE: "standard" | "full_access" = "standard";
const EMPLOYEE_ASSISTANT_DISPLAY_NAME = "智能体员工助手";
const EMPLOYEE_CREATOR_STARTER_PROMPT =
  "请帮我创建一个新的智能体员工。先问我 1-2 个关键问题，再给出配置草案，确认后再执行创建。";
const EMPLOYEE_ASSISTANT_QUICK_PROMPTS: Array<{ label: string; prompt: string }> = [
  {
    label: "加技能",
    prompt:
      "请帮我修改一个已有智能体员工：给目标员工增加技能。你先调用 list_employees 和 list_skills，然后给出 update_employee 草案（使用 add_skill_ids），我确认后再执行。",
  },
  {
    label: "删技能",
    prompt:
      "请帮我修改一个已有智能体员工：给目标员工移除技能。你先调用 list_employees，再给出 update_employee 草案（使用 remove_skill_ids），我确认后再执行。",
  },
  {
    label: "改主技能",
    prompt:
      "请帮我修改一个已有智能体员工：调整 primary_skill_id。你先确认员工与目标主技能，再给出 update_employee 草案，我确认后再执行。",
  },
  {
    label: "改飞书配置",
    prompt:
      "请帮我修改一个已有智能体员工的飞书配置（open_id / app_id / app_secret）。你先确认目标员工，再给出 update_employee 草案，我确认后再执行。",
  },
  {
    label: "更新画像",
    prompt:
      "请帮我更新已有员工的 AGENTS/SOUL/USER 配置。你先引导我补齐 mission/responsibilities/collaboration/tone/boundaries/user_profile，再给出 update_employee + profile_answers 草案，我确认后再执行。",
  },
];

const DEFAULT_QUICK_MODEL_PROVIDER = getModelProviderCatalogItem(DEFAULT_MODEL_PROVIDER_ID);

const MODEL_SETUP_STEPS: Array<{ title: string; description: string }> = [
  {
    title: "选择一个服务商模板",
    description: "优先选你已经有 API Key 的平台，系统会自动带出推荐参数。",
  },
  {
    title: "填入 API Key",
    description: "首次接入只需要这一步，其他字段后续都能在设置里细调。",
  },
  {
    title: "补齐搜索引擎",
    description: "模型保存成功后继续配置搜索，让智能体开箱即可联网检索。",
  },
];

const MODEL_SETUP_OUTCOMES = ["创建会话", "执行技能", "驱动智能体员工协作"];

type ImBridgeSessionContext = {
  threadId: string;
  primaryRoleName: string;
  roleName: string;
  streamBuffer: string;
  streamSentCount: number;
  waitingForAnswer: boolean;
  streamFlushTimer: ReturnType<typeof setTimeout> | null;
  lastStreamFlushAt: number;
  streamFlushInFlight: boolean;
};

function formatFeishuRoleMessage(roleName: string, text: string): string {
  const safeRole = (roleName || "").trim() || "智能体员工";
  const safeText = (text || "").trim();
  if (!safeText) return "";
  return `[${safeRole}] ${safeText}`;
}

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

const SHOW_DEV_MODEL_SETUP_TOOLS = import.meta.env.DEV || import.meta.env.MODE === "test";

function buildEmployeeAssistantUpdatePrompt(employee: AgentEmployee): string {
  const employeeCode = (employee.employee_id || employee.role_id || employee.id || "").trim();
  return `调整员工任务：请帮我修改智能体员工「${employee.name}」（employee_id: ${employeeCode}）。先确认修改目标，再给出 update_employee 配置草案（包含变更字段与理由），待我确认后再执行。`;
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
  const [operationPermissionMode, setOperationPermissionMode] = useState<"standard" | "full_access">(
    DEFAULT_OPERATION_PERMISSION_MODE
  );
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
  const [employeeGroups, setEmployeeGroups] = useState<EmployeeGroup[]>([]);
  const [selectedEmployeeId, setSelectedEmployeeId] = useState<string | null>(null);
  const [employeeCreatorHighlight, setEmployeeCreatorHighlight] = useState<{
    employeeId: string;
    name: string;
  } | null>(null);
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
  const [forceShowModelSetupGate, setForceShowModelSetupGate] = useState(false);
  const [quickSetupStep, setQuickSetupStep] = useState<"model" | "search">("model");
  const [quickModelPresetKey, setQuickModelPresetKey] = useState(DEFAULT_QUICK_MODEL_PROVIDER.id);
  const [quickModelForm, setQuickModelForm] = useState(() => ({
    ...buildModelFormFromCatalogItem(DEFAULT_QUICK_MODEL_PROVIDER),
    api_key: "",
  }));
  const [quickModelSaving, setQuickModelSaving] = useState(false);
  const [quickModelTesting, setQuickModelTesting] = useState(false);
  const [quickModelTestResult, setQuickModelTestResult] = useState<boolean | null>(null);
  const [quickModelError, setQuickModelError] = useState("");
  const [quickModelApiKeyVisible, setQuickModelApiKeyVisible] = useState(false);
  const [searchConfigs, setSearchConfigs] = useState<ModelConfig[]>([]);
  const [quickSearchForm, setQuickSearchForm] = useState(EMPTY_SEARCH_CONFIG_FORM);
  const [quickSearchSaving, setQuickSearchSaving] = useState(false);
  const [quickSearchTesting, setQuickSearchTesting] = useState(false);
  const [quickSearchTestResult, setQuickSearchTestResult] = useState<boolean | null>(null);
  const [quickSearchError, setQuickSearchError] = useState("");
  const [quickSearchApiKeyVisible, setQuickSearchApiKeyVisible] = useState(false);
  const [pendingInitialMessage, setPendingInitialMessage] = useState<{
    sessionId: string;
    message: string;
  } | null>(null);
  const [pendingSessionFocusRequest, setPendingSessionFocusRequest] = useState<{
    sessionId: string;
    snippet: string;
    nonce: number;
  } | null>(null);
  const [pendingSessionExecutionContext, setPendingSessionExecutionContext] = useState<{
    targetSessionId: string;
    sourceSessionId: string;
    sourceStepId: string;
    sourceEmployeeId?: string;
    assigneeEmployeeId?: string;
    sourceStepTimeline?: Array<{ eventId?: string; label: string; createdAt?: string }>;
  } | null>(null);
  const [pendingGroupRunStepFocusRequest, setPendingGroupRunStepFocusRequest] = useState<{
    sessionId: string;
    stepId: string;
    eventId?: string;
    nonce: number;
  } | null>(null);
  const [employeeAssistantSessionContexts, setEmployeeAssistantSessionContexts] = useState<
    Record<string, EmployeeAssistantSessionContext>
  >({});
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const employeesRef = useRef<AgentEmployee[]>([]);
  const quickModelApiKeyInputRef = useRef<HTMLInputElement | null>(null);
  const isBlockingInitialModelSetup = !showSettings && !hasCompletedInitialModelSetup;
  const isQuickSetupBusy =
    quickModelSaving || quickModelTesting || quickSearchSaving || quickSearchTesting;
  const canDismissQuickModelSetup = !isQuickSetupBusy && !isBlockingInitialModelSetup;
  const selectedQuickModelProvider = getModelProviderCatalogItem(quickModelPresetKey);

  function navigate(view: MainView) {
    setActiveMainView(view);
    if (typeof window !== "undefined") {
      window.location.hash = `/${view}`;
    }
  }

  async function createRuntimeSession(input: {
    skillId: string;
    modelId: string;
    workDir?: string;
    employeeId?: string;
    title?: string;
    sessionMode: SessionLaunchMode;
    teamId?: string;
  }) {
    return invoke<string>("create_session", {
      skillId: input.skillId,
      modelId: input.modelId,
      workDir: input.workDir || "",
      employeeId: input.employeeId || "",
      title: input.title,
      permissionMode: operationPermissionMode,
      sessionMode: input.sessionMode,
      teamId: input.sessionMode === "team_entry" ? input.teamId || "" : "",
    });
  }

  useEffect(() => {
    loadSkills();
    loadModels();
    loadSearchConfigs();
    loadRuntimePreferences();
    loadEmployees();
    loadEmployeeGroups();
    if (typeof window !== "undefined" && window.location.hash) {
      const raw = window.location.hash.replace(/^#\//, "");
      if (raw === "experts" || raw === "experts-new" || raw === "packaging" || raw === "start-task" || raw === "employees") {
        setActiveMainView(raw);
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function loadRuntimePreferences() {
    try {
      const prefs = await invoke<RuntimePreferences>("get_runtime_preferences");
      if (!prefs || typeof prefs !== "object") {
        setOperationPermissionMode(DEFAULT_OPERATION_PERMISSION_MODE);
        return;
      }
      setOperationPermissionMode(
        prefs.operation_permission_mode === "full_access" ? "full_access" : "standard"
      );
    } catch (error) {
      console.warn("加载运行时偏好失败:", error);
      setOperationPermissionMode(DEFAULT_OPERATION_PERMISSION_MODE);
    }
  }

  useEffect(() => {
    if (models.length === 0 || searchConfigs.length === 0) {
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
  }, [models.length, searchConfigs.length]);

  useEffect(() => {
    employeesRef.current = employees;
  }, [employees]);

  useEffect(() => {
    if (!showQuickModelSetup || typeof window === "undefined") {
      return;
    }

    const focusTimer = window.setTimeout(() => {
      quickModelApiKeyInputRef.current?.focus({ preventScroll: true });
    }, 0);

    return () => {
      window.clearTimeout(focusTimer);
    };
  }, [showQuickModelSetup]);

  useEffect(() => {
    if (!showQuickModelSetup || typeof window === "undefined") {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || !canDismissQuickModelSetup) {
        return;
      }
      event.preventDefault();
      closeQuickModelSetup();
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [showQuickModelSetup, canDismissQuickModelSetup]);

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
    const feishuRetryTimers = new Map<string, ReturnType<typeof setTimeout>>();
    const STREAM_CHUNK_SIZE = 120;
    const STREAM_FLUSH_INTERVAL_MS = 1200;
    const FEISHU_RETRY_DELAYS_MS = [1000, 3000, 8000];
    const FEISHU_MAX_ATTEMPTS = FEISHU_RETRY_DELAYS_MS.length + 1;
    const sanitizeInboundPrompt = (raw: string): string =>
      raw
        .replace(/(^|\s)@_[A-Za-z0-9_]+/g, "$1")
        .replace(/(^|\s)@[^\s@]+/g, "$1")
        .replace(/\s+/g, " ")
        .trim();

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

    const buildFeishuRetryKey = (threadId: string, text: string) => `${threadId}::${text}`;

    const clearFeishuRetryTimer = (key: string) => {
      const timer = feishuRetryTimers.get(key);
      if (timer) {
        clearTimeout(timer);
      }
      feishuRetryTimers.delete(key);
    };

    const invokeFeishuSend = async (threadId: string, text: string) => {
      await invoke("send_feishu_text_message", {
        chatId: threadId,
        text,
        appId: null,
        appSecret: null,
        sidecarBaseUrl: null,
      });
    };

    const scheduleFeishuRetry = (
      threadId: string,
      text: string,
      attempt: number,
      lastError: unknown
    ) => {
      const key = buildFeishuRetryKey(threadId, text);
      if (attempt > FEISHU_MAX_ATTEMPTS) {
        clearFeishuRetryTimer(key);
        console.error(
          "飞书消息转发失败，已降级为仅桌面可见",
          threadId,
          extractErrorMessage(lastError, "unknown error")
        );
        return;
      }
      if (feishuRetryTimers.has(key)) return;
      const delay = FEISHU_RETRY_DELAYS_MS[Math.max(0, attempt - 2)] ?? FEISHU_RETRY_DELAYS_MS[FEISHU_RETRY_DELAYS_MS.length - 1];
      const timer = setTimeout(() => {
        feishuRetryTimers.delete(key);
        void (async () => {
          try {
            await invokeFeishuSend(threadId, text);
          } catch (error) {
            scheduleFeishuRetry(threadId, text, attempt + 1, error);
          }
        })();
      }, delay);
      feishuRetryTimers.set(key, timer);
    };

    const sendTextToFeishu = async (threadId: string, text: string) => {
      const chatId = threadId.trim();
      const messageText = text.trim().slice(0, 1800);
      if (!chatId || !messageText) return;
      const key = buildFeishuRetryKey(chatId, messageText);
      clearFeishuRetryTimer(key);
      try {
        await invokeFeishuSend(chatId, messageText);
      } catch (error) {
        scheduleFeishuRetry(chatId, messageText, 2, error);
      }
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
          await sendTextToFeishu(ctx.threadId, formatFeishuRoleMessage(ctx.roleName, chunk));
          ctx.streamSentCount += 1;
          return;
        }
        let start = 0;
        while (start < chunk.length) {
          const part = chunk.slice(start, start + 1800);
          await sendTextToFeishu(ctx.threadId, formatFeishuRoleMessage(ctx.roleName, part));
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
      const cleanedPrompt = sanitizeInboundPrompt(payload.prompt || "");
      const dispatchPrompt = cleanedPrompt || (payload.prompt || "").trim();
      const key = `${payload.session_id}|${payload.role_id}|${dispatchPrompt}`;
      if (seen.has(key)) return;
      seen.add(key);

      const existing = sessionContexts.get(payload.session_id);
      const primaryRoleName = payload.role_name || payload.role_id;
      const ctx: ImBridgeSessionContext = {
        threadId: payload.thread_id,
        primaryRoleName,
        roleName: existing?.roleName || primaryRoleName,
        streamBuffer: existing?.streamBuffer ?? "",
        streamSentCount: 0,
        waitingForAnswer: existing?.waitingForAnswer ?? false,
        streamFlushTimer: existing?.streamFlushTimer ?? null,
        lastStreamFlushAt: existing?.lastStreamFlushAt ?? 0,
        streamFlushInFlight: existing?.streamFlushInFlight ?? false,
      };
      ctx.primaryRoleName = primaryRoleName;
      if (!ctx.roleName.trim()) {
        ctx.roleName = primaryRoleName;
      }
      sessionContexts.set(payload.session_id, ctx);
      markImManagedSession(payload.session_id);

      try {
        if (ctx.waitingForAnswer) {
          ctx.waitingForAnswer = false;
          await invoke("answer_user_question", { answer: dispatchPrompt });
        } else {
          await invoke("send_message", {
            sessionId: payload.session_id,
            userMessage: dispatchPrompt,
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
              formatFeishuRoleMessage(ctx.roleName, latestAssistant.content.slice(0, 1800))
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
      role_id?: string;
      role_name?: string;
    }>("stream-token", ({ payload }) => {
      const ctx = sessionContexts.get(payload.session_id);
      if (!ctx) return;
      if (payload.done) {
        void flushImStream(payload.session_id, { force: true });
        return;
      }
      if (payload.sub_agent) {
        const delegatedRole = (payload.role_name || payload.role_id || "").trim();
        if (delegatedRole) {
          if (ctx.roleName !== delegatedRole && ctx.streamBuffer.trim().length > 0) {
            void flushImStream(payload.session_id, { force: true });
          }
          ctx.roleName = delegatedRole;
        }
      } else if (ctx.roleName !== ctx.primaryRoleName) {
        if (ctx.streamBuffer.trim().length > 0) {
          void flushImStream(payload.session_id, { force: true });
        }
        ctx.roleName = ctx.primaryRoleName;
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
          formatFeishuRoleMessage(
            ctx.roleName,
            `${payload.question}${optionsText}\n请直接回复你的选择或补充信息。`
          )
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
      feishuRetryTimers.forEach((timer) => clearTimeout(timer));
      feishuRetryTimers.clear();
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

  async function loadSearchConfigs() {
    const list = await invoke<ModelConfig[]>("list_search_configs");
    setSearchConfigs(Array.isArray(list) ? list : []);
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

  async function loadEmployeeGroups(): Promise<EmployeeGroup[]> {
    try {
      const raw = await invoke<EmployeeGroup[] | null>("list_employee_groups");
      const list = Array.isArray(raw) ? raw : [];
      setEmployeeGroups(list);
      return list;
    } catch {
      setEmployeeGroups([]);
      return [];
    }
  }

  async function loadSessions(_skillId: string) {
    try {
      const list = await invoke<SessionInfo[]>("list_sessions");
      setSessions(Array.isArray(list) ? list : []);
    } catch (e) {
      console.error("加载会话列表失败:", e);
      setSessions([]);
    }
  }

  async function handleCreateSession(initialMessage = "") {
    const skillId = getDefaultSkillId(skills);
    const modelId = getDefaultModelId(models);
    if (!skillId || !modelId || creatingSession) return;

    setCreatingSession(true);
    setCreateSessionError(null);
    try {
      setSelectedEmployeeId(null);
      setSelectedSkillId(skillId);
      const id = await createRuntimeSession({
        skillId,
        modelId,
        sessionMode: "general",
      });
      const firstMessage = initialMessage.trim();
      await loadSessions(skillId);
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

  async function handleCreateTeamEntrySession(input: { teamId: string; initialMessage?: string }) {
    const teamId = (input.teamId || "").trim();
    const initialMessage = (input.initialMessage || "").trim();
    const modelId = getDefaultModelId(models);
    if (!teamId || !modelId || creatingSession) return;

    const group = employeeGroups.find((item) => item.id === teamId);
    if (!group) {
      setCreateSessionError("未找到可用的协作团队");
      return;
    }

    const entryEmployeeCode = (group.entry_employee_id || group.coordinator_employee_id || "").trim();
    const entryEmployee = employees.find((item) => {
      const code = (item.employee_id || item.role_id || "").trim();
      return code === entryEmployeeCode;
    });
    const skillId = entryEmployee?.primary_skill_id || getDefaultSkillId(skills);
    if (!skillId) return;

    setCreatingSession(true);
    setCreateSessionError(null);
    try {
      setSelectedEmployeeId(entryEmployee?.id || null);
      setSelectedSkillId(skillId);
      const sessionId = await createRuntimeSession({
        skillId,
        modelId,
        workDir: entryEmployee?.default_work_dir || "",
        employeeId: entryEmployee?.employee_id || entryEmployee?.role_id || "",
        title: group.name || "团队协作",
        sessionMode: "team_entry",
        teamId,
      });
      await loadSessions(skillId);
      setSelectedSessionId(sessionId);
      if (initialMessage) {
        setPendingInitialMessage({ sessionId, message: initialMessage });
      }
    } catch (e) {
      console.error("创建团队会话失败:", e);
      setCreateSessionError("创建团队会话失败，请稍后重试");
    } finally {
      setCreatingSession(false);
    }
  }

  async function handleDeleteSession(sessionId: string) {
    try {
      await invoke("delete_session", { sessionId });
      if (selectedSessionId === sessionId) setSelectedSessionId(null);
      setEmployeeAssistantSessionContexts((prev) => {
        if (!prev[sessionId]) return prev;
        const next = { ...prev };
        delete next[sessionId];
        return next;
      });
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
        const results = await invoke<SessionInfo[]>("search_sessions_global", {
          query: query.trim(),
        });
        setSessions(Array.isArray(results) ? results : []);
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
    const modelId = getDefaultModelId(models);
    if (modelId) {
      try {
        const sessionId = await createRuntimeSession({
          skillId,
          modelId,
          sessionMode: "general",
        });
        await loadSessions(skillId);
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
    }
    const previousEmployeeIds = new Set(
      employeesRef.current.map((item) => item.id),
    );
    void (async () => {
      try {
        const latest = await loadEmployees();
        if (selectedSkillId !== BUILTIN_EMPLOYEE_CREATOR_SKILL_ID) {
          return;
        }
        const created = latest.find(
          (item) => !previousEmployeeIds.has(item.id),
        );
        if (created) {
          setSelectedEmployeeId(created.id);
          setEmployeeCreatorHighlight({
            employeeId: created.id,
            name: created.name,
          });
        }
      } catch (e) {
        console.error("刷新员工列表失败:", e);
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedSkillId]);

  const handleSkillInstalledFromChat = useCallback(async (_skillId: string) => {
    await loadSkills();
  }, []);

  function handleOpenStartTask() {
    setShowSettings(false);
    setSelectedSessionId(null);
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
    if (employeeCreatorHighlight?.employeeId === employeeId) {
      setEmployeeCreatorHighlight(null);
    }
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

  const landingTeams = useMemo(() => {
    return employeeGroups.map((group) => {
      const entryCode = (group.entry_employee_id || group.coordinator_employee_id || "").trim();
      const entryEmployee = employees.find((item) => (item.employee_id || item.role_id || "").trim() === entryCode);
      const coordinatorEmployee = employees.find(
        (item) => (item.employee_id || item.role_id || "").trim() === (group.coordinator_employee_id || "").trim()
      );
      return {
        id: group.id,
        name: group.name,
        description: `入口：${entryEmployee?.name || entryCode || "未设置"} · 协调：${
          coordinatorEmployee?.name || group.coordinator_employee_id || "未设置"
        }`,
        memberCount: group.member_count || group.member_employee_ids?.length || 0,
      };
    });
  }, [employeeGroups, employees]);

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

  function resetFirstUseOnboardingForDevelopment() {
    setHasCompletedInitialModelSetup(false);
    setDismissedModelSetupHint(false);
    setShowQuickModelSetup(false);
    setQuickModelPresetKey(DEFAULT_QUICK_MODEL_PROVIDER.id);
    setQuickModelForm({
      ...buildModelFormFromCatalogItem(DEFAULT_QUICK_MODEL_PROVIDER),
      api_key: "",
    });
    setQuickModelError("");
    setQuickModelTestResult(null);
    setQuickModelApiKeyVisible(false);
    if (typeof window === "undefined") {
      return;
    }
    try {
      window.localStorage.removeItem(INITIAL_MODEL_SETUP_COMPLETED_KEY);
      window.localStorage.removeItem(MODEL_SETUP_HINT_DISMISSED_KEY);
    } catch {
      // ignore
    }
  }

  function openSettingsForModelSetup() {
    setShowQuickModelSetup(false);
    setForceShowModelSetupGate(false);
    setQuickSetupStep("model");
    setQuickModelError("");
    setQuickModelTestResult(null);
    setQuickModelApiKeyVisible(false);
    setQuickSearchForm(EMPTY_SEARCH_CONFIG_FORM);
    setQuickSearchError("");
    setQuickSearchTestResult(null);
    setQuickSearchApiKeyVisible(false);
    setShowSettings(true);
  }

  function openQuickModelSetup() {
    setShowQuickModelSetup(true);
    setQuickSetupStep("model");
    setQuickModelError("");
    setQuickModelTestResult(null);
    setQuickModelApiKeyVisible(false);
    setQuickSearchForm(EMPTY_SEARCH_CONFIG_FORM);
    setQuickSearchError("");
    setQuickSearchTestResult(null);
    setQuickSearchApiKeyVisible(false);
  }

  function openInitialModelSetupGate() {
    setForceShowModelSetupGate(true);
    openQuickModelSetup();
  }

  function closeQuickModelSetup() {
    if (!canDismissQuickModelSetup) {
      return;
    }
    setShowQuickModelSetup(false);
    setForceShowModelSetupGate(false);
    setQuickSetupStep("model");
    setQuickModelError("");
    setQuickModelTestResult(null);
    setQuickModelApiKeyVisible(false);
    setQuickSearchForm(EMPTY_SEARCH_CONFIG_FORM);
    setQuickSearchError("");
    setQuickSearchTestResult(null);
    setQuickSearchApiKeyVisible(false);
  }

  function applyQuickModelPreset(presetKey: string) {
    const provider = getModelProviderCatalogItem(presetKey);
    setQuickModelPresetKey(provider.id);
    setQuickModelForm((prev) => ({
      ...prev,
      ...buildModelFormFromCatalogItem(provider),
      api_key: prev.api_key,
    }));
    setQuickModelTestResult(null);
    setQuickModelError("");
  }

  function applyQuickSearchPreset(presetKey: string) {
    setQuickSearchForm((current) => applySearchPresetToForm(presetKey, current));
    setQuickSearchError("");
    setQuickSearchTestResult(null);
  }

  function getQuickModelConfig(isDefault: boolean) {
    return {
      id: "",
      name: quickModelForm.name.trim() || "快速配置模型",
      api_format: quickModelForm.api_format,
      base_url: quickModelForm.base_url.trim(),
      model_name: quickModelForm.model_name.trim(),
      is_default: isDefault,
    };
  }

  function validateQuickModelSetup() {
    if (!quickModelForm.base_url.trim()) {
      return "请输入 Base URL";
    }
    if (!quickModelForm.model_name.trim()) {
      return "请输入模型名";
    }
    if (!quickModelForm.api_key.trim()) {
      return "请输入 API Key";
    }
    return null;
  }

  function validateQuickSearchSetup() {
    return validateSearchConfigForm(quickSearchForm);
  }

  async function testQuickModelSetupConnection() {
    if (quickModelSaving || quickModelTesting) return;
    const validationError = validateQuickModelSetup();
    if (validationError) {
      setQuickModelError(validationError);
      setQuickModelTestResult(null);
      return;
    }
    const apiKey = quickModelForm.api_key.trim();
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
    const validationError = validateQuickModelSetup();
    if (validationError) {
      setQuickModelError(validationError);
      setQuickModelTestResult(null);
      return;
    }
    const apiKey = quickModelForm.api_key.trim();
    setQuickModelSaving(true);
    setQuickModelError("");
    try {
      const savedModelId = await invoke<string>("save_model_config", {
        config: getQuickModelConfig(models.length === 0),
        apiKey,
      });
      if (models.length > 0) {
        await invoke("set_default_model", { modelId: savedModelId });
      }
      await loadModels();
      setQuickModelForm((prev) => ({ ...prev, api_key: "" }));
      setQuickModelTestResult(null);
      setQuickModelApiKeyVisible(false);
      setQuickSetupStep("search");
    } catch (e) {
      setQuickModelError(String(e));
    } finally {
      setQuickModelSaving(false);
    }
  }

  async function testQuickSearchSetupConnection() {
    if (quickSearchSaving || quickSearchTesting) return;
    const validationError = validateQuickSearchSetup();
    if (validationError) {
      setQuickSearchError(validationError);
      setQuickSearchTestResult(null);
      return;
    }
    setQuickSearchTesting(true);
    setQuickSearchError("");
    setQuickSearchTestResult(null);
    try {
      const ok = await invoke<boolean>("test_search_connection", {
        config: {
          id: "",
          name: quickSearchForm.name.trim(),
          api_format: quickSearchForm.api_format,
          base_url: quickSearchForm.base_url.trim(),
          model_name: quickSearchForm.model_name.trim(),
          is_default: searchConfigs.length === 0,
        },
        apiKey: quickSearchForm.api_key.trim(),
      });
      setQuickSearchTestResult(ok);
      if (!ok) {
        setQuickSearchError("连接失败，请检查配置");
      }
    } catch (error) {
      setQuickSearchError(extractErrorMessage(error, "搜索连接测试失败"));
      setQuickSearchTestResult(false);
    } finally {
      setQuickSearchTesting(false);
    }
  }

  async function saveQuickSearchSetup() {
    if (quickSearchSaving || quickSearchTesting) return;
    const validationError = validateQuickSearchSetup();
    if (validationError) {
      setQuickSearchError(validationError);
      setQuickSearchTestResult(null);
      return;
    }
    setQuickSearchSaving(true);
    setQuickSearchError("");
    try {
      await invoke("save_model_config", {
        config: {
          id: "",
          name: quickSearchForm.name.trim(),
          api_format: quickSearchForm.api_format,
          base_url: quickSearchForm.base_url.trim(),
          model_name: quickSearchForm.model_name.trim(),
          is_default: searchConfigs.length === 0,
        },
        apiKey: quickSearchForm.api_key.trim(),
      });
      await loadSearchConfigs();
      setShowQuickModelSetup(false);
      setForceShowModelSetupGate(false);
      setQuickSetupStep("model");
      setQuickSearchForm(EMPTY_SEARCH_CONFIG_FORM);
      setQuickSearchTestResult(null);
      setQuickSearchApiKeyVisible(false);
    } catch (error) {
      setQuickSearchError(extractErrorMessage(error, "保存搜索配置失败"));
    } finally {
      setQuickSearchSaving(false);
    }
  }

  function skipQuickSearchSetup() {
    if (isBlockingInitialModelSetup || quickSearchSaving || quickSearchTesting) {
      return;
    }
    setShowQuickModelSetup(false);
    setQuickSetupStep("model");
    setQuickSearchForm(EMPTY_SEARCH_CONFIG_FORM);
    setQuickSearchError("");
    setQuickSearchTestResult(null);
    setQuickSearchApiKeyVisible(false);
  }

  async function handleStartTaskWithEmployee(employeeId: string) {
    if (creatingSession) return;
    const employee = employees.find((e) => e.id === employeeId);
    if (!employee) return;

    const skillId = employee.primary_skill_id || getDefaultSkillId(skills);
    const modelId = getDefaultModelId(models);

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
      const sessionId = await createRuntimeSession({
        skillId,
        modelId,
        workDir: employee.default_work_dir || "",
        employeeId: employee.employee_id || employee.role_id || "",
        sessionMode: "employee_direct",
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

  async function handleOpenEmployeeCreatorSkill(options?: EmployeeAssistantLaunchOptions) {
    if (creatingSession) return;
    setEmployeeCreatorHighlight(null);
    const requestedMode: EmployeeAssistantMode = options?.mode === "update" ? "update" : "create";
    const targetEmployee =
      requestedMode === "update"
        ? employees.find((item) => item.id === (options?.employeeId || selectedEmployeeId || ""))
        : null;
    const launchMode: EmployeeAssistantMode =
      requestedMode === "update" && targetEmployee ? "update" : "create";
    let nextSkills = skills;
    if (!nextSkills.some((item) => item.id === BUILTIN_EMPLOYEE_CREATOR_SKILL_ID)) {
      try {
        nextSkills = await loadSkills();
      } catch (e) {
        console.error(`加载${EMPLOYEE_ASSISTANT_DISPLAY_NAME}内置技能失败:`, e);
      }
    }

    if (!nextSkills.some((item) => item.id === BUILTIN_EMPLOYEE_CREATOR_SKILL_ID)) {
      setCreateSessionError(`${EMPLOYEE_ASSISTANT_DISPLAY_NAME}暂未就绪，请稍后重试`);
      navigate("experts");
      return;
    }

    const skillId = BUILTIN_EMPLOYEE_CREATOR_SKILL_ID;
    const modelId = getDefaultModelId(models);

    if (launchMode === "update" && targetEmployee) {
      setSelectedEmployeeId(targetEmployee.id);
    } else {
      setSelectedEmployeeId(null);
    }
    setSelectedSkillId(skillId);
    setSelectedSessionId(null);
    setCreateSessionError(null);
    navigate("start-task");

    if (!modelId) {
      return;
    }

    setCreatingSession(true);
    try {
      const employeeCode =
        launchMode === "update" && targetEmployee
          ? (targetEmployee.employee_id || targetEmployee.role_id || "").trim()
          : "";
      const sessionTitle =
        launchMode === "update" && targetEmployee
          ? `调整员工：${targetEmployee.name}`
          : "创建员工：新员工";
      const sessionId = await createRuntimeSession({
        skillId,
        modelId,
        workDir:
          launchMode === "update" && targetEmployee
            ? targetEmployee.default_work_dir || ""
            : "",
        employeeId: employeeCode,
        title: sessionTitle,
        sessionMode: employeeCode ? "employee_direct" : "general",
      });
      await loadSessions(skillId);
      setSelectedSessionId(sessionId);
      const initialMessage =
        launchMode === "update" && targetEmployee
          ? buildEmployeeAssistantUpdatePrompt(targetEmployee)
          : EMPLOYEE_CREATOR_STARTER_PROMPT;
      setPendingInitialMessage({
        sessionId,
        message: initialMessage,
      });
      setEmployeeAssistantSessionContexts((prev) => ({
        ...prev,
        [sessionId]:
          launchMode === "update" && targetEmployee
            ? {
                mode: "update",
                employeeName: targetEmployee.name,
                employeeCode: (targetEmployee.employee_id || targetEmployee.role_id || "").trim(),
              }
            : { mode: "create" },
      }));
    } catch (e) {
      console.error(`打开${EMPLOYEE_ASSISTANT_DISPLAY_NAME}失败:`, e);
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

  async function handleOpenGroupRunSession(sessionId: string, skillId: string) {
    setSelectedSkillId(skillId);
    setCreateSessionError(null);
    await loadSessions(skillId);
    setSelectedSessionId(sessionId);
    navigate("start-task");
  }

  const selectedSkill = skills.find((s) => s.id === selectedSkillId) ?? null;
  const selectedSession = sessions.find((s) => s.id === selectedSessionId);
  const selectedEmployeeAssistantContext = (() => {
    if (selectedSkill?.id !== BUILTIN_EMPLOYEE_CREATOR_SKILL_ID || !selectedSessionId) {
      return undefined;
    }
    const fromSession = employeeAssistantSessionContexts[selectedSessionId];
    if (fromSession) {
      return fromSession;
    }
    const sessionEmployeeId = (selectedSession?.employee_id || "").trim();
    if (!sessionEmployeeId) {
      return { mode: "create" as const };
    }
    const matchedEmployee = employees.find((item) => {
      const employeeCode = (item.employee_id || item.role_id || item.id || "").trim();
      return (
        employeeCode.toLowerCase() === sessionEmployeeId.toLowerCase() ||
        item.id.trim().toLowerCase() === sessionEmployeeId.toLowerCase()
      );
    });
    return {
      mode: "update" as const,
      employeeName: matchedEmployee?.name,
      employeeCode: sessionEmployeeId,
    };
  })();
  const selectedSessionImManaged = selectedSessionId ? imManagedSessionIds.includes(selectedSessionId) : false;
  const shouldShowModelSetupGate = isBlockingInitialModelSetup || forceShowModelSetupGate;
  const shouldShowModelSetupHint =
    !showSettings &&
    (models.length === 0 || searchConfigs.length === 0) &&
    hasCompletedInitialModelSetup &&
    !dismissedModelSetupHint;

  return (
    <div className="sm-app flex h-screen overflow-hidden">
      <Sidebar
        activeMainView={activeMainView}
        onOpenStartTask={handleOpenStartTask}
        onOpenExperts={() => {
          setShowSettings(false);
          navigate("experts");
        }}
        onOpenEmployees={() => {
          setShowSettings(false);
          navigate("employees");
        }}
        selectedSkillId={selectedSkillId}
        sessions={sessions}
        selectedSessionId={selectedSessionId}
        onSelectSession={setSelectedSessionId}
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
              className="relative overflow-hidden rounded-[28px] border border-[var(--sm-primary-soft)] bg-white px-5 py-5 shadow-[0_18px_60px_rgba(37,99,235,0.12)]"
            >
              <div className="absolute inset-y-0 right-0 hidden w-72 bg-[radial-gradient(circle_at_center,_rgba(37,99,235,0.16),_transparent_72%)] md:block" />
              <div className="relative flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
                <div className="min-w-0">
                  <div className="inline-flex items-center gap-2 rounded-full bg-[var(--sm-primary-soft)] px-3 py-1 text-[11px] font-semibold text-[var(--sm-primary-strong)]">
                    <Sparkles className="h-3.5 w-3.5" />
                    首次引导
                  </div>
                  <div className="mt-3 text-lg font-semibold text-[var(--sm-text)]">先连接一个大模型，智能体才能开始工作</div>
                  <div className="mt-2 max-w-2xl text-sm leading-6 text-[var(--sm-text-muted)]">
                    只需 1 分钟完成配置。配置后就能创建会话、执行技能和驱动智能体员工协作。
                  </div>
                  <div className="mt-3 flex flex-wrap gap-2">
                    {MODEL_SETUP_OUTCOMES.map((item) => (
                      <span
                        key={item}
                        className="inline-flex items-center gap-1.5 rounded-full border border-[var(--sm-border)] bg-[var(--sm-surface-muted)] px-3 py-1.5 text-xs text-[var(--sm-text-muted)]"
                      >
                        <BadgeCheck className="h-3.5 w-3.5 text-[var(--sm-primary)]" />
                        {item}
                      </span>
                    ))}
                  </div>
                </div>
                <div className="flex flex-col gap-3 xl:min-w-[320px]">
                  <div className="rounded-2xl border border-[var(--sm-border)] bg-[var(--sm-surface-muted)] px-4 py-3">
                    <div className="flex items-center gap-2 text-sm font-medium text-[var(--sm-text)]">
                      <Bot className="h-4 w-4 text-[var(--sm-primary)]" />
                      推荐先用快速配置
                    </div>
                    <div className="mt-1 text-xs leading-5 text-[var(--sm-text-muted)]">
                      默认模板会自动带出常用参数，先跑通连接，再到设置里做高级调整。
                    </div>
                  </div>
                  <div className="flex flex-col gap-2 sm:flex-row sm:flex-wrap">
                    <button
                      data-testid="model-setup-hint-open-quick-setup"
                      onClick={openQuickModelSetup}
                      className="sm-btn sm-btn-primary min-h-11 flex-1 rounded-xl px-4 text-sm"
                    >
                      快速配置（1分钟）
                    </button>
                    <button
                      data-testid="model-setup-hint-open-settings"
                      onClick={openSettingsForModelSetup}
                      className="sm-btn sm-btn-secondary min-h-11 rounded-xl px-4 text-sm"
                    >
                      打开设置
                    </button>
                    <button
                      data-testid="model-setup-hint-dismiss"
                      onClick={dismissModelSetupHint}
                      className="sm-btn sm-btn-ghost min-h-11 rounded-xl px-4 text-sm"
                    >
                      稍后再说
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}
        {showQuickModelSetup && (
          <div
            data-testid="quick-model-setup-dialog"
            className="fixed inset-0 z-40 flex items-start justify-center overflow-y-auto bg-slate-950/30 px-4 py-4 backdrop-blur-sm sm:py-6"
            onMouseDown={(event) => {
              if (event.target === event.currentTarget) {
                closeQuickModelSetup();
              }
            }}
          >
            <div
              data-testid="quick-model-setup-panel"
              role="dialog"
              aria-modal="true"
              aria-labelledby="quick-model-setup-title"
              className="h-[calc(100vh-2rem)] w-full max-w-[1120px] max-h-[960px] overflow-hidden rounded-[28px] border border-white/80 bg-white shadow-[0_36px_120px_rgba(15,23,42,0.24)]"
              onMouseDown={(event) => event.stopPropagation()}
            >
              <div className="flex h-full min-h-0 flex-col lg:grid lg:grid-cols-[0.9fr_1.1fr]">
                <div className="relative overflow-hidden bg-[linear-gradient(180deg,#eff6ff_0%,#f8fafc_100%)] p-6 sm:p-7 lg:overflow-y-auto lg:p-6">
                  <div className="absolute inset-x-0 top-0 h-28 bg-[radial-gradient(circle_at_top,_rgba(37,99,235,0.18),_transparent_72%)]" />
                  <div className="relative">
                    <div className="inline-flex items-center gap-2 rounded-full bg-white/80 px-3 py-1 text-[11px] font-semibold text-[var(--sm-primary-strong)] shadow-[var(--sm-shadow-sm)]">
                      <Wand2 className="h-3.5 w-3.5" />
                      一次配置，后续复用
                    </div>
                    <div className="mt-4 text-2xl font-semibold tracking-tight text-[var(--sm-text)]">1 分钟完成模型接入</div>
                    <div className="mt-3 text-sm leading-6 text-[var(--sm-text-muted)]">
                      先选服务商模板，再填入 API Key。默认参数已经按常见场景预填好，连接通过后即可直接开始任务。
                    </div>
                    <div className="mt-5 space-y-3">
                      {MODEL_SETUP_STEPS.map((step, index) => (
                        <div
                          key={step.title}
                          className="flex items-start gap-3 rounded-2xl border border-white/70 bg-white/70 px-4 py-3 backdrop-blur-sm"
                        >
                          <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full bg-[var(--sm-primary)] text-sm font-semibold text-white">
                            {index + 1}
                          </div>
                          <div>
                            <div className="text-sm font-medium text-[var(--sm-text)]">{step.title}</div>
                            <div className="mt-1 text-xs leading-5 text-[var(--sm-text-muted)]">{step.description}</div>
                          </div>
                        </div>
                      ))}
                    </div>
                    <div className="mt-5 flex flex-wrap gap-2">
                      {MODEL_SETUP_OUTCOMES.map((item) => (
                        <span
                          key={item}
                          className="inline-flex items-center gap-1.5 rounded-full border border-white/80 bg-white/85 px-3 py-1.5 text-xs text-[var(--sm-text-muted)] shadow-[var(--sm-shadow-sm)]"
                        >
                          <BadgeCheck className="h-3.5 w-3.5 text-[var(--sm-primary)]" />
                          {item}
                        </span>
                      ))}
                    </div>
                  </div>
                </div>
                <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden p-6 sm:p-7 lg:p-8">
                  <div className="flex items-start justify-between gap-4">
                    <div>
                      <div id="quick-model-setup-title" className="text-xl font-semibold text-[var(--sm-text)]">
                        {quickSetupStep === "model" ? "快速配置模型" : "搜索引擎"}
                      </div>
                      <div className="mt-2 text-sm leading-6 text-[var(--sm-text-muted)]">
                        {quickSetupStep === "model"
                          ? "先完成模型接入，保存后自动进入搜索引擎配置。"
                          : "补齐搜索配置后，智能体即可在首次使用时直接联网检索。"}
                      </div>
                    </div>
                    <button
                      type="button"
                      data-testid="quick-model-setup-close"
                      onClick={closeQuickModelSetup}
                      disabled={!canDismissQuickModelSetup}
                      aria-label="关闭快速配置"
                      className="sm-btn sm-btn-ghost h-10 w-10 rounded-xl disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      <X className="h-4 w-4" />
                    </button>
                  </div>

                  <div
                    data-testid="quick-model-setup-scroll-region"
                    className="min-h-0 flex-1 overflow-y-auto pr-1"
                  >
                  {quickSetupStep === "search" && (
                    <div className="mt-6">
                      <SearchConfigForm
                        form={quickSearchForm}
                        onFormChange={(next) => {
                          setQuickSearchForm(next);
                          setQuickSearchError("");
                          setQuickSearchTestResult(null);
                        }}
                        onApplyPreset={applyQuickSearchPreset}
                        showApiKey={quickSearchApiKeyVisible}
                        onToggleApiKey={() => setQuickSearchApiKeyVisible((value) => !value)}
                        error={quickSearchError}
                        testResult={quickSearchTestResult}
                        testing={quickSearchTesting}
                        saving={quickSearchSaving}
                        onTest={testQuickSearchSetupConnection}
                        onSave={saveQuickSearchSetup}
                        panelClassName="space-y-3"
                        actionClassName="mt-4 grid grid-cols-1 gap-2 sm:grid-cols-3"
                        saveLabel="保存搜索配置"
                        onSecondaryAction={!isBlockingInitialModelSetup ? skipQuickSearchSetup : undefined}
                        secondaryActionLabel={!isBlockingInitialModelSetup ? "稍后配置搜索" : undefined}
                      />
                    </div>
                  )}
                  {quickSetupStep === "model" && (
                  <div>
                  <div className="mt-6">
                    <div className="flex items-center justify-between gap-3">
                      <div className="sm-field-label mb-0">推荐模板</div>
                      <div className="text-[11px] text-[var(--sm-text-muted)]">先选模板，再补 API Key</div>
                    </div>
                    <div className="mt-2 grid grid-cols-1 gap-2 sm:grid-cols-2">
                      {MODEL_PROVIDER_CATALOG.map((provider) => {
                        const isActive = quickModelPresetKey === provider.id;
                        return (
                          <button
                            key={provider.id}
                            type="button"
                            onClick={() => applyQuickModelPreset(provider.id)}
                            className={`text-left rounded-2xl border px-3 py-3 transition-colors ${
                              isActive
                                ? "border-[var(--sm-primary)] bg-[var(--sm-primary-soft)] shadow-[var(--sm-shadow-sm)]"
                                : "border-[var(--sm-border)] bg-white hover:border-[var(--sm-primary)] hover:bg-[var(--sm-surface-soft)]"
                            }`}
                          >
                            <div className="flex items-start justify-between gap-3">
                              <div>
                                <div className="text-[11px] font-semibold text-[var(--sm-primary-strong)]">{provider.badge}</div>
                                <div className="mt-1 text-sm font-medium text-[var(--sm-text)]">{provider.label}</div>
                              </div>
                              {provider.id === DEFAULT_MODEL_PROVIDER_ID ? (
                                <span className="sm-badge-info">推荐</span>
                              ) : null}
                            </div>
                            <div className="mt-2 text-xs leading-5 text-[var(--sm-text-muted)]">{provider.helper}</div>
                          </button>
                        );
                      })}
                    </div>
                  </div>

                  <div className="mt-6 grid grid-cols-1 gap-3 sm:grid-cols-2">
                    <div>
                      <label className="sm-field-label">服务商</label>
                      <select
                        data-testid="quick-model-setup-preset"
                        value={quickModelPresetKey}
                        onChange={(e) => applyQuickModelPreset(e.target.value)}
                        className="sm-select h-11 bg-white px-3 text-sm"
                      >
                        {MODEL_PROVIDER_CATALOG.map((provider) => (
                          <option key={provider.id} value={provider.id}>
                            {provider.label}
                          </option>
                        ))}
                      </select>
                    </div>
                    <div>
                      <label className="sm-field-label">连接名称</label>
                      <input
                        value={quickModelForm.name}
                        onChange={(e) => {
                          setQuickModelForm((s) => ({ ...s, name: e.target.value }));
                          setQuickModelTestResult(null);
                        }}
                        className="sm-input h-11 px-3 text-sm"
                      />
                    </div>
                    <div>
                      <label className="sm-field-label">Base URL</label>
                      <input
                        data-testid="quick-model-setup-base-url"
                        value={quickModelForm.base_url}
                        onChange={(e) => {
                          setQuickModelForm((s) => ({ ...s, base_url: e.target.value }));
                          setQuickModelTestResult(null);
                        }}
                        className="sm-input h-11 px-3 text-sm"
                        placeholder={selectedQuickModelProvider.baseUrlPlaceholder}
                      />
                    </div>
                    <div>
                      <label className="sm-field-label">模型名</label>
                      <input
                        data-testid="quick-model-setup-model-name"
                        value={quickModelForm.model_name}
                        onChange={(e) => {
                          setQuickModelForm((s) => ({ ...s, model_name: e.target.value }));
                          setQuickModelTestResult(null);
                        }}
                        className="sm-input h-11 px-3 text-sm"
                        placeholder={selectedQuickModelProvider.modelNamePlaceholder}
                      />
                    </div>
                  </div>

                  <div className="mt-4 rounded-2xl border border-[var(--sm-border)] bg-[var(--sm-surface-muted)] px-4 py-4">
                    <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                      <div>
                        <div className="flex flex-wrap items-center gap-2">
                          <div className="text-sm font-medium text-[var(--sm-text)]">
                            {selectedQuickModelProvider.label}
                          </div>
                          <span className="inline-flex items-center rounded-full bg-white px-2.5 py-1 text-[11px] font-medium text-[var(--sm-primary-strong)]">
                            {selectedQuickModelProvider.protocolLabel}
                          </span>
                        </div>
                        <div className="mt-2 text-xs leading-5 text-[var(--sm-text-muted)]">
                          {selectedQuickModelProvider.helper}
                        </div>
                      </div>
                      {selectedQuickModelProvider.officialConsoleUrl ? (
                        <div className="flex flex-wrap gap-2">
                          <button
                            type="button"
                            onClick={() =>
                              openExternalUrl(selectedQuickModelProvider.officialConsoleUrl ?? "").catch(
                                (error) => {
                                  setQuickModelError(
                                    extractErrorMessage(error, "打开外部链接失败，请稍后重试"),
                                  );
                                },
                              )
                            }
                            className="sm-btn sm-btn-secondary min-h-10 rounded-xl px-4 text-sm"
                          >
                            {selectedQuickModelProvider.officialConsoleLabel ?? "获取 API Key"}
                          </button>
                          {selectedQuickModelProvider.officialDocsUrl ? (
                            <button
                              type="button"
                              onClick={() =>
                                openExternalUrl(selectedQuickModelProvider.officialDocsUrl ?? "").catch(
                                  (error) => {
                                    setQuickModelError(
                                      extractErrorMessage(error, "打开外部链接失败，请稍后重试"),
                                    );
                                  },
                                )
                              }
                              className="sm-btn sm-btn-ghost min-h-10 rounded-xl px-4 text-sm"
                            >
                              {selectedQuickModelProvider.officialDocsLabel ?? "查看文档"}
                            </button>
                          ) : null}
                        </div>
                      ) : null}
                    </div>
                    {selectedQuickModelProvider.isCustom ? (
                      <div
                        data-testid="quick-model-setup-custom-guidance"
                        className="mt-3 rounded-2xl border border-dashed border-[var(--sm-border)] bg-white px-3 py-3"
                      >
                        <div className="text-xs font-semibold text-[var(--sm-text)]">
                          {selectedQuickModelProvider.customGuidanceTitle}
                        </div>
                        <div className="mt-2 space-y-1.5 text-[12px] leading-5 text-[var(--sm-text-muted)]">
                          {selectedQuickModelProvider.customGuidanceLines?.map((line) => (
                            <div key={line}>{line}</div>
                          ))}
                        </div>
                      </div>
                    ) : null}
                  </div>

                  <div className="mt-3">
                    <label className="sm-field-label">API Key</label>
                    <div className="relative">
                      <input
                        ref={quickModelApiKeyInputRef}
                        data-testid="quick-model-setup-api-key"
                        type={quickModelApiKeyVisible ? "text" : "password"}
                        value={quickModelForm.api_key}
                        onChange={(e) => {
                          setQuickModelForm((s) => ({ ...s, api_key: e.target.value }));
                          setQuickModelTestResult(null);
                        }}
                        className="sm-input h-11 px-3 pr-12 text-sm"
                        placeholder="请输入 API Key"
                      />
                      <button
                        type="button"
                        data-testid="quick-model-setup-toggle-api-key-visibility"
                        onClick={() => setQuickModelApiKeyVisible((prev) => !prev)}
                        aria-label={quickModelApiKeyVisible ? "隐藏 API Key" : "显示 API Key"}
                        className="sm-btn sm-btn-ghost absolute right-1 top-1/2 h-9 w-9 -translate-y-1/2 rounded-lg"
                      >
                        {quickModelApiKeyVisible ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                      </button>
                    </div>
                    <div className="mt-2 flex items-start gap-2 rounded-2xl border border-[var(--sm-border)] bg-[var(--sm-surface-muted)] px-3 py-3 text-[12px] leading-5 text-[var(--sm-text-muted)]">
                      <KeyRound className="mt-0.5 h-4 w-4 flex-shrink-0 text-[var(--sm-primary)]" />
                      API Key 仅用于当前模型连接。若你更熟悉高级配置，也可以直接进入设置页调整更多参数。
                    </div>
                  </div>

                  {quickModelTestResult !== null && (
                    <div
                      data-testid="quick-model-setup-test-result"
                      className={`mt-4 flex items-start gap-2 rounded-2xl border px-3 py-3 text-xs ${
                        quickModelTestResult
                          ? "border-green-200 bg-green-50 text-green-700"
                          : "border-orange-200 bg-orange-50 text-orange-700"
                      }`}
                    >
                      {quickModelTestResult ? (
                        <CheckCircle2 className="mt-0.5 h-4 w-4 flex-shrink-0" />
                      ) : (
                        <CircleAlert className="mt-0.5 h-4 w-4 flex-shrink-0" />
                      )}
                      <span>{quickModelTestResult ? "连接成功，可直接保存并开始" : "连接失败，请检查后重试"}</span>
                    </div>
                  )}
                  {quickModelError && (
                    <div className="mt-4 flex items-start gap-2 rounded-2xl border border-red-200 bg-red-50 px-3 py-3 text-xs text-red-700">
                      <CircleAlert className="mt-0.5 h-4 w-4 flex-shrink-0" />
                      <span>{quickModelError}</span>
                    </div>
                  )}

                  </div>
                  )}
                  </div>

                  <div className="mt-6 border-t border-[var(--sm-border)] pt-4">
                    <div className="text-xs leading-5 text-[var(--sm-text-muted)]">
                      {isBlockingInitialModelSetup
                        ? "首次使用至少完成模型与搜索配置后，才能关闭这个向导。"
                        : "按 Esc 或点击遮罩可以关闭此弹窗。"}
                    </div>
                    <div data-testid="quick-model-setup-actions" className="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-2">
                      <button
                        type="button"
                        data-testid="quick-model-setup-cancel"
                        onClick={closeQuickModelSetup}
                        disabled={!canDismissQuickModelSetup}
                        className="sm-btn sm-btn-ghost min-h-11 rounded-xl px-4 text-sm disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        {isBlockingInitialModelSetup ? "完成后可关闭" : "取消"}
                      </button>
                      <button
                        type="button"
                        onClick={openSettingsForModelSetup}
                        disabled={isQuickSetupBusy}
                        className="sm-btn sm-btn-secondary min-h-11 rounded-xl px-4 text-sm disabled:opacity-60"
                      >
                        <Settings2 className="h-4 w-4" />
                        打开设置
                      </button>
                      {quickSetupStep === "model" && (
                        <>
                          <button
                            data-testid="quick-model-setup-test-connection"
                            onClick={testQuickModelSetupConnection}
                            disabled={quickModelSaving || quickModelTesting}
                            className="sm-btn sm-btn-secondary min-h-11 rounded-xl px-4 text-sm disabled:opacity-60"
                          >
                            {quickModelTesting ? "测试中..." : "测试连接"}
                          </button>
                          <button
                            data-testid="quick-model-setup-save"
                            onClick={saveQuickModelSetup}
                            disabled={quickModelSaving || quickModelTesting}
                            className="sm-btn sm-btn-primary min-h-11 rounded-xl px-4 text-sm disabled:opacity-60"
                          >
                            <ChevronRight className="h-4 w-4" />
                            {quickModelSaving ? "保存中..." : "保存并继续"}
                          </button>
                        </>
                      )}
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}
        {shouldShowModelSetupGate && (
          <div
            data-testid="model-setup-gate"
            className="fixed inset-0 z-30 flex items-center justify-center bg-[radial-gradient(circle_at_top,_rgba(37,99,235,0.16),_rgba(241,245,249,0.92)_46%,_rgba(241,245,249,0.98)_100%)] px-4 py-6 backdrop-blur-sm"
          >
            <div className="w-full max-w-4xl overflow-hidden rounded-[32px] border border-white/80 bg-white shadow-[0_40px_120px_rgba(15,23,42,0.18)]">
              <div className="grid gap-6 p-6 lg:grid-cols-[1.2fr_0.8fr] lg:p-8">
                <div>
                  <div className="inline-flex items-center gap-2 rounded-full bg-[var(--sm-primary-soft)] px-3 py-1 text-[11px] font-semibold text-[var(--sm-primary-strong)]">
                    <Sparkles className="h-3.5 w-3.5" />
                    首次启动必做一步
                  </div>
                  <div className="mt-4 text-[30px] font-semibold leading-tight tracking-tight text-[var(--sm-text)]">
                    首次使用需要先连接一个大模型
                  </div>
                  <div className="mt-3 max-w-2xl text-base leading-7 text-[var(--sm-text-muted)]">
                    完成模型配置后，才能开始任务、创建会话并驱动智能体员工执行技能。现在只需 1 分钟。
                  </div>
                  <div className="mt-5 flex flex-wrap gap-2">
                    {MODEL_SETUP_OUTCOMES.map((item) => (
                      <span
                        key={item}
                        className="inline-flex items-center gap-1.5 rounded-full border border-[var(--sm-border)] bg-[var(--sm-surface-muted)] px-3 py-1.5 text-xs text-[var(--sm-text-muted)]"
                      >
                        <BadgeCheck className="h-3.5 w-3.5 text-[var(--sm-primary)]" />
                        {item}
                      </span>
                    ))}
                  </div>
                  <div className="mt-6 flex flex-col gap-2 sm:flex-row sm:flex-wrap">
                    <button
                      data-testid="model-setup-gate-open-quick-setup"
                      onClick={openQuickModelSetup}
                      className="sm-btn sm-btn-primary min-h-12 rounded-xl px-5 text-sm"
                    >
                      快速配置（1分钟）
                    </button>
                    <button
                      data-testid="model-setup-gate-open-settings"
                      onClick={openSettingsForModelSetup}
                      className="sm-btn sm-btn-secondary min-h-12 rounded-xl px-5 text-sm"
                    >
                      打开设置
                    </button>
                  </div>
                </div>
                <div className="rounded-[26px] border border-[var(--sm-border)] bg-[var(--sm-surface-muted)] p-5">
                  <div className="flex items-center gap-2 text-sm font-medium text-[var(--sm-text)]">
                    <Bot className="h-4 w-4 text-[var(--sm-primary)]" />
                    推荐流程
                  </div>
                  <div className="mt-1 text-xs leading-5 text-[var(--sm-text-muted)]">
                    优先选择快速配置，模板会自动补齐常用 URL 和模型名。
                  </div>
                  <div className="mt-4 space-y-3">
                    {MODEL_SETUP_STEPS.map((step, index) => (
                      <div key={step.title} className="flex items-start gap-3">
                        <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full bg-white text-sm font-semibold text-[var(--sm-primary-strong)] shadow-[var(--sm-shadow-sm)]">
                          {index + 1}
                        </div>
                        <div>
                          <div className="text-sm font-medium text-[var(--sm-text)]">{step.title}</div>
                          <div className="mt-1 text-xs leading-5 text-[var(--sm-text-muted)]">{step.description}</div>
                        </div>
                      </div>
                    ))}
                  </div>
                  <div className="mt-5 rounded-2xl border border-white bg-white px-4 py-3">
                    <div className="text-xs font-semibold text-[var(--sm-text)]">支持模板</div>
                    <div className="mt-2 flex flex-wrap gap-2">
                      {MODEL_PROVIDER_CATALOG.map((provider) => (
                        <span
                          key={provider.id}
                          className="inline-flex items-center rounded-full bg-[var(--sm-primary-soft)] px-2.5 py-1 text-[11px] font-medium text-[var(--sm-primary-strong)]"
                        >
                          {provider.label}
                        </span>
                      ))}
                    </div>
                  </div>
                </div>
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
                showDevModelSetupTools={SHOW_DEV_MODEL_SETUP_TOOLS}
                onDevResetFirstUseOnboarding={resetFirstUseOnboardingForDevelopment}
                onDevOpenQuickModelSetup={openInitialModelSetupGate}
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
                highlightEmployeeId={employeeCreatorHighlight?.employeeId ?? null}
                highlightMessage={
                  employeeCreatorHighlight
                    ? `已由${EMPLOYEE_ASSISTANT_DISPLAY_NAME}生成：${employeeCreatorHighlight.name}`
                    : null
                }
                onDismissHighlight={() => setEmployeeCreatorHighlight(null)}
                onSelectEmployee={setSelectedEmployeeId}
                onSaveEmployee={handleSaveEmployee}
                onDeleteEmployee={handleDeleteEmployee}
                onSetAsMainAndEnter={handleSetAsMainAndEnter}
                onStartTaskWithEmployee={handleStartTaskWithEmployee}
                onOpenGroupRunSession={handleOpenGroupRunSession}
                onEmployeeGroupsChanged={() => {
                  void loadEmployeeGroups();
                }}
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
                onOpenSession={(nextSessionId, options) => {
                  const focusHint = (options?.focusHint || "").trim();
                  const groupRunStepFocusId = (options?.groupRunStepFocusId || "").trim();
                  const groupRunEventFocusId = (options?.groupRunEventFocusId || "").trim();
                  setPendingSessionFocusRequest(
                    focusHint
                      ? {
                          sessionId: nextSessionId,
                          snippet: focusHint,
                          nonce: Date.now(),
                        }
                      : null,
                  );
                  setPendingGroupRunStepFocusRequest(
                    groupRunStepFocusId
                      ? {
                          sessionId: nextSessionId,
                          stepId: groupRunStepFocusId,
                          eventId: groupRunEventFocusId || undefined,
                          nonce: Date.now(),
                        }
                      : null,
                  );
                  const sourceSessionId = (options?.sourceSessionId || "").trim();
                  const sourceStepId = (options?.sourceStepId || "").trim();
                  const sourceStepTimeline = (options?.sourceStepTimeline || [])
                    .map((item) => ({
                      eventId: (item?.eventId || "").trim() || undefined,
                      label: (item?.label || "").trim(),
                      createdAt: (item?.createdAt || "").trim() || undefined,
                    }))
                    .filter((item) => item.label.length > 0);
                  setPendingSessionExecutionContext(
                    sourceSessionId && sourceStepId
                      ? {
                          targetSessionId: nextSessionId,
                          sourceSessionId,
                          sourceStepId,
                          sourceEmployeeId: (options?.sourceEmployeeId || "").trim() || undefined,
                          assigneeEmployeeId: (options?.assigneeEmployeeId || "").trim() || undefined,
                          sourceStepTimeline: sourceStepTimeline.length > 0 ? sourceStepTimeline : undefined,
                        }
                      : null,
                  );
                  return handleOpenGroupRunSession(nextSessionId, selectedSkill.id);
                }}
                sessionFocusRequest={
                  pendingSessionFocusRequest &&
                  pendingSessionFocusRequest.sessionId === selectedSessionId
                    ? {
                        nonce: pendingSessionFocusRequest.nonce,
                        snippet: pendingSessionFocusRequest.snippet,
                      }
                    : undefined
                }
                groupRunStepFocusRequest={
                  pendingGroupRunStepFocusRequest &&
                  pendingGroupRunStepFocusRequest.sessionId === selectedSessionId
                    ? {
                        nonce: pendingGroupRunStepFocusRequest.nonce,
                        stepId: pendingGroupRunStepFocusRequest.stepId,
                        eventId: pendingGroupRunStepFocusRequest.eventId,
                      }
                    : undefined
                }
                sessionExecutionContext={
                  pendingSessionExecutionContext &&
                  pendingSessionExecutionContext.targetSessionId === selectedSessionId
                    ? {
                        sourceSessionId: pendingSessionExecutionContext.sourceSessionId,
                        sourceStepId: pendingSessionExecutionContext.sourceStepId,
                        sourceEmployeeId: pendingSessionExecutionContext.sourceEmployeeId,
                        assigneeEmployeeId: pendingSessionExecutionContext.assigneeEmployeeId,
                        sourceStepTimeline: pendingSessionExecutionContext.sourceStepTimeline,
                      }
                    : undefined
                }
                onReturnToSourceSession={(sourceSessionId) => {
                  setPendingGroupRunStepFocusRequest(null);
                  setPendingSessionExecutionContext(null);
                  return handleOpenGroupRunSession(sourceSessionId, selectedSkill.id);
                }}
                sessionSourceChannel={selectedSession?.source_channel}
                sessionSourceLabel={selectedSession?.source_label}
                sessionTitle={selectedSession?.title}
                sessionMode={selectedSession?.session_mode}
                operationPermissionMode={operationPermissionMode}
                onSessionUpdate={handleSessionRefresh}
                installedSkillIds={skills.map((s) => s.id)}
                onSkillInstalled={handleSkillInstalledFromChat}
                suppressAskUserPrompt={selectedSessionImManaged}
                initialMessage={
                  pendingInitialMessage && pendingInitialMessage.sessionId === selectedSessionId
                    ? pendingInitialMessage.message
                    : undefined
                }
                quickPrompts={
                  selectedSkill.id === BUILTIN_EMPLOYEE_CREATOR_SKILL_ID
                    ? EMPLOYEE_ASSISTANT_QUICK_PROMPTS
                    : []
                }
                employeeAssistantContext={
                  selectedEmployeeAssistantContext
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
                teams={landingTeams}
                creating={creatingSession}
                error={createSessionError}
                onSelectSession={setSelectedSessionId}
                onCreateSessionWithInitialMessage={handleCreateSession}
                onCreateTeamEntrySession={handleCreateTeamEntrySession}
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
