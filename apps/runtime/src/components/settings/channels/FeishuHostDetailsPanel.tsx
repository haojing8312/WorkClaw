import type { FeishuReplyHintShortcutTarget } from "./channelRegistryHelpers";

interface FeishuHostDetailsPanelProps {
  entrySummary: string;
  statusLabel: string;
  pluginVersionLabel: string;
  currentAccountLabel: string;
  lastEventAtLabel: string;
  recentIssueLabel: string;
  latestReplyStateLabel: string;
  latestReplyDetailLabel: string;
  latestReplyUpdatedAtLabel: string;
  latestReplyHintLabel: string;
  latestReplyShortcutTargets: FeishuReplyHintShortcutTarget[];
  runtimeLogsLabel: string;
  automationStatusLabel: string;
  recentActionLabel: string;
  retrying: boolean;
  running: boolean;
  actionPending: boolean;
  actionLabel: string;
  onOpenEmployees?: () => void | Promise<void>;
  onOpenFeishuAdvancedSettings?: () => void | Promise<void>;
  onRefresh: () => void | Promise<void>;
  onToggleRunning: () => void | Promise<void>;
}

export function FeishuHostDetailsPanel({
  entrySummary,
  statusLabel,
  pluginVersionLabel,
  currentAccountLabel,
  lastEventAtLabel,
  recentIssueLabel,
  latestReplyStateLabel,
  latestReplyDetailLabel,
  latestReplyUpdatedAtLabel,
  latestReplyHintLabel,
  latestReplyShortcutTargets,
  runtimeLogsLabel,
  automationStatusLabel,
  recentActionLabel,
  retrying,
  running,
  actionPending,
  actionLabel,
  onOpenEmployees,
  onOpenFeishuAdvancedSettings,
  onRefresh,
  onToggleRunning,
}: FeishuHostDetailsPanelProps) {
  return (
    <details className="rounded-lg border border-gray-200 bg-white p-4" data-testid="feishu-host-details-panel">
      <summary className="cursor-pointer text-sm font-medium text-gray-900">飞书宿主详情</summary>
      <div className="mt-2 text-xs text-gray-500">
        这里展示飞书 OpenClaw 插件宿主的运行状态、最近一次事件、最近回复状态与宿主日志，方便排查接入问题。
      </div>
      <div className="mt-3 rounded-lg border border-blue-100 bg-blue-50 px-3 py-3 text-sm text-blue-900">
        {entrySummary}
      </div>
      <div className="mt-3 flex flex-wrap gap-2">
        <button
          type="button"
          onClick={() => void onRefresh()}
          disabled={retrying || actionPending}
          className="h-8 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
        >
          {retrying ? "检测中..." : "刷新宿主状态"}
        </button>
        <button
          type="button"
          onClick={() => void onToggleRunning()}
          disabled={retrying || actionPending}
          className="h-8 px-3 rounded border border-blue-200 bg-blue-50 text-xs text-blue-700 hover:bg-blue-100 disabled:bg-gray-100 disabled:text-gray-400"
        >
          {actionPending ? "处理中..." : actionLabel}
        </button>
      </div>
      <div className="text-[11px] text-gray-500">{running ? "宿主已运行，可直接接收飞书事件。" : "宿主当前未运行，可在这里直接启动。"}</div>
      <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
        <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
          <div className="text-[11px] text-gray-500">当前状态</div>
          <div className="text-sm font-medium text-gray-900">{statusLabel}</div>
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
          <div className="text-[11px] text-gray-500">插件版本</div>
          <div className="text-sm font-medium text-gray-900">{pluginVersionLabel}</div>
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
          <div className="text-[11px] text-gray-500">当前接入账号</div>
          <div className="text-sm font-medium text-gray-900">{currentAccountLabel}</div>
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
          <div className="text-[11px] text-gray-500">最近一次事件</div>
          <div className="text-sm font-medium text-gray-900">{lastEventAtLabel}</div>
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2 md:col-span-2 xl:col-span-4">
          <div className="text-[11px] text-gray-500">最近回复状态</div>
          <div className="text-sm font-medium text-gray-900">{latestReplyStateLabel}</div>
          <div className="mt-1 text-xs text-gray-600">{latestReplyDetailLabel}</div>
          <div className="mt-1 text-[11px] text-gray-500">{latestReplyUpdatedAtLabel}</div>
          <div className="mt-2 rounded border border-blue-100 bg-blue-50 px-2 py-2 text-xs text-blue-800">
            {latestReplyHintLabel}
          </div>
          {latestReplyShortcutTargets.length ? (
            <div className="mt-2 flex flex-wrap gap-2">
              {latestReplyShortcutTargets.includes("employees") ? (
                <button
                  type="button"
                  onClick={() => void onOpenEmployees?.()}
                  className="h-7 rounded border border-blue-200 bg-white px-3 text-xs text-blue-700 hover:bg-blue-50"
                >
                  去员工关联入口
                </button>
              ) : null}
              {latestReplyShortcutTargets.includes("advanced") ? (
                <button
                  type="button"
                  onClick={() => void onOpenFeishuAdvancedSettings?.()}
                  className="h-7 rounded border border-blue-200 bg-white px-3 text-xs text-blue-700 hover:bg-blue-50"
                >
                  打开飞书高级配置
                </button>
              ) : null}
            </div>
          ) : null}
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2 md:col-span-2 xl:col-span-4">
          <div className="text-[11px] text-gray-500">最近问题</div>
          <div className="text-sm font-medium text-gray-900">{recentIssueLabel}</div>
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2 md:col-span-2">
          <div className="text-[11px] text-gray-500">最近自动恢复</div>
          <div className="text-sm font-medium text-gray-900">{automationStatusLabel}</div>
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2 md:col-span-2">
          <div className="text-[11px] text-gray-500">最近人工动作</div>
          <div className="text-sm font-medium text-gray-900">{recentActionLabel}</div>
        </div>
      </div>
      <details className="mt-3 rounded-lg border border-gray-100 bg-gray-50 p-3">
        <summary className="cursor-pointer text-xs font-medium text-gray-700">宿主日志（最近 3 条）</summary>
        <div className="mt-2 text-xs text-gray-700 whitespace-pre-wrap break-all">{runtimeLogsLabel}</div>
      </details>
    </details>
  );
}
