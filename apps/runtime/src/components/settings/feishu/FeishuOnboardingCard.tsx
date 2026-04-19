import type { FeishuSettingsSectionProps } from "./FeishuSettingsSection.types";

type FeishuOnboardingCardProps = Pick<
  FeishuSettingsSectionProps,
  | "onOpenEmployees"
  | "feishuInstallerSession"
  | "feishuInstallerBusy"
  | "feishuInstallerStartingMode"
  | "feishuPairingActionLoading"
  | "retryingFeishuConnector"
  | "feishuOnboardingState"
  | "onOpenFeishuOnboardingPath"
  | "onReopenFeishuOnboarding"
  | "onSkipFeishuOnboarding"
  | "feishuOnboardingProgressSignature"
  | "feishuOnboardingIsSkipped"
  | "feishuOnboardingEffectiveBranch"
  | "feishuOnboardingHeaderStep"
  | "feishuOnboardingHeaderMode"
  | "feishuOnboardingPanelDisplay"
  | "showFeishuInstallerGuidedPanel"
  | "feishuGuidedInlineError"
  | "feishuGuidedInlineNotice"
  | "feishuInstallerDisplayMode"
  | "feishuInstallerFlowLabel"
  | "feishuInstallerQrBlock"
  | "feishuInstallerDisplayLines"
  | "feishuInstallerStartupHint"
  | "feishuOnboardingPrimaryActionLabel"
  | "feishuOnboardingPrimaryActionDisabled"
  | "pendingFeishuPairingRequest"
  | "formatCompactDateTime"
  | "handleRefreshFeishuSetup"
  | "handleValidateFeishuCredentials"
  | "handleInstallOfficialFeishuPlugin"
  | "handleInstallAndStartFeishuConnector"
  | "handleResolveFeishuPairingRequest"
  | "handleStartFeishuInstaller"
  | "handleStopFeishuInstallerSession"
>;

export function FeishuOnboardingCard(props: FeishuOnboardingCardProps) {
  const {
    onOpenEmployees,
    feishuInstallerSession,
    feishuInstallerBusy,
    feishuInstallerStartingMode,
    feishuPairingActionLoading,
    retryingFeishuConnector,
    feishuOnboardingState,
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
    feishuInstallerDisplayMode,
    feishuInstallerFlowLabel,
    feishuInstallerQrBlock,
    feishuInstallerDisplayLines,
    feishuInstallerStartupHint,
    feishuOnboardingPrimaryActionLabel,
    feishuOnboardingPrimaryActionDisabled,
    pendingFeishuPairingRequest,
    formatCompactDateTime,
    handleRefreshFeishuSetup,
    handleValidateFeishuCredentials,
    handleInstallOfficialFeishuPlugin,
    handleInstallAndStartFeishuConnector,
    handleResolveFeishuPairingRequest,
    handleStartFeishuInstaller,
    handleStopFeishuInstallerSession,
  } = props;

  const feishuInstallerSummaryHint =
    feishuInstallerSession.prompt_hint &&
    feishuInstallerSession.prompt_hint !== feishuInstallerStartupHint &&
    !feishuInstallerDisplayLines.includes(feishuInstallerSession.prompt_hint)
      ? feishuInstallerSession.prompt_hint
      : null;

  return (
    <div className="space-y-3">
      <div
        data-testid="feishu-onboarding-state"
        data-current-step={feishuOnboardingState.currentStep}
        data-skipped={String(feishuOnboardingState.skipped)}
        className="rounded border border-blue-100 bg-white/70 px-3 py-2 text-xs text-blue-900"
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
        {(feishuOnboardingState.currentStep === "existing_robot" || feishuOnboardingState.currentStep === "create_robot") && (
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <button
              type="button"
              onClick={() => onOpenFeishuOnboardingPath("existing_robot")}
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
              onClick={() => onOpenFeishuOnboardingPath("create_robot")}
              className={`h-8 px-3 rounded border text-xs ${
                feishuOnboardingEffectiveBranch === "create_robot"
                  ? "border-blue-600 bg-blue-600 text-white"
                  : "border-blue-200 bg-white text-blue-700 hover:bg-blue-50"
              }`}
            >
              新建机器人
            </button>
          </div>
        )}
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
              if (feishuOnboardingIsSkipped) return onReopenFeishuOnboarding();
              if (feishuOnboardingEffectiveBranch === "create_robot") return void handleStartFeishuInstaller("create");
              if (feishuOnboardingEffectiveBranch === "existing_robot") return void handleValidateFeishuCredentials();
              if (feishuOnboardingHeaderStep === "environment") return void handleRefreshFeishuSetup();
              if (feishuOnboardingHeaderStep === "plugin") return void handleInstallOfficialFeishuPlugin();
              if (feishuOnboardingHeaderStep === "authorize") return void handleInstallAndStartFeishuConnector();
              if (feishuOnboardingHeaderStep === "approve_pairing" && pendingFeishuPairingRequest) {
                return void handleResolveFeishuPairingRequest(pendingFeishuPairingRequest.id, "approve");
              }
              if (feishuOnboardingHeaderStep === "routing") onOpenEmployees?.();
            }}
            disabled={feishuOnboardingPrimaryActionDisabled}
            className="h-8 px-3 rounded bg-blue-600 text-xs text-white hover:bg-blue-700"
          >
            {feishuOnboardingPrimaryActionLabel}
          </button>
          {!feishuOnboardingIsSkipped && (
            <button
              type="button"
              onClick={() => onSkipFeishuOnboarding(feishuOnboardingProgressSignature)}
              className="h-8 px-3 rounded border border-blue-200 bg-white text-xs text-blue-700 hover:bg-blue-50"
            >
              暂时跳过
            </button>
          )}
          {!feishuOnboardingIsSkipped &&
            (feishuOnboardingState.currentStep === "authorize" || feishuOnboardingState.currentStep === "approve_pairing") && (
              <button
                type="button"
                onClick={() => void handleRefreshFeishuSetup()}
                disabled={retryingFeishuConnector}
                className="h-8 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
              >
                {retryingFeishuConnector ? "检测中..." : "刷新授权状态"}
              </button>
            )}
        </div>
        {feishuGuidedInlineError && (
          <div className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700">{feishuGuidedInlineError}</div>
        )}
        {feishuGuidedInlineNotice && (
          <div className="rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-2 text-xs text-emerald-700">{feishuGuidedInlineNotice}</div>
        )}
        {!feishuOnboardingIsSkipped && feishuOnboardingHeaderStep === "approve_pairing" && pendingFeishuPairingRequest && (
          <div className="mt-3 rounded-lg border border-amber-200 bg-white p-3" data-testid="feishu-guided-pairing-panel">
            <div className="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
              <div className="space-y-1">
                <div className="text-sm font-medium text-gray-900">飞书已经发来了接入请求</div>
                <div className="text-xs text-gray-600">这一步不是再去授权，而是由 WorkClaw 批准这次接入请求。批准后，这个飞书发送者才能真正开始和机器人对话。</div>
              </div>
              <div className="rounded-full border border-amber-200 bg-amber-50 px-3 py-1 text-[11px] font-medium text-amber-700">等待批准</div>
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
        )}
        {feishuInstallerStartupHint && !showFeishuInstallerGuidedPanel && (
          <div className="mt-3 rounded-lg border border-indigo-200 bg-indigo-50 px-3 py-2 text-xs text-indigo-700">{feishuInstallerStartupHint}</div>
        )}
        {showFeishuInstallerGuidedPanel && (
          <div className="mt-3 rounded-lg border border-indigo-200 bg-white p-3" data-testid="feishu-guided-installer-panel">
            <div className="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
              <div className="space-y-1">
                <div className="text-sm font-medium text-gray-900">{feishuInstallerFlowLabel}正在这里继续</div>
                <div className="text-xs text-gray-600">不用再往下翻到飞书接入控制台。扫码、等待结果和下一步提示都会先显示在这里。</div>
              </div>
              <div className="rounded-full border border-indigo-200 bg-indigo-50 px-3 py-1 text-[11px] font-medium text-indigo-700">
                {feishuInstallerBusy && feishuInstallerStartingMode ? "正在启动" : feishuInstallerSession.running ? "向导运行中" : "向导已结束"}
              </div>
            </div>
            {feishuInstallerStartupHint && (
              <div className="mt-3 rounded-lg border border-indigo-200 bg-indigo-50 px-3 py-2 text-xs text-indigo-700">{feishuInstallerStartupHint}</div>
            )}
            <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-3">
              <div className="rounded border border-gray-200 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">当前模式</div>
                <div className="text-sm font-medium text-gray-900">
                  {feishuInstallerDisplayMode === "create" ? "新建机器人" : feishuInstallerDisplayMode === "link" ? "绑定已有机器人" : "未启动"}
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
                  {feishuInstallerBusy && feishuInstallerStartingMode ? "等待向导启动" : feishuInstallerSession.running ? "按当前提示继续" : "准备启动连接并完成授权"}
                </div>
              </div>
            </div>
            {feishuInstallerQrBlock.length > 0 && (
              <div className="mt-3 rounded-lg border border-gray-900 bg-[#050816] px-3 py-3 text-xs text-gray-100">
                <div className="mb-2 text-[11px] font-medium text-indigo-200">请使用飞书扫码继续</div>
                <pre data-testid="feishu-guided-installer-qr" className="overflow-x-auto whitespace-pre font-mono leading-none">
                  {feishuInstallerQrBlock.join("\n")}
                </pre>
              </div>
            )}
            <div className="mt-3 rounded-lg border border-gray-900 bg-[#050816] px-3 py-3 text-xs text-gray-100">
              <div className="mb-2 text-[11px] font-medium text-indigo-200">向导输出</div>
              <pre className="max-h-48 overflow-auto whitespace-pre-wrap break-all font-mono">
                {feishuInstallerDisplayLines.length > 0 ? feishuInstallerDisplayLines.join("\n") : feishuInstallerStartupHint || "暂无安装向导输出"}
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
        )}
        {feishuOnboardingIsSkipped && (
          <div className="mt-3 rounded-lg border border-blue-100 bg-white/80 px-3 py-2 text-xs text-blue-900">{feishuOnboardingPanelDisplay.body}</div>
        )}
      </div>
    </div>
  );
}
