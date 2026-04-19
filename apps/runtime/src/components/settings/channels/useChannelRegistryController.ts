import { useEffect, useMemo, useState } from "react";
import type {
  ImChannelHostActionRecord,
  ImChannelRestoreEntry,
  ImChannelRegistryEntry,
  OpenClawPluginFeishuRuntimeStatus,
  WecomConnectorStatus,
  WecomGatewaySettings,
} from "../../../types";
import type { SettingsTabName } from "../SettingsTabNav";
import {
  buildConnectorStatusDisplay,
  describeFeishuReplyCompletionHint,
  resolveFeishuReplyCompletionShortcutTargets,
  normalizeWecomGatewaySettings,
  summarizeRegistryIssue,
} from "./channelRegistryHelpers";
import {
  extractFeishuRegistryEntry,
  extractFeishuRuntimeStatusFromEntry,
  extractWecomRegistryEntry,
  extractWecomRuntimeStatusFromEntry,
  loadImChannelRegistry,
  saveWecomGatewaySettings,
  setImChannelHostRunning,
  startWecomConnector,
} from "./channelRegistryService";

interface UseChannelRegistryControllerOptions {
  activeTab: SettingsTabName;
}

function mapFeishuReplyCompletionStateLabel(
  state: string | null | undefined,
  phase?: string | null | undefined,
) {
  const normalizedPhase = String(phase || "").trim().toLowerCase();
  switch (state) {
    case "running":
      if (
        normalizedPhase === "ask_user_answered" ||
        normalizedPhase === "approval_resolved" ||
        normalizedPhase === "resumed"
      ) {
        return "已恢复处理中";
      }
      return "处理中";
    case "waiting_for_idle":
      return "等待空闲";
    case "idle_reached":
      return "已到空闲点";
    case "awaiting_user":
      return "等待用户";
    case "awaiting_approval":
      return "等待审批";
    case "interrupted":
      return "已中断";
    case "completed":
      return "已完成";
    case "failed":
      return "失败";
    case "stopped":
      return "已停止";
    default:
      return "暂无回复记录";
  }
}

function describeFeishuReplyCompletion(status: OpenClawPluginFeishuRuntimeStatus | null) {
  const completion = status?.latest_reply_completion;
  if (!completion) {
    return {
      stateLabel: "暂无回复记录",
      detailLabel: "飞书宿主最近还没有可投影的回复完成状态。",
      updatedAtLabel: "暂无更新时间",
    };
  }

  const phaseLabel = String(completion.phase || "").trim() || "unknown";
  const logicalReplyId = String(completion.logicalReplyId || "").trim() || "未识别";
  return {
    stateLabel: mapFeishuReplyCompletionStateLabel(completion.state, completion.phase),
    detailLabel: `reply=${logicalReplyId} · phase=${phaseLabel}`,
    updatedAtLabel: completion.updatedAt || "暂无更新时间",
  };
}

export function useChannelRegistryController({
  activeTab,
}: UseChannelRegistryControllerOptions) {
  const [entries, setEntries] = useState<ImChannelRegistryEntry[]>([]);
  const [wecomSettings, setWecomSettings] = useState<WecomGatewaySettings>(
    normalizeWecomGatewaySettings(null),
  );
  const [loading, setLoading] = useState(false);
  const [savingWecom, setSavingWecom] = useState(false);
  const [retryingWecom, setRetryingWecom] = useState(false);
  const [channelActionPending, setChannelActionPending] = useState<string | null>(null);
  const [error, setError] = useState("");

  async function loadRegistry() {
    setLoading(true);
    try {
      const nextEntries = await loadImChannelRegistry().catch(() => []);
      const normalizedEntries = Array.isArray(nextEntries) ? nextEntries : [];
      setEntries(normalizedEntries);
      const nextWecomEntry =
        normalizedEntries.find((entry) => entry.channel === "wecom") || null;
      setWecomSettings(
        normalizeWecomGatewaySettings(
          (nextWecomEntry?.connector_settings as WecomGatewaySettings | null | undefined) ?? null,
        ),
      );
      setError("");
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : String(nextError));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    if (activeTab !== "feishu") {
      return;
    }
    void loadRegistry();
  }, [activeTab]);

  useEffect(() => {
    if (activeTab !== "feishu") {
      return;
    }
    const timer = window.setInterval(() => {
      void loadRegistry();
    }, 5000);
    return () => window.clearInterval(timer);
  }, [activeTab]);

  const wecomEntry = extractWecomRegistryEntry(entries);
  const feishuEntry = extractFeishuRegistryEntry(entries);
  const feishuRuntimeStatus = extractFeishuRuntimeStatusFromEntry(feishuEntry);
  const wecomRuntimeStatus = extractWecomRuntimeStatusFromEntry(wecomEntry);
  const feishuRestoreEntry =
    (feishuEntry?.automation_status as ImChannelRestoreEntry | null | undefined) ?? null;
  const wecomRestoreEntry =
    (wecomEntry?.automation_status as ImChannelRestoreEntry | null | undefined) ?? null;
  const feishuRecentAction =
    (feishuEntry?.recent_action as ImChannelHostActionRecord | null | undefined) ?? null;
  const wecomRecentAction =
    (wecomEntry?.recent_action as ImChannelHostActionRecord | null | undefined) ?? null;
  const feishuReplyCompletion = describeFeishuReplyCompletion(feishuRuntimeStatus);

  async function handleSaveWecomSettings() {
    setSavingWecom(true);
    try {
      await saveWecomGatewaySettings(wecomSettings);
      setError("");
      await loadRegistry();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : String(nextError));
    } finally {
      setSavingWecom(false);
    }
  }

  async function handleRetryWecomConnector() {
    setRetryingWecom(true);
    try {
      await startWecomConnector(wecomSettings);
      setError("");
      await loadRegistry();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : String(nextError));
    } finally {
      setRetryingWecom(false);
    }
  }

  async function handleSetChannelRunning(channel: string, desiredRunning: boolean) {
    setChannelActionPending(channel);
    try {
      await setImChannelHostRunning(channel, desiredRunning);
      setError("");
      await loadRegistry();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : String(nextError));
    } finally {
      setChannelActionPending((current) => (current === channel ? null : current));
    }
  }

  return {
    entries: useMemo(() => entries, [entries]),
    loading,
    error,
    feishuHostPanel: {
      visible: Boolean(feishuEntry),
      summary: feishuEntry?.summary || "飞书宿主状态暂不可用。",
      statusLabel: feishuEntry?.detail || "未识别",
      pluginVersionLabel: feishuEntry?.plugin_host?.version || "未识别",
      currentAccountLabel:
        feishuRuntimeStatus?.account_id || feishuEntry?.instance_id || "未识别",
      lastEventAtLabel: feishuRuntimeStatus?.last_event_at || "暂无事件",
      recentIssueLabel: summarizeRegistryIssue(feishuEntry?.last_error),
      latestReplyStateLabel: feishuReplyCompletion.stateLabel,
      latestReplyDetailLabel: feishuReplyCompletion.detailLabel,
      latestReplyUpdatedAtLabel: feishuReplyCompletion.updatedAtLabel,
      latestReplyHintLabel: describeFeishuReplyCompletionHint(feishuRuntimeStatus),
      latestReplyShortcutTargets: resolveFeishuReplyCompletionShortcutTargets(feishuRuntimeStatus),
      runtimeLogsLabel:
        feishuRuntimeStatus?.recent_logs?.length
          ? feishuRuntimeStatus.recent_logs.join("\n")
          : "暂无宿主日志",
      automationStatusLabel: feishuRestoreEntry
        ? `${feishuRestoreEntry.restored ? "已执行" : "未执行"} · ${feishuRestoreEntry.detail}`
        : "暂无自动恢复记录",
      recentActionLabel: feishuRecentAction
        ? `${feishuRecentAction.ok ? "成功" : "失败"} · ${feishuRecentAction.detail}`
        : "暂无人工动作记录",
      retrying: loading,
      running: Boolean(feishuRuntimeStatus?.running),
      actionPending: channelActionPending === "feishu",
      actionLabel: feishuRuntimeStatus?.running ? "停止宿主" : "启动宿主",
      onToggleRunning: () => handleSetChannelRunning("feishu", !feishuRuntimeStatus?.running),
    },
    wecomPanel: {
      values: {
        corpId: wecomSettings.corp_id,
        agentId: wecomSettings.agent_id,
        agentSecret: wecomSettings.agent_secret,
      },
      status: buildConnectorStatusDisplay(wecomEntry?.status || "stopped", wecomEntry?.last_error),
      saving: savingWecom,
      retrying: retryingWecom,
      diagnostics: wecomEntry?.diagnostics
        ? [
            {
              label: "连接实例",
              value: wecomEntry.diagnostics.health.instance_id,
            },
            {
              label: "待处理消息",
              value: String(wecomEntry.diagnostics.health.queue_depth),
            },
            {
              label: "后台同步",
              value: wecomEntry?.monitor_status?.running
                ? `${wecomEntry.monitor_status.total_synced} 条`
                : "未启动",
            },
          ]
        : [],
      onChange: (key: string, value: string) =>
        setWecomSettings((current) => ({
          ...current,
          corp_id: key === "corpId" ? value : current.corp_id,
          agent_id: key === "agentId" ? value : current.agent_id,
          agent_secret: key === "agentSecret" ? value : current.agent_secret,
        })),
      onSave: handleSaveWecomSettings,
      onRetry: handleRetryWecomConnector,
      diagnosticsDetail: wecomEntry?.diagnostics || null,
      monitorStatus: wecomEntry?.monitor_status || null,
    },
    wecomHostPanel: {
      visible: Boolean(wecomEntry),
      summary: wecomEntry?.summary || "企业微信宿主状态暂不可用。",
      statusLabel: wecomEntry?.detail || "未识别",
      instanceIdLabel:
        wecomRuntimeStatus?.instance_id ||
        wecomEntry?.instance_id ||
        wecomEntry?.diagnostics?.health.instance_id ||
        "未识别",
      startedAtLabel: wecomRuntimeStatus?.started_at || "尚未启动",
      queueDepthLabel: String(
        wecomRuntimeStatus?.queue_depth ?? wecomEntry?.diagnostics?.health.queue_depth ?? 0,
      ),
      reconnectAttemptsLabel: String(
        wecomRuntimeStatus?.reconnect_attempts ??
          wecomEntry?.diagnostics?.health.reconnect_attempts ??
          0,
      ),
      monitorLabel: wecomEntry?.monitor_status?.running
        ? `运行中 · 已同步 ${wecomEntry.monitor_status.total_synced} 条 · 轮询 ${wecomEntry.monitor_status.interval_ms}ms / ${wecomEntry.monitor_status.limit} 条`
        : "未启动",
      recentIssueLabel: summarizeRegistryIssue(wecomEntry?.last_error),
      automationStatusLabel: wecomRestoreEntry
        ? `${wecomRestoreEntry.restored ? "已执行" : "未执行"} · ${wecomRestoreEntry.detail}`
        : "暂无自动恢复记录",
      recentActionLabel: wecomRecentAction
        ? `${wecomRecentAction.ok ? "成功" : "失败"} · ${wecomRecentAction.detail}`
        : "暂无人工动作记录",
      retrying: loading || retryingWecom,
      running: Boolean(wecomRuntimeStatus?.running),
      actionPending: channelActionPending === "wecom",
      actionLabel: wecomRuntimeStatus?.running ? "停止宿主" : "启动宿主",
      onToggleRunning: () => handleSetChannelRunning("wecom", !wecomRuntimeStatus?.running),
    },
    refresh: loadRegistry,
  };
}
