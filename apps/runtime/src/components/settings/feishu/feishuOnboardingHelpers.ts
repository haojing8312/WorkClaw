import type {
  FeishuSetupProgress,
  OpenClawLarkInstallerMode,
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

export interface FeishuOnboardingPanelDisplay extends FeishuOnboardingStepDisplay {
  badgeLabel: string;
  badgeClassName: string;
  primaryActionLabel: string;
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

  if (input.summaryState === "env_missing") return "environment";
  if (input.summaryState === "ready_to_bind") return "create_robot";
  if (input.summaryState === "plugin_not_installed") return "plugin";
  if (input.summaryState === "plugin_starting" || input.summaryState === "awaiting_auth") return "authorize";
  if (input.summaryState === "awaiting_pairing_approval") return "approve_pairing";
  if (input.summaryState === "ready_for_routing") return "routing";
  if (input.summaryState === "runtime_error") return "environment";

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
        body: "先在这里确认你已有机器人的信息，再到飞书接入控制台保存完整配置。",
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
