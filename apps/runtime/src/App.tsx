import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppShellLayout } from "./components/AppShellLayout";
import {
  BUILTIN_EMPLOYEE_CREATOR_SKILL_ID,
  type EmployeeAssistantSessionContext,
} from "./scenes/employees/employeeAssistantService";
import {
  resolveEmployeeAssistantQuickPrompts,
} from "./scenes/employees/employeeSessionSelectors";
import { useEmployeeHubCoordinator } from "./scenes/employees/useEmployeeHubCoordinator";
import { useEmployeeSessionLaunchCoordinator } from "./scenes/employees/useEmployeeSessionLaunchCoordinator";
import { useCatalogDataCoordinator } from "./scenes/useCatalogDataCoordinator";
import { useAppShellCoordinator } from "./scenes/useAppShellCoordinator";
import { useAppViewActions } from "./scenes/useAppViewActions";
import { useChatSessionBridge } from "./scenes/useChatSessionBridge";
import { useExpertSkillCoordinator } from "./scenes/useExpertSkillCoordinator";
import { useGeneralSessionLaunchCoordinator } from "./scenes/useGeneralSessionLaunchCoordinator";
import { useImBridgeIntegration } from "./scenes/useImBridgeIntegration";
import { useQuickSetupCoordinator } from "./scenes/useQuickSetupCoordinator";
import { useRuntimeSessionCoordinator } from "./scenes/useRuntimeSessionCoordinator";
import { useRuntimePreferencesCoordinator } from "./scenes/useRuntimePreferencesCoordinator";
import { useSessionDisplayState } from "./scenes/useSessionDisplayState";
import { useSessionRuntimeStateCoordinator } from "./scenes/useSessionRuntimeStateCoordinator";
import { useSessionSidebarCoordinator } from "./scenes/useSessionSidebarCoordinator";
import { useSessionViewCoordinator } from "./scenes/useSessionViewCoordinator";
import { buildAppShellRenderProps } from "./scenes/buildAppShellRenderProps";
import {
  DEFAULT_MODEL_PROVIDER_ID,
} from "./model-provider-catalog";
import { getDefaultModelId } from "./lib/default-model";
import { openExternalUrl } from "./utils/openExternalUrl";
import { reportFrontendDiagnostic } from "./diagnostics";
import {
  createSessionTab,
  createStartTaskTab,
  readPersistedLastSelectedSessionId,
  readPersistedLastSelectedSessionSnapshot,
  type WorkTab,
} from "./session-bootstrap";
import {
  AgentEmployee,
  EmployeeGroup,
  LandingSessionLaunchInput,
  ModelConfig,
  PendingAttachment,
  PersistedChatRuntimeState,
  SessionInfo,
} from "./types";
import { extractErrorMessage, getAdjacentSessionId, getDefaultSkillId } from "./app-shell-utils";

type MainView = "start-task" | "experts" | "experts-new" | "packaging" | "employees";
type SkillAction = "refresh" | "delete" | "check-update" | "update";
const MODEL_SETUP_HINT_DISMISSED_KEY = "workclaw:model-setup-hint-dismissed";
const INITIAL_MODEL_SETUP_COMPLETED_KEY = "workclaw:initial-model-setup-completed";

const DEFAULT_SESSION_TITLE = "New Chat";

export default function App() {
  const initialSelectedSessionId = readPersistedLastSelectedSessionId();
  const initialSelectedSessionSnapshot = readPersistedLastSelectedSessionSnapshot();
  const initialWorkTab = initialSelectedSessionId
    ? createSessionTab(initialSelectedSessionId)
    : createStartTaskTab();
  const [selectedSkillId, setSelectedSkillId] = useState<string | null>(
    () => initialSelectedSessionSnapshot?.skill_id?.trim() || null
  );
  const [tabs, setTabs] = useState<WorkTab[]>(() => [initialWorkTab]);
  const [activeTabId, setActiveTabId] = useState<string>(initialWorkTab.id);
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [liveSessionRuntimeStatusById, setLiveSessionRuntimeStatusById] = useState<Record<string, string>>({});
  const [sessionRuntimeStateById, setSessionRuntimeStateById] = useState<Record<string, PersistedChatRuntimeState>>({});
  const [showInstall, setShowInstall] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [settingsInitialTab, setSettingsInitialTab] = useState<
    "models" | "desktop" | "capabilities" | "health" | "mcp" | "search" | "routing" | "feishu"
  >("models");
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
  const openSettingsAtTab = useCallback(
    (
      tab: "models" | "desktop" | "capabilities" | "health" | "mcp" | "search" | "routing" | "feishu",
    ) => {
      setSettingsInitialTab(tab);
      setShowSettings(true);
    },
    [],
  );
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const loadSessionsRequestIdRef = useRef(0);
  const hasLoadedSessionsRef = useRef(false);
  const initialPersistedSessionIdRef = useRef<string | null>(initialSelectedSessionId);
  const initialSelectedSessionSnapshotRef = useRef<SessionInfo | null>(initialSelectedSessionSnapshot);
  const hasResolvedInitialPersistedSessionRef = useRef(false);
  const {
    hasHydratedModelConfigs,
    hasHydratedSearchConfigs,
    loadModels,
    loadSearchConfigs,
    loadSkills,
    models,
    searchConfigs,
    skills,
  } = useCatalogDataCoordinator({
    setSelectedSkillId,
  });
  const {
    defaultWorkDir,
    loadRuntimePreferences,
    operationPermissionMode,
    resolveSessionLaunchWorkDir,
  } = useRuntimePreferencesCoordinator();
  const navigate = useCallback((view: MainView) => {
    setActiveMainView(view);
    if (typeof window !== "undefined") {
      window.location.hash = `/${view}`;
    }
  }, []);

  const {
    employeeHubOpenRequest,
    handleEmployeeGroupsChanged,
    handleOpenEmployeeHubFeishuSettings,
    handleSessionRefresh,
    loadEmployeeGroups,
    loadEmployees,
    openEmployeeHub,
    retargetEmployeeHub,
  } = useEmployeeHubCoordinator({
    employees,
    selectedSkillId,
    loadModels,
    loadSessions: (skillId) => loadSessions(skillId),
    setEmployees,
    setEmployeeGroups,
    setShowSettings,
    navigate: (view) => navigate(view),
    openSettingsAtTab: (tab) => openSettingsAtTab(tab),
  });
  const {
    handleCreateTeamEntrySession,
    handleOpenEmployeeCreatorSkill,
    handleOpenGroupRunSession,
    handleStartTaskWithEmployee,
  } = useEmployeeSessionLaunchCoordinator({
    employees,
    employeeGroups,
    skills,
    models,
    defaultSkillId: getDefaultSkillId(skills),
    creatingSession,
    loadEmployees,
    loadEmployeeGroups,
    loadSkills,
    loadSessions: (skillId) => loadSessions(skillId),
    navigate: (view) => navigate(view),
    prepareTabForNewTask,
    setSelectedSkillId,
    setCreateSessionError,
    setCreatingSession,
    resolveSessionLaunchWorkDir,
    createRuntimeSession,
    appendOptimisticEmployeeSession: ({
      sessionId,
      skillId,
      modelId,
      title,
      employeeId,
      sessionMode,
      teamId,
      workDir,
    }) => appendRuntimeOptimisticSession({
      sessionId,
      skillId,
      modelId,
      title,
      employeeId,
      sessionMode,
      teamId,
      workDir,
    }),
    activateSessionTab: activateRuntimeSessionTab,
    activateExistingSession: (sessionId) => {
      openSessionInActiveTab(sessionId);
    },
    setPendingInitialMessage,
    setEmployeeAssistantSessionContexts,
  });
  const { handleCreateSession, handleStartTaskWithSkill } =
    useGeneralSessionLaunchCoordinator({
      skills,
      models,
      defaultSkillId: getDefaultSkillId(skills),
      creatingSession,
      prepareTabForNewTask,
      setSelectedSkillId,
      setCreateSessionError,
      setCreatingSession,
      resolveSessionLaunchWorkDir,
      createRuntimeSession,
      appendOptimisticGeneralSession: ({
        sessionId,
        skillId,
        modelId,
        title,
        initialUserMessage,
        workDir,
      }) => appendRuntimeOptimisticSession({
        sessionId,
        skillId,
        modelId,
        title,
        initialUserMessage,
        sessionMode: "general",
        workDir,
      }),
      activateSessionTab: activateRuntimeSessionTab,
      loadSessions: (skillId) => loadSessions(skillId),
      navigate: (view) => navigate(view),
      setPendingInitialMessage,
      setPendingInitialAttachments,
    });
  const {
    handleOpenSession,
    handleReturnToSourceSession,
    resolveGroupRunStepFocusRequest,
    resolveSessionExecutionContext,
    resolveSessionFocusRequest,
  } = useChatSessionBridge({
    openGroupRunSession: handleOpenGroupRunSession,
  });
  const {
    actions: {
      applyQuickModelPreset,
      applyQuickSearchPreset,
      closeQuickModelSetup,
      dismissModelSetupHint,
      openInitialModelSetupGate,
      openQuickModelSetup,
      openQuickFeishuSetupFromDialog,
      resetFirstUseOnboardingForDevelopment,
      saveQuickModelSetup,
      saveQuickSearchSetup,
      skipQuickFeishuSetup,
      skipQuickSearchSetup,
      testQuickModelSetupConnection,
      testQuickSearchSetupConnection,
    },
    canDismissQuickModelSetup,
    isBlockingInitialModelSetup,
    quickModelApiKeyInputRef,
    quickModelApiKeyVisible,
    quickModelError,
    quickModelForm,
    quickModelPresetKey,
    quickModelSaving,
    quickModelTestDisplay,
    quickModelTestResult,
    quickModelTesting,
    quickSearchApiKeyVisible,
    quickSearchError,
    quickSearchForm,
    quickSearchSaving,
    quickSearchTestResult,
    quickSearchTesting,
    quickSetupStep,
    selectedQuickModelProvider,
    setQuickModelApiKeyVisible,
    setQuickModelError,
    setQuickModelForm,
    setQuickModelTestResult,
    setQuickSearchApiKeyVisible,
    setQuickSearchError,
    setQuickSearchForm,
    setQuickSearchTestResult,
    shouldShowModelSetupGate,
    shouldShowModelSetupHint,
    shouldShowQuickModelRawMessage,
    showQuickModelSetup,
  } = useQuickSetupCoordinator({
    defaultProviderId: DEFAULT_MODEL_PROVIDER_ID,
    initialModelSetupCompletedKey: INITIAL_MODEL_SETUP_COMPLETED_KEY,
    modelSetupHintDismissedKey: MODEL_SETUP_HINT_DISMISSED_KEY,
    models,
    searchConfigs,
    hasHydratedModelConfigs,
    hasHydratedSearchConfigs,
    showSettings,
    loadModels,
    loadSearchConfigs,
    openSettingsAtTab: (tab) => openSettingsAtTab(tab),
  });

  const {
    activeTab,
    closeTaskTab,
    handleSelectSession,
    hydratedSelectedSession,
    openFreshStartTaskTab,
    openSessionInActiveTab,
    openStartTaskInActiveTab,
    replaceTab,
    selectedSessionId,
    visibleSessions,
  } = useSessionViewCoordinator({
    tabs,
    activeTabId,
    sessions,
    skills,
    selectedSkillId,
    setSelectedSkillId,
    setActiveTabId,
    setTabs,
    setLiveSessionRuntimeStatusById,
    activeMainView,
    setCreateSessionError,
    navigate,
    createStartTaskTab,
    createSessionTab,
    initialSelectedSessionSnapshotRef,
    initialPersistedSessionIdRef,
    hasLoadedSessionsRef,
    hasResolvedInitialPersistedSessionRef,
  });
  const {
    getEffectiveSessionRuntimeStatus,
    handlePersistSessionRuntimeState,
    handleSelectedSessionBlockingStateChange,
    isSessionBlockingStartTaskReuse,
  } = useSessionRuntimeStateCoordinator({
    selectedSessionId,
    liveSessionRuntimeStatusById,
    setLiveSessionRuntimeStatusById,
    setSessionRuntimeStateById,
  });
  const runtimeSessionCoordinator = useRuntimeSessionCoordinator({
    selectedSkillId,
    operationPermissionMode,
    activeTab,
    visibleSessions,
    loadSessionsRequestIdRef,
    hasLoadedSessionsRef,
    openFreshStartTaskTab,
    openStartTaskInActiveTab,
    createStartTaskTab,
    createSessionTab,
    isSessionBlockingStartTaskReuse,
    setSessions,
    setTabs,
    setActiveTabId,
  });
  const {
    handleEnterStartTask,
    handleOpenStartTask,
    handlePickLandingWorkDir,
  } = useAppShellCoordinator({
    defaultWorkDir,
    employees,
    skills,
    navigate,
    openSettingsAtTab,
    setActiveMainView,
    setShowSettings,
    setSelectedSkillId,
    loadSkills,
    loadModels,
    loadSearchConfigs,
    loadRuntimePreferences,
    loadEmployees,
    loadEmployeeGroups,
    prepareTabForNewTask,
  });
  const {
    handleBackToExperts,
    handleOpenCreateExpertView,
    handleOpenEmployeesFromSettings,
    handleOpenEmployeesView,
    handleOpenExpertsView,
    handleOpenInstallDialog,
    handleOpenPackagingView,
    handleOpenSettingsFromSidebar,
  } = useAppViewActions({
    navigate,
    openEmployeeHub,
    retargetEmployeeHub,
    setExpertCreateError,
    setExpertSavedPath,
    setPendingImportDir,
    setShowInstall,
    setShowSettings,
    openSettingsAtTab,
  });
  const selectedSessionRuntimeState = selectedSessionId
    ? sessionRuntimeStateById[selectedSessionId]
    : undefined;
  const {
    landingTeams,
    selectedEmployeeAssistantContext,
    selectedSession,
    selectedSessionEmployeeName,
    selectedSessionImManaged,
    selectedSkill,
    taskTabs,
  } = useSessionDisplayState({
    employeeAssistantSessionContexts,
    employeeGroups,
    employees,
    getEffectiveSessionRuntimeStatus,
    imManagedSessionIds,
    selectedSessionId,
    selectedSkillId,
    skills,
    tabs,
    visibleSessions,
  });
  const {
    handleCheckClawhubUpdate,
    handleCreateExpertSkill,
    handleDeleteSkill,
    handleInstallFromLibrary,
    handlePickSkillDirectory,
    handleRefreshLocalSkill,
    handleRenderExpertPreview,
    handleRetryExpertImport,
    handleSkillInstalledFromChat,
    handleUpdateClawhubSkill,
  } = useExpertSkillCoordinator({
    skillActionState,
    selectedSkillId,
    pendingImportDir,
    retryingExpertImport,
    loadSkills,
    navigate: (view) => navigate(view),
    openStartTaskInActiveTab,
    setSelectedSkillId,
    setCreatingExpertSkill,
    setExpertCreateError,
    setExpertSavedPath,
    setPendingImportDir,
    setRetryingExpertImport,
    setSkillActionState,
    setClawhubUpdateStatus,
  });

  function appendRuntimeOptimisticSession(input: {
    sessionId: string;
    skillId: string;
    modelId: string;
    title?: string;
    initialUserMessage?: string;
    employeeId?: string;
    sessionMode: "general" | "employee_direct" | "team_entry";
    teamId?: string;
    workDir?: string;
  }) {
    runtimeSessionCoordinator.appendOptimisticSession(input);
  }

  function activateRuntimeSessionTab(sessionId: string, tabId: string) {
    runtimeSessionCoordinator.activateSessionTab(sessionId, tabId);
  }

  async function createRuntimeSession(input: {
    skillId: string;
    modelId: string;
    workDir?: string;
    employeeId?: string;
    title?: string;
    sessionMode: "general" | "employee_direct" | "team_entry";
    teamId?: string;
  }) {
    return runtimeSessionCoordinator.createRuntimeSession(input);
  }

  useImBridgeIntegration({
    setImManagedSessionIds,
  });

  async function loadSessions(_skillId: string, options?: { requestId?: number; attempt?: number }) {
    return runtimeSessionCoordinator.loadSessions(_skillId, options);
  }

  const {
    handleDeleteSession,
    handleExportSession,
    handleInstalled,
    handleSearchSessions,
  } = useSessionSidebarCoordinator({
    sessions,
    visibleSessions,
    selectedSessionId,
    selectedSkillId,
    activeMainView,
    skillActionLoadSessions: (skillId) => loadSessions(skillId),
    handleSelectSession,
    getAdjacentSessionId,
    openStartTaskInActiveTab,
    setSessions,
    setTabs,
    setEmployeeAssistantSessionContexts,
    setLiveSessionRuntimeStatusById,
    setSessionRuntimeStateById,
    searchTimerRef,
    loadSkills,
    setSelectedSkillId,
    models,
    prepareTabForNewTask,
    resolveSessionLaunchWorkDir,
    createRuntimeSession,
    replaceTab,
    createSessionTab,
    setActiveTabId,
  });

  function prepareTabForNewTask(): string {
    return runtimeSessionCoordinator.prepareTabForNewTask();
  }

  const { quickModelSetupDialogProps, appMainContentProps } =
    buildAppShellRenderProps({
      showQuickModelSetup,
      quickSetupStep,
      canDismissQuickModelSetup,
      isBlockingInitialModelSetup,
      quickModelApiKeyInputRef,
      quickModelApiKeyVisible,
      quickModelError,
      quickModelForm,
      quickModelPresetKey,
      quickModelSaving,
      quickModelTestDisplay,
      quickModelTestResult,
      quickModelTesting,
      quickSearchApiKeyVisible,
      quickSearchError,
      quickSearchForm,
      quickSearchSaving,
      quickSearchTestResult,
      quickSearchTesting,
      selectedQuickModelProvider,
      shouldShowQuickModelRawMessage,
      applyQuickModelPreset,
      applyQuickSearchPreset,
      closeQuickModelSetup,
      openExternalQuickModelLink: (url: string) => {
        openExternalUrl(url).catch((error) => {
          setQuickModelError(
            extractErrorMessage(error, "打开外部链接失败，请稍后重试"),
          );
        });
      },
      setQuickModelForm,
      setQuickModelApiKeyVisible,
      setQuickModelError,
      setQuickModelTestResult,
      setQuickSearchForm,
      setQuickSearchApiKeyVisible,
      setQuickSearchError,
      setQuickSearchTestResult,
    saveQuickModelSetup,
    saveQuickSearchSetup,
    skipQuickFeishuSetup,
    skipQuickSearchSetup,
    openQuickFeishuSetupFromDialog,
    testQuickModelSetupConnection,
    testQuickSearchSetupConnection,
      showSettings,
      activeMainView,
      taskTabs,
      activeTabId,
      setActiveTabId,
      createNewTab: () => {
        setShowSettings(false);
        openFreshStartTaskTab();
        navigate("start-task");
      },
      closeTaskTab,
      settingsInitialTab,
      closeSettings: async () => {
        await loadModels();
        setShowSettings(false);
      },
      handleOpenEmployeesFromSettings,
      resetFirstUseOnboardingForDevelopment,
      openInitialModelSetupGate,
      creatingExpertSkill,
      expertCreateError,
      expertSavedPath,
      pendingImportDir,
      retryingExpertImport,
      handleBackToExperts,
      handleOpenPackagingView,
      handlePickSkillDirectory,
      handleCreateExpertSkill,
      handleRetryExpertImport,
      handleRenderExpertPreview,
      skills,
      createSessionError,
      handleOpenInstallDialog,
      handleOpenCreateExpertView,
      handleInstallFromLibrary,
      handleStartTaskWithSkill,
      handleRefreshLocalSkill,
      handleCheckClawhubUpdate,
      handleUpdateClawhubSkill,
      handleDeleteSkill,
      clawhubUpdateStatus,
      skillActionState,
      employees,
      employeeHubOpenRequest,
      loadEmployees,
      handleEmployeeGroupsChanged,
      handleEnterStartTask,
      handleStartTaskWithEmployee,
      handleOpenGroupRunSession,
      handleOpenEmployeeCreatorSkill,
      handleOpenEmployeeHubFeishuSettings,
      selectedSkill,
      models,
      selectedSessionId,
      selectedSession,
      selectedSessionEmployeeName,
      operationPermissionMode,
      handleOpenSession,
      resolveSessionFocusRequest,
      resolveGroupRunStepFocusRequest,
      resolveSessionExecutionContext,
      handleReturnToSourceSession,
      handleSessionRefresh,
      handleSelectedSessionBlockingStateChange,
      selectedSessionRuntimeState,
      handlePersistRuntimeState: handlePersistSessionRuntimeState,
      handleSkillInstalledFromChat,
      selectedSessionImManaged,
      pendingInitialMessage,
      pendingInitialAttachments,
      resolveEmployeeAssistantQuickPrompts,
      selectedEmployeeAssistantContext,
      setPendingInitialMessage,
      setPendingInitialAttachments,
      visibleSessions,
      landingTeams,
      defaultWorkDir,
      handleCreateSession,
      handleCreateTeamEntrySession,
      handlePickLandingWorkDir,
      handleSelectSession,
      creatingSession,
    });

  return (
    <AppShellLayout
      activeMainView={activeMainView}
      selectedSkillId={selectedSkillId}
      visibleSessions={visibleSessions}
      selectedSessionId={selectedSessionId}
      handleOpenStartTask={handleOpenStartTask}
      handleOpenExpertsView={handleOpenExpertsView}
      handleOpenEmployeesView={handleOpenEmployeesView}
      handleSelectSession={handleSelectSession}
      handleDeleteSession={handleDeleteSession}
      handleOpenSettingsFromSidebar={handleOpenSettingsFromSidebar}
      handleSearchSessions={handleSearchSessions}
      handleExportSession={handleExportSession}
      sidebarCollapsed={sidebarCollapsed}
      setSidebarCollapsed={setSidebarCollapsed}
      shouldShowModelSetupHint={shouldShowModelSetupHint}
      dismissModelSetupHint={dismissModelSetupHint}
      openQuickModelSetup={openQuickModelSetup}
      quickModelSetupDialogProps={quickModelSetupDialogProps}
      shouldShowModelSetupGate={shouldShowModelSetupGate}
      appMainContentProps={appMainContentProps}
      showInstall={showInstall}
      handleInstalled={handleInstalled}
      setShowInstall={setShowInstall}
    />
  );
}
