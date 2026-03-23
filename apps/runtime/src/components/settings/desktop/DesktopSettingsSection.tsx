import { useEffect, useState } from "react";
import { RiskConfirmDialog } from "../../RiskConfirmDialog";
import type { ModelConfig } from "../../../types";
import {
  DEFAULT_RUNTIME_PREFERENCES,
  clearDesktopCacheAndLogs,
  exportDesktopDiagnosticsBundle,
  exportDesktopEnvironmentSummary,
  getDesktopDiagnosticsStatus,
  getDesktopLifecyclePaths,
  getRuntimePreferences,
  normalizeRuntimePreferences,
  openDesktopDiagnosticsDir,
  openDesktopPath,
  saveDesktopRuntimePreferences,
  saveRuntimeLanguagePreferences,
  type DesktopRuntimePreferencesInput,
  type RuntimeLanguagePreferencesInput,
  type DesktopDiagnosticsStatus,
  type DesktopLifecyclePaths,
} from "./desktopSettingsService";

interface DesktopSettingsSectionProps {
  models: ModelConfig[];
  visible: boolean;
}

export function DesktopSettingsSection({ models, visible }: DesktopSettingsSectionProps) {
  const [runtimePreferences, setRuntimePreferences] = useState(DEFAULT_RUNTIME_PREFERENCES);
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
    useState<"idle" | "opening" | "clearing" | "exporting">("idle");
  const [desktopLifecycleError, setDesktopLifecycleError] = useState("");
  const [desktopLifecycleMessage, setDesktopLifecycleMessage] = useState("");
  const [desktopDiagnosticsStatus, setDesktopDiagnosticsStatus] =
    useState<DesktopDiagnosticsStatus | null>(null);

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
        operation_permission_mode: runtimePreferences.operation_permission_mode,
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

  async function handleOpenDesktopDiagnosticsDir() {
    setDesktopLifecycleActionState("opening");
    setDesktopLifecycleError("");
    setDesktopLifecycleMessage("");
    try {
      await openDesktopDiagnosticsDir();
    } catch (cause) {
      setDesktopLifecycleError("打开诊断目录失败: " + String(cause));
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

  return (
    <>
      <div className="bg-white rounded-lg p-4 space-y-3">
        <div className="text-xs font-medium text-gray-500">语言与沉浸式翻译</div>
        <div>
          <label className={labelCls}>默认语言</label>
          <select
            aria-label="默认语言"
            className={inputCls}
            value={runtimePreferences.default_language}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({ ...prev, default_language: e.target.value }))
            }
          >
            <option value="zh-CN">简体中文 (zh-CN)</option>
            <option value="en-US">English (en-US)</option>
          </select>
        </div>
        <label className="flex items-center gap-2 text-xs text-gray-600">
          <input
            aria-label="启用沉浸式翻译"
            type="checkbox"
            checked={runtimePreferences.immersive_translation_enabled}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({
                ...prev,
                immersive_translation_enabled: e.target.checked,
              }))
            }
          />
          启用沉浸式翻译
        </label>
        <div>
          <label className={labelCls}>显示模式</label>
          <select
            aria-label="翻译显示模式"
            className={inputCls}
            value={runtimePreferences.immersive_translation_display}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({
                ...prev,
                immersive_translation_display:
                  e.target.value === "bilingual_inline" ? "bilingual_inline" : "translated_only",
              }))
            }
          >
            <option value="translated_only">仅译文</option>
            <option value="bilingual_inline">双语对照</option>
          </select>
        </div>
        <div>
          <label className={labelCls}>翻译触发方式</label>
          <select
            aria-label="翻译触发方式"
            className={inputCls}
            value={runtimePreferences.immersive_translation_trigger}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({
                ...prev,
                immersive_translation_trigger: e.target.value === "manual" ? "manual" : "auto",
              }))
            }
          >
            <option value="auto">自动翻译（默认）</option>
            <option value="manual">手动触发</option>
          </select>
        </div>
        <div>
          <label className={labelCls}>翻译引擎策略</label>
          <select
            aria-label="翻译引擎策略"
            className={inputCls}
            value={runtimePreferences.translation_engine}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({
                ...prev,
                translation_engine:
                  e.target.value === "model_only" || e.target.value === "free_only"
                    ? e.target.value
                    : "model_then_free",
                translation_model_id: e.target.value === "free_only" ? "" : prev.translation_model_id,
              }))
            }
          >
            <option value="model_then_free">优先模型，失败回退免费翻译（推荐）</option>
            <option value="model_only">仅使用翻译模型</option>
            <option value="free_only">仅使用免费翻译</option>
          </select>
        </div>
        <div>
          <label className={labelCls}>翻译模型</label>
          <select
            aria-label="翻译模型"
            className={inputCls}
            disabled={runtimePreferences.translation_engine === "free_only"}
            value={runtimePreferences.translation_model_id}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({
                ...prev,
                translation_model_id: e.target.value,
              }))
            }
          >
            <option value="">跟随默认模型</option>
            {models.map((model) => (
              <option key={model.id} value={model.id}>
                {model.name || model.model_name || model.id}
              </option>
            ))}
          </select>
        </div>
        {runtimePreferences.translation_engine !== "free_only" && models.length === 0 && (
          <div className="bg-amber-50 text-amber-700 text-xs px-2 py-1 rounded">
            当前未配置可用模型。翻译会尝试免费翻译接口；若策略为“仅使用翻译模型”则可能失败。
          </div>
        )}
        {runtimePreferences.translation_engine === "model_only" && models.length === 0 && (
          <div className="bg-red-50 text-red-700 text-xs px-2 py-1 rounded">
            已选择仅模型翻译，但当前无可用模型配置。建议切换到“优先模型，失败回退免费翻译”。
          </div>
        )}
        {runtimePreferences.translation_model_id &&
          !models.some((model) => model.id === runtimePreferences.translation_model_id) && (
            <div className="bg-amber-50 text-amber-700 text-xs px-2 py-1 rounded">
              选中的翻译模型不存在，将自动跟随默认模型或回退免费翻译。
            </div>
          )}
        {runtimePreferencesError && (
          <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">
            {runtimePreferencesError}
          </div>
        )}
        {runtimePreferencesSaveState === "saved" && (
          <div className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">已保存</div>
        )}
        <button
          onClick={handleSaveRuntimePreferences}
          disabled={runtimePreferencesSaveState === "saving"}
          className="w-full bg-blue-500 hover:bg-blue-600 disabled:opacity-50 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
        >
          {runtimePreferencesSaveState === "saving" ? "保存中..." : "保存语言与翻译设置"}
        </button>
      </div>

      <div className="bg-white rounded-lg p-4 space-y-3 mt-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <div className="text-xs font-medium text-gray-500">桌面运行</div>
            <div className="mt-1 text-xs text-gray-400">控制应用的开机、自启动窗口状态和关闭行为。</div>
          </div>
        </div>
        <section className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3 space-y-3">
          <div>
            <div className="text-xs font-medium text-gray-500">操作权限</div>
            <div className="mt-1 text-xs text-gray-400">控制智能体执行本地操作时的默认确认方式。</div>
          </div>
          <label className="flex items-start gap-2 rounded-lg border border-gray-200 bg-white px-3 py-2 text-xs text-gray-700">
            <input
              type="radio"
              name="operation-permission-mode"
              aria-label="标准模式（推荐）"
              checked={runtimePreferences.operation_permission_mode === "standard"}
              onChange={() => requestOperationPermissionModeChange("standard")}
            />
            <span>
              <span className="block font-medium text-gray-800">标准模式（推荐）</span>
              <span className="mt-1 block text-gray-500">
                大部分操作自动执行，仅在删除、永久覆盖、外部提交等高危操作时确认。
              </span>
            </span>
          </label>
          <label className="flex items-start gap-2 rounded-lg border border-gray-200 bg-white px-3 py-2 text-xs text-gray-700">
            <input
              type="radio"
              name="operation-permission-mode"
              aria-label="全自动模式"
              checked={runtimePreferences.operation_permission_mode === "full_access"}
              onChange={() => requestOperationPermissionModeChange("full_access")}
            />
            <span>
              <span className="block font-medium text-gray-800">全自动模式</span>
              <span className="mt-1 block text-gray-500">所有操作自动执行，适合可信任务与熟悉环境。</span>
            </span>
          </label>
        </section>
        <label className="flex items-center gap-2 text-xs text-gray-600">
          <input
            aria-label="开机启动"
            type="checkbox"
            checked={runtimePreferences.launch_at_login}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({
                ...prev,
                launch_at_login: e.target.checked,
              }))
            }
          />
          开机启动
        </label>
        <label className="flex items-center gap-2 text-xs text-gray-600">
          <input
            aria-label="启动时最小化"
            type="checkbox"
            checked={runtimePreferences.launch_minimized}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({
                ...prev,
                launch_minimized: e.target.checked,
              }))
            }
          />
          启动时最小化
        </label>
        <label className="flex items-center gap-2 text-xs text-gray-600">
          <input
            aria-label="关闭时最小化到托盘"
            type="checkbox"
            checked={runtimePreferences.close_to_tray}
            onChange={(e) =>
              setRuntimePreferences((prev) => ({
                ...prev,
                close_to_tray: e.target.checked,
              }))
            }
          />
          关闭时最小化到托盘
        </label>
        <div className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3 text-xs text-gray-600 space-y-1">
          <div>建议保持“关闭时最小化到托盘”开启，避免误关后中断后台任务。</div>
          <div>如果启用“开机启动”，通常建议按需再开启“启动时最小化”。</div>
        </div>
        {desktopPreferencesError && (
          <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">{desktopPreferencesError}</div>
        )}
        {desktopPreferencesSaveState === "saved" && (
          <div className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">桌面设置已保存</div>
        )}
        <button
          onClick={handleSaveDesktopPreferences}
          disabled={desktopPreferencesSaveState === "saving"}
          className="w-full bg-blue-500 hover:bg-blue-600 disabled:opacity-50 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
        >
          {desktopPreferencesSaveState === "saving" ? "保存中..." : "保存桌面设置"}
        </button>
      </div>

      <div className="bg-white rounded-lg p-4 space-y-3 mt-4">
        <div className="text-xs font-medium text-gray-500">本机目录与清理</div>
        {desktopLifecycleLoading && (
          <div className="bg-gray-50 text-gray-500 text-xs px-2 py-1 rounded">正在读取本地目录</div>
        )}
        {desktopLifecyclePaths && (
          <div className="space-y-3">
            <div className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3">
              <div className="text-xs font-medium text-gray-500">应用数据目录</div>
              <div className="mt-1 break-all text-xs text-gray-700">{desktopLifecyclePaths.app_data_dir}</div>
              <button
                type="button"
                onClick={() => void handleOpenDesktopPath(desktopLifecyclePaths.app_data_dir)}
                disabled={desktopLifecycleActionState === "opening"}
                className="mt-2 bg-white hover:bg-gray-100 border border-gray-200 text-gray-700 text-xs px-3 py-1.5 rounded-lg transition-all active:scale-[0.97]"
              >
                打开应用数据目录
              </button>
            </div>
            <div className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3">
              <div className="text-xs font-medium text-gray-500">缓存目录</div>
              <div className="mt-1 break-all text-xs text-gray-700">{desktopLifecyclePaths.cache_dir}</div>
              <button
                type="button"
                onClick={() => void handleOpenDesktopPath(desktopLifecyclePaths.cache_dir)}
                disabled={desktopLifecycleActionState === "opening"}
                className="mt-2 bg-white hover:bg-gray-100 border border-gray-200 text-gray-700 text-xs px-3 py-1.5 rounded-lg transition-all active:scale-[0.97]"
              >
                打开缓存目录
              </button>
            </div>
            <div className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3">
              <div className="text-xs font-medium text-gray-500">日志目录</div>
              <div className="mt-1 break-all text-xs text-gray-700">{desktopLifecyclePaths.log_dir}</div>
              <button
                type="button"
                onClick={() => void handleOpenDesktopPath(desktopLifecyclePaths.log_dir)}
                disabled={desktopLifecycleActionState === "opening"}
                className="mt-2 bg-white hover:bg-gray-100 border border-gray-200 text-gray-700 text-xs px-3 py-1.5 rounded-lg transition-all active:scale-[0.97]"
              >
                打开日志目录
              </button>
            </div>
            <div className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3">
              <div className="text-xs font-medium text-gray-500">诊断目录</div>
              <div className="mt-1 break-all text-xs text-gray-700">{desktopLifecyclePaths.diagnostics_dir}</div>
              <button
                type="button"
                onClick={() => void handleOpenDesktopDiagnosticsDir()}
                disabled={desktopLifecycleActionState === "opening"}
                className="mt-2 bg-white hover:bg-gray-100 border border-gray-200 text-gray-700 text-xs px-3 py-1.5 rounded-lg transition-all active:scale-[0.97]"
              >
                打开诊断目录
              </button>
            </div>
            <div className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3">
              <div className="text-xs font-medium text-gray-500">默认工作目录</div>
              <div className="mt-1 break-all text-xs text-gray-700">
                {desktopLifecyclePaths.default_work_dir || runtimePreferences.default_work_dir || "未设置"}
              </div>
              <button
                type="button"
                onClick={() =>
                  void handleOpenDesktopPath(
                    desktopLifecyclePaths.default_work_dir || runtimePreferences.default_work_dir,
                  )
                }
                disabled={
                  desktopLifecycleActionState === "opening" ||
                  !(desktopLifecyclePaths.default_work_dir || runtimePreferences.default_work_dir).trim()
                }
                className="mt-2 bg-white hover:bg-gray-100 border border-gray-200 text-gray-700 text-xs px-3 py-1.5 rounded-lg transition-all active:scale-[0.97] disabled:opacity-50"
              >
                打开工作目录
              </button>
            </div>
            {desktopDiagnosticsStatus && (
              <div className="rounded-lg border border-blue-100 bg-blue-50 px-3 py-3 space-y-2">
                <div className="text-xs font-medium text-blue-700">诊断状态</div>
                <div className="text-xs text-blue-700 break-all">当前运行 ID：{desktopDiagnosticsStatus.current_run_id}</div>
                <div className="text-xs text-blue-700 break-all">导出目录：{desktopDiagnosticsStatus.exports_dir}</div>
                <div className="text-xs text-blue-700 break-all">审计目录：{desktopDiagnosticsStatus.audit_dir}</div>
                {desktopDiagnosticsStatus.abnormal_previous_run && (
                  <div className="text-xs text-amber-700">检测到上次运行可能异常退出</div>
                )}
                {desktopDiagnosticsStatus.last_clean_exit_at && (
                  <div className="text-xs text-blue-700">
                    上次正常退出：{desktopDiagnosticsStatus.last_clean_exit_at}
                  </div>
                )}
                {desktopDiagnosticsStatus.latest_crash && (
                  <div className="text-xs text-red-700 break-all">
                    最近崩溃：{desktopDiagnosticsStatus.latest_crash.timestamp}{" "}
                    {desktopDiagnosticsStatus.latest_crash.message}
                  </div>
                )}
              </div>
            )}
          </div>
        )}
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => void handleClearDesktopCacheAndLogs()}
            disabled={desktopLifecycleActionState === "clearing"}
            className="flex-1 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {desktopLifecycleActionState === "clearing" ? "清理中..." : "清理缓存与日志"}
          </button>
          <button
            type="button"
            onClick={() => void handleExportDesktopEnvironmentSummary()}
            disabled={desktopLifecycleActionState === "exporting"}
            className="flex-1 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {desktopLifecycleActionState === "exporting" ? "导出中..." : "导出环境摘要"}
          </button>
          <button
            type="button"
            onClick={() => void handleExportDesktopDiagnosticsBundle()}
            disabled={desktopLifecycleActionState === "exporting"}
            className="flex-1 bg-blue-50 hover:bg-blue-100 disabled:opacity-50 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97] text-blue-700"
          >
            {desktopLifecycleActionState === "exporting" ? "导出中..." : "导出诊断包"}
          </button>
        </div>
        <div className="rounded-lg border border-amber-100 bg-amber-50 px-3 py-3 text-xs text-amber-700 space-y-1">
          <div>卸载程序不会删除你的工作目录。</div>
          <div>如需彻底清理，请先清理缓存与日志，再手动删除应用数据目录。</div>
        </div>
        {desktopLifecycleError && (
          <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">{desktopLifecycleError}</div>
        )}
        {desktopLifecycleMessage && (
          <div className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">{desktopLifecycleMessage}</div>
        )}
      </div>

      <RiskConfirmDialog
        open={showPermissionModeConfirm}
        level="high"
        title="切换到全自动模式"
        summary="全自动模式会允许智能体自动执行所有本地操作。"
        impact="这会显著降低人工确认频率，适合可信任务与受控环境。"
        irreversible={false}
        confirmLabel="切换为全自动模式"
        cancelLabel="暂不切换"
        loading={false}
        onConfirm={handleConfirmOperationPermissionMode}
        onCancel={handleCancelOperationPermissionMode}
      />
    </>
  );
}
