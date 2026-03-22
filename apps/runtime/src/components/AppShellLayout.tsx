import type { ComponentProps } from "react";
import { DesktopTitleBar } from "./DesktopTitleBar";
import { Sidebar } from "./Sidebar";
import { AppMainContent } from "./AppMainContent";
import { InstallDialog } from "./InstallDialog";
import { ModelSetupHintBanner } from "./ModelSetupHintBanner";
import {
  ModelSetupGate,
  QuickModelSetupDialog,
} from "./ModelSetupOverlays";

type SidebarProps = ComponentProps<typeof Sidebar>;
type QuickModelSetupDialogProps = ComponentProps<typeof QuickModelSetupDialog>;
type AppMainContentProps = ComponentProps<typeof AppMainContent>;
type InstallDialogProps = ComponentProps<typeof InstallDialog>;

interface AppShellLayoutProps {
  activeMainView: SidebarProps["activeMainView"];
  selectedSkillId: string | null;
  visibleSessions: SidebarProps["sessions"];
  selectedSessionId: string | null;
  handleOpenStartTask: () => void;
  handleOpenExpertsView: () => void;
  handleOpenEmployeesView: () => void;
  handleSelectSession: (sessionId: string) => void;
  handleDeleteSession: (sessionId: string) => void;
  handleOpenSettingsFromSidebar: () => void;
  handleSearchSessions: (query: string) => void;
  handleExportSession: (sessionId: string) => void;
  sidebarCollapsed: boolean;
  setSidebarCollapsed: React.Dispatch<React.SetStateAction<boolean>>;
  shouldShowModelSetupHint: boolean;
  dismissModelSetupHint: () => void;
  openQuickModelSetup: () => void;
  quickModelSetupDialogProps: QuickModelSetupDialogProps;
  shouldShowModelSetupGate: boolean;
  appMainContentProps: AppMainContentProps;
  showInstall: boolean;
  handleInstalled: InstallDialogProps["onInstalled"];
  setShowInstall: React.Dispatch<React.SetStateAction<boolean>>;
}

export function AppShellLayout({
  activeMainView,
  selectedSkillId,
  visibleSessions,
  selectedSessionId,
  handleOpenStartTask,
  handleOpenExpertsView,
  handleOpenEmployeesView,
  handleSelectSession,
  handleDeleteSession,
  handleOpenSettingsFromSidebar,
  handleSearchSessions,
  handleExportSession,
  sidebarCollapsed,
  setSidebarCollapsed,
  shouldShowModelSetupHint,
  dismissModelSetupHint,
  openQuickModelSetup,
  quickModelSetupDialogProps,
  shouldShowModelSetupGate,
  appMainContentProps,
  showInstall,
  handleInstalled,
  setShowInstall,
}: AppShellLayoutProps) {
  return (
    <div className="sm-app flex h-screen flex-col overflow-hidden">
      <DesktopTitleBar />
      <div className="flex min-h-0 flex-1 overflow-hidden">
        <Sidebar
          activeMainView={activeMainView}
          onOpenStartTask={handleOpenStartTask}
          onOpenExperts={handleOpenExpertsView}
          onOpenEmployees={handleOpenEmployeesView}
          selectedSkillId={selectedSkillId}
          sessions={visibleSessions}
          selectedSessionId={selectedSessionId}
          onSelectSession={handleSelectSession}
          onDeleteSession={handleDeleteSession}
          onSettings={handleOpenSettingsFromSidebar}
          onSearchSessions={handleSearchSessions}
          onExportSession={handleExportSession}
          onCollapse={() => setSidebarCollapsed((prev) => !prev)}
          collapsed={sidebarCollapsed}
        />
        <div className="flex flex-1 flex-col overflow-hidden">
          <ModelSetupHintBanner
            show={shouldShowModelSetupHint}
            onDismiss={dismissModelSetupHint}
            onOpenQuickSetup={openQuickModelSetup}
          />
          <QuickModelSetupDialog {...quickModelSetupDialogProps} />
          <ModelSetupGate
            show={shouldShowModelSetupGate}
            onOpenQuickModelSetup={openQuickModelSetup}
          />
          <AppMainContent {...appMainContentProps} />
        </div>
      </div>
      {showInstall && (
        <InstallDialog onInstalled={handleInstalled} onClose={() => setShowInstall(false)} />
      )}
    </div>
  );
}
