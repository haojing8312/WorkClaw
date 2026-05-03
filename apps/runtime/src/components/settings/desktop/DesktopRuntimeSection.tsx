import { RiskConfirmDialog } from "../../RiskConfirmDialog";
import type { RuntimePreferences } from "../../../types";

interface DesktopRuntimeSectionProps {
  desktopPreferencesError: string;
  desktopPreferencesSaveState: "idle" | "saving" | "saved" | "error";
  onCancelOperationPermissionMode: () => void;
  onConfirmOperationPermissionMode: () => void;
  onDesktopPreferencesChange: (
    updater: (prev: RuntimePreferences) => RuntimePreferences,
  ) => void;
  onRequestOperationPermissionModeChange: (mode: "standard" | "full_access") => void;
  onSaveDesktopPreferences: () => void | Promise<void>;
  runtimePreferences: RuntimePreferences;
  showPermissionModeConfirm: boolean;
}

export function DesktopRuntimeSection({
  desktopPreferencesError,
  desktopPreferencesSaveState,
  onCancelOperationPermissionMode,
  onConfirmOperationPermissionMode,
  onDesktopPreferencesChange,
  onRequestOperationPermissionModeChange,
  onSaveDesktopPreferences,
  runtimePreferences,
  showPermissionModeConfirm,
}: DesktopRuntimeSectionProps) {
  return (
    <>
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
              onChange={() => onRequestOperationPermissionModeChange("standard")}
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
              onChange={() => onRequestOperationPermissionModeChange("full_access")}
            />
            <span>
              <span className="block font-medium text-gray-800">全自动模式</span>
              <span className="mt-1 block text-gray-500">
                所有操作自动执行，文件工具可访问会话目录外的普通路径；敏感路径仍会被保护。
              </span>
            </span>
          </label>
        </section>
        <label className="flex items-center gap-2 text-xs text-gray-600">
          <input
            aria-label="开机启动"
            type="checkbox"
            checked={runtimePreferences.launch_at_login}
            onChange={(e) =>
              onDesktopPreferencesChange((prev) => ({
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
              onDesktopPreferencesChange((prev) => ({
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
              onDesktopPreferencesChange((prev) => ({
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
          onClick={onSaveDesktopPreferences}
          disabled={desktopPreferencesSaveState === "saving"}
          className="w-full bg-blue-500 hover:bg-blue-600 disabled:opacity-50 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
        >
          {desktopPreferencesSaveState === "saving" ? "保存中..." : "保存桌面设置"}
        </button>
      </div>

      <RiskConfirmDialog
        open={showPermissionModeConfirm}
        level="high"
        title="切换到全自动模式"
        summary="全自动模式会允许智能体自动执行本地操作，并让文件工具访问会话目录外的普通路径。"
        impact="这会显著降低人工确认频率；敏感路径仍会被保护，但请只在可信任务与受控环境中使用。"
        irreversible={false}
        confirmLabel="切换为全自动模式"
        cancelLabel="暂不切换"
        loading={false}
        onConfirm={onConfirmOperationPermissionMode}
        onCancel={onCancelOperationPermissionMode}
      />
    </>
  );
}
