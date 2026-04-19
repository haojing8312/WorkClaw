import type { FeishuAdvancedConsoleSectionProps } from "./FeishuAdvancedConsoleSection";
import type { FeishuAdvancedSectionProps } from "./FeishuAdvancedSection";
import type { FeishuSettingsSectionProps } from "./FeishuSettingsSection";
import {
  formatCompactDateTime,
  getFeishuEnvironmentLabel,
  summarizeConnectorIssue,
  summarizeOfficialFeishuRuntimeLogs,
} from "./feishuSelectors";
import type { FeishuSettingsControllerViewModelInput } from "./feishuSettingsControllerViewModel.types";

function resolveOnboardingPrimaryActionLabel(input: {
  feishuOnboardingIsSkipped: boolean;
  feishuOnboardingHeaderStep: FeishuSettingsSectionProps["feishuOnboardingHeaderStep"];
  retryingFeishuConnector: boolean;
  installingOfficialFeishuPlugin: boolean;
  feishuInstallerBusy: boolean;
  feishuInstallerStartingMode: FeishuSettingsSectionProps["feishuInstallerStartingMode"];
  feishuOnboardingPanelDisplay: FeishuSettingsSectionProps["feishuOnboardingPanelDisplay"];
  feishuAuthorizationAction: FeishuSettingsSectionProps["feishuAuthorizationAction"];
  feishuSetupProgress: FeishuSettingsControllerViewModelInput["feishuSetupProgress"];
  feishuPairingActionLoading: "approve" | "deny" | null;
  feishuRoutingStatus: FeishuSettingsSectionProps["feishuRoutingStatus"];
}) {
  if (input.feishuOnboardingIsSkipped) {
    return "重新打开引导";
  }
  if (input.feishuOnboardingHeaderStep === "environment") {
    return input.retryingFeishuConnector ? "检测中..." : "重新检测环境";
  }
  if (input.feishuOnboardingHeaderStep === "plugin") {
    return input.installingOfficialFeishuPlugin ? "安装中..." : "安装官方插件";
  }
  if (input.feishuOnboardingHeaderStep === "create_robot") {
    return input.feishuInstallerBusy && input.feishuInstallerStartingMode === "create"
      ? "启动中..."
      : input.feishuOnboardingPanelDisplay.primaryActionLabel;
  }
  if (input.feishuOnboardingHeaderStep === "authorize") {
    return input.retryingFeishuConnector || input.installingOfficialFeishuPlugin
      ? input.feishuAuthorizationAction.busyLabel
      : input.feishuSetupProgress?.plugin_installed
        ? "启动连接"
        : "安装并启动";
  }
  if (input.feishuOnboardingHeaderStep === "approve_pairing") {
    return input.feishuPairingActionLoading === "approve" ? "批准中..." : "批准这次接入";
  }
  if (input.feishuOnboardingHeaderStep === "routing") {
    return input.feishuRoutingStatus.actionLabel;
  }
  return input.feishuOnboardingPanelDisplay.primaryActionLabel;
}

function resolveOnboardingPrimaryActionDisabled(input: {
  feishuOnboardingHeaderStep: FeishuSettingsSectionProps["feishuOnboardingHeaderStep"];
  retryingFeishuConnector: boolean;
  installingOfficialFeishuPlugin: boolean;
  validatingFeishuCredentials: boolean;
  feishuInstallerBusy: boolean;
  pendingFeishuPairingRequest: FeishuSettingsSectionProps["pendingFeishuPairingRequest"];
  feishuPairingActionLoading: "approve" | "deny" | null;
}) {
  if (input.feishuOnboardingHeaderStep === "environment") {
    return input.retryingFeishuConnector;
  }
  if (input.feishuOnboardingHeaderStep === "plugin") {
    return input.installingOfficialFeishuPlugin;
  }
  if (input.feishuOnboardingHeaderStep === "existing_robot") {
    return input.validatingFeishuCredentials;
  }
  if (input.feishuOnboardingHeaderStep === "create_robot") {
    return input.feishuInstallerBusy;
  }
  if (input.feishuOnboardingHeaderStep === "approve_pairing") {
    return !input.pendingFeishuPairingRequest || input.feishuPairingActionLoading !== null;
  }
  if (input.feishuOnboardingHeaderStep === "authorize") {
    return input.retryingFeishuConnector || input.installingOfficialFeishuPlugin;
  }
  return false;
}

export function buildFeishuSettingsSectionProps(input: {
  source: FeishuSettingsControllerViewModelInput;
  feishuOnboardingState: FeishuSettingsSectionProps["feishuOnboardingState"];
  feishuOnboardingProgressSignature: string;
  feishuOnboardingIsSkipped: boolean;
  feishuOnboardingEffectiveBranch: FeishuSettingsSectionProps["feishuOnboardingEffectiveBranch"];
  feishuOnboardingHeaderStep: FeishuSettingsSectionProps["feishuOnboardingHeaderStep"];
  feishuOnboardingHeaderMode: FeishuSettingsSectionProps["feishuOnboardingHeaderMode"];
  feishuOnboardingPanelDisplay: FeishuSettingsSectionProps["feishuOnboardingPanelDisplay"];
  showFeishuInstallerGuidedPanel: boolean;
  feishuGuidedInlineError: string | null;
  feishuGuidedInlineNotice: string | null;
  feishuAuthorizationInlineError: string | null;
  feishuInstallerDisplayMode: FeishuSettingsSectionProps["feishuInstallerDisplayMode"];
  feishuInstallerFlowLabel: string;
  feishuInstallerQrBlock: string[];
  feishuInstallerDisplayLines: string[];
  feishuInstallerStartupHint: string | null;
  feishuAuthorizationAction: FeishuSettingsSectionProps["feishuAuthorizationAction"];
  feishuRoutingStatus: FeishuSettingsSectionProps["feishuRoutingStatus"];
  feishuSetupSummary: FeishuSettingsSectionProps["feishuSetupSummary"];
  pendingFeishuPairingCount: number;
  pendingFeishuPairingRequest: FeishuSettingsSectionProps["pendingFeishuPairingRequest"];
}) {
  return {
    feishuConnectorSettings: input.source.feishuConnectorSettings,
    onUpdateFeishuConnectorSettings: input.source.actions.updateFeishuConnectorSettings,
    feishuEnvironmentStatus: input.source.feishuEnvironmentStatus,
    feishuSetupProgress: input.source.feishuSetupProgress,
    validatingFeishuCredentials: input.source.validatingFeishuCredentials,
    feishuCredentialProbe: input.source.feishuCredentialProbe,
    feishuInstallerSession: input.source.feishuInstallerSession,
    feishuInstallerInput: input.source.feishuInstallerInput,
    onUpdateFeishuInstallerInput: input.source.actions.updateFeishuInstallerInput,
    feishuInstallerBusy: input.source.feishuInstallerBusy,
    feishuInstallerStartingMode: input.source.feishuInstallerStartingMode,
    feishuPairingActionLoading: input.source.feishuPairingActionLoading,
    savingFeishuConnector: input.source.savingFeishuConnector,
    retryingFeishuConnector: input.source.retryingFeishuConnector,
    installingOfficialFeishuPlugin: input.source.installingOfficialFeishuPlugin,
    feishuConnectorNotice: input.source.feishuConnectorNotice,
    feishuConnectorError: input.source.feishuConnectorError,
    feishuOnboardingState: input.feishuOnboardingState,
    feishuOnboardingPanelMode: input.source.feishuOnboardingPanelMode,
    feishuOnboardingSelectedPath: input.source.feishuOnboardingSelectedPath,
    feishuOnboardingSkippedSignature: input.source.feishuOnboardingSkippedSignature,
    onOpenFeishuOnboardingPath: input.source.actions.openFeishuOnboardingPath,
    onReopenFeishuOnboarding: input.source.actions.reopenFeishuOnboarding,
    onSkipFeishuOnboarding: input.source.actions.skipFeishuOnboarding,
    feishuOnboardingProgressSignature: input.feishuOnboardingProgressSignature,
    feishuOnboardingIsSkipped: input.feishuOnboardingIsSkipped,
    feishuOnboardingEffectiveBranch: input.feishuOnboardingEffectiveBranch,
    feishuOnboardingHeaderStep: input.feishuOnboardingHeaderStep,
    feishuOnboardingHeaderMode: input.feishuOnboardingHeaderMode,
    feishuOnboardingPanelDisplay: input.feishuOnboardingPanelDisplay,
    showFeishuInstallerGuidedPanel: input.showFeishuInstallerGuidedPanel,
    feishuGuidedInlineError: input.feishuGuidedInlineError,
    feishuGuidedInlineNotice: input.feishuGuidedInlineNotice,
    feishuAuthorizationInlineError: input.feishuAuthorizationInlineError,
    feishuInstallerDisplayMode: input.feishuInstallerDisplayMode,
    feishuInstallerFlowLabel: input.feishuInstallerFlowLabel,
    feishuInstallerQrBlock: input.feishuInstallerQrBlock,
    feishuInstallerDisplayLines: input.feishuInstallerDisplayLines,
    feishuInstallerStartupHint: input.feishuInstallerStartupHint,
    feishuAuthorizationAction: input.feishuAuthorizationAction,
    feishuRoutingStatus: input.feishuRoutingStatus,
    feishuRoutingActionAvailable: input.source.feishuSetupProgress?.auth_status === "approved",
    feishuOnboardingPrimaryActionLabel: resolveOnboardingPrimaryActionLabel({
      feishuOnboardingIsSkipped: input.feishuOnboardingIsSkipped,
      feishuOnboardingHeaderStep: input.feishuOnboardingHeaderStep,
      retryingFeishuConnector: input.source.retryingFeishuConnector,
      installingOfficialFeishuPlugin: input.source.installingOfficialFeishuPlugin,
      feishuInstallerBusy: input.source.feishuInstallerBusy,
      feishuInstallerStartingMode: input.source.feishuInstallerStartingMode,
      feishuOnboardingPanelDisplay: input.feishuOnboardingPanelDisplay,
      feishuAuthorizationAction: input.feishuAuthorizationAction,
      feishuSetupProgress: input.source.feishuSetupProgress,
      feishuPairingActionLoading: input.source.feishuPairingActionLoading,
      feishuRoutingStatus: input.feishuRoutingStatus,
    }),
    feishuOnboardingPrimaryActionDisabled: resolveOnboardingPrimaryActionDisabled({
      feishuOnboardingHeaderStep: input.feishuOnboardingHeaderStep,
      retryingFeishuConnector: input.source.retryingFeishuConnector,
      installingOfficialFeishuPlugin: input.source.installingOfficialFeishuPlugin,
      validatingFeishuCredentials: input.source.validatingFeishuCredentials,
      feishuInstallerBusy: input.source.feishuInstallerBusy,
      pendingFeishuPairingRequest: input.pendingFeishuPairingRequest,
      feishuPairingActionLoading: input.source.feishuPairingActionLoading,
    }),
    feishuSetupSummary: input.feishuSetupSummary,
    pendingFeishuPairingCount: input.pendingFeishuPairingCount,
    pendingFeishuPairingRequest: input.pendingFeishuPairingRequest,
    getFeishuEnvironmentLabel,
    formatCompactDateTime,
    handleRefreshFeishuSetup: input.source.actions.handleRefreshFeishuSetup,
    handleOpenFeishuOfficialDocs: input.source.actions.handleOpenFeishuOfficialDocs,
    handleValidateFeishuCredentials: input.source.actions.handleValidateFeishuCredentials,
    handleSaveFeishuConnector: input.source.actions.handleSaveFeishuConnector,
    handleInstallOfficialFeishuPlugin: input.source.actions.handleInstallOfficialFeishuPlugin,
    handleInstallAndStartFeishuConnector: input.source.actions.handleInstallAndStartFeishuConnector,
    handleResolveFeishuPairingRequest: input.source.actions.handleResolveFeishuPairingRequest,
    handleStartFeishuInstaller: input.source.actions.handleStartFeishuInstaller,
    handleStopFeishuInstallerSession: input.source.actions.handleStopFeishuInstallerSession,
    handleSendFeishuInstallerInput: input.source.actions.handleSendFeishuInstallerInput,
  } satisfies FeishuSettingsSectionProps;
}

export function buildFeishuAdvancedConsoleSectionProps(input: {
  source: FeishuSettingsControllerViewModelInput;
  pendingFeishuPairingCount: number;
  pendingFeishuPairingRequest: FeishuSettingsSectionProps["pendingFeishuPairingRequest"];
  feishuOnboardingEffectiveBranch: FeishuSettingsSectionProps["feishuOnboardingEffectiveBranch"];
  feishuAuthorizationInlineError: string | null;
  feishuOnboardingHeaderStep: FeishuSettingsSectionProps["feishuOnboardingHeaderStep"];
  feishuInstallerDisplayMode: FeishuSettingsSectionProps["feishuInstallerDisplayMode"];
  feishuInstallerStartupHint: string | null;
  feishuAuthorizationAction: FeishuSettingsSectionProps["feishuAuthorizationAction"];
  feishuRoutingStatus: FeishuSettingsSectionProps["feishuRoutingStatus"];
}) {
  return {
    feishuConnectorSettings: input.source.feishuConnectorSettings,
    onUpdateFeishuConnectorSettings: input.source.actions.updateFeishuConnectorSettings,
    feishuEnvironmentStatus: input.source.feishuEnvironmentStatus,
    feishuSetupProgress: input.source.feishuSetupProgress,
    officialFeishuRuntimeStatus: input.source.officialFeishuRuntimeStatus,
    feishuCredentialProbe: input.source.feishuCredentialProbe,
    validatingFeishuCredentials: input.source.validatingFeishuCredentials,
    savingFeishuConnector: input.source.savingFeishuConnector,
    retryingFeishuConnector: input.source.retryingFeishuConnector,
    installingOfficialFeishuPlugin: input.source.installingOfficialFeishuPlugin,
    feishuInstallerSession: input.source.feishuInstallerSession,
    feishuInstallerInput: input.source.feishuInstallerInput,
    onUpdateFeishuInstallerInput: input.source.actions.updateFeishuInstallerInput,
    feishuInstallerBusy: input.source.feishuInstallerBusy,
    feishuInstallerStartingMode: input.source.feishuInstallerStartingMode,
    feishuPairingActionLoading: input.source.feishuPairingActionLoading,
    pendingFeishuPairingCount: input.pendingFeishuPairingCount,
    pendingFeishuPairingRequest: input.pendingFeishuPairingRequest,
    feishuOnboardingEffectiveBranch: input.feishuOnboardingEffectiveBranch,
    feishuAuthorizationInlineError: input.feishuAuthorizationInlineError,
    feishuOnboardingHeaderStep: input.feishuOnboardingHeaderStep,
    feishuInstallerDisplayMode: input.feishuInstallerDisplayMode,
    feishuInstallerStartupHint: input.feishuInstallerStartupHint,
    feishuAuthorizationAction: input.feishuAuthorizationAction,
    feishuRoutingStatus: input.feishuRoutingStatus,
    getFeishuEnvironmentLabel,
    formatCompactDateTime,
    handleValidateFeishuCredentials: input.source.actions.handleValidateFeishuCredentials,
    handleSaveFeishuConnector: input.source.actions.handleSaveFeishuConnector,
    handleInstallAndStartFeishuConnector: input.source.actions.handleInstallAndStartFeishuConnector,
    handleRefreshFeishuSetup: input.source.actions.handleRefreshFeishuSetup,
    handleResolveFeishuPairingRequest: input.source.actions.handleResolveFeishuPairingRequest,
    handleStartFeishuInstaller: input.source.actions.handleStartFeishuInstaller,
    handleStopFeishuInstallerSession: input.source.actions.handleStopFeishuInstallerSession,
    handleSendFeishuInstallerInput: input.source.actions.handleSendFeishuInstallerInput,
  } satisfies FeishuAdvancedConsoleSectionProps;
}

export function buildFeishuAdvancedSectionProps(input: {
  source: FeishuSettingsControllerViewModelInput;
}) {
  return {
    feishuAdvancedSettings: input.source.feishuAdvancedSettings,
    onUpdateFeishuAdvancedSettings: input.source.actions.updateFeishuAdvancedSettings,
    savingFeishuAdvancedSettings: input.source.savingFeishuAdvancedSettings,
    onSaveFeishuAdvancedSettings: input.source.actions.handleSaveFeishuAdvancedSettings,
  } satisfies FeishuAdvancedSectionProps;
}
