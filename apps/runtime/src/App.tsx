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
    if (
      typeof window === "undefined" ||
      !(window as unknown as { __TAURI_INTERNALS__?: { transformCallback?: unknown } })
        .__TAURI_INTERNALS__?.transformCallback
    ) {
      return;
    }
    const seen = new Set<string>();
    const unlistenPromise = listen<ImRoleDispatchRequest>("im-role-dispatch-request", async ({ payload }) => {
      const key = `${payload.session_id}|${payload.role_id}|${payload.prompt}`;
      if (seen.has(key)) return;
      seen.add(key);
      try {
        await invoke("send_message", {
          sessionId: payload.session_id,
          userMessage: payload.prompt,
        });

        const messages = await invoke<Message[]>("get_messages", {
          sessionId: payload.session_id,
        });
        const latestAssistant = [...messages]
          .reverse()
          .find((m) => m.role === "assistant" && m.content?.trim().length > 0);
        if (latestAssistant) {
          await invoke("send_feishu_text_message", {
            chatId: payload.thread_id,
            text: `${payload.role_name}: ${latestAssistant.content.slice(0, 1800)}`,
            appId: null,
            appSecret: null,
            sidecarBaseUrl: null,
          });
        }
      } catch (e) {
        console.error("IM 分发执行失败:", e);
      } finally {
        setTimeout(() => seen.delete(key), 30_000);
      }
    });
    return () => {
      unlistenPromise.then((fn) => fn());
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
      const list = await invoke<AgentEmployee[]>("list_agent_employees");
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

    // 弹出目录选择器
    const dir = await open({ directory: true, title: "选择工作目录" });
    if (!dir || typeof dir !== "string") return; // 用户取消

    setCreatingSession(true);
    setCreateSessionError(null);
    try {
      const selectedEmployee = employees.find((e) => e.id === selectedEmployeeId);
      const chosenSkill = selectedSkillId || selectedEmployee?.primary_skill_id || BUILTIN_GENERAL_SKILL_ID;
      const id = await invoke<string>("create_session", {
        skillId: chosenSkill,
        modelId,
        workDir: dir,
        permissionMode: newSessionPermissionMode,
      });
      setSelectedSessionId(id);
      if (selectedSkillId) await loadSessions(selectedSkillId);

      const firstMessage = initialMessage.trim();
      if (firstMessage) {
        try {
          await invoke("send_message", { sessionId: id, userMessage: firstMessage });
        } catch (sendError) {
          console.error("自动发送首条消息失败:", sendError);
        }
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
  async function handleInstalled(skillId: string) {
    await loadSkills();
    setSelectedSkillId(skillId);
    const modelId = models[0]?.id;
    if (modelId) {
      const dir = await open({ directory: true, title: "选择工作目录" });
      if (!dir || typeof dir !== "string") return;
      try {
        const sessionId = await invoke<string>("create_session", {
          skillId,
          modelId,
          workDir: dir,
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
    const result = await invoke<{ manifest: SkillManifest; missing_mcp: string[] }>("install_clawhub_skill", {
      slug,
      githubUrl: null,
    });
    await loadSkills();
    if (result?.manifest?.id) {
      setSelectedSkillId(result.manifest.id);
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
    if (selectedSkillId) loadSessions(selectedSkillId);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedSkillId]);

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
    const target = input.id
      ? latest.find((e) => e.id === input.id)
      : latest.find((e) => e.name === input.name && e.role_id === input.role_id);
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
        name: employee.name,
        role_id: employee.role_id,
        persona: employee.persona,
        feishu_open_id: employee.feishu_open_id,
        feishu_app_id: employee.feishu_app_id,
        feishu_app_secret: employee.feishu_app_secret,
        primary_skill_id: employee.primary_skill_id,
        default_work_dir: employee.default_work_dir,
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

  function handleStartTaskWithSkill(skillId: string) {
    setSelectedSkillId(skillId);
    setSelectedSessionId(null);
    setCreateSessionError(null);
    navigate("start-task");
  }

  const selectedSkill = skills.find((s) => s.id === selectedSkillId) ?? null;
  const selectedSession = sessions.find((s) => s.id === selectedSessionId);

  return (
    <div className="flex h-screen bg-gray-50 text-gray-800 overflow-hidden">
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
                onOpenExperts={() => navigate("experts")}
              />
            </motion.div>
          ) : selectedSkill && models.length === 0 ? (
            <div className="flex items-center justify-center h-full text-gray-400 text-sm">
              请先在设置中配置模型和 API Key
            </div>
          ) : (
            <div className="flex items-center justify-center h-full text-gray-400 text-sm">
              从左侧选择一个技能，开始任务
            </div>
          )}
        </AnimatePresence>
      </div>
      {showInstall && (
        <InstallDialog onInstalled={handleInstalled} onClose={() => setShowInstall(false)} />
      )}
    </div>
  );
}
