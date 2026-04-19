import {
  buildFeishuOnboardingState,
  extractFeishuInstallerQrBlock,
  formatCompactDateTime,
  getFeishuAuthorizationAction,
  getFeishuConnectionDetailSummary,
  getFeishuRoutingStatus,
  getFeishuSetupSummary,
  resolveFeishuAuthorizationInlineError,
  resolveFeishuConnectorStatus,
  resolveFeishuGuidedInlineError,
  resolveFeishuGuidedInlineNotice,
  resolveFeishuInstallerFlowLabel,
  resolveFeishuOnboardingPanelDisplay,
  sanitizeFeishuInstallerDisplayLines,
  shouldShowFeishuInstallerGuidedPanel,
  summarizeConnectorIssue,
  summarizeOfficialFeishuRuntimeLogs,
} from "./feishuSelectors";
import {
  buildFeishuAdvancedConsoleSectionProps,
  buildFeishuAdvancedSectionProps,
  buildFeishuSettingsSectionProps,
} from "./feishuSettingsControllerViewModelSections";
import type { FeishuSettingsControllerViewModelInput } from "./feishuSettingsControllerViewModel.types";

export function buildFeishuSettingsControllerViewModel(input: FeishuSettingsControllerViewModelInput) {
  const primaryPluginChannelHost =
    input.pluginChannelHosts.find((host) => host.channel === "feishu") ?? input.pluginChannelHosts[0] ?? null;
  const hasInstalledOfficialFeishuPlugin =
    input.pluginChannelHosts.length > 0 || input.feishuSetupProgress?.plugin_installed === true;
  const runtimeRunning =
    input.officialFeishuRuntimeStatus?.running === true || input.feishuSetupProgress?.runtime_running === true;
  const pendingFeishuPairingCount =
    input.feishuSetupProgress?.pending_pairings ??
    input.feishuPairingRequests.filter((request) => request.status === "pending").length;
  const pendingFeishuPairingRequest =
    input.feishuPairingRequests.find((request) => request.status === "pending") ?? null;
  const feishuOnboardingState = buildFeishuOnboardingState({
    summaryState: input.feishuSetupProgress?.summary_state ?? null,
    setupProgress: input.feishuSetupProgress,
    installerMode: input.feishuInstallerSession?.mode ?? null,
  });
  const feishuOnboardingProgressSignature = [
    feishuOnboardingState.currentStep,
    feishuOnboardingState.mode,
    feishuOnboardingState.canContinue ? "continue" : "blocked",
    feishuOnboardingState.skipped ? "backend-skipped" : "active",
  ].join("|");
  const feishuOnboardingIsSkipped =
    feishuOnboardingState.skipped ||
    (input.feishuOnboardingPanelMode === "skipped" &&
      input.feishuOnboardingSkippedSignature === feishuOnboardingProgressSignature);
  const feishuOnboardingBackendBranch =
    feishuOnboardingState.currentStep === "existing_robot" || feishuOnboardingState.currentStep === "create_robot"
      ? feishuOnboardingState.currentStep
      : null;
  const feishuOnboardingEffectiveBranch = input.feishuOnboardingSelectedPath ?? feishuOnboardingBackendBranch;
  const feishuOnboardingHeaderStep = feishuOnboardingBackendBranch
    ? feishuOnboardingEffectiveBranch ?? feishuOnboardingState.currentStep
    : feishuOnboardingState.currentStep;
  const feishuOnboardingHeaderMode = feishuOnboardingEffectiveBranch ?? feishuOnboardingState.mode;
  const feishuOnboardingPanelDisplay = resolveFeishuOnboardingPanelDisplay(
    feishuOnboardingState,
    feishuOnboardingIsSkipped,
    feishuOnboardingEffectiveBranch,
  );
  const showFeishuInstallerGuidedPanel = shouldShowFeishuInstallerGuidedPanel(
    feishuOnboardingEffectiveBranch,
    input.feishuInstallerSession,
  );
  const feishuGuidedInlineError = resolveFeishuGuidedInlineError(
    input.feishuConnectorError,
    feishuOnboardingHeaderStep,
    feishuOnboardingEffectiveBranch,
  );
  const feishuGuidedInlineNotice = resolveFeishuGuidedInlineNotice(
    input.feishuConnectorNotice,
    feishuOnboardingHeaderStep,
    feishuOnboardingEffectiveBranch,
  );
  const feishuAuthorizationInlineError = resolveFeishuAuthorizationInlineError(input.feishuConnectorError);
  const feishuInstallerDisplayMode =
    input.feishuInstallerSession.mode ??
    (feishuOnboardingEffectiveBranch === "create_robot"
      ? "create"
      : feishuOnboardingEffectiveBranch
        ? "link"
        : null);
  const feishuInstallerFlowLabel = resolveFeishuInstallerFlowLabel(feishuInstallerDisplayMode);
  const feishuInstallerQrBlock = extractFeishuInstallerQrBlock(input.feishuInstallerSession.recent_output ?? []);
  const feishuInstallerDisplayLines = sanitizeFeishuInstallerDisplayLines(input.feishuInstallerSession.recent_output ?? []);
  const feishuInstallerStartupHint =
    input.feishuInstallerBusy && input.feishuInstallerStartingMode
      ? `正在启动${resolveFeishuInstallerFlowLabel(input.feishuInstallerStartingMode)}，请稍候...`
      : input.feishuInstallerSession.prompt_hint || null;
  const feishuAuthorizationAction = getFeishuAuthorizationAction({
    runtimeRunning,
    pluginInstalled: input.feishuSetupProgress?.plugin_installed === true,
  });
  const feishuRoutingStatus = getFeishuRoutingStatus({
    authApproved: input.feishuSetupProgress?.auth_status === "approved",
    defaultRoutingEmployeeName: input.feishuSetupProgress?.default_routing_employee_name,
    scopedRoutingCount: input.feishuSetupProgress?.scoped_routing_count,
  });
  const feishuSetupSummary = getFeishuSetupSummary({
    skipped: feishuOnboardingState.skipped,
    summaryState: input.feishuSetupProgress?.summary_state ?? null,
    runtimeRunning,
    authApproved: input.feishuSetupProgress?.auth_status === "approved",
    runtimeLastError: input.feishuSetupProgress?.runtime_last_error,
    officialRuntimeLastError: input.officialFeishuRuntimeStatus?.last_error,
  });
  const feishuConnectorStatus = resolveFeishuConnectorStatus({
    running: runtimeRunning,
    lastError: input.officialFeishuRuntimeStatus?.last_error ?? "",
    hasInstalledOfficialFeishuPlugin,
  });
  const feishuConnectionDetailSummary = getFeishuConnectionDetailSummary({
    connectorStatus: feishuConnectorStatus,
    runtimeRunning,
    authApproved: input.feishuSetupProgress?.auth_status === "approved",
    defaultRoutingEmployeeName: input.feishuSetupProgress?.default_routing_employee_name,
    scopedRoutingCount: input.feishuSetupProgress?.scoped_routing_count,
    pendingPairings: pendingFeishuPairingCount,
  });

  const settingsSectionProps = buildFeishuSettingsSectionProps({
    source: input,
    feishuOnboardingState,
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
    feishuSetupSummary,
    pendingFeishuPairingCount,
    pendingFeishuPairingRequest,
  });

  const advancedConsoleSectionProps = buildFeishuAdvancedConsoleSectionProps({
    source: input,
    pendingFeishuPairingCount,
    pendingFeishuPairingRequest,
    feishuOnboardingEffectiveBranch,
    feishuAuthorizationInlineError,
    feishuOnboardingHeaderStep,
    feishuInstallerDisplayMode,
    feishuInstallerStartupHint,
    feishuAuthorizationAction,
    feishuRoutingStatus,
  });

  const advancedSectionProps = buildFeishuAdvancedSectionProps({
    source: input,
  });

  return {
    pendingFeishuPairingCount,
    primaryPluginChannelHost,
    hasInstalledOfficialFeishuPlugin,
    sections: {
      settingsSectionProps,
      advancedConsoleSectionProps,
      advancedSectionProps,
    },
  };
}
