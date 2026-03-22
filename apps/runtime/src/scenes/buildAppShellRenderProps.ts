import type { Dispatch, SetStateAction } from "react";
import type { AppMainContentProps } from "../components/AppMainContent";
import type { QuickModelSetupDialogProps } from "../components/ModelSetupOverlays";
import type { PendingAttachment } from "../types";

interface BuildAppShellRenderPropsResult {
  quickModelSetupDialogProps: QuickModelSetupDialogProps;
  appMainContentProps: AppMainContentProps;
}

interface BuildAppShellRenderPropsOptions {
  showQuickModelSetup: QuickModelSetupDialogProps["show"];
  quickSetupStep: QuickModelSetupDialogProps["quickSetupStep"];
  canDismissQuickModelSetup: QuickModelSetupDialogProps["canDismissQuickModelSetup"];
  isBlockingInitialModelSetup: QuickModelSetupDialogProps["isBlockingInitialModelSetup"];
  quickModelApiKeyInputRef: QuickModelSetupDialogProps["quickModelApiKeyInputRef"];
  quickModelApiKeyVisible: QuickModelSetupDialogProps["quickModelApiKeyVisible"];
  quickModelError: QuickModelSetupDialogProps["quickModelError"];
  quickModelForm: QuickModelSetupDialogProps["quickModelForm"];
  quickModelPresetKey: QuickModelSetupDialogProps["quickModelPresetKey"];
  quickModelSaving: QuickModelSetupDialogProps["quickModelSaving"];
  quickModelTestDisplay: QuickModelSetupDialogProps["quickModelTestDisplay"];
  quickModelTestResult: QuickModelSetupDialogProps["quickModelTestResult"];
  quickModelTesting: QuickModelSetupDialogProps["quickModelTesting"];
  quickSearchApiKeyVisible: QuickModelSetupDialogProps["quickSearchApiKeyVisible"];
  quickSearchError: QuickModelSetupDialogProps["quickSearchError"];
  quickSearchForm: QuickModelSetupDialogProps["quickSearchForm"];
  quickSearchSaving: QuickModelSetupDialogProps["quickSearchSaving"];
  quickSearchTestResult: QuickModelSetupDialogProps["quickSearchTestResult"];
  quickSearchTesting: QuickModelSetupDialogProps["quickSearchTesting"];
  selectedQuickModelProvider: QuickModelSetupDialogProps["selectedQuickModelProvider"];
  shouldShowQuickModelRawMessage: QuickModelSetupDialogProps["shouldShowQuickModelRawMessage"];
  applyQuickModelPreset: QuickModelSetupDialogProps["onApplyQuickModelPreset"];
  applyQuickSearchPreset: QuickModelSetupDialogProps["onApplyQuickSearchPreset"];
  closeQuickModelSetup: QuickModelSetupDialogProps["onCloseQuickModelSetup"];
  openExternalQuickModelLink: QuickModelSetupDialogProps["onOpenExternalLink"];
  setQuickModelForm: QuickModelSetupDialogProps["onQuickModelFormChange"];
  setQuickModelApiKeyVisible: Dispatch<SetStateAction<boolean>>;
  setQuickModelError: QuickModelSetupDialogProps["onQuickModelErrorChange"];
  setQuickModelTestResult: QuickModelSetupDialogProps["onQuickModelTestResultChange"];
  setQuickSearchForm: QuickModelSetupDialogProps["onQuickSearchFormChange"];
  setQuickSearchApiKeyVisible: Dispatch<SetStateAction<boolean>>;
  setQuickSearchError: QuickModelSetupDialogProps["onQuickSearchErrorChange"];
  setQuickSearchTestResult: QuickModelSetupDialogProps["onQuickSearchTestResultChange"];
  saveQuickModelSetup: QuickModelSetupDialogProps["onSaveQuickModelSetup"];
  saveQuickSearchSetup: QuickModelSetupDialogProps["onSaveQuickSearchSetup"];
  skipQuickSearchSetup: QuickModelSetupDialogProps["onSkipQuickSearchSetup"];
  skipQuickFeishuSetup: QuickModelSetupDialogProps["onSkipQuickFeishuSetup"];
  openQuickFeishuSetupFromDialog: QuickModelSetupDialogProps["onOpenQuickFeishuSetupFromDialog"];
  testQuickModelSetupConnection: QuickModelSetupDialogProps["onTestQuickModelSetupConnection"];
  testQuickSearchSetupConnection: QuickModelSetupDialogProps["onTestQuickSearchSetupConnection"];
  showSettings: AppMainContentProps["showSettings"];
  activeMainView: AppMainContentProps["activeMainView"];
  taskTabs: AppMainContentProps["taskTabs"];
  activeTabId: AppMainContentProps["activeTabId"];
  setActiveTabId: Dispatch<SetStateAction<string>>;
  createNewTab: AppMainContentProps["onCreateTab"];
  closeTaskTab: AppMainContentProps["onCloseTab"];
  settingsInitialTab: AppMainContentProps["settingsInitialTab"];
  closeSettings: AppMainContentProps["onCloseSettings"];
  handleOpenEmployeesFromSettings: AppMainContentProps["onOpenEmployeesFromSettings"];
  resetFirstUseOnboardingForDevelopment: AppMainContentProps["onDevResetFirstUseOnboarding"];
  openInitialModelSetupGate: AppMainContentProps["onDevOpenQuickModelSetup"];
  creatingExpertSkill: AppMainContentProps["creatingExpertSkill"];
  expertCreateError: AppMainContentProps["expertCreateError"];
  expertSavedPath: AppMainContentProps["expertSavedPath"];
  pendingImportDir: AppMainContentProps["pendingImportDir"];
  retryingExpertImport: AppMainContentProps["retryingExpertImport"];
  handleBackToExperts: AppMainContentProps["onBackToExperts"];
  handleOpenPackagingView: AppMainContentProps["onOpenPackagingView"];
  handlePickSkillDirectory: AppMainContentProps["onPickSkillDirectory"];
  handleCreateExpertSkill: AppMainContentProps["onCreateExpertSkill"];
  handleRetryExpertImport: AppMainContentProps["onRetryExpertImport"];
  handleRenderExpertPreview: AppMainContentProps["onRenderExpertPreview"];
  skills: AppMainContentProps["skills"];
  createSessionError: AppMainContentProps["createSessionError"];
  handleOpenInstallDialog: AppMainContentProps["onOpenInstallDialog"];
  handleOpenCreateExpertView: AppMainContentProps["onOpenCreateExpertView"];
  handleInstallFromLibrary: AppMainContentProps["onInstallFromLibrary"];
  handleStartTaskWithSkill: AppMainContentProps["onStartTaskWithSkill"];
  handleRefreshLocalSkill: AppMainContentProps["onRefreshLocalSkill"];
  handleCheckClawhubUpdate: AppMainContentProps["onCheckClawhubUpdate"];
  handleUpdateClawhubSkill: AppMainContentProps["onUpdateClawhubSkill"];
  handleDeleteSkill: AppMainContentProps["onDeleteSkill"];
  clawhubUpdateStatus: AppMainContentProps["clawhubUpdateStatus"];
  skillActionState: { skillId: string; action: NonNullable<AppMainContentProps["busyAction"]> } | null;
  employees: AppMainContentProps["employees"];
  employeeHubOpenRequest: AppMainContentProps["employeeHubOpenRequest"];
  loadEmployees: AppMainContentProps["onRefreshEmployees"];
  handleEmployeeGroupsChanged: AppMainContentProps["onRefreshEmployeeGroups"];
  handleEnterStartTask: AppMainContentProps["onEnterStartTask"];
  handleStartTaskWithEmployee: AppMainContentProps["onStartTaskWithEmployee"];
  handleOpenGroupRunSession: AppMainContentProps["onOpenGroupRunSession"];
  handleOpenEmployeeCreatorSkill: AppMainContentProps["onLaunchEmployeeCreatorSkill"];
  handleOpenEmployeeHubFeishuSettings: AppMainContentProps["onOpenEmployeeHubFeishuSettings"];
  selectedSkill: AppMainContentProps["selectedSkill"];
  models: AppMainContentProps["models"];
  selectedSessionId: AppMainContentProps["selectedSessionId"];
  selectedSession: AppMainContentProps["selectedSession"];
  selectedSessionEmployeeName: AppMainContentProps["selectedSessionEmployeeName"];
  operationPermissionMode: AppMainContentProps["operationPermissionMode"];
  handleOpenSession: (
    nextSessionId: string,
    skillId: string,
    options?: Parameters<AppMainContentProps["onOpenSession"]>[1],
  ) => void;
  resolveSessionFocusRequest: (sessionId: string) => AppMainContentProps["sessionFocusRequest"];
  resolveGroupRunStepFocusRequest: (sessionId: string) => AppMainContentProps["groupRunStepFocusRequest"];
  resolveSessionExecutionContext: (sessionId: string) => AppMainContentProps["sessionExecutionContext"];
  handleReturnToSourceSession: (sourceSessionId: string, skillId: string) => void;
  handleSessionRefresh: AppMainContentProps["onSessionUpdate"];
  handleSelectedSessionBlockingStateChange: AppMainContentProps["onSessionBlockingStateChange"];
  selectedSessionRuntimeState: AppMainContentProps["persistedRuntimeState"];
  handlePersistRuntimeState: (
    sessionId: string,
    state: NonNullable<AppMainContentProps["persistedRuntimeState"]>,
  ) => void;
  handleSkillInstalledFromChat: AppMainContentProps["onSkillInstalled"];
  selectedSessionImManaged: AppMainContentProps["suppressAskUserPrompt"];
  pendingInitialMessage: { sessionId: string; message: string } | null;
  pendingInitialAttachments: { sessionId: string; attachments: PendingAttachment[] } | null;
  resolveEmployeeAssistantQuickPrompts: (skillId: string) => AppMainContentProps["quickPrompts"] | null | undefined;
  selectedEmployeeAssistantContext: AppMainContentProps["employeeAssistantContext"];
  setPendingInitialMessage: Dispatch<SetStateAction<{ sessionId: string; message: string } | null>>;
  setPendingInitialAttachments: Dispatch<
    SetStateAction<{ sessionId: string; attachments: PendingAttachment[] } | null>
  >;
  visibleSessions: AppMainContentProps["visibleSessions"];
  landingTeams: AppMainContentProps["landingTeams"];
  defaultWorkDir: AppMainContentProps["defaultWorkDir"];
  handleCreateSession: AppMainContentProps["onCreateSessionWithInitialMessage"];
  handleCreateTeamEntrySession: AppMainContentProps["onCreateTeamEntrySession"];
  handlePickLandingWorkDir: AppMainContentProps["onPickLandingWorkDir"];
  handleSelectSession: AppMainContentProps["onSelectSession"];
  creatingSession: AppMainContentProps["creatingSession"];
}

export function buildAppShellRenderProps(
  options: BuildAppShellRenderPropsOptions,
): BuildAppShellRenderPropsResult {
  const {
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
    openExternalQuickModelLink,
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
    skipQuickSearchSetup,
    skipQuickFeishuSetup,
    openQuickFeishuSetupFromDialog,
    testQuickModelSetupConnection,
    testQuickSearchSetupConnection,
    showSettings,
    activeMainView,
    taskTabs,
    activeTabId,
    createNewTab,
    closeTaskTab,
    settingsInitialTab,
    closeSettings,
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
    handlePersistRuntimeState,
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
  } = options;

  const quickModelSetupDialogProps: QuickModelSetupDialogProps = {
      show: showQuickModelSetup,
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
      onApplyQuickModelPreset: applyQuickModelPreset,
      onApplyQuickSearchPreset: applyQuickSearchPreset,
      onCloseQuickModelSetup: closeQuickModelSetup,
      onOpenExternalLink: openExternalQuickModelLink,
      onQuickModelFormChange: setQuickModelForm,
      onQuickModelApiKeyVisibilityToggle: () =>
        setQuickModelApiKeyVisible((prev: boolean) => !prev),
      onQuickModelErrorChange: setQuickModelError,
      onQuickModelTestResultChange: setQuickModelTestResult,
      onQuickSearchFormChange: setQuickSearchForm,
      onQuickSearchApiKeyVisibilityToggle: () =>
        setQuickSearchApiKeyVisible((value: boolean) => !value),
      onQuickSearchErrorChange: setQuickSearchError,
      onQuickSearchTestResultChange: setQuickSearchTestResult,
      onSaveQuickModelSetup: saveQuickModelSetup,
      onSaveQuickSearchSetup: saveQuickSearchSetup,
      onSkipQuickSearchSetup: skipQuickSearchSetup,
      onSkipQuickFeishuSetup: skipQuickFeishuSetup,
      onOpenQuickFeishuSetupFromDialog: openQuickFeishuSetupFromDialog,
      onTestQuickModelSetupConnection: testQuickModelSetupConnection,
      onTestQuickSearchSetupConnection: testQuickSearchSetupConnection,
    };

  const appMainContentProps: AppMainContentProps = {
      showSettings,
      activeMainView,
      taskTabs,
      activeTabId,
      onSelectTab: options.setActiveTabId,
      onCreateTab: createNewTab,
      onCloseTab: closeTaskTab,
      settingsInitialTab,
      onCloseSettings: closeSettings,
      onOpenEmployeesFromSettings: handleOpenEmployeesFromSettings,
      onDevResetFirstUseOnboarding: resetFirstUseOnboardingForDevelopment,
      onDevOpenQuickModelSetup: openInitialModelSetupGate,
      creatingExpertSkill,
      expertCreateError,
      expertSavedPath,
      pendingImportDir,
      retryingExpertImport,
      onBackToExperts: handleBackToExperts,
      onOpenPackagingView: handleOpenPackagingView,
      onPickSkillDirectory: handlePickSkillDirectory,
      onCreateExpertSkill: handleCreateExpertSkill,
      onRetryExpertImport: handleRetryExpertImport,
      onRenderExpertPreview: handleRenderExpertPreview,
      skills,
      createSessionError,
      onOpenInstallDialog: handleOpenInstallDialog,
      onOpenCreateExpertView: handleOpenCreateExpertView,
      onInstallFromLibrary: handleInstallFromLibrary,
      onStartTaskWithSkill: handleStartTaskWithSkill,
      onRefreshLocalSkill: handleRefreshLocalSkill,
      onCheckClawhubUpdate: handleCheckClawhubUpdate,
      onUpdateClawhubSkill: handleUpdateClawhubSkill,
      onDeleteSkill: handleDeleteSkill,
      clawhubUpdateStatus,
      busySkillId: skillActionState?.skillId,
      busyAction: skillActionState?.action ?? null,
      employees,
      employeeHubOpenRequest,
      onRefreshEmployees: loadEmployees,
      onRefreshEmployeeGroups: handleEmployeeGroupsChanged,
      onEnterStartTask: handleEnterStartTask,
      onStartTaskWithEmployee: handleStartTaskWithEmployee,
      onOpenGroupRunSession: handleOpenGroupRunSession,
      onLaunchEmployeeCreatorSkill: handleOpenEmployeeCreatorSkill,
      onOpenEmployeeHubFeishuSettings: handleOpenEmployeeHubFeishuSettings,
      selectedSkill,
      models,
      selectedSessionId,
      selectedSession,
      selectedSessionEmployeeName,
      operationPermissionMode,
      onOpenSession: (nextSessionId, openOptions) =>
        handleOpenSession(nextSessionId, selectedSkill?.id ?? "", openOptions),
      sessionFocusRequest: selectedSessionId
        ? resolveSessionFocusRequest(selectedSessionId)
        : undefined,
      groupRunStepFocusRequest: selectedSessionId
        ? resolveGroupRunStepFocusRequest(selectedSessionId)
        : undefined,
      sessionExecutionContext: selectedSessionId
        ? resolveSessionExecutionContext(selectedSessionId)
        : undefined,
      onReturnToSourceSession: (sourceSessionId: string) =>
        handleReturnToSourceSession(sourceSessionId, selectedSkill?.id ?? ""),
      onSessionUpdate: handleSessionRefresh,
      onSessionBlockingStateChange: handleSelectedSessionBlockingStateChange,
      persistedRuntimeState: selectedSessionRuntimeState,
      onPersistRuntimeState: (state) => {
        if (!selectedSessionId) return;
        handlePersistRuntimeState(selectedSessionId, state);
      },
      installedSkillIds: skills.map((skill: { id: string }) => skill.id),
      onSkillInstalled: handleSkillInstalledFromChat,
      suppressAskUserPrompt: selectedSessionImManaged,
      initialMessage:
        pendingInitialMessage && pendingInitialMessage.sessionId === selectedSessionId
          ? pendingInitialMessage.message
          : undefined,
      initialAttachments:
        pendingInitialAttachments && pendingInitialAttachments.sessionId === selectedSessionId
          ? pendingInitialAttachments.attachments
          : undefined,
      quickPrompts: selectedSkill
        ? resolveEmployeeAssistantQuickPrompts(selectedSkill.id) ?? []
        : [],
      employeeAssistantContext: selectedEmployeeAssistantContext,
      onInitialMessageConsumed: () => {
        setPendingInitialMessage((prev: { sessionId: string; message: string } | null) =>
          prev && prev.sessionId === selectedSessionId ? null : prev,
        );
      },
      onInitialAttachmentsConsumed: () => {
        setPendingInitialAttachments((prev: { sessionId: string; attachments: PendingAttachment[] } | null) =>
          prev && prev.sessionId === selectedSessionId ? null : prev,
        );
      },
      visibleSessions,
      landingTeams,
      defaultWorkDir,
      onCreateSessionWithInitialMessage: handleCreateSession,
      onCreateTeamEntrySession: handleCreateTeamEntrySession,
      onPickLandingWorkDir: handlePickLandingWorkDir,
      onSelectSession: handleSelectSession,
      creatingSession,
    };

  return {
    quickModelSetupDialogProps,
    appMainContentProps,
  };
}
