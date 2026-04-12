import { useCallback } from "react";
import { useImBridgeIntegration } from "./useImBridgeIntegration";
import { useSessionSidebarCoordinator } from "./useSessionSidebarCoordinator";
import type {
  ModelConfig,
  PersistedChatRuntimeState,
  SessionInfo,
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

export function useAppSidebarControllers(options: {
  activeMainView: "start-task" | "experts" | "experts-new" | "packaging" | "employees";
  createRuntimeSession: (input: {
    skillId: string;
    modelId: string;
    workDir?: string;
    employeeId?: string;
    title?: string;
    sessionMode: "general" | "employee_direct" | "team_entry";
    teamId?: string;
  }) => Promise<string>;
  createSessionTab: (sessionId: string, id?: string) => WorkTab;
  getAdjacentSessionId: (list: SessionInfo[], sessionId: string) => string | null;
  handleSelectSession: (sessionId: string, options?: { openChatView?: boolean }) => void;
  loadSessions: (skillId: string) => Promise<void> | void;
  loadSkills: () => Promise<unknown>;
  models: ModelConfig[];
  openStartTaskInActiveTab: () => void;
  prepareTabForNewTask: () => string;
  replaceTab: (tabId: string, nextTab: WorkTab) => void;
  resolveSessionLaunchWorkDir: (preferredWorkDir?: string) => Promise<string>;
  searchTimerRef: React.MutableRefObject<ReturnType<typeof setTimeout> | null>;
  selectedSessionId: string | null;
  selectedSkillId: string | null;
  sessions: SessionInfo[];
  setActiveTabId: React.Dispatch<React.SetStateAction<string>>;
  setEmployeeAssistantSessionContexts: React.Dispatch<
    React.SetStateAction<Record<string, { mode: "create" | "update"; employeeName?: string; employeeCode?: string }>>
  >;
  setImManagedSessionIds: React.Dispatch<React.SetStateAction<string[]>>;
  setLiveSessionRuntimeStatusById: React.Dispatch<React.SetStateAction<Record<string, string>>>;
  setSelectedSkillId: React.Dispatch<React.SetStateAction<string | null>>;
  setSessionRuntimeStateById: React.Dispatch<
    React.SetStateAction<Record<string, PersistedChatRuntimeState>>
  >;
  setSessions: React.Dispatch<React.SetStateAction<SessionInfo[]>>;
  setTabs: React.Dispatch<React.SetStateAction<WorkTab[]>>;
  visibleSessions: SessionInfo[];
}) {
  const {
    selectedSkillId,
    setImManagedSessionIds,
    loadSessions,
    ...sidebarOptions
  } = options;

  const refreshImSessionList = useCallback(() => {
    void loadSessions(selectedSkillId ?? "");
  }, [loadSessions, selectedSkillId]);

  useImBridgeIntegration({
    setImManagedSessionIds,
    refreshSessionList: refreshImSessionList,
  });

  return useSessionSidebarCoordinator({
    ...sidebarOptions,
    selectedSkillId,
    skillActionLoadSessions: (skillId) => loadSessions(skillId),
  });
}
