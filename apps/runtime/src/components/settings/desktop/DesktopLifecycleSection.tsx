import type {
  DesktopDiagnosticsStatus,
  DesktopLifecyclePaths,
} from "./desktopSettingsService";

interface DesktopLifecycleSectionProps {
  desktopDiagnosticsStatus: DesktopDiagnosticsStatus | null;
  desktopLifecycleActionState: "idle" | "opening" | "clearing" | "exporting" | "migrating";
  desktopLifecycleError: string;
  desktopLifecycleLoading: boolean;
  desktopLifecycleMessage: string;
  desktopLifecyclePaths: DesktopLifecyclePaths | null;
  hasPendingRuntimeRootSelection: boolean;
  onCancelRuntimeRootMigration: () => void;
  onChooseRuntimeRoot: () => void | Promise<void>;
  onClearDesktopCacheAndLogs: () => void | Promise<void>;
  onExportDesktopDiagnosticsBundle: () => void | Promise<void>;
  onExportDesktopEnvironmentSummary: () => void | Promise<void>;
  onOpenDesktopPath: (path: string) => void | Promise<void>;
  onScheduleRuntimeRootMigration: () => void | Promise<void>;
  pendingRuntimeRootSelection: string;
  runtimeMigrationState: { tone: "amber" | "green" | "red"; title: string; message: string } | null;
}

export function DesktopLifecycleSection({
  desktopDiagnosticsStatus,
  desktopLifecycleActionState,
  desktopLifecycleError,
  desktopLifecycleLoading,
  desktopLifecycleMessage,
  desktopLifecyclePaths,
  hasPendingRuntimeRootSelection,
  onCancelRuntimeRootMigration,
  onChooseRuntimeRoot,
  onClearDesktopCacheAndLogs,
  onExportDesktopDiagnosticsBundle,
  onExportDesktopEnvironmentSummary,
  onOpenDesktopPath,
  onScheduleRuntimeRootMigration,
  pendingRuntimeRootSelection,
  runtimeMigrationState,
}: DesktopLifecycleSectionProps) {
  return (
    <div className="bg-white rounded-lg p-4 space-y-3 mt-4">
      <div className="text-xs font-medium text-gray-500">本机目录与清理</div>
      {desktopLifecycleLoading && (
        <div className="bg-gray-50 text-gray-500 text-xs px-2 py-1 rounded">正在读取本地目录</div>
      )}
      {desktopLifecyclePaths && (
        <div className="space-y-3">
          <div className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3">
            <div className="text-xs font-medium text-gray-500">数据根目录</div>
            <div className="mt-1 break-all text-xs text-gray-700">{desktopLifecyclePaths.runtime_root_dir}</div>
            <div className="mt-2 space-y-1 text-xs text-gray-500">
              <div>数据库、缓存、日志、诊断、插件状态和会话记录都会保存在这个目录下。</div>
              <div>默认工作目录会自动使用该目录下的 workspace 子目录，仍可在单次会话中按需覆盖。</div>
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => void onChooseRuntimeRoot()}
                disabled={desktopLifecycleActionState === "migrating"}
                className="bg-white hover:bg-gray-100 border border-gray-200 text-gray-700 text-xs px-3 py-1.5 rounded-lg transition-all active:scale-[0.97] disabled:opacity-50"
              >
                选择目录
              </button>
              <button
                type="button"
                onClick={() => void onOpenDesktopPath(desktopLifecyclePaths.runtime_root_dir)}
                disabled={desktopLifecycleActionState === "opening"}
                className="bg-white hover:bg-gray-100 border border-gray-200 text-gray-700 text-xs px-3 py-1.5 rounded-lg transition-all active:scale-[0.97] disabled:opacity-50"
              >
                打开目录
              </button>
            </div>
          </div>
          {hasPendingRuntimeRootSelection && (
            <div className="rounded-lg border border-amber-100 bg-amber-50 px-3 py-3 space-y-2">
              <div className="text-xs font-medium text-amber-700">准备迁移到新的数据根目录</div>
              <div className="break-all text-xs text-amber-700">{pendingRuntimeRootSelection}</div>
              <div className="text-xs text-amber-700">
                应用将在重启时自动迁移数据库、缓存、日志、诊断、插件状态和会话记录。迁移失败时会自动回退到旧目录。
              </div>
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() => void onScheduleRuntimeRootMigration()}
                  disabled={desktopLifecycleActionState === "migrating"}
                  className="bg-amber-600 hover:bg-amber-700 text-white text-xs px-3 py-1.5 rounded-lg transition-all active:scale-[0.97] disabled:opacity-50"
                >
                  {desktopLifecycleActionState === "migrating" ? "准备中..." : "迁移并重启"}
                </button>
                <button
                  type="button"
                  onClick={onCancelRuntimeRootMigration}
                  disabled={desktopLifecycleActionState === "migrating"}
                  className="bg-white hover:bg-amber-100 border border-amber-200 text-amber-700 text-xs px-3 py-1.5 rounded-lg transition-all active:scale-[0.97] disabled:opacity-50"
                >
                  取消
                </button>
              </div>
            </div>
          )}
          {runtimeMigrationState && !hasPendingRuntimeRootSelection && (
            <div
              className={[
                "rounded-lg border px-3 py-3 space-y-1",
                runtimeMigrationState.tone === "green"
                  ? "border-green-100 bg-green-50"
                  : runtimeMigrationState.tone === "red"
                    ? "border-red-100 bg-red-50"
                    : "border-amber-100 bg-amber-50",
              ].join(" ")}
            >
              <div
                className={[
                  "text-xs font-medium",
                  runtimeMigrationState.tone === "green"
                    ? "text-green-700"
                    : runtimeMigrationState.tone === "red"
                      ? "text-red-700"
                      : "text-amber-700",
                ].join(" ")}
              >
                {runtimeMigrationState.title}
              </div>
              <div
                className={[
                  "break-all text-xs",
                  runtimeMigrationState.tone === "green"
                    ? "text-green-700"
                    : runtimeMigrationState.tone === "red"
                      ? "text-red-700"
                      : "text-amber-700",
                ].join(" ")}
              >
                {runtimeMigrationState.message}
              </div>
            </div>
          )}
          {desktopDiagnosticsStatus && (
            <div className="rounded-lg border border-blue-100 bg-blue-50 px-3 py-3 space-y-2">
              <div className="text-xs font-medium text-blue-700">诊断状态</div>
              <div className="text-xs text-blue-700 break-all">
                当前运行 ID：{desktopDiagnosticsStatus.current_run_id}
              </div>
              {desktopDiagnosticsStatus.abnormal_previous_run && (
                <div className="text-xs text-amber-700">检测到上次运行可能异常退出</div>
              )}
              {desktopDiagnosticsStatus.last_clean_exit_at && (
                <div className="text-xs text-blue-700">上次正常退出：{desktopDiagnosticsStatus.last_clean_exit_at}</div>
              )}
              {desktopDiagnosticsStatus.latest_crash && (
                <div className="text-xs text-red-700 break-all">
                  最近崩溃：{desktopDiagnosticsStatus.latest_crash.timestamp} {desktopDiagnosticsStatus.latest_crash.message}
                </div>
              )}
            </div>
          )}
        </div>
      )}
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => void onClearDesktopCacheAndLogs()}
          disabled={desktopLifecycleActionState === "clearing"}
          className="flex-1 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
        >
          {desktopLifecycleActionState === "clearing" ? "清理中..." : "清理缓存与日志"}
        </button>
        <button
          type="button"
          onClick={() => void onExportDesktopEnvironmentSummary()}
          disabled={desktopLifecycleActionState === "exporting"}
          className="flex-1 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
        >
          {desktopLifecycleActionState === "exporting" ? "导出中..." : "导出环境摘要"}
        </button>
        <button
          type="button"
          onClick={() => void onExportDesktopDiagnosticsBundle()}
          disabled={desktopLifecycleActionState === "exporting"}
          className="flex-1 bg-blue-50 hover:bg-blue-100 disabled:opacity-50 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97] text-blue-700"
        >
          {desktopLifecycleActionState === "exporting" ? "导出中..." : "导出诊断包"}
        </button>
      </div>
      <div className="rounded-lg border border-amber-100 bg-amber-50 px-3 py-3 text-xs text-amber-700 space-y-1">
        <div>卸载程序不会删除你的数据根目录。</div>
        <div>如需彻底清理，请先清理缓存与日志，再手动删除数据根目录。</div>
      </div>
      {desktopLifecycleError && (
        <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">{desktopLifecycleError}</div>
      )}
      {desktopLifecycleMessage && (
        <div className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">{desktopLifecycleMessage}</div>
      )}
    </div>
  );
}
