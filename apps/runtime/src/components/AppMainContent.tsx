import type { ComponentProps } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { ChatView } from "./ChatView";
import { NewSessionLanding } from "./NewSessionLanding";
import { PackagingView } from "./packaging/PackagingView";
import { SettingsView } from "./SettingsView";
import { TaskTabStrip, type TaskTabStripItem } from "./TaskTabStrip";
import { EmployeeHubScene } from "../scenes/employees/EmployeeHubScene";
import { ExpertCreateView } from "./experts/ExpertCreateView";
import { ExpertsView } from "./experts/ExpertsView";
import { SHOW_DEV_MODEL_SETUP_TOOLS } from "../app-shell-constants";
import type {
  AgentEmployee,
  ModelConfig,
  PendingAttachment,
  PersistedChatRuntimeState,
  SessionInfo,
  SkillManifest,
} from "../types";
import type { EmployeeHubOpenRequest } from "../scenes/employees/EmployeeHubScene";
import type { EmployeeAssistantSessionContext } from "../scenes/employees/employeeAssistantService";

type ChatViewProps = ComponentProps<typeof ChatView>;
type NewSessionLandingProps = ComponentProps<typeof NewSessionLanding>;
type ExpertCreateViewProps = ComponentProps<typeof ExpertCreateView>;
type ExpertsViewProps = ComponentProps<typeof ExpertsView>;
type EmployeeHubSceneProps = ComponentProps<typeof EmployeeHubScene>;

type MainView = "start-task" | "experts" | "experts-new" | "packaging" | "employees";
type SettingsTab =
  | "models"
  | "desktop"
  | "capabilities"
  | "health"
  | "mcp"
  | "search"
  | "routing"
  | "feishu";

export interface AppMainContentProps {
  showSettings: boolean;
  activeMainView: MainView;
  taskTabs: TaskTabStripItem[];
  activeTabId: string;
  onSelectTab: (tabId: string) => void;
  onCreateTab: () => void;
  onCloseTab: (tabId: string) => void;
  settingsInitialTab: SettingsTab;
  onCloseSettings: () => Promise<void>;
  onOpenEmployeesFromSettings: () => void;
  onDevResetFirstUseOnboarding: () => void;
  onDevOpenQuickModelSetup: () => void;
  creatingExpertSkill: boolean;
  expertCreateError: string | null;
  expertSavedPath: string | null;
  pendingImportDir: string | null;
  retryingExpertImport: boolean;
  onBackToExperts: () => void;
  onOpenPackagingView: () => void;
  onPickSkillDirectory: () => Promise<string | null>;
  onCreateExpertSkill: ExpertCreateViewProps["onSave"];
  onRetryExpertImport: () => Promise<void>;
  onRenderExpertPreview: ExpertCreateViewProps["onRenderPreview"];
  skills: SkillManifest[];
  createSessionError: string | null;
  onOpenInstallDialog: () => void;
  onOpenCreateExpertView: () => void;
  onInstallFromLibrary: ExpertsViewProps["onInstallFromLibrary"];
  onStartTaskWithSkill: (skillId: string) => Promise<void>;
  onRefreshLocalSkill: (skillId: string) => Promise<void>;
  onCheckClawhubUpdate: (skillId: string) => Promise<void>;
  onUpdateClawhubSkill: (skillId: string) => Promise<void>;
  onDeleteSkill: (skillId: string) => Promise<void>;
  clawhubUpdateStatus: Record<string, { hasUpdate: boolean; message: string }>;
  busySkillId: string | undefined;
  busyAction: "refresh" | "delete" | "check-update" | "update" | null;
  employees: AgentEmployee[];
  employeeHubOpenRequest: EmployeeHubOpenRequest | null;
  onRefreshEmployees: NonNullable<EmployeeHubSceneProps["onRefreshEmployees"]>;
  onRefreshEmployeeGroups: NonNullable<EmployeeHubSceneProps["onRefreshEmployeeGroups"]>;
  onEnterStartTask: () => void;
  onStartTaskWithEmployee: (employeeId: string) => Promise<void>;
  onOpenGroupRunSession: (sessionId: string, skillId: string) => void | Promise<void>;
  onLaunchEmployeeCreatorSkill: (options?: {
    employeeId?: string;
    employeeName?: string;
  }) => Promise<void>;
  onOpenEmployeeHubFeishuSettings: () => void;
  selectedSkill: SkillManifest | null;
  models: ModelConfig[];
  selectedSessionId: string | null;
  selectedSession: SessionInfo | null | undefined;
  selectedSessionEmployeeName?: string;
  operationPermissionMode: NonNullable<ChatViewProps["operationPermissionMode"]>;
  onOpenSession: NonNullable<ChatViewProps["onOpenSession"]>;
  sessionFocusRequest?: ChatViewProps["sessionFocusRequest"];
  groupRunStepFocusRequest?: ChatViewProps["groupRunStepFocusRequest"];
  sessionExecutionContext?: ChatViewProps["sessionExecutionContext"];
  onReturnToSourceSession: (sourceSessionId: string) => void;
  onSessionUpdate: NonNullable<ChatViewProps["onSessionUpdate"]>;
  onSessionBlockingStateChange: NonNullable<ChatViewProps["onSessionBlockingStateChange"]>;
  persistedRuntimeState?: PersistedChatRuntimeState;
  onPersistRuntimeState: (state: PersistedChatRuntimeState) => void;
  installedSkillIds: string[];
  onSkillInstalled: () => Promise<void>;
  suppressAskUserPrompt: boolean;
  initialMessage?: string;
  initialAttachments?: PendingAttachment[];
  quickPrompts: { label: string; prompt: string }[];
  employeeAssistantContext?: EmployeeAssistantSessionContext;
  onInitialMessageConsumed: () => void;
  onInitialAttachmentsConsumed: () => void;
  visibleSessions: SessionInfo[];
  landingTeams: NonNullable<NewSessionLandingProps["teams"]>;
  defaultWorkDir?: string | null;
  onCreateSessionWithInitialMessage: NewSessionLandingProps["onCreateSessionWithInitialMessage"];
  onCreateTeamEntrySession: NonNullable<NewSessionLandingProps["onCreateTeamEntrySession"]>;
  onPickLandingWorkDir: (currentWorkDir?: string) => Promise<string | null>;
  onSelectSession: (sessionId: string) => void;
  creatingSession: boolean;
}

export function AppMainContent(props: AppMainContentProps) {
  const {
    showSettings,
    activeMainView,
    taskTabs,
    activeTabId,
    onSelectTab,
    onCreateTab,
    onCloseTab,
    settingsInitialTab,
    onCloseSettings,
    onOpenEmployeesFromSettings,
    onDevResetFirstUseOnboarding,
    onDevOpenQuickModelSetup,
    creatingExpertSkill,
    expertCreateError,
    expertSavedPath,
    pendingImportDir,
    retryingExpertImport,
    onBackToExperts,
    onOpenPackagingView,
    onPickSkillDirectory,
    onCreateExpertSkill,
    onRetryExpertImport,
    onRenderExpertPreview,
    skills,
    createSessionError,
    onOpenInstallDialog,
    onOpenCreateExpertView,
    onInstallFromLibrary,
    onStartTaskWithSkill,
    onRefreshLocalSkill,
    onCheckClawhubUpdate,
    onUpdateClawhubSkill,
    onDeleteSkill,
    clawhubUpdateStatus,
    busySkillId,
    busyAction,
    employees,
    employeeHubOpenRequest,
    onRefreshEmployees,
    onRefreshEmployeeGroups,
    onEnterStartTask,
    onStartTaskWithEmployee,
    onOpenGroupRunSession,
    onLaunchEmployeeCreatorSkill,
    onOpenEmployeeHubFeishuSettings,
    selectedSkill,
    models,
    selectedSessionId,
    selectedSession,
    selectedSessionEmployeeName,
    operationPermissionMode,
    onOpenSession,
    sessionFocusRequest,
    groupRunStepFocusRequest,
    sessionExecutionContext,
    onReturnToSourceSession,
    onSessionUpdate,
    onSessionBlockingStateChange,
    persistedRuntimeState,
    onPersistRuntimeState,
    installedSkillIds,
    onSkillInstalled,
    suppressAskUserPrompt,
    initialMessage,
    initialAttachments,
    quickPrompts,
    employeeAssistantContext,
    onInitialMessageConsumed,
    onInitialAttachmentsConsumed,
    visibleSessions,
    landingTeams,
    defaultWorkDir,
    onCreateSessionWithInitialMessage,
    onCreateTeamEntrySession,
    onPickLandingWorkDir,
    onSelectSession,
    creatingSession,
  } = props;

  return (
    <div className="flex-1 overflow-hidden flex flex-col">
      {!showSettings && activeMainView === "start-task" ? (
        <TaskTabStrip
          tabs={taskTabs}
          activeTabId={activeTabId}
          onSelectTab={onSelectTab}
          onCreateTab={onCreateTab}
          onCloseTab={onCloseTab}
        />
      ) : null}
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
                initialTab={settingsInitialTab}
                onClose={onCloseSettings}
                onOpenEmployees={onOpenEmployeesFromSettings}
                showDevModelSetupTools={SHOW_DEV_MODEL_SETUP_TOOLS}
                onDevResetFirstUseOnboarding={onDevResetFirstUseOnboarding}
                onDevOpenQuickModelSetup={onDevOpenQuickModelSetup}
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
                onBack={onBackToExperts}
                onOpenPackaging={onOpenPackagingView}
                onPickDirectory={onPickSkillDirectory}
                onSave={onCreateExpertSkill}
                onRetryImport={onRetryExpertImport}
                onRenderPreview={onRenderExpertPreview}
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
                launchError={createSessionError}
                onInstallSkill={onOpenInstallDialog}
                onCreate={onOpenCreateExpertView}
                onOpenPackaging={onOpenPackagingView}
                onInstallFromLibrary={onInstallFromLibrary}
                onStartTaskWithSkill={onStartTaskWithSkill}
                onRefreshLocalSkill={onRefreshLocalSkill}
                onCheckClawhubUpdate={onCheckClawhubUpdate}
                onUpdateClawhubSkill={onUpdateClawhubSkill}
                onDeleteSkill={onDeleteSkill}
                clawhubUpdateStatus={clawhubUpdateStatus}
                busySkillId={busySkillId}
                busyAction={busyAction}
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
              <EmployeeHubScene
                employees={employees}
                skills={skills}
                openRequest={employeeHubOpenRequest}
                onRefreshEmployees={onRefreshEmployees}
                onRefreshEmployeeGroups={onRefreshEmployeeGroups}
                onEnterStartTask={onEnterStartTask}
                onStartTaskWithEmployee={onStartTaskWithEmployee}
                onOpenGroupRunSession={onOpenGroupRunSession}
                onLaunchEmployeeCreatorSkill={onLaunchEmployeeCreatorSkill}
                onOpenFeishuSettingsPanel={onOpenEmployeeHubFeishuSettings}
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
                sessionModelId={selectedSession?.model_id}
                workDir={selectedSession?.work_dir}
                onOpenSession={onOpenSession}
                sessionFocusRequest={sessionFocusRequest}
                groupRunStepFocusRequest={groupRunStepFocusRequest}
                sessionExecutionContext={sessionExecutionContext}
                onReturnToSourceSession={onReturnToSourceSession}
                sessionSourceChannel={selectedSession?.source_channel}
                sessionSourceLabel={selectedSession?.source_label}
                sessionTitle={selectedSession?.display_title || selectedSession?.title}
                sessionMode={selectedSession?.session_mode}
                sessionEmployeeName={selectedSessionEmployeeName}
                operationPermissionMode={operationPermissionMode}
                onSessionUpdate={onSessionUpdate}
                onSessionBlockingStateChange={onSessionBlockingStateChange}
                persistedRuntimeState={persistedRuntimeState}
                onPersistRuntimeState={onPersistRuntimeState}
                installedSkillIds={installedSkillIds}
                onSkillInstalled={onSkillInstalled}
                suppressAskUserPrompt={suppressAskUserPrompt}
                initialMessage={initialMessage}
                initialAttachments={initialAttachments}
                quickPrompts={quickPrompts}
                employeeAssistantContext={employeeAssistantContext}
                onInitialMessageConsumed={onInitialMessageConsumed}
                onInitialAttachmentsConsumed={onInitialAttachmentsConsumed}
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
                sessions={visibleSessions}
                teams={landingTeams}
                creating={creatingSession}
                error={createSessionError}
                defaultWorkDir={defaultWorkDir ?? undefined}
                onSelectSession={onSelectSession}
                onCreateSessionWithInitialMessage={onCreateSessionWithInitialMessage}
                onCreateTeamEntrySession={onCreateTeamEntrySession}
                onPickWorkDir={onPickLandingWorkDir}
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
  );
}
