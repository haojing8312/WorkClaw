import type {
  FeishuGatewaySettings,
  FeishuPairingRequestRecord,
  FeishuPluginEnvironmentStatus,
  FeishuSetupProgress,
  OpenClawLarkInstallerMode,
  OpenClawLarkInstallerSessionStatus,
  OpenClawPluginFeishuCredentialProbeResult,
} from "../../../types";

type FeishuOnboardingStep = "environment" | "plugin" | "existing_robot" | "create_robot" | "authorize" | "approve_pairing" | "routing" | "skipped";

type FeishuOnboardingPanelDisplay = {
  title: string;
  body: string;
  badgeLabel: string;
  badgeClassName: string;
  primaryActionLabel: string;
};

type FeishuOnboardingState = {
  currentStep: FeishuOnboardingStep;
  canContinue: boolean;
  skipped: boolean;
  mode: "existing_robot" | "create_robot";
};

type FeishuAuthorizationAction = {
  label: string;
  busyLabel: string;
};

type FeishuRoutingStatus = {
  label: string;
  description: string;
  actionLabel: string;
};

type FeishuSetupSummary = {
  title: string;
  description: string;
};

export interface FeishuSettingsSectionProps {
  onOpenEmployees?: () => void;
  feishuConnectorSettings: FeishuGatewaySettings;
  onUpdateFeishuConnectorSettings: (patch: Partial<FeishuGatewaySettings>) => void;
  feishuEnvironmentStatus: FeishuPluginEnvironmentStatus | null;
  feishuSetupProgress: FeishuSetupProgress | null;
  validatingFeishuCredentials: boolean;
  feishuCredentialProbe: OpenClawPluginFeishuCredentialProbeResult | null;
  feishuInstallerSession: OpenClawLarkInstallerSessionStatus;
  feishuInstallerInput: string;
  onUpdateFeishuInstallerInput: (value: string) => void;
  feishuInstallerBusy: boolean;
  feishuInstallerStartingMode: OpenClawLarkInstallerMode | null;
  feishuPairingActionLoading: "approve" | "deny" | null;
  savingFeishuConnector: boolean;
  retryingFeishuConnector: boolean;
  installingOfficialFeishuPlugin: boolean;
  feishuConnectorNotice: string;
  feishuConnectorError: string;
  feishuOnboardingState: FeishuOnboardingState;
  feishuOnboardingPanelMode: "guided" | "skipped";
  feishuOnboardingSelectedPath: "existing_robot" | "create_robot" | null;
  feishuOnboardingSkippedSignature: string | null;
  onOpenFeishuOnboardingPath: (path: "existing_robot" | "create_robot") => void;
  onReopenFeishuOnboarding: () => void;
  onSkipFeishuOnboarding: (signature: string) => void;
  feishuOnboardingProgressSignature: string;
  feishuOnboardingIsSkipped: boolean;
  feishuOnboardingEffectiveBranch: "existing_robot" | "create_robot" | null;
  feishuOnboardingHeaderStep: FeishuOnboardingStep;
  feishuOnboardingHeaderMode: "existing_robot" | "create_robot";
  feishuOnboardingPanelDisplay: FeishuOnboardingPanelDisplay;
  showFeishuInstallerGuidedPanel: boolean;
  feishuGuidedInlineError: string | null;
  feishuGuidedInlineNotice: string | null;
  feishuAuthorizationInlineError: string | null;
  feishuInstallerDisplayMode: OpenClawLarkInstallerMode | null;
  feishuInstallerFlowLabel: string;
  feishuInstallerQrBlock: string[];
  feishuInstallerDisplayLines: string[];
  feishuInstallerStartupHint: string | null;
  feishuAuthorizationAction: FeishuAuthorizationAction;
  feishuRoutingStatus: FeishuRoutingStatus;
  feishuRoutingActionAvailable: boolean;
  feishuOnboardingPrimaryActionLabel: string;
  feishuOnboardingPrimaryActionDisabled: boolean;
  feishuSetupSummary: FeishuSetupSummary;
  pendingFeishuPairingCount: number;
  pendingFeishuPairingRequest: FeishuPairingRequestRecord | null;
  getFeishuEnvironmentLabel: (ready: boolean, fallback: string) => string;
  formatCompactDateTime: (value: string | null | undefined) => string;
  handleRefreshFeishuSetup: () => Promise<void>;
  handleOpenFeishuOfficialDocs: () => Promise<void>;
  handleValidateFeishuCredentials: () => Promise<void>;
  handleSaveFeishuConnector: () => Promise<void>;
  handleInstallOfficialFeishuPlugin: () => Promise<void>;
  handleInstallAndStartFeishuConnector: () => Promise<void>;
  handleResolveFeishuPairingRequest: (requestId: string, action: "approve" | "deny") => Promise<void>;
  handleStartFeishuInstaller: (mode: "create" | "link") => Promise<void>;
  handleStopFeishuInstallerSession: () => Promise<void>;
  handleSendFeishuInstallerInput: () => Promise<void>;
}

export function FeishuSettingsSection({
  onOpenEmployees,
  feishuConnectorSettings,
  onUpdateFeishuConnectorSettings,
  feishuEnvironmentStatus,
  feishuSetupProgress,
  validatingFeishuCredentials,
  feishuCredentialProbe,
  feishuInstallerSession,
  feishuInstallerInput,
  onUpdateFeishuInstallerInput,
  feishuInstallerBusy,
  feishuInstallerStartingMode,
  feishuPairingActionLoading,
  savingFeishuConnector,
  retryingFeishuConnector,
  installingOfficialFeishuPlugin,
  feishuConnectorNotice,
  feishuConnectorError,
  feishuOnboardingState,
  feishuOnboardingPanelMode,
  feishuOnboardingSelectedPath,
  feishuOnboardingSkippedSignature,
  onOpenFeishuOnboardingPath,
  onReopenFeishuOnboarding,
  onSkipFeishuOnboarding,
  feishuOnboardingProgressSignature,
  feishuOnboardingIsSkipped,
  feishuOnboardingEffectiveBranch,
  feishuOnboardingHeaderStep,
  feishuOnboardingHeaderMode,
  feishuOnboardingPanelDisplay,
  showFeishuInstallerGuidedPanel,
  feishuGuidedInlineError,
  feishuGuidedInlineNotice,
  feishuAuthorizationInlineError,
  feishuInstallerDisplayMode,
  feishuInstallerFlowLabel,
  feishuInstallerQrBlock,
  feishuInstallerDisplayLines,
  feishuInstallerStartupHint,
  feishuAuthorizationAction,
  feishuRoutingStatus,
  feishuRoutingActionAvailable,
  feishuOnboardingPrimaryActionLabel,
  feishuOnboardingPrimaryActionDisabled,
  feishuSetupSummary,
  pendingFeishuPairingCount,
  pendingFeishuPairingRequest,
  getFeishuEnvironmentLabel,
  formatCompactDateTime,
  handleRefreshFeishuSetup,
  handleOpenFeishuOfficialDocs,
  handleValidateFeishuCredentials,
  handleSaveFeishuConnector,
  handleInstallOfficialFeishuPlugin,
  handleInstallAndStartFeishuConnector,
  handleResolveFeishuPairingRequest,
  handleStartFeishuInstaller,
  handleStopFeishuInstallerSession,
  handleSendFeishuInstallerInput,
}: FeishuSettingsSectionProps) {
  const feishuInstallerSummaryHint =
    feishuInstallerSession.prompt_hint &&
    feishuInstallerSession.prompt_hint !== feishuInstallerStartupHint &&
    !feishuInstallerDisplayLines.includes(feishuInstallerSession.prompt_hint)
      ? feishuInstallerSession.prompt_hint
      : null;

  return (
    <div data-testid="connector-panel-feishu" className="space-y-3">
      <div className="bg-white rounded-lg p-4 space-y-4">
        <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
          <div className="space-y-1">
            <div className="text-sm font-medium text-gray-900">飞书连接</div>
            <div className="text-xs text-gray-500">先完成机器人接入，再安装飞书官方插件并完成授权，最后补充接待员工设置。</div>
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => void handleRefreshFeishuSetup()}
              disabled={retryingFeishuConnector}
              className="h-8 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
            >
              {retryingFeishuConnector ? "检测中..." : "重新检测"}
            </button>
            <button
              type="button"
              onClick={() => void handleOpenFeishuOfficialDocs()}
              className="inline-flex h-8 items-center rounded border border-blue-200 bg-blue-50 px-3 text-xs text-blue-700 hover:bg-blue-100"
            >
              查看官方文档
            </button>
          </div>
        </div>

        {feishuConnectorError && !feishuGuidedInlineError && !feishuAuthorizationInlineError ? (
          <div className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700">{feishuConnectorError}</div>
        ) : null}
        {feishuConnectorNotice && !feishuGuidedInlineNotice ? (
          <div className="rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-2 text-xs text-emerald-700">{feishuConnectorNotice}</div>
        ) : null}

        <div className="rounded-xl border border-blue-200 bg-blue-50 p-4">
          <div className="text-base font-medium text-blue-950">{feishuSetupSummary.title}</div>
          <div className="mt-1 text-sm text-blue-900">{feishuSetupSummary.description}</div>
          <div className="mt-3 grid grid-cols-1 gap-2 md:grid-cols-4">
            <div className="rounded border border-blue-100 bg-white/80 px-3 py-2">
              <div className="text-[11px] text-blue-700">运行环境</div>
              <div className="text-sm font-medium text-gray-900">
                {feishuEnvironmentStatus?.can_start_runtime ? "已准备好" : "待检查"}
              </div>
            </div>
            <div className="rounded border border-blue-100 bg-white/80 px-3 py-2">
              <div className="text-[11px] text-blue-700">机器人信息</div>
              <div className="text-sm font-medium text-gray-900">
                {feishuSetupProgress?.credentials_configured ? "已填写" : "未填写"}
              </div>
            </div>
            <div className="rounded border border-blue-100 bg-white/80 px-3 py-2">
              <div className="text-[11px] text-blue-700">连接组件</div>
              <div className="text-sm font-medium text-gray-900">
                {feishuSetupProgress?.plugin_installed ? "已安装" : "未安装"}
              </div>
            </div>
            <div className="rounded border border-blue-100 bg-white/80 px-3 py-2">
              <div className="text-[11px] text-blue-700">授权与接待</div>
              <div className="text-sm font-medium text-gray-900">{feishuRoutingStatus.label}</div>
            </div>
          </div>
          <div className="mt-3 grid grid-cols-1 gap-2 md:grid-cols-2">
            <div className="rounded border border-blue-100 bg-white/80 px-3 py-2">
              <div className="text-sm font-medium text-gray-900">飞书接入概览</div>
              <div className="mt-1 text-xs text-gray-600">查看飞书连接是否已启动并可接收事件。</div>
            </div>
            <div className="rounded border border-blue-100 bg-white/80 px-3 py-2">
              <div className="text-sm font-medium text-gray-900">员工关联入口</div>
              <div className="mt-1 text-xs text-gray-600">先完成飞书连接，再到员工详情中指定谁来接待飞书消息。</div>
              <div className="mt-1 text-xs text-gray-600">飞书连接成功后，请前往员工详情中的“飞书接待”配置默认接待员工或指定群聊范围。</div>
            </div>
          </div>
          <div
            data-testid="feishu-onboarding-state"
            data-current-step={feishuOnboardingState.currentStep}
            data-skipped={String(feishuOnboardingState.skipped)}
            className="mt-3 rounded border border-blue-100 bg-white/70 px-3 py-2 text-xs text-blue-900"
          >
            引导步骤：{feishuOnboardingPanelDisplay.title} ·
            {feishuOnboardingHeaderStep === "create_robot"
              ? "创建机器人"
              : feishuOnboardingHeaderStep === "existing_robot"
                ? "绑定已有机器人"
                : feishuOnboardingHeaderStep === "plugin"
                  ? "安装官方插件"
                  : feishuOnboardingHeaderMode === "create_robot"
                    ? "创建机器人"
                    : "绑定已有机器人"} ·
            {feishuInstallerBusy && feishuInstallerStartingMode
              ? "正在启动向导"
              : feishuOnboardingState.canContinue
                ? "可继续使用其余功能"
                : "仍需完成当前引导"}
          </div>
          <div data-testid="feishu-onboarding-step" className="rounded-lg border border-blue-100 bg-white/90 px-3 py-3">
            {feishuOnboardingState.currentStep === "existing_robot" || feishuOnboardingState.currentStep === "create_robot" ? (
              <div className="mb-3 flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  onClick={() => {
                    onOpenFeishuOnboardingPath("existing_robot");
                  }}
                  className={`h-8 px-3 rounded border text-xs ${
                    feishuOnboardingEffectiveBranch === "existing_robot"
                      ? "border-blue-600 bg-blue-600 text-white"
                      : "border-blue-200 bg-white text-blue-700 hover:bg-blue-50"
                  }`}
                >
                  绑定已有机器人
                </button>
                <button
                  type="button"
                  onClick={() => {
                    onOpenFeishuOnboardingPath("create_robot");
                  }}
                  className={`h-8 px-3 rounded border text-xs ${
                    feishuOnboardingEffectiveBranch === "create_robot"
                      ? "border-blue-600 bg-blue-600 text-white"
                      : "border-blue-200 bg-white text-blue-700 hover:bg-blue-50"
                  }`}
                >
                  新建机器人
                </button>
              </div>
            ) : null}
            <div className="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
              <div className="space-y-1">
                <div className="text-sm font-medium text-blue-950">{feishuOnboardingPanelDisplay.title}</div>
                <div className="text-xs text-blue-900">{feishuOnboardingPanelDisplay.body}</div>
              </div>
              <div className={`rounded-full border px-3 py-1 text-[11px] font-medium ${feishuOnboardingPanelDisplay.badgeClassName}`}>
                {feishuOnboardingPanelDisplay.badgeLabel}
              </div>
            </div>
            <div className="mt-4 flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => {
                  if (feishuOnboardingIsSkipped) {
                    onReopenFeishuOnboarding();
                    return;
                  }
                  if (feishuOnboardingEffectiveBranch === "create_robot") {
                    void handleStartFeishuInstaller("create");
                    return;
                  }
                  if (feishuOnboardingEffectiveBranch === "existing_robot") {
                    void handleValidateFeishuCredentials();
                    return;
                  }
                  if (feishuOnboardingHeaderStep === "environment") {
                    void handleRefreshFeishuSetup();
                    return;
                  }
                  if (feishuOnboardingHeaderStep === "plugin") {
                    void handleInstallOfficialFeishuPlugin();
                    return;
                  }
                  if (feishuOnboardingHeaderStep === "authorize") {
                    void handleInstallAndStartFeishuConnector();
                    return;
                  }
                  if (feishuOnboardingHeaderStep === "approve_pairing") {
                    if (feishuPairingActionLoading !== null && pendingFeishuPairingRequest) {
                      void handleResolveFeishuPairingRequest(pendingFeishuPairingRequest.id, "approve");
                    } else if (pendingFeishuPairingRequest) {
                      void handleResolveFeishuPairingRequest(pendingFeishuPairingRequest.id, "approve");
                    }
                    return;
                  }
                  if (feishuOnboardingHeaderStep === "routing") {
                    onOpenEmployees?.();
                  }
                }}
                disabled={feishuOnboardingPrimaryActionDisabled}
                className="h-8 px-3 rounded bg-blue-600 text-xs text-white hover:bg-blue-700"
              >
                {feishuOnboardingPrimaryActionLabel}
              </button>
              {!feishuOnboardingIsSkipped ? (
                <button
                  type="button"
                  onClick={() => {
                    onSkipFeishuOnboarding(feishuOnboardingProgressSignature);
                  }}
                  className="h-8 px-3 rounded border border-blue-200 bg-white text-xs text-blue-700 hover:bg-blue-50"
                >
                  暂时跳过
                </button>
              ) : null}
              {!feishuOnboardingIsSkipped &&
              (feishuOnboardingState.currentStep === "authorize" || feishuOnboardingState.currentStep === "approve_pairing") ? (
                <button
                  type="button"
                  onClick={() => void handleRefreshFeishuSetup()}
                  disabled={retryingFeishuConnector}
                  className="h-8 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
                >
                  {retryingFeishuConnector ? "检测中..." : "刷新授权状态"}
                </button>
              ) : null}
            </div>
            {feishuGuidedInlineError ? (
              <div className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700">{feishuGuidedInlineError}</div>
            ) : null}
            {feishuGuidedInlineNotice ? (
              <div className="rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-2 text-xs text-emerald-700">{feishuGuidedInlineNotice}</div>
            ) : null}
            {!feishuOnboardingIsSkipped &&
            feishuOnboardingHeaderStep === "approve_pairing" &&
            pendingFeishuPairingRequest ? (
              <div className="mt-3 rounded-lg border border-amber-200 bg-white p-3" data-testid="feishu-guided-pairing-panel">
                <div className="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
                  <div className="space-y-1">
                    <div className="text-sm font-medium text-gray-900">飞书已经发来了接入请求</div>
                    <div className="text-xs text-gray-600">
                      这一步不是再去授权，而是由 WorkClaw 批准这次接入请求。批准后，这个飞书发送者才能真正开始和机器人对话。
                    </div>
                  </div>
                  <div className="rounded-full border border-amber-200 bg-amber-50 px-3 py-1 text-[11px] font-medium text-amber-700">
                    等待批准
                  </div>
                </div>
                <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-3">
                  <div className="rounded border border-gray-200 bg-gray-50 px-3 py-2">
                    <div className="text-[11px] text-gray-500">发送者</div>
                    <div className="text-sm font-medium text-gray-900 break-all">{pendingFeishuPairingRequest.sender_id}</div>
                  </div>
                  <div className="rounded border border-gray-200 bg-gray-50 px-3 py-2">
                    <div className="text-[11px] text-gray-500">Pairing Code</div>
                    <div className="text-sm font-medium text-gray-900">{pendingFeishuPairingRequest.code || "未返回"}</div>
                  </div>
                  <div className="rounded border border-gray-200 bg-gray-50 px-3 py-2">
                    <div className="text-[11px] text-gray-500">发起时间</div>
                    <div className="text-sm font-medium text-gray-900">{formatCompactDateTime(pendingFeishuPairingRequest.created_at)}</div>
                  </div>
                </div>
                <div className="mt-3 flex flex-wrap gap-2">
                  <button
                    type="button"
                    onClick={() => void handleResolveFeishuPairingRequest(pendingFeishuPairingRequest.id, "approve")}
                    disabled={feishuPairingActionLoading !== null}
                    className="h-8 px-3 rounded bg-amber-600 text-xs text-white hover:bg-amber-700 disabled:bg-amber-300"
                  >
                    {feishuPairingActionLoading === "approve" ? "批准中..." : "批准这次接入"}
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleResolveFeishuPairingRequest(pendingFeishuPairingRequest.id, "deny")}
                    disabled={feishuPairingActionLoading !== null}
                    className="h-8 px-3 rounded border border-red-200 bg-white text-xs text-red-700 hover:bg-red-50 disabled:bg-gray-100"
                  >
                    {feishuPairingActionLoading === "deny" ? "拒绝中..." : "拒绝这次接入"}
                  </button>
                </div>
              </div>
            ) : null}
            {feishuInstallerStartupHint && !showFeishuInstallerGuidedPanel ? (
              <div className="mt-3 rounded-lg border border-indigo-200 bg-indigo-50 px-3 py-2 text-xs text-indigo-700">
                {feishuInstallerStartupHint}
              </div>
            ) : null}
            {showFeishuInstallerGuidedPanel ? (
              <div className="mt-3 rounded-lg border border-indigo-200 bg-white p-3" data-testid="feishu-guided-installer-panel">
                <div className="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
                  <div className="space-y-1">
                    <div className="text-sm font-medium text-gray-900">{feishuInstallerFlowLabel}正在这里继续</div>
                    <div className="text-xs text-gray-600">
                      不用再往下翻到高级控制台。扫码、等待结果和下一步提示都会先显示在这里。
                    </div>
                  </div>
                  <div className="rounded-full border border-indigo-200 bg-indigo-50 px-3 py-1 text-[11px] font-medium text-indigo-700">
                    {feishuInstallerBusy && feishuInstallerStartingMode ? "正在启动" : feishuInstallerSession.running ? "向导运行中" : "向导已结束"}
                  </div>
                </div>
                {feishuInstallerStartupHint ? (
                  <div className="mt-3 rounded-lg border border-indigo-200 bg-indigo-50 px-3 py-2 text-xs text-indigo-700">
                    {feishuInstallerStartupHint}
                  </div>
                ) : null}
                <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-3">
                  <div className="rounded border border-gray-200 bg-gray-50 px-3 py-2">
                    <div className="text-[11px] text-gray-500">当前模式</div>
                    <div className="text-sm font-medium text-gray-900">
                      {feishuInstallerDisplayMode === "create"
                        ? "新建机器人"
                        : feishuInstallerDisplayMode === "link"
                          ? "绑定已有机器人"
                          : "未启动"}
                    </div>
                  </div>
                  <div className="rounded border border-gray-200 bg-gray-50 px-3 py-2">
                    <div className="text-[11px] text-gray-500">提示</div>
                    <div className="text-sm font-medium text-gray-900">
                      {feishuInstallerSummaryHint || (feishuInstallerStartupHint ? "按上方提示继续" : "查看下方输出")}
                    </div>
                  </div>
                  <div className="rounded border border-gray-200 bg-gray-50 px-3 py-2">
                    <div className="text-[11px] text-gray-500">下一步</div>
                    <div className="text-sm font-medium text-gray-900">
                      {feishuInstallerBusy && feishuInstallerStartingMode
                        ? "等待向导启动"
                        : feishuInstallerSession.running
                          ? "按当前提示继续"
                          : "准备启动连接并完成授权"}
                    </div>
                  </div>
                </div>
                {feishuInstallerQrBlock.length > 0 ? (
                  <div className="mt-3 rounded-lg border border-gray-900 bg-[#050816] px-3 py-3 text-xs text-gray-100">
                    <div className="mb-2 text-[11px] font-medium text-indigo-200">请使用飞书扫码继续</div>
                    <pre data-testid="feishu-guided-installer-qr" className="overflow-x-auto whitespace-pre font-mono leading-none">
                      {feishuInstallerQrBlock.join("\n")}
                    </pre>
                  </div>
                ) : null}
                <div className="mt-3 rounded-lg border border-gray-900 bg-[#050816] px-3 py-3 text-xs text-gray-100">
                  <div className="mb-2 text-[11px] font-medium text-indigo-200">向导输出</div>
                  <pre className="max-h-48 overflow-auto whitespace-pre-wrap break-all font-mono">
                    {feishuInstallerDisplayLines.length > 0
                      ? feishuInstallerDisplayLines.join("\n")
                      : feishuInstallerStartupHint || "暂无安装向导输出"}
                  </pre>
                </div>
                <div className="mt-3 flex flex-wrap gap-2">
                  <button
                    type="button"
                    onClick={() => void handleRefreshFeishuSetup()}
                    disabled={retryingFeishuConnector}
                    className="h-8 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
                  >
                    {retryingFeishuConnector ? "检测中..." : "刷新状态"}
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleStopFeishuInstallerSession()}
                    disabled={feishuInstallerBusy || !feishuInstallerSession.running}
                    className="h-8 px-3 rounded border border-red-200 bg-white text-xs text-red-700 hover:bg-red-50 disabled:bg-gray-100"
                  >
                    停止向导
                  </button>
                </div>
              </div>
            ) : null}
            {feishuOnboardingIsSkipped ? (
              <div className="mt-3 rounded-lg border border-blue-100 bg-white/80 px-3 py-2 text-xs text-blue-900">
                {feishuOnboardingPanelDisplay.body}
              </div>
            ) : null}
          </div>
        </div>
      </div>
    </div>
  );
}
