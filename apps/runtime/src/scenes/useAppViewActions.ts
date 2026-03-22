import { useCallback } from "react";
import type { EmployeeHubTab } from "../components/employees/EmployeeHubView";

type MainView = "start-task" | "experts" | "experts-new" | "packaging" | "employees";

export function useAppViewActions(options: {
  navigate: (view: MainView) => void;
  openEmployeeHub: (targetView?: "employees") => Promise<void> | void;
  retargetEmployeeHub: (tab: EmployeeHubTab) => void;
  setExpertCreateError: (value: string | null) => void;
  setExpertSavedPath: (value: string | null) => void;
  setPendingImportDir: (value: string | null) => void;
  setShowInstall: (value: boolean) => void;
  setShowSettings: (value: boolean) => void;
  openSettingsAtTab: (
    tab: "models" | "desktop" | "capabilities" | "health" | "mcp" | "search" | "routing" | "feishu",
  ) => void;
}) {
  const {
    navigate,
    openEmployeeHub,
    openSettingsAtTab,
    retargetEmployeeHub,
    setExpertCreateError,
    setExpertSavedPath,
    setPendingImportDir,
    setShowInstall,
    setShowSettings,
  } = options;

  const handleOpenExpertsView = useCallback(() => {
    setShowSettings(false);
    navigate("experts");
  }, [navigate, setShowSettings]);

  const handleOpenEmployeesView = useCallback(() => {
    setShowSettings(false);
    retargetEmployeeHub("overview");
    navigate("employees");
  }, [navigate, retargetEmployeeHub, setShowSettings]);

  const handleOpenSettingsFromSidebar = useCallback(() => {
    navigate("start-task");
    openSettingsAtTab("models");
  }, [navigate, openSettingsAtTab]);

  const handleOpenEmployeesFromSettings = useCallback(() => {
    void openEmployeeHub("employees");
  }, [openEmployeeHub]);

  const handleBackToExperts = useCallback(() => {
    setExpertCreateError(null);
    setExpertSavedPath(null);
    setPendingImportDir(null);
    navigate("experts");
  }, [navigate, setExpertCreateError, setExpertSavedPath, setPendingImportDir]);

  const handleOpenPackagingView = useCallback(() => {
    navigate("packaging");
  }, [navigate]);

  const handleOpenCreateExpertView = useCallback(() => {
    setExpertCreateError(null);
    setExpertSavedPath(null);
    setPendingImportDir(null);
    navigate("experts-new");
  }, [navigate, setExpertCreateError, setExpertSavedPath, setPendingImportDir]);

  const handleOpenInstallDialog = useCallback(() => {
    setShowInstall(true);
  }, [setShowInstall]);

  return {
    handleBackToExperts,
    handleOpenCreateExpertView,
    handleOpenEmployeesFromSettings,
    handleOpenEmployeesView,
    handleOpenExpertsView,
    handleOpenInstallDialog,
    handleOpenPackagingView,
    handleOpenSettingsFromSidebar,
  };
}
