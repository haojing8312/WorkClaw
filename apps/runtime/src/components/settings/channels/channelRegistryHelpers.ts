import type {
  ChannelConnectorDescriptor,
  ChannelConnectorDiagnostics,
  ChannelConnectorMonitorStatus,
  ImChannelRegistryEntry,
  ImChannelRegistryStatus,
  OpenClawPluginChannelHost,
  OpenClawPluginFeishuRuntimeStatus,
  WecomConnectorStatus,
  WecomGatewaySettings,
} from "../../../types";

export type FeishuReplyHintShortcutTarget = "employees" | "advanced";

export function normalizeWecomGatewaySettings(
  settings: WecomGatewaySettings | null | undefined,
): WecomGatewaySettings {
  return {
    corp_id: settings?.corp_id || "",
    agent_id: settings?.agent_id || "",
    agent_secret: settings?.agent_secret || "",
    sidecar_base_url: settings?.sidecar_base_url || "",
  };
}

export function hasWecomCredentials(settings: WecomGatewaySettings | null | undefined) {
  return Boolean(
    settings?.corp_id?.trim() &&
      settings?.agent_id?.trim() &&
      settings?.agent_secret?.trim(),
  );
}

export function buildFeishuRegistryEntry(
  host: OpenClawPluginChannelHost | null,
  runtimeStatus: OpenClawPluginFeishuRuntimeStatus | null,
): ImChannelRegistryEntry {
  const baseCapabilities = host?.capabilities || [];
  const status: ImChannelRegistryStatus = runtimeStatus?.running
    ? "running"
    : host?.status === "ready"
      ? "ready"
      : host?.error || runtimeStatus?.last_error
        ? "degraded"
        : "stopped";

  const detailParts = [
    host?.version ? `插件版本 ${host.version}` : "未识别插件版本",
    runtimeStatus?.account_id ? `账号 ${runtimeStatus.account_id}` : null,
    runtimeStatus?.started_at ? `已启动` : "运行时未启动",
  ].filter(Boolean);

  return {
    channel: "feishu",
    display_name: host?.display_name || "飞书",
    host_kind: "openclaw_plugin",
    status,
    summary: runtimeStatus?.running
      ? "通过 OpenClaw 官方飞书插件接收与回复消息。"
      : "飞书渠道由 OpenClaw 官方插件宿主提供，WorkClaw 只负责路由、会话与回复生命周期。",
    detail: detailParts.join(" · "),
    capabilities: baseCapabilities,
    instance_id: runtimeStatus?.account_id || null,
    last_error: runtimeStatus?.last_error || host?.error || null,
    plugin_host: host,
    runtime_status: runtimeStatus,
    diagnostics: null,
    monitor_status: null,
  };
}

export function buildWecomRegistryEntry(input: {
  descriptor: ChannelConnectorDescriptor | null;
  settings: WecomGatewaySettings;
  connectorStatus: WecomConnectorStatus | null;
  diagnostics: ChannelConnectorDiagnostics | null;
  monitorStatus: ChannelConnectorMonitorStatus | null;
}): ImChannelRegistryEntry {
  const configured = hasWecomCredentials(input.settings);
  const status: ImChannelRegistryStatus = !configured
    ? "not_configured"
    : input.connectorStatus?.running
      ? "running"
      : input.connectorStatus?.last_error || input.monitorStatus?.last_error
        ? "degraded"
        : input.connectorStatus?.state === "ready"
          ? "ready"
          : "stopped";
  const detailParts = [
    configured ? "凭据已配置" : "未配置凭据",
    input.connectorStatus?.instance_id || null,
    input.monitorStatus?.running
      ? `后台同步 ${input.monitorStatus.total_synced} 条`
      : null,
  ].filter(Boolean);

  return {
    channel: "wecom",
    display_name: input.descriptor?.display_name || "企业微信",
    host_kind: "connector",
    status,
    summary: configured
      ? "通过 sidecar channel connector 接入企业微信，再由 WorkClaw 统一路由与回复。"
      : "企业微信渠道将复用与 OpenClaw 兼容的 connector host 形态接入。",
    detail: detailParts.join(" · ") || "等待配置连接器",
    capabilities: input.descriptor?.capabilities || [],
    instance_id: input.connectorStatus?.instance_id || input.diagnostics?.health.instance_id || null,
    last_error:
      input.connectorStatus?.last_error ||
      input.monitorStatus?.last_error ||
      input.diagnostics?.health.last_error ||
      null,
    plugin_host: null,
    runtime_status: input.connectorStatus,
    diagnostics: input.diagnostics,
    monitor_status: input.monitorStatus,
  };
}

export function buildConnectorStatusDisplay(status: ImChannelRegistryStatus, error?: string | null) {
  switch (status) {
    case "running":
      return {
        dotClass: "bg-emerald-500",
        label: "运行中",
        detail: "连接器已启动，正在接收并同步消息。",
        error: error || "",
      };
    case "ready":
      return {
        dotClass: "bg-blue-500",
        label: "已就绪",
        detail: "配置已存在，等待下一次启动或自动恢复。",
        error: error || "",
      };
    case "degraded":
      return {
        dotClass: "bg-amber-500",
        label: "异常降级",
        detail: "连接器存在错误，需要重试或检查配置。",
        error: error || "",
      };
    case "not_configured":
      return {
        dotClass: "bg-gray-300",
        label: "待配置",
        detail: "先填写连接器凭据，再启动后台监听。",
        error: error || "",
      };
    default:
      return {
        dotClass: "bg-gray-400",
        label: "未启动",
        detail: "连接器当前未运行。",
        error: error || "",
      };
  }
}

export function describeRegistryStatus(status: ImChannelRegistryStatus) {
  switch (status) {
    case "running":
      return "运行中";
    case "ready":
      return "已就绪";
    case "degraded":
      return "异常降级";
    case "not_configured":
      return "待配置";
    default:
      return "未启动";
  }
}

export function summarizeRegistryIssue(error?: string | null) {
  const normalized = error?.trim();
  return normalized ? normalized : "暂无明显异常";
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

export function describeFeishuReplyCompletionSummary(
  status: OpenClawPluginFeishuRuntimeStatus | null | undefined,
) {
  const completion = status?.latest_reply_completion;
  if (!completion) {
    return "最近回复：暂无记录";
  }

  const logicalReplyId = String(completion.logicalReplyId || "").trim();
  const detail = logicalReplyId ? ` · ${logicalReplyId}` : "";
  return `最近回复：${mapFeishuReplyCompletionStateLabel(completion.state, completion.phase)}${detail}`;
}

export function describeFeishuReplyCompletionHint(
  status: OpenClawPluginFeishuRuntimeStatus | null | undefined,
) {
  const state = status?.latest_reply_completion?.state;
  const phase = String(status?.latest_reply_completion?.phase || "").trim().toLowerCase();
  switch (state) {
    case "awaiting_user":
      return "下一步建议：去飞书线程里补充用户输入，补充后可点击“刷新宿主状态”；如果不确定谁会接待后续消息，可查看“员工关联入口”。";
    case "awaiting_approval":
      return "下一步建议：先完成审批或确认，完成后点击“刷新宿主状态”；如果审批链或连接配置有疑问，可查看“飞书高级配置”。";
    case "failed":
      return status?.running
        ? "下一步建议：先查看“最近问题”和“宿主日志”，必要时点击“刷新宿主状态”；如果持续失败，可回到“飞书高级配置”检查连接。"
        : "下一步建议：先查看“最近问题”和“宿主日志”，必要时点击“启动宿主”；如果持续失败，可回到“飞书高级配置”检查连接。";
    case "stopped":
      return "下一步建议：如果这条回复不该中断，可以点击“启动宿主”；如需确认接待关系，再查看“员工关联入口”。";
    case "interrupted":
      return "下一步建议：先检查是否有人工打断或宿主切换，再决定是否点击“刷新宿主状态”；如涉及接待切换，可查看“员工关联入口”。";
    case "running":
      if (
        phase === "ask_user_answered" ||
        phase === "approval_resolved" ||
        phase === "resumed"
      ) {
        return "下一步建议：宿主已收到继续执行所需的输入或审批结果，当前正在恢复推进这条回复；可先点击“刷新宿主状态”并观察飞书线程是否继续更新。";
      }
      return "下一步建议：宿主仍在推进这条回复，可先点击“刷新宿主状态”并观察飞书线程是否继续更新。";
    case "waiting_for_idle":
    case "idle_reached":
      return "下一步建议：宿主仍在推进这条回复，可先点击“刷新宿主状态”并观察飞书线程是否继续更新。";
    case "completed":
      return "下一步建议：这条回复已结束；如需确认最新状态，可点击“刷新宿主状态”，如需调整后续接待可查看“员工关联入口”。";
    default:
      return status?.running
        ? "下一步建议：宿主尚未形成可投影的回复状态，可点击“刷新宿主状态”；如需确认接待范围，可查看“员工关联入口”。"
        : "下一步建议：先点击“启动宿主”，再观察是否收到新消息；如需确认接待范围，可查看“员工关联入口”。";
  }
}

export function resolveFeishuReplyCompletionShortcutTargets(
  status: OpenClawPluginFeishuRuntimeStatus | null | undefined,
): FeishuReplyHintShortcutTarget[] {
  const state = status?.latest_reply_completion?.state;
  switch (state) {
    case "awaiting_approval":
    case "failed":
      return ["advanced"];
    case "awaiting_user":
    case "stopped":
    case "interrupted":
    case "completed":
      return ["employees"];
    default:
      return ["employees"];
  }
}
