import { useEffect, useCallback, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppShellLayout } from "./components/AppShellLayout";
import {
  BUILTIN_EMPLOYEE_CREATOR_SKILL_ID,
} from "./scenes/employees/employeeAssistantService";
import {
  resolveEmployeeAssistantQuickPrompts,
} from "./scenes/employees/employeeSessionSelectors";
import { useEmployeeHubCoordinator } from "./scenes/employees/useEmployeeHubCoordinator";
import { useCatalogDataCoordinator } from "./scenes/useCatalogDataCoordinator";
import { useAppShellCoordinator } from "./scenes/useAppShellCoordinator";
import { useAppViewActions } from "./scenes/useAppViewActions";
import { useAppExpertSkillController } from "./scenes/useAppExpertSkillController";
import { useChatSessionBridge } from "./scenes/useChatSessionBridge";
import { useAppSidebarControllers } from "./scenes/useAppSidebarControllers";
import { useQuickSetupCoordinator } from "./scenes/useQuickSetupCoordinator";
import { useAppSessionControllers } from "./scenes/useAppSessionControllers";
import { useSessionLaunchControllers } from "./scenes/useSessionLaunchControllers";
import { useRuntimePreferencesCoordinator } from "./scenes/useRuntimePreferencesCoordinator";
import { useAppUiState, type MainView } from "./scenes/useAppUiState";
import { useAppRenderProps } from "./scenes/useAppRenderProps";
import {
  DEFAULT_MODEL_PROVIDER_ID,
} from "./model-provider-catalog";
import { getDefaultModelId } from "./lib/default-model";
import { reportFrontendDiagnostic } from "./diagnostics";
import {
  createSessionTab,
  createStartTaskTab,
  readPersistedLastSelectedSessionId,
  readPersistedLastSelectedSessionSnapshot,
} from "./session-bootstrap";
import { LandingSessionLaunchInput, ModelConfig, type SessionInfo } from "./types";
import { getAdjacentSessionId, getDefaultSkillId } from "./app-shell-utils";
import { storageKey } from "./lib/branding";

const MODEL_SETUP_HINT_DISMISSED_KEY = storageKey("model-setup-hint-dismissed");
const INITIAL_MODEL_SETUP_COMPLETED_KEY = storageKey("initial-model-setup-completed");

const DEFAULT_SESSION_TITLE = "New Chat";

export default function App() {
  const initialSelectedSessionId = readPersistedLastSelectedSessionId();
  const initialSelectedSessionSnapshot = readPersistedLastSelectedSessionSnapshot();
  const initialWorkTab = initialSelectedSessionId
    ? createSessionTab(initialSelectedSessionId)
    : createStartTaskTab();
  const {
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
  } = useAppUiState({
    initialSelectedSkillId: initialSelectedSessionSnapshot?.skill_id?.trim() || null,
    initialWorkTab,
  });
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

  const quickSetup = useQuickSetupCoordinator({
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
    closeTaskTab,
    handlePersistSessionRuntimeState,
    handleSelectSession,
    handleSelectedSessionBlockingStateChange,
    hydratedSelectedSession,
    landingTeams,
    loadSessions,
    openFreshStartTaskTab,
    openSessionInActiveTab,
    openStartTaskInActiveTab,
    replaceTab,
    selectedEmployeeAssistantContext,
    selectedSession,
    selectedSessionEmployeeName,
    selectedSessionId,
    selectedSessionImManaged,
    selectedSessionRuntimeState,
    selectedSkill,
    taskTabs,
    visibleSessions,
    appendRuntimeOptimisticSession,
    activateRuntimeSessionTab,
    createRuntimeSession,
    getEffectiveSessionRuntimeStatus,
    prepareTabForNewTask,
  } = useAppSessionControllers({
    activeMainView,
    activeTabId,
    createSessionTab,
    createStartTaskTab,
    employeeAssistantSessionContexts,
    employeeGroups,
    employees,
    hasLoadedSessionsRef,
    hasResolvedInitialPersistedSessionRef,
    imManagedSessionIds,
    initialPersistedSessionIdRef,
    initialSelectedSessionSnapshotRef,
    liveSessionRuntimeStatusById,
    loadSessionsRequestIdRef,
    navigate,
    operationPermissionMode,
    selectedSkillId,
    sessionRuntimeStateById,
    sessions,
    setActiveTabId,
    setCreateSessionError,
    setLiveSessionRuntimeStatusById,
    setSelectedSkillId,
    setSessionRuntimeStateById,
    setSessions,
    setTabs,
    skills,
    tabs,
  });
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
  function activateExistingSession(sessionId: string) {
    openSessionInActiveTab(sessionId);
  }
  const {
    handleCreateSession,
    handleCreateTeamEntrySession,
    handleOpenEmployeeCreatorSkill,
    handleOpenGroupRunSession,
    handleStartTaskWithEmployee,
    handleStartTaskWithSkill,
  } = useSessionLaunchControllers({
    activateRuntimeSessionTab,
    appendRuntimeOptimisticSession,
    createRuntimeSession,
    creatingSession,
    defaultSkillId: getDefaultSkillId(skills),
    employeeGroups,
    employees,
    loadEmployeeGroups,
    loadEmployees,
    loadSessions,
    loadSkills,
    models,
    navigate,
    openSessionInActiveTab: activateExistingSession,
    prepareTabForNewTask,
    resolveSessionLaunchWorkDir,
    setCreateSessionError,
    setCreatingSession,
    setEmployeeAssistantSessionContexts,
    setPendingInitialAttachments,
    setPendingInitialMessage,
    setSelectedSkillId,
    skills,
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
    handleEnterStartTask,
    handleOpenStartTask,
    handlePickLandingWorkDir,
  } = useAppShellCoordinator({
    defaultWorkDir,
    employees,
    skills,
    showSettings,
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
  } = useAppExpertSkillController({
    clawhubUpdateStatusSetter: setClawhubUpdateStatus,
    creatingSkillSetter: setCreatingExpertSkill,
    expertCreateErrorSetter: setExpertCreateError,
    expertSavedPathSetter: setExpertSavedPath,
    loadSkills,
    navigate,
    openStartTaskInActiveTab,
    pendingImportDir,
    pendingImportDirSetter: setPendingImportDir,
    retryingExpertImport,
    retryingExpertImportSetter: setRetryingExpertImport,
    selectedSkillId,
    selectedSkillIdSetter: setSelectedSkillId,
    skillActionState,
    skillActionStateSetter: setSkillActionState,
  });

  const {
    handleDeleteSession,
    handleExportSession,
    handleInstalled,
    handleSearchSessions,
  } = useAppSidebarControllers({
    sessions,
    visibleSessions,
    selectedSessionId,
    selectedSkillId,
    activeMainView,
    handleSelectSession,
    getAdjacentSessionId,
    openStartTaskInActiveTab,
    setSessions,
    setTabs,
    setEmployeeAssistantSessionContexts,
    setImManagedSessionIds,
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

  const { quickModelSetupDialogProps, appMainContentProps } = useAppRenderProps({
    quickSetup,
    showSettings,
    activeMainView,
    taskTabs,
    activeTabId,
    setActiveTabId,
    closeTaskTab,
    settingsInitialTab,
    handleOpenEmployeesFromSettings,
    resetFirstUseOnboardingForDevelopment: quickSetup.actions.resetFirstUseOnboardingForDevelopment,
    openInitialModelSetupGate: quickSetup.actions.openInitialModelSetupGate,
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
    navigate,
    onAfterCreateTab: () => {
      setShowSettings(false);
      openFreshStartTaskTab();
    },
    onCloseSettings: async () => {
      await loadModels();
      await loadSessions(selectedSkillId ?? "");
      setShowSettings(false);
    },
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
      shouldShowModelSetupHint={quickSetup.shouldShowModelSetupHint}
      dismissModelSetupHint={quickSetup.actions.dismissModelSetupHint}
      openQuickModelSetup={quickSetup.actions.openQuickModelSetup}
      quickModelSetupDialogProps={quickModelSetupDialogProps}
      shouldShowModelSetupGate={quickSetup.shouldShowModelSetupGate}
      appMainContentProps={appMainContentProps}
      showInstall={showInstall}
      handleInstalled={handleInstalled}
      setShowInstall={setShowInstall}
    />
  );
}
