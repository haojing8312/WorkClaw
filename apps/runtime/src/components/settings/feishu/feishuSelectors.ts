import type {
  OpenClawPluginFeishuRuntimeStatus,
} from "../../../types";
import {
  buildFeishuOnboardingState,
  formatFeishuOnboardingStepLabel,
  type FeishuOnboardingInput,
  type FeishuOnboardingPanelDisplay,
  type FeishuOnboardingState,
  type FeishuOnboardingStep,
  resolveFeishuAuthorizationInlineError,
  resolveFeishuGuidedInlineError,
  resolveFeishuGuidedInlineNotice,
  resolveFeishuOnboardingPanelDisplay,
} from "./feishuOnboardingHelpers";
import {
  extractFeishuInstallerQrBlock,
  getLatestInstallerOutputLine,
  resolveFeishuInstallerCompletionNotice,
  resolveFeishuInstallerFlowLabel,
  sanitizeFeishuInstallerDisplayLines,
  shouldShowFeishuInstallerGuidedPanel,
} from "./feishuInstallerHelpers";

export {
  buildFeishuOnboardingState,
  formatFeishuOnboardingStepLabel,
  resolveFeishuAuthorizationInlineError,
  resolveFeishuGuidedInlineError,
  resolveFeishuGuidedInlineNotice,
  resolveFeishuOnboardingPanelDisplay,
} from "./feishuOnboardingHelpers";

export type {
  FeishuOnboardingInput,
  FeishuOnboardingPanelDisplay,
  FeishuOnboardingState,
  FeishuOnboardingStep,
} from "./feishuOnboardingHelpers";

export {
  extractFeishuInstallerQrBlock,
  getLatestInstallerOutputLine,
  resolveFeishuInstallerCompletionNotice,
  resolveFeishuInstallerFlowLabel,
  sanitizeFeishuInstallerDisplayLines,
  shouldShowFeishuInstallerGuidedPanel,
} from "./feishuInstallerHelpers";

interface FeishuSetupSummary {
  title: string;
  description: string;
  primaryActionLabel: string;
}

interface FeishuAuthorizationAction {
  label: string;
  busyLabel: string;
}

interface FeishuRoutingStatus {
  label: string;
  description: string;
  actionLabel: string;
}

interface FeishuConnectorStatus {
  dotClass: string;
  stateLabel: string;
  label: string;
  detail: string;
  error: string;
}

interface FeishuSetupSummaryInput {
  skipped: boolean;
  summaryState: string | null | undefined;
  runtimeRunning: boolean;
  authApproved: boolean;
  runtimeLastError: string | null | undefined;
  officialRuntimeLastError: string | null | undefined;
}

interface FeishuConnectorStatusInput {
  running: boolean;
  lastError: string | null | undefined;
  hasInstalledOfficialFeishuPlugin: boolean;
}

interface FeishuConnectionDetailSummaryInput {
  connectorStatus: FeishuConnectorStatus;
  runtimeRunning: boolean;
  authApproved: boolean;
  pendingPairings: number | null | undefined;
  defaultRoutingEmployeeName: string | null | undefined;
  scopedRoutingCount: number | null | undefined;
}

interface FeishuDiagnosticSummaryInput {
  connectorStatus: FeishuConnectorStatus;
  pluginVersion: string | null | undefined;
  defaultAccountId: string | null | undefined;
  authApproved: boolean;
  defaultRoutingEmployeeName: string | null | undefined;
  scopedRoutingCount: number | null | undefined;
  lastEventAtLabel: string;
  connectionDetailSummary: string;
  recentLogsSummary: string;
}


export function summarizeConnectorIssue(rawIssue: string | null | undefined) {
  const normalized = String(rawIssue || "").trim().toLowerCase();
  if (!normalized) {
    return "无";
  }
  if (normalized.includes("signature mismatch")) {
    return "签名校验失败";
  }
  if (normalized.includes("secret") || normalized.includes("credential") || normalized.includes("token")) {
    return "凭据配置异常";
  }
  if (normalized.includes("timeout")) {
    return "连接超时";
  }
  if (normalized.includes("refused") || normalized.includes("unreachable") || normalized.includes("network")) {
    return "连接不可用";
  }
  return "连接异常";
}

export function resolveFeishuConnectorStatus(input: FeishuConnectorStatusInput): FeishuConnectorStatus {
  if (input.running) {
    return {
      dotClass: "bg-emerald-500",
      stateLabel: "运行中",
      label: "飞书官方插件运行中",
      detail: "官方插件宿主已启动，正在按 OpenClaw 兼容模式接收飞书消息。",
      error: input.lastError ?? "",
    };
  }
  return {
    dotClass: "bg-gray-300",
    stateLabel: "未启动",
    label: "未启动",
    detail: input.hasInstalledOfficialFeishuPlugin
      ? "官方插件宿主尚未启动。保存配置或刷新插件状态后会尝试拉起运行时。"
      : "当前飞书只支持官方插件主路径。请先安装或绑定飞书官方插件，再启动运行时。",
    error: input.lastError ?? "",
  };
}

export function summarizeOfficialFeishuRuntimeLogs(
  status: OpenClawPluginFeishuRuntimeStatus | null | undefined,
) {
  const logs = Array.isArray(status?.recent_logs)
    ? status?.recent_logs.filter((entry) => String(entry ?? "").trim())
    : [];
  if (!logs || logs.length === 0) {
    return "暂无";
  }
  return logs.slice(-3).join(" | ");
}

export function formatCompactDateTime(value: string | null | undefined) {
  const normalized = String(value || "").trim();
  if (!normalized) return "未知时间";
  const date = new Date(normalized);
  if (Number.isNaN(date.getTime())) {
    return normalized;
  }
  const year = date.getUTCFullYear();
  const month = String(date.getUTCMonth() + 1).padStart(2, "0");
  const day = String(date.getUTCDate()).padStart(2, "0");
  const hours = String(date.getUTCHours()).padStart(2, "0");
  const minutes = String(date.getUTCMinutes()).padStart(2, "0");
  return `${year}-${month}-${day} ${hours}:${minutes}`;
}

interface FeishuDiagnosticsClipboardInput {
  connectorStatus: FeishuConnectorStatusInput;
  pluginVersion: string | null | undefined;
  defaultAccountId: string | null | undefined;
  authApproved: boolean;
  pendingPairings: number | null | undefined;
  defaultRoutingEmployeeName: string | null | undefined;
  scopedRoutingCount: number | null | undefined;
  lastEventAt: string | null | undefined;
  runtimeStatus: OpenClawPluginFeishuRuntimeStatus | null | undefined;
  pluginChannelHosts: number;
  pluginInstalled: boolean;
}

export function buildFeishuDiagnosticsClipboardText(input: FeishuDiagnosticsClipboardInput) {
  const connectorStatus = resolveFeishuConnectorStatus(input.connectorStatus);
  return buildFeishuDiagnosticSummary({
    connectorStatus,
    pluginVersion: input.pluginVersion || "未识别",
    defaultAccountId: input.defaultAccountId || "未识别",
    authApproved: input.authApproved,
    defaultRoutingEmployeeName: input.defaultRoutingEmployeeName || "未设置",
    scopedRoutingCount: input.scopedRoutingCount ?? 0,
    lastEventAtLabel: formatCompactDateTime(input.lastEventAt),
    connectionDetailSummary: getFeishuConnectionDetailSummary({
      connectorStatus,
      runtimeRunning: input.connectorStatus.running,
      authApproved: input.authApproved,
      pendingPairings: input.pendingPairings,
      defaultRoutingEmployeeName: input.defaultRoutingEmployeeName,
      scopedRoutingCount: input.scopedRoutingCount,
    }),
    recentLogsSummary: summarizeOfficialFeishuRuntimeLogs(input.runtimeStatus),
  });
}

export function getFeishuConnectionDetailSummary(input: FeishuConnectionDetailSummaryInput) {
  if (input.connectorStatus.error.trim()) {
    return summarizeConnectorIssue(input.connectorStatus.error);
  }
  if (!input.authApproved) {
    return "连接已启动，但还需要在飞书里完成授权。";
  }
  if ((input.pendingPairings ?? 0) > 0) {
    return "连接正常，但有新的接入请求等待批准。";
  }
  if (!input.defaultRoutingEmployeeName && (input.scopedRoutingCount ?? 0) === 0) {
    return "连接正常，但还没有设置默认接待员工或群聊范围。";
  }
  if (input.runtimeRunning) {
    return "连接正常，正在接收飞书消息。";
  }
  return "当前连接尚未启动。";
}

export function buildFeishuDiagnosticSummary(input: FeishuDiagnosticSummaryInput) {
  const lines = [
    `当前状态: ${input.connectorStatus.label}`,
    `插件版本: ${input.pluginVersion || "未识别"}`,
    `当前接入账号: ${input.defaultAccountId || "未识别"}`,
    `授权状态: ${input.authApproved ? "已完成" : "待完成"}`,
    `默认接待员工: ${input.defaultRoutingEmployeeName || "未设置"}`,
    `群聊范围规则: ${input.scopedRoutingCount ?? 0} 条`,
    `最近一次事件: ${input.lastEventAtLabel}`,
    `诊断摘要: ${input.connectionDetailSummary}`,
    `最近日志: ${input.recentLogsSummary}`,
  ];
  return lines.join("\n");
}


export function getFeishuSetupSummary(input: FeishuSetupSummaryInput): FeishuSetupSummary {
  if (input.skipped) {
    return {
      title: "已跳过飞书引导，可以继续使用其他功能",
      description: "飞书接入暂时不会阻塞设置窗口里的其他入口，后续仍可随时回来继续完成。",
      primaryActionLabel: "继续使用",
    };
  }
  if (input.summaryState === "env_missing") {
    return {
      title: "这台电脑还没有准备好飞书连接环境",
      description: "请先安装或升级到 Node.js 22 LTS，完成后回到这里重新检测环境。",
      primaryActionLabel: "重新检测环境",
    };
  }
  if (input.summaryState === "plugin_not_installed") {
    return {
      title: "先安装飞书官方插件，再继续机器人接入",
      description: "当前电脑还没有安装飞书官方插件。安装完成后，才能继续新建机器人或绑定已有机器人。",
      primaryActionLabel: "安装官方插件",
    };
  }
  if (input.summaryState === "plugin_starting") {
    return {
      title: input.authApproved ? "正在恢复飞书连接" : "机器人信息已准备好，下一步启动飞书连接组件",
      description: input.authApproved
        ? "WorkClaw 会自动尝试恢复上次已接通的飞书连接；如果恢复失败，再手动点击“启动连接”。"
        : "可以继续安装并启动官方插件，然后回到飞书完成授权。",
      primaryActionLabel: input.authApproved ? "重新启动连接" : "安装并启动",
    };
  }
  if (input.summaryState === "awaiting_auth") {
    return {
      title: "飞书连接已启动，还需要完成飞书授权",
      description: "请回到飞书中的机器人会话完成授权，然后回到这里刷新状态。",
      primaryActionLabel: "刷新授权状态",
    };
  }
  if (input.summaryState === "awaiting_pairing_approval") {
    return {
      title: "飞书里已有新的接入请求，WorkClaw 还需要你批准",
      description: "机器人已经返回 pairing code。请在这里批准这次接入请求，批准后它才能真正开始收发消息。",
      primaryActionLabel: "批准这次接入",
    };
  }
  if (input.summaryState === "ready_for_routing") {
    return {
      title: "飞书已接通，还需要设置谁来接待消息",
      description: "接待员工或群聊范围未完全配置，完成后才能稳定接待飞书消息。",
      primaryActionLabel: "查看接待设置",
    };
  }
  if (input.summaryState === "runtime_error") {
    return {
      title: "飞书连接出现异常，需要重新检查",
      description: summarizeConnectorIssue(input.runtimeLastError || input.officialRuntimeLastError),
      primaryActionLabel: "重新检测",
    };
  }
  if (input.summaryState === "ready_to_bind") {
    return {
      title: "官方插件已准备好，下一步请选择接入方式",
      description: "你可以直接新建机器人，也可以切换到绑定已有机器人后继续完成接入。",
      primaryActionLabel: "继续设置",
    };
  }
  if (input.runtimeRunning) {
    return {
      title: "飞书已接通，正在接收消息",
      description: "后续可以继续调整接待员工、群聊规则和高级设置。",
      primaryActionLabel: "查看连接详情",
    };
  }
  return {
    title: "飞书还未接入，完成下方步骤后即可开始接待消息",
    description: "请先安装官方插件，再创建机器人或绑定已有机器人，最后完成授权和接待设置。",
    primaryActionLabel: "开始设置",
  };
}

export function getFeishuEnvironmentLabel(ready: boolean, fallback: string) {
  return ready ? "已就绪" : fallback;
}

export function getFeishuAuthorizationAction(input: {
  runtimeRunning: boolean;
  pluginInstalled: boolean;
}): FeishuAuthorizationAction {
  if (input.runtimeRunning) {
    return {
      label: "重新启动连接",
      busyLabel: "处理中...",
    };
  }
  if (input.pluginInstalled) {
    return {
      label: "启动连接",
      busyLabel: "处理中...",
    };
  }
  return {
    label: "安装并启动",
    busyLabel: "处理中...",
  };
}

export function getFeishuRoutingStatus(input: {
  authApproved: boolean;
  defaultRoutingEmployeeName: string | null | undefined;
  scopedRoutingCount: number | null | undefined;
}): FeishuRoutingStatus {
  const hasDefaultEmployee = Boolean(input.defaultRoutingEmployeeName?.trim());
  const scopedCount = input.scopedRoutingCount ?? 0;

  if (!input.authApproved) {
    return {
      label: "待完成授权",
      description: "先回到飞书完成授权，接待设置才会生效。",
      actionLabel: "先完成授权",
    };
  }

  if (hasDefaultEmployee) {
    return {
      label: scopedCount > 0 ? "已可接待" : "已可接待",
      description:
        scopedCount > 0
          ? `默认接待员工和 ${scopedCount} 条群聊范围规则都已生效。`
          : "默认接待员工已设置，未命中规则的消息会自动回退给该员工。",
      actionLabel: scopedCount > 0 ? "调整接待设置" : "查看接待员工",
    };
  }

  if (scopedCount > 0) {
    return {
      label: "部分可接待",
      description: `已配置 ${scopedCount} 条群聊范围规则，但还没有默认接待员工。未命中规则的消息暂时无人接待。`,
      actionLabel: "补充默认员工",
    };
  }

  return {
    label: "还差默认员工",
    description: "飞书已接通，但还没有默认接待员工或群聊范围规则，消息暂时不会稳定分配。",
    actionLabel: "去设置接待员工",
  };
}
