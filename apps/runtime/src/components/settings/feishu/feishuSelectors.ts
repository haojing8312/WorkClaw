import type {
  FeishuSetupProgress,
  OpenClawLarkInstallerMode,
  OpenClawLarkInstallerSessionStatus,
  OpenClawPluginFeishuRuntimeStatus,
} from "../../../types";

export type FeishuOnboardingStep =
  | "environment"
  | "plugin"
  | "existing_robot"
  | "create_robot"
  | "authorize"
  | "approve_pairing"
  | "routing"
  | "skipped";

export interface FeishuOnboardingInput {
  summaryState?: string | null;
  setupProgress?: Partial<FeishuSetupProgress> | null;
  installerMode?: OpenClawLarkInstallerMode | null;
  skipped?: boolean;
}

export interface FeishuOnboardingState {
  currentStep: FeishuOnboardingStep;
  stepOrder: FeishuOnboardingStep[];
  canContinue: boolean;
  skipped: boolean;
  mode: "existing_robot" | "create_robot";
}

interface NormalizedFeishuOnboardingInput {
  summaryState: string | null;
  installerMode: OpenClawLarkInstallerMode | null;
  skipped: boolean;
  runtimeRunning: boolean;
  authApproved: boolean;
  pendingPairings: number;
}

interface FeishuOnboardingStepDisplay {
  title: string;
  body: string;
}

interface FeishuOnboardingPanelDisplay extends FeishuOnboardingStepDisplay {
  badgeLabel: string;
  badgeClassName: string;
  primaryActionLabel: string;
}

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

function normalizeFeishuOnboardingInput(input: FeishuOnboardingInput): NormalizedFeishuOnboardingInput {
  const summaryState = input.summaryState ?? input.setupProgress?.summary_state ?? null;
  return {
    summaryState,
    installerMode: input.installerMode ?? null,
    skipped: input.skipped === true || summaryState === "skipped",
    runtimeRunning: input.setupProgress?.runtime_running === true,
    authApproved: input.setupProgress?.auth_status === "approved",
    pendingPairings: input.setupProgress?.pending_pairings ?? 0,
  };
}

function resolveFeishuOnboardingMode(installerMode: OpenClawLarkInstallerMode | null) {
  return installerMode === "create" ? "create_robot" : "existing_robot";
}

function resolveFeishuOnboardingCurrentStep(input: NormalizedFeishuOnboardingInput): FeishuOnboardingStep {
  if (input.skipped) {
    return "skipped";
  }

  if (input.summaryState === "env_missing") {
    return "environment";
  }
  if (input.summaryState === "ready_to_bind") {
    return "create_robot";
  }
  if (input.summaryState === "plugin_not_installed") {
    return "plugin";
  }
  if (input.summaryState === "plugin_starting") {
    return "authorize";
  }
  if (input.summaryState === "awaiting_auth") {
    return "authorize";
  }
  if (input.summaryState === "awaiting_pairing_approval") {
    return "approve_pairing";
  }
  if (input.summaryState === "ready_for_routing") {
    return "routing";
  }
  if (input.summaryState === "runtime_error") {
    return "environment";
  }

  if (input.runtimeRunning) {
    if (input.pendingPairings > 0) {
      return "approve_pairing";
    }
    return input.authApproved ? "routing" : "authorize";
  }

  return "environment";
}

function canContinueFeishuOnboarding(input: NormalizedFeishuOnboardingInput, currentStep: FeishuOnboardingStep) {
  return input.skipped || currentStep === "routing";
}

function resolveFeishuOnboardingStepOrder(
  input: NormalizedFeishuOnboardingInput,
  mode: "existing_robot" | "create_robot",
) {
  if (input.skipped) {
    return ["skipped"] as FeishuOnboardingStep[];
  }

  return ["environment", "plugin", mode, "authorize", "approve_pairing", "routing"] as FeishuOnboardingStep[];
}

export function buildFeishuOnboardingState(input: FeishuOnboardingInput): FeishuOnboardingState {
  const normalized = normalizeFeishuOnboardingInput(input);
  const mode =
    normalized.summaryState === "ready_to_bind"
      ? "create_robot"
      : resolveFeishuOnboardingMode(normalized.installerMode);
  const currentStep = resolveFeishuOnboardingCurrentStep(normalized);
  return {
    currentStep,
    stepOrder: resolveFeishuOnboardingStepOrder(normalized, mode),
    canContinue: canContinueFeishuOnboarding(normalized, currentStep),
    skipped: normalized.skipped,
    mode,
  };
}

export function formatFeishuOnboardingStepLabel(step: FeishuOnboardingStep) {
  switch (step) {
    case "environment":
      return "检查运行环境";
    case "existing_robot":
      return "绑定已有机器人";
    case "plugin":
      return "安装官方插件";
    case "create_robot":
      return "新建机器人";
    case "authorize":
      return "完成授权";
    case "approve_pairing":
      return "批准接入";
    case "routing":
      return "设置接待";
    case "skipped":
      return "已跳过引导";
    default:
      return step;
  }
}

function resolveFeishuOnboardingStepDisplay(step: FeishuOnboardingStep): FeishuOnboardingStepDisplay {
  switch (step) {
    case "environment":
      return {
        title: "检查运行环境",
        body: "先确认 Node.js 和 npm 可用，再继续安装和授权。",
      };
    case "existing_robot":
      return {
        title: "绑定已有机器人",
        body: "先在这里确认你已有机器人的信息，再到高级控制台保存完整配置。",
      };
    case "plugin":
      return {
        title: "安装官方插件",
        body: "先安装飞书官方插件。安装完成后，再继续新建机器人或绑定已有机器人。",
      };
    case "create_robot":
      return {
        title: "新建机器人",
        body: "点击“新建机器人向导”后会打开飞书官方安装向导。完成创建后，WorkClaw 会自动回填机器人信息，再继续安装与授权。",
      };
    case "authorize":
      return {
        title: "完成授权",
        body: "完成官方插件启动后，回到飞书会话里走完授权；如果机器人提示 access not configured，下一步还需要批准这次接入。",
      };
    case "approve_pairing":
      return {
        title: "批准接入请求",
        body: "飞书里的会话已经发起接入请求。请在这里批准这次接入，机器人才能真正开始收发消息。",
      };
    case "routing":
      return {
        title: "设置接待",
        body: "授权完成后，把默认接待员工和群聊范围补齐，消息才会稳定落到正确员工。",
      };
    case "skipped":
      return {
        title: "已跳过引导",
        body: "你已经暂时跳过引导，随时可以重新打开。",
      };
    default:
      return {
        title: step,
        body: step,
      };
  }
}

export function resolveFeishuOnboardingPanelDisplay(
  state: FeishuOnboardingState,
  isSkipped: boolean,
  selectedPath: "existing_robot" | "create_robot" | null,
): FeishuOnboardingPanelDisplay {
  if (isSkipped) {
    return {
      title: "已跳过引导",
      body: "已跳过引导。需要时随时点击“重新打开引导”。",
      badgeLabel: "已跳过引导",
      badgeClassName: "border-gray-200 bg-gray-100 text-gray-700",
      primaryActionLabel: "重新打开引导",
    };
  }

  const branchStep =
    state.currentStep === "existing_robot" || state.currentStep === "create_robot"
      ? selectedPath ?? state.currentStep
      : state.currentStep;
  const stepDisplay = resolveFeishuOnboardingStepDisplay(branchStep);
  if (branchStep === "create_robot") {
    return {
      ...stepDisplay,
      badgeLabel: state.canContinue ? "可继续使用" : "仍需完成当前步骤",
      badgeClassName: "border-blue-200 bg-blue-50 text-blue-700",
      primaryActionLabel: "新建机器人向导（高级）",
    };
  }
  if (branchStep === "existing_robot") {
    return {
      ...stepDisplay,
      badgeLabel: state.canContinue ? "可继续使用" : "仍需完成当前步骤",
      badgeClassName: "border-blue-200 bg-blue-50 text-blue-700",
      primaryActionLabel: "验证机器人信息",
    };
  }
  if (branchStep === "plugin") {
    return {
      ...stepDisplay,
      badgeLabel: state.canContinue ? "可继续使用" : "仍需完成当前步骤",
      badgeClassName: "border-blue-200 bg-blue-50 text-blue-700",
      primaryActionLabel: "安装官方插件",
    };
  }
  if (branchStep === "approve_pairing") {
    return {
      ...stepDisplay,
      badgeLabel: state.canContinue ? "可继续使用" : "等待批准接入",
      badgeClassName: "border-amber-200 bg-amber-50 text-amber-700",
      primaryActionLabel: "批准这次接入",
    };
  }
  return {
    ...stepDisplay,
    badgeLabel: state.canContinue ? "可继续使用" : "仍需完成当前步骤",
    badgeClassName: "border-blue-200 bg-blue-50 text-blue-700",
    primaryActionLabel: "继续引导设置",
  };
}

export function resolveFeishuGuidedInlineError(
  errorMessage: string,
  step: FeishuOnboardingStep,
  branch: "existing_robot" | "create_robot" | null,
): string | null {
  if (!errorMessage.trim()) {
    return null;
  }
  if (
    errorMessage.startsWith("请先填写已有机器人的 App ID 和 App Secret") ||
    errorMessage.startsWith("已有机器人校验失败:") ||
    errorMessage.startsWith("验证机器人信息失败:") ||
    errorMessage.startsWith("启动飞书官方创建机器人向导失败:") ||
    errorMessage.startsWith("启动飞书官方绑定机器人向导失败:")
  ) {
    return errorMessage;
  }
  if (branch === "existing_robot" && errorMessage.startsWith("请先填写并保存已有机器人的 App ID 和 App Secret")) {
    return errorMessage;
  }
  if (step === "plugin" && errorMessage.startsWith("安装飞书官方插件失败:")) {
    return errorMessage;
  }
  if (
    step === "authorize" &&
    (errorMessage.startsWith("安装并启动飞书连接失败:") ||
      errorMessage.startsWith("刷新飞书官方插件状态失败:") ||
      errorMessage.startsWith("官方插件启动失败:") ||
      errorMessage.startsWith("启动飞书官方插件失败:"))
  ) {
    return errorMessage;
  }
  if (
    step === "approve_pairing" &&
    (errorMessage.startsWith("批准飞书接入请求失败:") ||
      errorMessage.startsWith("拒绝飞书接入请求失败:"))
  ) {
    return errorMessage;
  }
  if (step === "environment" && errorMessage.startsWith("刷新飞书接入状态失败:")) {
    return errorMessage;
  }
  return null;
}

export function resolveFeishuAuthorizationInlineError(errorMessage: string): string | null {
  if (!errorMessage.trim()) {
    return null;
  }
  if (
    errorMessage.startsWith("安装并启动飞书连接失败:") ||
    errorMessage.startsWith("刷新飞书官方插件状态失败:") ||
    errorMessage.startsWith("官方插件启动失败:") ||
    errorMessage.startsWith("启动飞书官方插件失败:")
  ) {
    return errorMessage;
  }
  return null;
}

export function resolveFeishuGuidedInlineNotice(
  noticeMessage: string,
  step: FeishuOnboardingStep,
  branch: "existing_robot" | "create_robot" | null,
): string | null {
  if (!noticeMessage.trim()) {
    return null;
  }
  if (
    (branch === "existing_robot" && noticeMessage.startsWith("机器人信息验证成功")) ||
    (branch === "create_robot" && noticeMessage.startsWith("已启动飞书官方创建机器人向导")) ||
    (branch === "existing_robot" && noticeMessage.startsWith("已启动飞书官方绑定机器人向导"))
  ) {
    return noticeMessage;
  }
  if (step === "plugin" && noticeMessage.startsWith("飞书官方插件已安装")) {
    return noticeMessage;
  }
  if (
    step === "authorize" &&
    (noticeMessage.startsWith("飞书连接组件已启动") || noticeMessage.startsWith("已尝试启动飞书连接组件"))
  ) {
    return noticeMessage;
  }
  if (
    step === "approve_pairing" &&
    (noticeMessage.startsWith("已批准飞书接入请求") || noticeMessage.startsWith("已拒绝飞书接入请求"))
  ) {
    return noticeMessage;
  }
  return null;
}

export function getLatestInstallerOutputLine(session: OpenClawLarkInstallerSessionStatus) {
  return session.recent_output.length > 0
    ? session.recent_output[session.recent_output.length - 1] ?? ""
    : "";
}

export function isFeishuInstallerFinished(session: OpenClawLarkInstallerSessionStatus) {
  return (
    !session.running &&
    !!session.mode &&
    session.recent_output.some((line) => line.includes("[system] official installer finished"))
  );
}

export function resolveFeishuInstallerCompletionNotice(session: OpenClawLarkInstallerSessionStatus) {
  if (!isFeishuInstallerFinished(session)) {
    return "";
  }

  const output = session.recent_output.join("\n");
  if (
    session.mode === "create" &&
    (output.includes("Success! Bot configured.") || output.includes("机器人配置成功"))
  ) {
    return "机器人创建已完成，请点击“启动连接”继续完成授权。";
  }

  if (session.mode === "link") {
    return "机器人关联已完成，请点击“启动连接”继续完成授权。";
  }

  return "安装向导已完成，请继续启动连接并完成授权。";
}

export function shouldShowFeishuInstallerGuidedPanel(
  branch: "existing_robot" | "create_robot" | null,
  session: OpenClawLarkInstallerSessionStatus,
) {
  return branch === "create_robot" && (session.running || session.recent_output.length > 0);
}

export function resolveFeishuInstallerFlowLabel(mode: OpenClawLarkInstallerMode | null) {
  if (mode === "create") {
    return "飞书官方创建机器人向导";
  }
  if (mode === "link") {
    return "飞书官方绑定机器人向导";
  }
  return "飞书官方向导";
}

export function looksLikeInstallerQrLine(line: string) {
  return /[█▀▄▌▐]/.test(line);
}

export function extractFeishuInstallerQrBlock(lines: string[]) {
  let bestStart = -1;
  let bestLength = 0;
  let currentStart = -1;
  let currentLength = 0;

  for (let index = 0; index < lines.length; index += 1) {
    if (looksLikeInstallerQrLine(lines[index] || "")) {
      if (currentStart === -1) {
        currentStart = index;
        currentLength = 0;
      }
      currentLength += 1;
      if (currentLength > bestLength) {
        bestStart = currentStart;
        bestLength = currentLength;
      }
    } else {
      currentStart = -1;
      currentLength = 0;
    }
  }

  if (bestStart === -1 || bestLength < 3) {
    return [];
  }
  return lines.slice(bestStart, bestStart + bestLength);
}

export function sanitizeFeishuInstallerDisplayLines(lines: string[]) {
  const qrBlock = extractFeishuInstallerQrBlock(lines);
  const qrSet = new Set(qrBlock);
  const filtered: string[] = [];
  let skipDebugObject = false;

  for (const rawLine of lines) {
    const line = rawLine ?? "";
    if (qrSet.has(line)) {
      continue;
    }
    if (line.startsWith("[DEBUG]") && line.includes("{")) {
      skipDebugObject = true;
      continue;
    }
    if (skipDebugObject) {
      if (line.trim() === "}") {
        skipDebugObject = false;
      }
      continue;
    }
    if (line.startsWith("[DEBUG]")) {
      continue;
    }
    filtered.push(line);
  }

  return filtered;
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
      description: "请先安装 Node.js LTS，完成后回到这里重新检测环境。",
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
