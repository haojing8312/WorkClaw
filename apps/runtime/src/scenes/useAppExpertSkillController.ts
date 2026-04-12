import { useExpertSkillCoordinator } from "./useExpertSkillCoordinator";
import type { MainView, SkillAction } from "./useAppUiState";
import type { SkillManifest } from "../types";

export function useAppExpertSkillController(options: {
  clawhubUpdateStatusSetter: React.Dispatch<
    React.SetStateAction<Record<string, { hasUpdate: boolean; message: string }>>
  >;
  creatingSkillSetter: React.Dispatch<React.SetStateAction<boolean>>;
  expertCreateErrorSetter: React.Dispatch<React.SetStateAction<string | null>>;
  expertSavedPathSetter: React.Dispatch<React.SetStateAction<string | null>>;
  loadSkills: () => Promise<SkillManifest[]>;
  navigate: (view: MainView) => void;
  openStartTaskInActiveTab: () => void;
  pendingImportDir: string | null;
  retryingExpertImport: boolean;
  retryingExpertImportSetter: React.Dispatch<React.SetStateAction<boolean>>;
  selectedSkillId: string | null;
  selectedSkillIdSetter: React.Dispatch<React.SetStateAction<string | null>>;
  skillActionState: { skillId: string; action: SkillAction } | null;
  skillActionStateSetter: React.Dispatch<
    React.SetStateAction<{ skillId: string; action: SkillAction } | null>
  >;
  pendingImportDirSetter: React.Dispatch<React.SetStateAction<string | null>>;
}) {
  return useExpertSkillCoordinator({
    skillActionState: options.skillActionState,
    selectedSkillId: options.selectedSkillId,
    pendingImportDir: options.pendingImportDir,
    retryingExpertImport: options.retryingExpertImport,
    loadSkills: options.loadSkills,
    navigate: (view) => options.navigate(view),
    openStartTaskInActiveTab: options.openStartTaskInActiveTab,
    setSelectedSkillId: options.selectedSkillIdSetter,
    setCreatingExpertSkill: options.creatingSkillSetter,
    setExpertCreateError: options.expertCreateErrorSetter,
    setExpertSavedPath: options.expertSavedPathSetter,
    setPendingImportDir: options.pendingImportDirSetter,
    setRetryingExpertImport: options.retryingExpertImportSetter,
    setSkillActionState: options.skillActionStateSetter,
    setClawhubUpdateStatus: options.clawhubUpdateStatusSetter,
  });
}
