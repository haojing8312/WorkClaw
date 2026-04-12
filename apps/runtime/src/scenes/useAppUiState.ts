import { useCallback, useState } from "react";
import type { EmployeeAssistantSessionContext } from "./employees/employeeAssistantService";
import type { WorkTab } from "../session-bootstrap";
import type {
  AgentEmployee,
  EmployeeGroup,
  PendingAttachment,
  PersistedChatRuntimeState,
  SessionInfo,
} from "../types";

export type MainView = "start-task" | "experts" | "experts-new" | "packaging" | "employees";
export type SkillAction = "refresh" | "delete" | "check-update" | "update";
export type SettingsTab =
  | "models"
  | "desktop"
  | "capabilities"
  | "health"
  | "mcp"
  | "search"
  | "routing"
  | "feishu";

export function useAppUiState(input: {
  initialSelectedSkillId: string | null;
  initialWorkTab: WorkTab;
}) {
  const { initialSelectedSkillId, initialWorkTab } = input;
  const [selectedSkillId, setSelectedSkillId] = useState<string | null>(() => initialSelectedSkillId);
  const [tabs, setTabs] = useState<WorkTab[]>(() => [initialWorkTab]);
  const [activeTabId, setActiveTabId] = useState<string>(initialWorkTab.id);
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [liveSessionRuntimeStatusById, setLiveSessionRuntimeStatusById] = useState<Record<string, string>>({});
  const [sessionRuntimeStateById, setSessionRuntimeStateById] = useState<Record<string, PersistedChatRuntimeState>>({});
  const [showInstall, setShowInstall] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [settingsInitialTab, setSettingsInitialTab] = useState<SettingsTab>("models");
  const [activeMainView, setActiveMainView] = useState<MainView>("start-task");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
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
  const [imManagedSessionIds, setImManagedSessionIds] = useState<string[]>([]);
  const [pendingInitialMessage, setPendingInitialMessage] = useState<{
    sessionId: string;
    message: string;
  } | null>(null);
  const [pendingInitialAttachments, setPendingInitialAttachments] = useState<{
    sessionId: string;
    attachments: PendingAttachment[];
  } | null>(null);
  const [employeeAssistantSessionContexts, setEmployeeAssistantSessionContexts] = useState<
    Record<string, EmployeeAssistantSessionContext>
  >({});

  const openSettingsAtTab = useCallback((tab: SettingsTab) => {
    setSettingsInitialTab(tab);
    setShowSettings(true);
  }, []);

  return {
    activeMainView,
    activeTabId,
    clawhubUpdateStatus,
    createSessionError,
    creatingExpertSkill,
    creatingSession,
    employeeAssistantSessionContexts,
    employeeGroups,
    employees,
    expertCreateError,
    expertSavedPath,
    imManagedSessionIds,
    liveSessionRuntimeStatusById,
    openSettingsAtTab,
    pendingImportDir,
    pendingInitialAttachments,
    pendingInitialMessage,
    retryingExpertImport,
    sessionRuntimeStateById,
    selectedSkillId,
    sessions,
    settingsInitialTab,
    showInstall,
    showSettings,
    sidebarCollapsed,
    skillActionState,
    tabs,
    setActiveMainView,
    setActiveTabId,
    setClawhubUpdateStatus,
    setCreateSessionError,
    setCreatingExpertSkill,
    setCreatingSession,
    setEmployeeAssistantSessionContexts,
    setEmployeeGroups,
    setEmployees,
    setExpertCreateError,
    setExpertSavedPath,
    setImManagedSessionIds,
    setLiveSessionRuntimeStatusById,
    setPendingImportDir,
    setPendingInitialAttachments,
    setPendingInitialMessage,
    setRetryingExpertImport,
    setSelectedSkillId,
    setSessionRuntimeStateById,
    setSessions,
    setShowInstall,
    setShowSettings,
    setSidebarCollapsed,
    setSkillActionState,
    setTabs,
  };
}
