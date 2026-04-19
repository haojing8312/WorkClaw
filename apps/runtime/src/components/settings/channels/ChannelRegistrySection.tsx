import { ConnectorConfigPanel } from "../../connectors/ConnectorConfigPanel";
import { ConnectorDiagnosticsPanel } from "../../connectors/ConnectorDiagnosticsPanel";
import { getConnectorSchema } from "../../connectors/connectorSchemas";
import type { ImChannelRegistryEntry } from "../../../types";
import {
  describeFeishuReplyCompletionHint,
  describeFeishuReplyCompletionSummary,
  resolveFeishuReplyCompletionShortcutTargets,
  describeRegistryStatus,
} from "./channelRegistryHelpers";
import { FeishuHostDetailsPanel } from "./FeishuHostDetailsPanel";
import { WecomHostDetailsPanel } from "./WecomHostDetailsPanel";

interface ChannelRegistrySectionProps {
  entries: ImChannelRegistryEntry[];
  loading: boolean;
  error: string;
  wecomPanel: {
    values: Record<string, string>;
    status: {
      dotClass: string;
      label: string;
      detail: string;
      error: string;
    };
    saving: boolean;
    retrying: boolean;
    diagnostics: Array<{ label: string; value: string }>;
    diagnosticsDetail: ImChannelRegistryEntry["diagnostics"];
    monitorStatus: ImChannelRegistryEntry["monitor_status"];
    onChange: (key: string, value: string) => void;
    onSave: () => void | Promise<void>;
    onRetry: () => void | Promise<void>;
  };
  feishuHostPanel: {
    visible: boolean;
    summary: string;
    statusLabel: string;
    pluginVersionLabel: string;
    currentAccountLabel: string;
    lastEventAtLabel: string;
    recentIssueLabel: string;
    latestReplyStateLabel: string;
    latestReplyDetailLabel: string;
    latestReplyUpdatedAtLabel: string;
    latestReplyHintLabel: string;
    latestReplyShortcutTargets: ("employees" | "advanced")[];
    runtimeLogsLabel: string;
    automationStatusLabel: string;
    recentActionLabel: string;
    retrying: boolean;
    running: boolean;
    actionPending: boolean;
    actionLabel: string;
    onToggleRunning: () => void | Promise<void>;
  };
  wecomHostPanel: {
    visible: boolean;
    summary: string;
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
    onToggleRunning: () => void | Promise<void>;
  };
  onOpenEmployees?: () => void | Promise<void>;
  onOpenFeishuAdvancedSettings?: () => void | Promise<void>;
  onRefresh: () => void | Promise<void>;
}

function hostKindLabel(kind: ImChannelRegistryEntry["host_kind"]) {
  return kind === "openclaw_plugin" ? "OpenClaw 插件宿主" : "Connector 宿主";
}

export function ChannelRegistrySection({
  entries,
  loading,
  error,
  wecomPanel,
  feishuHostPanel,
  wecomHostPanel,
  onOpenEmployees,
  onOpenFeishuAdvancedSettings,
  onRefresh,
}: ChannelRegistrySectionProps) {
  return (
    <div className="bg-white rounded-lg p-4 space-y-4" data-testid="channel-registry-section">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div className="space-y-1">
          <div className="text-sm font-medium text-gray-900">渠道宿主总览</div>
          <div className="text-xs text-gray-500">
            这里展示 WorkClaw 当前接入的 IM 渠道宿主形态。飞书走 OpenClaw 官方插件宿主，企业微信走 connector 宿主。
          </div>
        </div>
        <button
          type="button"
          onClick={() => void onRefresh()}
          className="h-8 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50"
        >
          {loading ? "刷新中..." : "刷新总览"}
        </button>
      </div>

      {error ? (
        <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
          {error}
        </div>
      ) : null}

      <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
        {entries.map((entry) => (
          <div
            key={entry.channel}
            className="rounded-lg border border-gray-200 bg-gray-50 p-3 space-y-3"
            data-testid={`channel-registry-card-${entry.channel}`}
          >
            <div className="flex items-start justify-between gap-3">
              <div className="space-y-1">
                <div className="text-sm font-medium text-gray-900">{entry.display_name}</div>
                <div className="text-xs text-gray-500">{entry.summary}</div>
              </div>
              <span className="inline-flex items-center rounded-full bg-white px-2 py-1 text-[11px] text-gray-600 border border-gray-200">
                {hostKindLabel(entry.host_kind)}
              </span>
            </div>

            <div className="flex flex-wrap gap-2">
              <span className="inline-flex items-center rounded-full bg-white px-2 py-1 text-[11px] text-gray-700 border border-gray-200">
                {describeRegistryStatus(entry.status)}
              </span>
              {entry.capabilities.map((capability) => (
                <span
                  key={capability}
                  className="inline-flex items-center rounded-full bg-white px-2 py-1 text-[11px] text-gray-600 border border-gray-200"
                >
                  {capability}
                </span>
              ))}
            </div>

            <div className="text-xs text-gray-600">{entry.detail}</div>
            {entry.channel === "feishu" ? (
              <div className="space-y-2">
                <div className="rounded border border-blue-100 bg-blue-50 px-2 py-2 text-xs text-blue-800">
                  {describeFeishuReplyCompletionSummary(entry.runtime_status)}
                </div>
                <div className="rounded border border-slate-200 bg-white px-2 py-2 text-xs text-slate-600">
                  {describeFeishuReplyCompletionHint(entry.runtime_status)}
                </div>
                <div className="flex flex-wrap gap-2">
                  {resolveFeishuReplyCompletionShortcutTargets(entry.runtime_status).includes("employees") ? (
                    <button
                      type="button"
                      onClick={() => void onOpenEmployees?.()}
                      className="h-7 rounded border border-blue-200 bg-white px-3 text-xs text-blue-700 hover:bg-blue-50"
                    >
                      去员工关联入口
                    </button>
                  ) : null}
                  {resolveFeishuReplyCompletionShortcutTargets(entry.runtime_status).includes("advanced") ? (
                    <button
                      type="button"
                      onClick={() => void onOpenFeishuAdvancedSettings?.()}
                      className="h-7 rounded border border-blue-200 bg-white px-3 text-xs text-blue-700 hover:bg-blue-50"
                    >
                      打开飞书高级配置
                    </button>
                  ) : null}
                </div>
              </div>
            ) : null}
            {entry.last_error ? (
              <div className="rounded border border-red-100 bg-red-50 px-2 py-2 text-xs text-red-700">
                {entry.last_error}
              </div>
            ) : null}
            {entry.monitor_status?.running ? (
              <div className="rounded border border-emerald-100 bg-emerald-50 px-2 py-2 text-xs text-emerald-800">
                {`后台同步运行中：累计 ${entry.monitor_status.total_synced} 条，轮询 ${entry.monitor_status.interval_ms}ms / ${entry.monitor_status.limit} 条。`}
              </div>
            ) : null}
          </div>
        ))}
      </div>

      <div className="space-y-3">
        {feishuHostPanel.visible ? (
          <FeishuHostDetailsPanel
            entrySummary={feishuHostPanel.summary}
            statusLabel={feishuHostPanel.statusLabel}
            pluginVersionLabel={feishuHostPanel.pluginVersionLabel}
            currentAccountLabel={feishuHostPanel.currentAccountLabel}
            lastEventAtLabel={feishuHostPanel.lastEventAtLabel}
            recentIssueLabel={feishuHostPanel.recentIssueLabel}
            latestReplyStateLabel={feishuHostPanel.latestReplyStateLabel}
            latestReplyDetailLabel={feishuHostPanel.latestReplyDetailLabel}
            latestReplyUpdatedAtLabel={feishuHostPanel.latestReplyUpdatedAtLabel}
            latestReplyHintLabel={feishuHostPanel.latestReplyHintLabel}
            latestReplyShortcutTargets={feishuHostPanel.latestReplyShortcutTargets}
            runtimeLogsLabel={feishuHostPanel.runtimeLogsLabel}
            automationStatusLabel={feishuHostPanel.automationStatusLabel}
            recentActionLabel={feishuHostPanel.recentActionLabel}
            retrying={feishuHostPanel.retrying}
            running={feishuHostPanel.running}
            actionPending={feishuHostPanel.actionPending}
            actionLabel={feishuHostPanel.actionLabel}
            onOpenEmployees={onOpenEmployees}
            onOpenFeishuAdvancedSettings={onOpenFeishuAdvancedSettings}
            onRefresh={onRefresh}
            onToggleRunning={feishuHostPanel.onToggleRunning}
          />
        ) : null}
        {wecomHostPanel.visible ? (
          <WecomHostDetailsPanel
            entrySummary={wecomHostPanel.summary}
            statusLabel={wecomHostPanel.statusLabel}
            instanceIdLabel={wecomHostPanel.instanceIdLabel}
            startedAtLabel={wecomHostPanel.startedAtLabel}
            queueDepthLabel={wecomHostPanel.queueDepthLabel}
            reconnectAttemptsLabel={wecomHostPanel.reconnectAttemptsLabel}
            monitorLabel={wecomHostPanel.monitorLabel}
            recentIssueLabel={wecomHostPanel.recentIssueLabel}
            automationStatusLabel={wecomHostPanel.automationStatusLabel}
            recentActionLabel={wecomHostPanel.recentActionLabel}
            retrying={wecomHostPanel.retrying}
            running={wecomHostPanel.running}
            actionPending={wecomHostPanel.actionPending}
            actionLabel={wecomHostPanel.actionLabel}
            onRefresh={onRefresh}
            onToggleRunning={wecomHostPanel.onToggleRunning}
          />
        ) : null}
        <ConnectorConfigPanel
          schema={getConnectorSchema("wecom")}
          status={wecomPanel.status}
          values={wecomPanel.values}
          saving={wecomPanel.saving}
          retrying={wecomPanel.retrying}
          diagnostics={wecomPanel.diagnostics}
          onChange={wecomPanel.onChange}
          onSave={wecomPanel.onSave}
          onRetry={wecomPanel.onRetry}
        />
        {wecomPanel.diagnosticsDetail ? (
          <ConnectorDiagnosticsPanel
            title="企业微信连接器诊断"
            diagnostics={wecomPanel.diagnosticsDetail}
          />
        ) : null}
      </div>
    </div>
  );
}
