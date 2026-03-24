import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback } from "react";
import type { Dispatch, SetStateAction } from "react";
import type {
  ClawhubInstallRequest,
  SkillManifest,
} from "../types";
import type {
  ExpertCreatePayload,
  ExpertPreviewPayload,
  ExpertPreviewResult,
} from "../components/experts/ExpertCreateView";

type SkillAction = "refresh" | "delete" | "check-update" | "update";
type LocalImportBatchResult = {
  installed: { manifest: SkillManifest }[];
  failed: { dir_path: string; name_hint: string; error: string }[];
  missing_mcp: string[];
};

function extractErrorMessage(error: unknown, fallback: string): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message || fallback;
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message?: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }
  return fallback;
}

function extractDuplicateSkillName(error: unknown): string | null {
  const message = extractErrorMessage(error, "");
  const prefix = "DUPLICATE_SKILL_NAME:";
  if (!message.includes(prefix)) {
    return null;
  }
  return message.split(prefix)[1]?.trim() || null;
}

function getFirstInstalledSkillId(result: LocalImportBatchResult | null | undefined): string | null {
  return result?.installed?.[0]?.manifest?.id ?? null;
}

export function useExpertSkillCoordinator(options: {
  skillActionState: { skillId: string; action: SkillAction } | null;
  selectedSkillId: string | null;
  pendingImportDir: string | null;
  retryingExpertImport: boolean;
  loadSkills: () => Promise<SkillManifest[]>;
  navigate: (view: "experts" | "start-task") => void;
  openStartTaskInActiveTab: () => void;
  setSelectedSkillId: Dispatch<SetStateAction<string | null>>;
  setCreatingExpertSkill: Dispatch<SetStateAction<boolean>>;
  setExpertCreateError: Dispatch<SetStateAction<string | null>>;
  setExpertSavedPath: Dispatch<SetStateAction<string | null>>;
  setPendingImportDir: Dispatch<SetStateAction<string | null>>;
  setRetryingExpertImport: Dispatch<SetStateAction<boolean>>;
  setSkillActionState: Dispatch<
    SetStateAction<{ skillId: string; action: SkillAction } | null>
  >;
  setClawhubUpdateStatus: Dispatch<
    SetStateAction<Record<string, { hasUpdate: boolean; message: string }>>
  >;
}) {
  const {
    skillActionState,
    selectedSkillId,
    pendingImportDir,
    retryingExpertImport,
    loadSkills,
    navigate,
    openStartTaskInActiveTab,
    setClawhubUpdateStatus,
    setCreatingExpertSkill,
    setExpertCreateError,
    setExpertSavedPath,
    setPendingImportDir,
    setRetryingExpertImport,
    setSelectedSkillId,
    setSkillActionState,
  } = options;

  const handlePickSkillDirectory = useCallback(async () => {
    const dir = await open({ directory: true, title: "选择技能保存目录" });
    if (!dir || typeof dir !== "string") return null;
    return dir;
  }, []);

  const handleCreateExpertSkill = useCallback(
    async (payload: ExpertCreatePayload) => {
      setCreatingExpertSkill(true);
      setExpertCreateError(null);
      setExpertSavedPath(null);
      setPendingImportDir(null);
      try {
        const skillDir = await invoke<string>("create_local_skill", {
          name: payload.name,
          description: payload.description,
          whenToUse: payload.whenToUse,
          targetDir: payload.targetDir ?? null,
        });
        setExpertSavedPath(skillDir);
        setPendingImportDir(skillDir);

        try {
          const importResult = await invoke<LocalImportBatchResult>(
            "import_local_skill",
            {
              dirPath: skillDir,
            },
          );
          await loadSkills();
          const importedSkillId = getFirstInstalledSkillId(importResult);
          if (importedSkillId) {
            setSelectedSkillId(importedSkillId);
          }
          setExpertSavedPath(null);
          setPendingImportDir(null);
          navigate("experts");
        } catch (importError) {
          const duplicateName = extractDuplicateSkillName(importError);
          if (duplicateName) {
            setExpertCreateError(
              `技能名称冲突：已存在「${duplicateName}」（文件已保存到：${skillDir}）`,
            );
            return;
          }
          const message = extractErrorMessage(importError, "导入失败，请稍后重试。");
          setExpertCreateError(`${message}（文件已保存到：${skillDir}）`);
        }
      } catch (error) {
        console.error("创建专家技能失败:", error);
        setExpertCreateError(
          extractErrorMessage(error, "创建失败，请检查目录权限后重试。"),
        );
      } finally {
        setCreatingExpertSkill(false);
      }
    },
    [
      loadSkills,
      navigate,
      setCreatingExpertSkill,
      setExpertCreateError,
      setExpertSavedPath,
      setPendingImportDir,
      setSelectedSkillId,
    ],
  );

  const handleRetryExpertImport = useCallback(async () => {
    if (!pendingImportDir || retryingExpertImport) return;
    setRetryingExpertImport(true);
    setExpertCreateError(null);
    try {
      const importResult = await invoke<LocalImportBatchResult>(
        "import_local_skill",
        {
          dirPath: pendingImportDir,
        },
      );
      await loadSkills();
      const importedSkillId = getFirstInstalledSkillId(importResult);
      if (importedSkillId) {
        setSelectedSkillId(importedSkillId);
      }
      setPendingImportDir(null);
      setExpertSavedPath(null);
      navigate("experts");
    } catch (error) {
      const duplicateName = extractDuplicateSkillName(error);
      if (duplicateName) {
        setExpertCreateError(
          `技能名称冲突：已存在「${duplicateName}」（文件已保存到：${pendingImportDir}）`,
        );
        return;
      }
      const message = extractErrorMessage(error, "导入失败，请稍后重试。");
      setExpertCreateError(`${message}（文件已保存到：${pendingImportDir}）`);
    } finally {
      setRetryingExpertImport(false);
    }
  }, [
    loadSkills,
    navigate,
    pendingImportDir,
    retryingExpertImport,
    setExpertCreateError,
    setExpertSavedPath,
    setPendingImportDir,
    setRetryingExpertImport,
    setSelectedSkillId,
  ]);

  const handleRefreshLocalSkill = useCallback(
    async (skillId: string) => {
      if (skillActionState) return;
      setSkillActionState({ skillId, action: "refresh" });
      try {
        await invoke("refresh_local_skill", { skillId });
        await loadSkills();
      } catch (error) {
        console.error("刷新本地技能失败:", error);
      } finally {
        setSkillActionState(null);
      }
    },
    [loadSkills, setSkillActionState, skillActionState],
  );

  const handleDeleteSkill = useCallback(
    async (skillId: string) => {
      if (skillActionState) return;
      setSkillActionState({ skillId, action: "delete" });
      try {
        await invoke("delete_skill", { skillId });
        if (selectedSkillId === skillId) {
          openStartTaskInActiveTab();
        }
        await loadSkills();
      } catch (error) {
        console.error("移除技能失败:", error);
      } finally {
        setSkillActionState(null);
      }
    },
    [
      loadSkills,
      openStartTaskInActiveTab,
      selectedSkillId,
      setSkillActionState,
      skillActionState,
    ],
  );

  const handleCheckClawhubUpdate = useCallback(
    async (skillId: string) => {
      if (skillActionState) return;
      setSkillActionState({ skillId, action: "check-update" });
      try {
        const result = await invoke<{ has_update: boolean; message: string }>(
          "check_clawhub_skill_update",
          {
            skillId,
          },
        );
        setClawhubUpdateStatus((prev) => ({
          ...prev,
          [skillId]: {
            hasUpdate: result.has_update,
            message: result.message,
          },
        }));
      } catch (error) {
        console.error("检查 ClawHub 更新失败:", error);
        setClawhubUpdateStatus((prev) => ({
          ...prev,
          [skillId]: {
            hasUpdate: false,
            message: "检查失败，请稍后重试",
          },
        }));
      } finally {
        setSkillActionState(null);
      }
    },
    [setClawhubUpdateStatus, setSkillActionState, skillActionState],
  );

  const handleUpdateClawhubSkill = useCallback(
    async (skillId: string) => {
      if (skillActionState) return;
      setSkillActionState({ skillId, action: "update" });
      try {
        const result = await invoke<{ manifest: SkillManifest }>(
          "update_clawhub_skill",
          { skillId },
        );
        await loadSkills();
        if (result?.manifest?.id) {
          setSelectedSkillId(result.manifest.id);
        }
        setClawhubUpdateStatus((prev) => ({
          ...prev,
          [skillId]: {
            hasUpdate: false,
            message: "已更新到最新版本",
          },
        }));
      } catch (error) {
        console.error("更新 ClawHub 技能失败:", error);
        setClawhubUpdateStatus((prev) => ({
          ...prev,
          [skillId]: {
            hasUpdate: true,
            message: "更新失败，请稍后重试",
          },
        }));
      } finally {
        setSkillActionState(null);
      }
    },
    [
      loadSkills,
      setClawhubUpdateStatus,
      setSelectedSkillId,
      setSkillActionState,
      skillActionState,
    ],
  );

  const handleInstallFromLibrary = useCallback(
    async (request: ClawhubInstallRequest) => {
      try {
        const result = await invoke<{
          manifest: SkillManifest;
          missing_mcp: string[];
        }>("install_clawhub_skill", {
          slug: request.slug,
          githubUrl: request.githubUrl ?? request.sourceUrl ?? null,
        });
        await loadSkills();
        if (result?.manifest?.id) {
          setSelectedSkillId(result.manifest.id);
        }
      } catch (error) {
        const duplicateName = extractDuplicateSkillName(error);
        if (duplicateName) {
          throw new Error(
            `技能名称冲突：已存在「${duplicateName}」，请先重命名后再安装。`,
          );
        }
        throw error;
      }
    },
    [loadSkills, setSelectedSkillId],
  );

  const handleRenderExpertPreview = useCallback(
    async (payload: ExpertPreviewPayload): Promise<ExpertPreviewResult> => {
      const result = await invoke<{ markdown: string; save_path: string }>(
        "render_local_skill_preview",
        {
          name: payload.name,
          description: payload.description,
          whenToUse: payload.whenToUse,
          targetDir: payload.targetDir ?? null,
        },
      );

      return {
        markdown: result.markdown,
        savePath: result.save_path,
      };
    },
    [],
  );

  const handleSkillInstalledFromChat = useCallback(async () => {
    await loadSkills();
  }, [loadSkills]);

  return {
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
  };
}
