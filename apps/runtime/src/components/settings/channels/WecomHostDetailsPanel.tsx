interface WecomHostDetailsPanelProps {
  entrySummary: string;
  statusLabel: string;
  instanceIdLabel: string;
  startedAtLabel: string;
  queueDepthLabel: string;
  reconnectAttemptsLabel: string;
  monitorLabel: string;
  recentIssueLabel: string;
  automationStatusLabel: string;
  recentActionLabel: string;
  retrying: boolean;
  running: boolean;
  actionPending: boolean;
  actionLabel: string;
  onRefresh: () => void | Promise<void>;
  onToggleRunning: () => void | Promise<void>;
}

export function WecomHostDetailsPanel({
  entrySummary,
  statusLabel,
  instanceIdLabel,
  startedAtLabel,
  queueDepthLabel,
  reconnectAttemptsLabel,
  monitorLabel,
  recentIssueLabel,
  automationStatusLabel,
  recentActionLabel,
  retrying,
  running,
  actionPending,
  actionLabel,
  onRefresh,
  onToggleRunning,
}: WecomHostDetailsPanelProps) {
  return (
    <details className="rounded-lg border border-gray-200 bg-white p-4" data-testid="wecom-host-details-panel">
      <summary className="cursor-pointer text-sm font-medium text-gray-900">企业微信宿主详情</summary>
      <div className="mt-2 text-xs text-gray-500">
        这里展示企业微信 connector 宿主的运行状态、后台监控与重连信息，方便和飞书一样统一排查渠道接入问题。
      </div>
      <div className="mt-3 rounded-lg border border-emerald-100 bg-emerald-50 px-3 py-3 text-sm text-emerald-900">
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
          className="h-8 px-3 rounded border border-emerald-200 bg-emerald-50 text-xs text-emerald-700 hover:bg-emerald-100 disabled:bg-gray-100 disabled:text-gray-400"
        >
          {actionPending ? "处理中..." : actionLabel}
        </button>
      </div>
      <div className="text-[11px] text-gray-500">{running ? "connector 宿主已运行，后台监控会继续回放和确认消息。" : "connector 宿主当前未运行，可在这里直接启动。"}</div>
      <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
        <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
          <div className="text-[11px] text-gray-500">当前状态</div>
          <div className="text-sm font-medium text-gray-900">{statusLabel}</div>
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
          <div className="text-[11px] text-gray-500">连接实例</div>
          <div className="text-sm font-medium text-gray-900 break-all">{instanceIdLabel}</div>
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
          <div className="text-[11px] text-gray-500">启动时间</div>
          <div className="text-sm font-medium text-gray-900">{startedAtLabel}</div>
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
          <div className="text-[11px] text-gray-500">待处理消息</div>
          <div className="text-sm font-medium text-gray-900">{queueDepthLabel}</div>
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
          <div className="text-[11px] text-gray-500">重连次数</div>
          <div className="text-sm font-medium text-gray-900">{reconnectAttemptsLabel}</div>
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2 md:col-span-2">
          <div className="text-[11px] text-gray-500">后台监控</div>
          <div className="text-sm font-medium text-gray-900">{monitorLabel}</div>
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
    </details>
  );
}
