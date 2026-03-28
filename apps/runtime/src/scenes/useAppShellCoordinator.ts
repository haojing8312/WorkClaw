import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect } from "react";
import type { Dispatch, SetStateAction } from "react";
import type { AgentEmployee, SkillManifest } from "../types";
import { getDefaultSkillId } from "../app-shell-utils";

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

const SHOULD_BLOCK_DESKTOP_RELOAD_SHORTCUTS =
  import.meta.env.PROD || import.meta.env.MODE === "test";

export function useAppShellCoordinator(options: {
  defaultWorkDir: string;
  employees: AgentEmployee[];
  skills: SkillManifest[];
  showSettings: boolean;
  navigate: (view: MainView) => void;
  openSettingsAtTab: (tab: SettingsTab) => void;
  setActiveMainView: Dispatch<SetStateAction<MainView>>;
  setShowSettings: Dispatch<SetStateAction<boolean>>;
  setSelectedSkillId: Dispatch<SetStateAction<string | null>>;
  loadSkills: () => Promise<unknown>;
  loadModels: () => Promise<unknown>;
  loadSearchConfigs: () => Promise<unknown>;
  loadRuntimePreferences: () => Promise<unknown>;
  loadEmployees: () => Promise<unknown>;
  loadEmployeeGroups: () => Promise<unknown>;
  prepareTabForNewTask: () => string;
}) {
  const {
    defaultWorkDir,
    employees,
    loadEmployeeGroups,
    loadEmployees,
    loadModels,
    loadRuntimePreferences,
    loadSearchConfigs,
    loadSkills,
    navigate,
    openSettingsAtTab,
    prepareTabForNewTask,
    setActiveMainView,
    setSelectedSkillId,
    setShowSettings,
    showSettings,
    skills,
  } = options;

  const handlePickLandingWorkDir = useCallback(
    async (currentWorkDir?: string) => {
      const dir = await open({
        directory: true,
        title: "选择工作目录",
        defaultPath: (currentWorkDir || defaultWorkDir || "").trim() || undefined,
      });
      if (!dir || typeof dir !== "string") return null;
      return dir;
    },
    [defaultWorkDir],
  );

  const handleOpenStartTask = useCallback(async () => {
    if (showSettings) {
      await Promise.all([loadModels(), loadSearchConfigs()]);
    }
    setShowSettings(false);
    prepareTabForNewTask();
    const mainEmployee = employees.find((e) => e.is_default) ?? employees[0];
    if (mainEmployee?.primary_skill_id) {
      setSelectedSkillId(mainEmployee.primary_skill_id);
    }
    setSelectedSkillId((prev) => {
      if (prev && skills.some((item) => item.id === prev)) {
        return prev;
      }
      return getDefaultSkillId(skills);
    });
    navigate("start-task");
  }, [
    employees,
    loadModels,
    loadSearchConfigs,
    navigate,
    prepareTabForNewTask,
    setSelectedSkillId,
    setShowSettings,
    showSettings,
    skills,
  ]);

  const handleEnterStartTask = useCallback(
    async (skillId?: string | null) => {
      if (showSettings) {
        await Promise.all([loadModels(), loadSearchConfigs()]);
      }
      setShowSettings(false);
      prepareTabForNewTask();
      if (skillId) {
        setSelectedSkillId(skillId);
      }
      setSelectedSkillId((prev) => {
        if (prev && skills.some((item) => item.id === prev)) {
          return prev;
        }
        return skillId || getDefaultSkillId(skills);
      });
      navigate("start-task");
    },
    [
      loadModels,
      loadSearchConfigs,
      navigate,
      prepareTabForNewTask,
      setSelectedSkillId,
      setShowSettings,
      showSettings,
      skills,
    ],
  );

  useEffect(() => {
    void loadSkills();
    void loadModels();
    void loadSearchConfigs();
    void loadRuntimePreferences();
    void loadEmployees();
    void loadEmployeeGroups();
    if (typeof window !== "undefined" && window.location.hash) {
      const raw = window.location.hash.replace(/^#\//, "");
      if (
        raw === "experts" ||
        raw === "experts-new" ||
        raw === "packaging" ||
        raw === "start-task" ||
        raw === "employees"
      ) {
        setActiveMainView(raw);
      }
    }
  }, [
    loadEmployeeGroups,
    loadEmployees,
    loadModels,
    loadRuntimePreferences,
    loadSearchConfigs,
    loadSkills,
    setActiveMainView,
  ]);

  useEffect(() => {
    if (!SHOULD_BLOCK_DESKTOP_RELOAD_SHORTCUTS || typeof window === "undefined") {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      const key = event.key.toLowerCase();
      if (key === "f5" || ((event.ctrlKey || event.metaKey) && key === "r")) {
        event.preventDefault();
        event.stopPropagation();
      }
    };

    window.addEventListener("keydown", handleKeyDown, { capture: true });
    return () => {
      window.removeEventListener("keydown", handleKeyDown, { capture: true });
    };
  }, []);

  return {
    handleEnterStartTask,
    handleOpenStartTask,
    handlePickLandingWorkDir,
  };
}
