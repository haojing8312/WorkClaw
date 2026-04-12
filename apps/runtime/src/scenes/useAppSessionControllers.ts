import { useCallback } from "react";
import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import { useRuntimeSessionCoordinator } from "./useRuntimeSessionCoordinator";
import { useSessionDisplayState } from "./useSessionDisplayState";
import { useSessionRuntimeStateCoordinator } from "./useSessionRuntimeStateCoordinator";
import { useSessionViewCoordinator } from "./useSessionViewCoordinator";
import type {
  AgentEmployee,
  EmployeeGroup,
  PersistedChatRuntimeState,
  SessionInfo,
  SkillManifest,
} from "../types";
import type { EmployeeAssistantSessionContext } from "./employees/employeeAssistantService";
import type { MainView } from "./useAppUiState";
import type { WorkTab } from "../session-bootstrap";

export function useAppSessionControllers(options: {
  activeMainView: MainView;
  activeTabId: string;
  createSessionTab: (sessionId: string, id?: string) => WorkTab;
  createStartTaskTab: (id?: string) => WorkTab;
  employeeAssistantSessionContexts: Record<string, EmployeeAssistantSessionContext>;
  employeeGroups: EmployeeGroup[];
  employees: AgentEmployee[];
  hasLoadedSessionsRef: MutableRefObject<boolean>;
  hasResolvedInitialPersistedSessionRef: MutableRefObject<boolean>;
  imManagedSessionIds: string[];
  initialPersistedSessionIdRef: MutableRefObject<string | null>;
  initialSelectedSessionSnapshotRef: MutableRefObject<SessionInfo | null>;
  liveSessionRuntimeStatusById: Record<string, string>;
  loadSessionsRequestIdRef: MutableRefObject<number>;
  navigate: (view: MainView) => void;
  operationPermissionMode: string;
  selectedSkillId: string | null;
  sessionRuntimeStateById: Record<string, PersistedChatRuntimeState>;
  sessions: SessionInfo[];
  setActiveTabId: Dispatch<SetStateAction<string>>;
  setCreateSessionError: Dispatch<SetStateAction<string | null>>;
  setLiveSessionRuntimeStatusById: Dispatch<SetStateAction<Record<string, string>>>;
  setSelectedSkillId: Dispatch<SetStateAction<string | null>>;
  setSessionRuntimeStateById: Dispatch<SetStateAction<Record<string, PersistedChatRuntimeState>>>;
  setSessions: Dispatch<SetStateAction<SessionInfo[]>>;
  setTabs: Dispatch<SetStateAction<WorkTab[]>>;
  skills: SkillManifest[];
  tabs: WorkTab[];
}) {
  const sessionView = useSessionViewCoordinator({
    tabs: options.tabs,
    activeTabId: options.activeTabId,
    sessions: options.sessions,
    skills: options.skills,
    selectedSkillId: options.selectedSkillId,
    setSelectedSkillId: options.setSelectedSkillId,
    setActiveTabId: options.setActiveTabId,
    setTabs: options.setTabs,
    setLiveSessionRuntimeStatusById: options.setLiveSessionRuntimeStatusById,
    activeMainView: options.activeMainView,
    setCreateSessionError: options.setCreateSessionError,
    navigate: options.navigate,
    createStartTaskTab: options.createStartTaskTab,
    createSessionTab: options.createSessionTab,
    initialSelectedSessionSnapshotRef: options.initialSelectedSessionSnapshotRef,
    initialPersistedSessionIdRef: options.initialPersistedSessionIdRef,
    hasLoadedSessionsRef: options.hasLoadedSessionsRef,
    hasResolvedInitialPersistedSessionRef: options.hasResolvedInitialPersistedSessionRef,
  });

  const sessionRuntimeState = useSessionRuntimeStateCoordinator({
    selectedSessionId: sessionView.selectedSessionId,
    liveSessionRuntimeStatusById: options.liveSessionRuntimeStatusById,
    setLiveSessionRuntimeStatusById: options.setLiveSessionRuntimeStatusById,
    setSessionRuntimeStateById: options.setSessionRuntimeStateById,
  });

  const runtimeSessionCoordinator = useRuntimeSessionCoordinator({
    selectedSkillId: options.selectedSkillId,
    operationPermissionMode: options.operationPermissionMode,
    activeTab: sessionView.activeTab,
    visibleSessions: sessionView.visibleSessions,
    loadSessionsRequestIdRef: options.loadSessionsRequestIdRef,
    hasLoadedSessionsRef: options.hasLoadedSessionsRef,
    openFreshStartTaskTab: sessionView.openFreshStartTaskTab,
    openStartTaskInActiveTab: sessionView.openStartTaskInActiveTab,
    createStartTaskTab: options.createStartTaskTab,
    createSessionTab: options.createSessionTab,
    isSessionBlockingStartTaskReuse: sessionRuntimeState.isSessionBlockingStartTaskReuse,
    setSessions: options.setSessions,
    setTabs: options.setTabs,
    setActiveTabId: options.setActiveTabId,
  });

  const appendRuntimeOptimisticSession = useCallback(
    (input: Parameters<typeof runtimeSessionCoordinator.appendOptimisticSession>[0]) => {
      runtimeSessionCoordinator.appendOptimisticSession(input);
    },
    [runtimeSessionCoordinator],
  );

  const activateRuntimeSessionTab = useCallback(
    (sessionId: string, tabId: string) => {
      runtimeSessionCoordinator.activateSessionTab(sessionId, tabId);
    },
    [runtimeSessionCoordinator],
  );

  const createRuntimeSession = useCallback(
    (input: Parameters<typeof runtimeSessionCoordinator.createRuntimeSession>[0]) =>
      runtimeSessionCoordinator.createRuntimeSession(input),
    [runtimeSessionCoordinator],
  );

  const loadSessions = useCallback(
    (_skillId: string, state?: { requestId?: number; attempt?: number }) =>
      runtimeSessionCoordinator.loadSessions(_skillId, state),
    [runtimeSessionCoordinator],
  );

  const prepareTabForNewTask = useCallback(
    () => runtimeSessionCoordinator.prepareTabForNewTask(),
    [runtimeSessionCoordinator],
  );

  const sessionDisplay = useSessionDisplayState({
    employeeAssistantSessionContexts: options.employeeAssistantSessionContexts,
    employeeGroups: options.employeeGroups,
    employees: options.employees,
    getEffectiveSessionRuntimeStatus: sessionRuntimeState.getEffectiveSessionRuntimeStatus,
    imManagedSessionIds: options.imManagedSessionIds,
    selectedSessionId: sessionView.selectedSessionId,
    selectedSkillId: options.selectedSkillId,
    skills: options.skills,
    tabs: options.tabs,
    visibleSessions: sessionView.visibleSessions,
  });

  return {
    ...sessionView,
    ...sessionRuntimeState,
    ...sessionDisplay,
    appendRuntimeOptimisticSession,
    activateRuntimeSessionTab,
    createRuntimeSession,
    loadSessions,
    prepareTabForNewTask,
    selectedSessionRuntimeState: sessionView.selectedSessionId
      ? options.sessionRuntimeStateById[sessionView.selectedSessionId]
      : undefined,
  };
}
