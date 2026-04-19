import type { Dispatch, SetStateAction } from "react";
import type {
  FeishuGatewaySettings,
  FeishuPairingRequestRecord,
  FeishuSetupProgress,
  OpenClawLarkInstallerMode,
  OpenClawLarkInstallerSessionStatus,
  OpenClawPluginChannelHost,
  OpenClawPluginFeishuAdvancedSettings,
  OpenClawPluginFeishuCredentialProbeResult,
  OpenClawPluginFeishuRuntimeStatus,
} from "../../../types";

export type SetState<T> = Dispatch<SetStateAction<T>>;

export interface FeishuSettingsControllerActionDeps {
  feishuConnectorSettings: FeishuGatewaySettings;
  feishuAdvancedSettings: OpenClawPluginFeishuAdvancedSettings;
  feishuInstallerInput: string;
  pluginChannelHosts: OpenClawPluginChannelHost[];
  feishuSetupProgress: FeishuSetupProgress | null;
  setFeishuConnectorSettings: SetState<FeishuGatewaySettings>;
  setFeishuAdvancedSettings: SetState<OpenClawPluginFeishuAdvancedSettings>;
  setPluginChannelHosts: SetState<OpenClawPluginChannelHost[]>;
  setPluginChannelHostsError: SetState<string>;
  setValidatingFeishuCredentials: SetState<boolean>;
  setFeishuCredentialProbe: SetState<OpenClawPluginFeishuCredentialProbeResult | null>;
  setFeishuInstallerSession: SetState<OpenClawLarkInstallerSessionStatus>;
  setFeishuInstallerInput: SetState<string>;
  setFeishuInstallerBusy: SetState<boolean>;
  setFeishuInstallerStartingMode: SetState<OpenClawLarkInstallerMode | null>;
  setFeishuPairingRequests: SetState<FeishuPairingRequestRecord[]>;
  setFeishuPairingRequestsError: SetState<string>;
  setFeishuPairingActionLoading: SetState<"approve" | "deny" | null>;
  setSavingFeishuConnector: SetState<boolean>;
  setSavingFeishuAdvancedSettings: SetState<boolean>;
  setRetryingFeishuConnector: SetState<boolean>;
  setInstallingOfficialFeishuPlugin: SetState<boolean>;
  setFeishuConnectorNotice: SetState<string>;
  setFeishuConnectorError: SetState<string>;
  setOfficialFeishuRuntimeStatus: SetState<OpenClawPluginFeishuRuntimeStatus | null>;
  loadConnectorStatuses: () => Promise<void>;
  loadFeishuSetupProgress: () => Promise<void>;
}
