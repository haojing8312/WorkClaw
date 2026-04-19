import type {
  FeishuGatewaySettings,
  FeishuPairingRequestRecord,
  FeishuPluginEnvironmentStatus,
  FeishuSetupProgress,
  OpenClawLarkInstallerMode,
  OpenClawLarkInstallerSessionStatus,
  OpenClawPluginChannelHost,
  OpenClawPluginFeishuAdvancedSettings,
  OpenClawPluginFeishuCredentialProbeResult,
  OpenClawPluginFeishuRuntimeStatus,
} from "../../../types";

export type FeishuActionHandlers = {
  updateFeishuConnectorSettings: (patch: Partial<FeishuGatewaySettings>) => void;
  updateFeishuAdvancedSettings: (patch: Partial<OpenClawPluginFeishuAdvancedSettings>) => void;
  updateFeishuInstallerInput: (value: string) => void;
  openFeishuOnboardingPath: (path: "existing_robot" | "create_robot") => void;
  reopenFeishuOnboarding: () => void;
  skipFeishuOnboarding: (signature: string) => void;
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
  handleSaveFeishuAdvancedSettings: () => Promise<void>;
  handleCopyFeishuDiagnostics: () => Promise<void>;
};

export interface FeishuSettingsControllerViewModelInput {
  feishuConnectorSettings: FeishuGatewaySettings;
  feishuAdvancedSettings: OpenClawPluginFeishuAdvancedSettings;
  pluginChannelHosts: OpenClawPluginChannelHost[];
  feishuEnvironmentStatus: FeishuPluginEnvironmentStatus | null;
  feishuSetupProgress: FeishuSetupProgress | null;
  officialFeishuRuntimeStatus: OpenClawPluginFeishuRuntimeStatus | null;
  feishuCredentialProbe: OpenClawPluginFeishuCredentialProbeResult | null;
  feishuInstallerSession: OpenClawLarkInstallerSessionStatus;
  feishuInstallerInput: string;
  feishuInstallerBusy: boolean;
  feishuInstallerStartingMode: OpenClawLarkInstallerMode | null;
  feishuPairingRequests: FeishuPairingRequestRecord[];
  feishuPairingActionLoading: "approve" | "deny" | null;
  savingFeishuConnector: boolean;
  savingFeishuAdvancedSettings: boolean;
  retryingFeishuConnector: boolean;
  installingOfficialFeishuPlugin: boolean;
  validatingFeishuCredentials: boolean;
  feishuConnectorNotice: string;
  feishuConnectorError: string;
  feishuOnboardingPanelMode: "guided" | "skipped";
  feishuOnboardingSelectedPath: "existing_robot" | "create_robot" | null;
  feishuOnboardingSkippedSignature: string | null;
  actions: FeishuActionHandlers;
}
