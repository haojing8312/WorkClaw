import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { useCallback } from "react";
import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import { buildSessionExportFilename } from "../lib/session-export-filename";
import { getDefaultModelId } from "../lib/default-model";
import type {
  PersistedChatRuntimeState,
  SessionInfo,
  SkillManifest,
  ModelConfig,
} from "../types";

type WorkTab =
  | {
      id: string;
      kind: "start-task";
    }
  | {
      id: string;
      kind: "session";
      sessionId: string;
    };

export function useSessionSidebarCoordinator(options: {
  sessions: SessionInfo[];
  visibleSessions: SessionInfo[];
  selectedSessionId: string | null;
  selectedSkillId: string | null;
  activeMainView: "start-task" | "experts" | "experts-new" | "packaging" | "employees";
  skillActionLoadSessions: (skillId: string) => Promise<void> | void;
  handleSelectSession: (
    sessionId: string,
    options?: { openChatView?: boolean },
  ) => void;
  getAdjacentSessionId: (list: SessionInfo[], sessionId: string) => string | null;
  openStartTaskInActiveTab: () => void;
  setSessions: Dispatch<SetStateAction<SessionInfo[]>>;
  setTabs: Dispatch<SetStateAction<WorkTab[]>>;
  setEmployeeAssistantSessionContexts: Dispatch<
    SetStateAction<Record<string, { mode: "create" | "update"; employeeName?: string; employeeCode?: string }>>
  >;
  setLiveSessionRuntimeStatusById: Dispatch<SetStateAction<Record<string, string>>>;
  setSessionRuntimeStateById: Dispatch<SetStateAction<Record<string, PersistedChatRuntimeState>>>;
  searchTimerRef: MutableRefObject<ReturnType<typeof setTimeout> | null>;
  loadSkills: () => Promise<SkillManifest[]>;
  setSelectedSkillId: React.Dispatch<React.SetStateAction<string | null>>;
  models: ModelConfig[];
  prepareTabForNewTask: () => string;
  resolveSessionLaunchWorkDir: (preferredWorkDir?: string) => Promise<string>;
  createRuntimeSession: (input: {
    skillId: string;
    modelId: string;
    workDir?: string;
    employeeId?: string;
    title?: string;
    sessionMode: "general" | "employee_direct" | "team_entry";
    teamId?: string;
  }) => Promise<string>;
  replaceTab: (tabId: string, nextTab: WorkTab) => void;
  createSessionTab: (sessionId: string, id?: string) => WorkTab;
  setActiveTabId: Dispatch<SetStateAction<string>>;
}) {
  const {
    activeMainView,
    createRuntimeSession,
    createSessionTab,
    getAdjacentSessionId,
    handleSelectSession,
    loadSkills,
    models,
    openStartTaskInActiveTab,
    prepareTabForNewTask,
    replaceTab,
    resolveSessionLaunchWorkDir,
    searchTimerRef,
    selectedSessionId,
    selectedSkillId,
    sessions,
    setActiveTabId,
    setEmployeeAssistantSessionContexts,
    setLiveSessionRuntimeStatusById,
    setSelectedSkillId,
    setSessionRuntimeStateById,
    setSessions,
    setTabs,
    skillActionLoadSessions,
    visibleSessions,
  } = options;

  const handleDeleteSession = useCallback(
    async (sessionId: string) => {
      const deletingSelectedSession = selectedSessionId === sessionId;
      const fallbackSessionId = deletingSelectedSession
        ? getAdjacentSessionId(sessions, sessionId)
        : null;
      try {
        await invoke("delete_session", { sessionId });
        setSessions((prev) => prev.filter((item) => item.id !== sessionId));
        setTabs((prev) =>
          prev.map((tab) =>
            tab.kind === "session" && tab.sessionId === sessionId
              ? { id: tab.id, kind: "start-task" as const }
              : tab,
          ),
        );
        if (deletingSelectedSession) {
          if (fallbackSessionId) {
            handleSelectSession(fallbackSessionId, {
              openChatView: activeMainView === "start-task",
            });
          } else {
            openStartTaskInActiveTab();
          }
        }
        setEmployeeAssistantSessionContexts((prev) => {
          if (!prev[sessionId]) return prev;
          const next = { ...prev };
          delete next[sessionId];
          return next;
        });
        setLiveSessionRuntimeStatusById((prev) => {
          if (!prev[sessionId]) return prev;
          const next = { ...prev };
          delete next[sessionId];
          return next;
        });
        setSessionRuntimeStateById((prev) => {
          if (!prev[sessionId]) return prev;
          const next = { ...prev };
          delete next[sessionId];
          return next;
        });
        if (selectedSkillId) {
          await skillActionLoadSessions(selectedSkillId);
        }
      } catch (error) {
        console.error("删除会话失败:", error);
      }
    },
    [
      activeMainView,
      getAdjacentSessionId,
      handleSelectSession,
      openStartTaskInActiveTab,
      selectedSessionId,
      selectedSkillId,
      sessions,
      setEmployeeAssistantSessionContexts,
      setLiveSessionRuntimeStatusById,
      setSessionRuntimeStateById,
      setSessions,
      setTabs,
      skillActionLoadSessions,
    ],
  );

  const handleSearchSessions = useCallback(
    (query: string) => {
      if (searchTimerRef.current) {
        clearTimeout(searchTimerRef.current);
      }
      if (!selectedSkillId) return;

      if (!query.trim()) {
        searchTimerRef.current = setTimeout(() => {
          void skillActionLoadSessions(selectedSkillId);
        }, 100);
        return;
      }

      searchTimerRef.current = setTimeout(async () => {
        try {
          const results = await invoke<SessionInfo[]>("search_sessions_global", {
            query: query.trim(),
          });
          setSessions(Array.isArray(results) ? results : []);
        } catch (error) {
          console.error("搜索会话失败:", error);
        }
      }, 300);
    },
    [searchTimerRef, selectedSkillId, setSessions, skillActionLoadSessions],
  );

  const handleExportSession = useCallback(
    async (sessionId: string) => {
      try {
        const md = await invoke<string>("export_session", { sessionId });
        const session =
          visibleSessions.find((item) => item.id === sessionId) ?? null;
        const filePath = await save({
          defaultPath: buildSessionExportFilename(session),
          filters: [{ name: "Markdown", extensions: ["md"] }],
        });
        if (filePath) {
          await invoke("write_export_file", { path: filePath, content: md });
        }
      } catch (error) {
        console.error("导出会话失败:", error);
      }
    },
    [visibleSessions],
  );

  const handleInstalled = useCallback(
    async (skillId: string, options?: { createSession?: boolean }) => {
      await loadSkills();
      setSelectedSkillId(skillId);
      if (options?.createSession === false) {
        return;
      }
      const modelId = getDefaultModelId(models);
      if (!modelId) {
        return;
      }
      try {
        const targetTabId = prepareTabForNewTask();
        const workDir = await resolveSessionLaunchWorkDir();
        const sessionId = await createRuntimeSession({
          skillId,
          modelId,
          workDir,
          sessionMode: "general",
        });
        replaceTab(targetTabId, createSessionTab(sessionId, targetTabId));
        setActiveTabId(targetTabId);
        void skillActionLoadSessions(skillId);
      } catch (error) {
        console.error("自动创建会话失败:", error);
      }
    },
    [
      createRuntimeSession,
      createSessionTab,
      loadSkills,
      models,
      prepareTabForNewTask,
      replaceTab,
      resolveSessionLaunchWorkDir,
      setActiveTabId,
      setSelectedSkillId,
      skillActionLoadSessions,
    ],
  );

  return {
    handleDeleteSession,
    handleExportSession,
    handleInstalled,
    handleSearchSessions,
  };
}
