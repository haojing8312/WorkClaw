import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import type { ModelConfig, RuntimePreferences } from "../../../types";
import { DesktopLanguageSection } from "./DesktopLanguageSection";
import { DesktopLifecycleSection } from "./DesktopLifecycleSection";
import { DesktopRuntimeSection } from "./DesktopRuntimeSection";
import {
  DEFAULT_RUNTIME_PREFERENCES,
  clearDesktopCacheAndLogs,
  exportDesktopDiagnosticsBundle,
  exportDesktopEnvironmentSummary,
  getDesktopDiagnosticsStatus,
  getDesktopLifecyclePaths,
  getRuntimePreferences,
  normalizeRuntimePreferences,
  openDesktopPath,
  saveDesktopRuntimePreferences,
  saveRuntimeLanguagePreferences,
  scheduleDesktopRuntimeRootMigration,
  type DesktopRuntimePreferencesInput,
  type DesktopDiagnosticsStatus,
  type DesktopLifecyclePaths,
  type RuntimeLanguagePreferencesInput,
} from "./desktopSettingsService";

interface DesktopSettingsSectionProps {
  models: ModelConfig[];
  visible: boolean;
}

function describeRuntimeMigrationState(paths: DesktopLifecyclePaths | null) {
  if (!paths) {
    return null;
  }
  if (paths.pending_runtime_root_dir?.trim()) {
    return {
      tone: "amber" as const,
      title: "等待下次启动迁移",
      message: `下次启动时会自动迁移到：${paths.pending_runtime_root_dir.trim()}`,
    };
  }
  switch (paths.last_runtime_migration_status) {
    case "completed":
      return {
        tone: "green" as const,
        title: "最近一次迁移已完成",
        message:
          paths.last_runtime_migration_message?.trim() || `当前正在使用：${paths.runtime_root_dir}`,
      };
    case "failed":
    case "rolled_back":
      return {
        tone: "red" as const,
        title: "最近一次迁移未完成",
        message:
          paths.last_runtime_migration_message?.trim() ||
          "应用已自动回退到旧目录，你可以调整目标目录后重试。",
      };
    case "in_progress":
      return {
        tone: "amber" as const,
        title: "迁移进行中",
        message:
          paths.last_runtime_migration_message?.trim() || "应用会在启动早期继续完成目录迁移。",
      };
    default:
      return null;
  }
}

export function DesktopSettingsSection({ models, visible }: DesktopSettingsSectionProps) {
  const [runtimePreferences, setRuntimePreferences] =
    useState<RuntimePreferences>(DEFAULT_RUNTIME_PREFERENCES);
  const [runtimePreferencesSaveState, setRuntimePreferencesSaveState] =
    useState<"idle" | "saving" | "saved" | "error">("idle");
  const [runtimePreferencesError, setRuntimePreferencesError] = useState("");
  const [desktopPreferencesSaveState, setDesktopPreferencesSaveState] =
    useState<"idle" | "saving" | "saved" | "error">("idle");
  const [desktopPreferencesError, setDesktopPreferencesError] = useState("");
  const [pendingPermissionMode, setPendingPermissionMode] = useState<"standard" | "full_access" | null>(null);
  const [showPermissionModeConfirm, setShowPermissionModeConfirm] = useState(false);
  const [desktopLifecyclePaths, setDesktopLifecyclePaths] = useState<DesktopLifecyclePaths | null>(null);
  const [desktopLifecycleLoading, setDesktopLifecycleLoading] = useState(false);
  const [desktopLifecycleActionState, setDesktopLifecycleActionState] =
    useState<"idle" | "opening" | "clearing" | "exporting" | "migrating">("idle");
  const [desktopLifecycleError, setDesktopLifecycleError] = useState("");
  const [desktopLifecycleMessage, setDesktopLifecycleMessage] = useState("");
  const [desktopDiagnosticsStatus, setDesktopDiagnosticsStatus] =
    useState<DesktopDiagnosticsStatus | null>(null);
  const [pendingRuntimeRootSelection, setPendingRuntimeRootSelection] = useState("");

  const inputCls = "sm-input w-full text-sm py-1.5";
  const labelCls = "sm-field-label";

  useEffect(() => {
    let cancelled = false;

    async function loadRuntimePreferences() {
      try {
        const prefs = await getRuntimePreferences();
        if (cancelled) return;
        setRuntimePreferences(normalizeRuntimePreferences(prefs));
      } catch (cause) {
        console.warn("加载运行时偏好失败:", cause);
        if (!cancelled) {
          setRuntimePreferences(DEFAULT_RUNTIME_PREFERENCES);
        }
      }
    }

    void loadRuntimePreferences();
    void refreshDesktopLifecycleData(() => cancelled);

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (runtimePreferencesSaveState !== "saved") return;
    const timer = window.setTimeout(() => setRuntimePreferencesSaveState("idle"), 1200);
    return () => window.clearTimeout(timer);
  }, [runtimePreferencesSaveState]);

  useEffect(() => {
    if (desktopPreferencesSaveState !== "saved") return;
    const timer = window.setTimeout(() => setDesktopPreferencesSaveState("idle"), 1200);
    return () => window.clearTimeout(timer);
  }, [desktopPreferencesSaveState]);

  if (!visible) {
    return null;
  }

  async function refreshDesktopLifecycleData(isCancelled: () => boolean = () => false) {
    if (isCancelled()) return;
    setDesktopLifecycleLoading(true);
    setDesktopLifecycleError("");
    try {
      const [paths, diagnostics] = await Promise.all([
        getDesktopLifecyclePaths(),
        getDesktopDiagnosticsStatus(),
      ]);
      if (isCancelled()) return;
      setDesktopLifecyclePaths(paths);
      setDesktopDiagnosticsStatus(diagnostics);
    } catch (cause) {
      if (!isCancelled()) {
        setDesktopLifecycleError("加载数据目录失败: " + String(cause));
      }
    } finally {
      if (!isCancelled()) {
        setDesktopLifecycleLoading(false);
      }
    }
  }

  async function handleSaveRuntimePreferences() {
    setRuntimePreferencesSaveState("saving");
    setRuntimePreferencesError("");
    try {
      const input: RuntimeLanguagePreferencesInput = {
        default_language: runtimePreferences.default_language,
        immersive_translation_enabled: runtimePreferences.immersive_translation_enabled,
        immersive_translation_display: runtimePreferences.immersive_translation_display,
        immersive_translation_trigger: runtimePreferences.immersive_translation_trigger,
        translation_engine: runtimePreferences.translation_engine,
        translation_model_id: runtimePreferences.translation_model_id,
      };
      const saved = await saveRuntimeLanguagePreferences(input);
      setRuntimePreferences(normalizeRuntimePreferences(saved));
      setRuntimePreferencesSaveState("saved");
    } catch (cause) {
      setRuntimePreferencesSaveState("error");
      setRuntimePreferencesError("保存语言与翻译设置失败: " + String(cause));
    }
  }

  async function handleSaveDesktopPreferences() {
    setDesktopPreferencesSaveState("saving");
    setDesktopPreferencesError("");
    try {
      const input: DesktopRuntimePreferencesInput = {
        launch_at_login: runtimePreferences.launch_at_login,
        launch_minimized: runtimePreferences.launch_minimized,
        close_to_tray: runtimePreferences.close_to_tray,
        operation_permission_mode:
          runtimePreferences.operation_permission_mode === "full_access" ? "full_access" : "standard",
      };
      const saved = await saveDesktopRuntimePreferences(input);
      setRuntimePreferences(normalizeRuntimePreferences(saved));
      setDesktopPreferencesSaveState("saved");
    } catch (cause) {
      setDesktopPreferencesSaveState("error");
      setDesktopPreferencesError("保存桌面设置失败: " + String(cause));
    }
  }

  function requestOperationPermissionModeChange(nextMode: "standard" | "full_access") {
    if (nextMode !== "full_access") {
      setRuntimePreferences((prev) => ({
        ...prev,
        operation_permission_mode: "standard",
      }));
      return;
    }

    if (runtimePreferences.operation_permission_mode === "full_access") {
      return;
    }

    setPendingPermissionMode(nextMode);
    setShowPermissionModeConfirm(true);
  }

  function handleConfirmOperationPermissionMode() {
    if (pendingPermissionMode) {
      setRuntimePreferences((prev) => ({
        ...prev,
        operation_permission_mode: pendingPermissionMode,
      }));
    }
    setPendingPermissionMode(null);
    setShowPermissionModeConfirm(false);
  }

  function handleCancelOperationPermissionMode() {
    setPendingPermissionMode(null);
    setShowPermissionModeConfirm(false);
  }

  async function handleOpenDesktopPath(path: string) {
    if (!path.trim()) return;
    setDesktopLifecycleActionState("opening");
    setDesktopLifecycleError("");
    setDesktopLifecycleMessage("");
    try {
      await openDesktopPath(path);
    } catch (cause) {
      setDesktopLifecycleError("打开目录失败: " + String(cause));
    } finally {
      setDesktopLifecycleActionState("idle");
    }
  }

  async function handleChooseRuntimeRoot() {
    setDesktopLifecycleError("");
    setDesktopLifecycleMessage("");
    try {
      const selection = await open({
        directory: true,
        multiple: false,
        title: "选择数据根目录",
        defaultPath: desktopLifecyclePaths?.runtime_root_dir || undefined,
      });
      const selectedPath = Array.isArray(selection) ? selection[0] : selection;
      if (typeof selectedPath !== "string") {
        return;
      }
      const trimmedPath = selectedPath.trim();
      if (!trimmedPath) {
        return;
      }
      const currentRoot = desktopLifecyclePaths?.runtime_root_dir?.trim() || "";
      if (trimmedPath === currentRoot) {
        setPendingRuntimeRootSelection("");
        setDesktopLifecycleMessage("当前已在使用该目录");
        return;
      }
      setPendingRuntimeRootSelection(trimmedPath);
    } catch (cause) {
      setDesktopLifecycleError("选择数据根目录失败: " + String(cause));
    }
  }

  async function handleScheduleRuntimeRootMigration() {
    const targetRoot = pendingRuntimeRootSelection.trim();
    if (!targetRoot) {
      return;
    }
    setDesktopLifecycleActionState("migrating");
    setDesktopLifecycleError("");
    setDesktopLifecycleMessage("");
    try {
      await scheduleDesktopRuntimeRootMigration(targetRoot);
      setDesktopLifecycleMessage("已准备迁移，应用即将重启");
    } catch (cause) {
      setDesktopLifecycleError("设置数据根目录失败: " + String(cause));
    } finally {
      setDesktopLifecycleActionState("idle");
    }
  }

  function handleCancelRuntimeRootMigration() {
    setPendingRuntimeRootSelection("");
  }

  async function handleClearDesktopCacheAndLogs() {
    setDesktopLifecycleActionState("clearing");
    setDesktopLifecycleError("");
    setDesktopLifecycleMessage("");
    try {
      const result = await clearDesktopCacheAndLogs();
      setDesktopLifecycleMessage(`已清理 ${result.removed_files} 个文件，删除 ${result.removed_dirs} 个目录`);
      await refreshDesktopLifecycleData();
    } catch (cause) {
      setDesktopLifecycleError("清理缓存与日志失败: " + String(cause));
    } finally {
      setDesktopLifecycleActionState("idle");
    }
  }

  async function handleExportDesktopEnvironmentSummary() {
    setDesktopLifecycleActionState("exporting");
    setDesktopLifecycleError("");
    setDesktopLifecycleMessage("");
    try {
      const summary = await exportDesktopEnvironmentSummary();
      await navigator?.clipboard?.writeText?.(summary);
      setDesktopLifecycleMessage("环境摘要已复制到剪贴板");
    } catch (cause) {
      setDesktopLifecycleError("导出环境摘要失败: " + String(cause));
    } finally {
      setDesktopLifecycleActionState("idle");
    }
  }

  async function handleExportDesktopDiagnosticsBundle() {
    setDesktopLifecycleActionState("exporting");
    setDesktopLifecycleError("");
    setDesktopLifecycleMessage("");
    try {
      const bundlePath = await exportDesktopDiagnosticsBundle();
      setDesktopLifecycleMessage(`诊断包已导出：${bundlePath}`);
      await refreshDesktopLifecycleData();
    } catch (cause) {
      setDesktopLifecycleError("导出诊断包失败: " + String(cause));
    } finally {
      setDesktopLifecycleActionState("idle");
    }
  }

  const hasPendingRuntimeRootSelection =
    pendingRuntimeRootSelection.trim().length > 0 &&
    pendingRuntimeRootSelection.trim() !== (desktopLifecyclePaths?.runtime_root_dir?.trim() || "");
  const runtimeMigrationState = describeRuntimeMigrationState(desktopLifecyclePaths);

  return (
    <>
      <DesktopLanguageSection
        inputCls={inputCls}
        labelCls={labelCls}
        models={models}
        runtimePreferences={runtimePreferences}
        runtimePreferencesError={runtimePreferencesError}
        runtimePreferencesSaveState={runtimePreferencesSaveState}
        onRuntimePreferencesChange={(updater) => setRuntimePreferences(updater)}
        onSaveRuntimePreferences={handleSaveRuntimePreferences}
      />

      <DesktopRuntimeSection
        desktopPreferencesError={desktopPreferencesError}
        desktopPreferencesSaveState={desktopPreferencesSaveState}
        onCancelOperationPermissionMode={handleCancelOperationPermissionMode}
        onConfirmOperationPermissionMode={handleConfirmOperationPermissionMode}
        onDesktopPreferencesChange={(updater) => setRuntimePreferences(updater)}
        onRequestOperationPermissionModeChange={requestOperationPermissionModeChange}
        onSaveDesktopPreferences={handleSaveDesktopPreferences}
        runtimePreferences={runtimePreferences}
        showPermissionModeConfirm={showPermissionModeConfirm}
      />

      <DesktopLifecycleSection
        desktopDiagnosticsStatus={desktopDiagnosticsStatus}
        desktopLifecycleActionState={desktopLifecycleActionState}
        desktopLifecycleError={desktopLifecycleError}
        desktopLifecycleLoading={desktopLifecycleLoading}
        desktopLifecycleMessage={desktopLifecycleMessage}
        desktopLifecyclePaths={desktopLifecyclePaths}
        hasPendingRuntimeRootSelection={hasPendingRuntimeRootSelection}
        onCancelRuntimeRootMigration={handleCancelRuntimeRootMigration}
        onChooseRuntimeRoot={handleChooseRuntimeRoot}
        onClearDesktopCacheAndLogs={handleClearDesktopCacheAndLogs}
        onExportDesktopDiagnosticsBundle={handleExportDesktopDiagnosticsBundle}
        onExportDesktopEnvironmentSummary={handleExportDesktopEnvironmentSummary}
        onOpenDesktopPath={handleOpenDesktopPath}
        onScheduleRuntimeRootMigration={handleScheduleRuntimeRootMigration}
        pendingRuntimeRootSelection={pendingRuntimeRootSelection}
        runtimeMigrationState={runtimeMigrationState}
      />
    </>
  );
}
