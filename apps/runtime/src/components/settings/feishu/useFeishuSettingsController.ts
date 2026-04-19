import { useEffect, useRef, useState } from "react";
import type {
  FeishuGatewaySettings,
  FeishuPairingRequestRecord,
  OpenClawLarkInstallerMode,
  OpenClawPluginChannelHost,
  OpenClawPluginFeishuAdvancedSettings,
  OpenClawPluginFeishuCredentialProbeResult,
} from "../../../types";
import type { SettingsTabName } from "../SettingsTabNav";
import { buildFeishuDiagnosticsClipboardText, getLatestInstallerOutputLine, resolveFeishuInstallerCompletionNotice } from "./feishuSelectors";
import { DEFAULT_FEISHU_ADVANCED_SETTINGS, DEFAULT_FEISHU_CONNECTOR_SETTINGS } from "./feishuSettingsControllerDefaults";
import { createFeishuSettingsControllerActions } from "./feishuSettingsControllerActions";
import { buildFeishuSettingsControllerViewModel } from "./feishuSettingsControllerViewModel";
import { useFeishuInstallerSessionController } from "./useFeishuInstallerSessionController";
import { useFeishuRuntimeStatusController } from "./useFeishuRuntimeStatusController";
import { useFeishuSetupProgressController } from "./useFeishuSetupProgressController";

interface UseFeishuSettingsControllerOptions {
  activeTab: SettingsTabName;
}

export function useFeishuSettingsController({
  activeTab,
}: UseFeishuSettingsControllerOptions) {
  const [feishuConnectorSettings, setFeishuConnectorSettings] =
    useState<FeishuGatewaySettings>(DEFAULT_FEISHU_CONNECTOR_SETTINGS);
  const [feishuAdvancedSettings, setFeishuAdvancedSettings] =
    useState<OpenClawPluginFeishuAdvancedSettings>(DEFAULT_FEISHU_ADVANCED_SETTINGS);
  const [pluginChannelHosts, setPluginChannelHosts] = useState<OpenClawPluginChannelHost[]>([]);
  const [, setPluginChannelHostsError] = useState("");
  const {
    officialFeishuRuntimeStatus,
    setOfficialFeishuRuntimeStatus,
    loadConnectorStatuses,
  } = useFeishuRuntimeStatusController({ activeTab });
  const {
    feishuEnvironmentStatus,
    feishuSetupProgress,
    loadFeishuSetupProgress,
  } = useFeishuSetupProgressController({ activeTab });
  const [validatingFeishuCredentials, setValidatingFeishuCredentials] = useState(false);
  const [feishuCredentialProbe, setFeishuCredentialProbe] =
    useState<OpenClawPluginFeishuCredentialProbeResult | null>(null);
  const {
    feishuInstallerSession,
    setFeishuInstallerSession,
  } = useFeishuInstallerSessionController({ activeTab });
  const [feishuInstallerInput, setFeishuInstallerInput] = useState("");
  const [feishuInstallerBusy, setFeishuInstallerBusy] = useState(false);
  const [feishuInstallerStartingMode, setFeishuInstallerStartingMode] =
    useState<OpenClawLarkInstallerMode | null>(null);
  const handledFeishuInstallerCompletionRef = useRef("");
  const [feishuPairingRequests, setFeishuPairingRequests] = useState<FeishuPairingRequestRecord[]>([]);
  const [, setFeishuPairingRequestsError] = useState("");
  const [feishuPairingActionLoading, setFeishuPairingActionLoading] =
    useState<"approve" | "deny" | null>(null);
  const [savingFeishuConnector, setSavingFeishuConnector] = useState(false);
  const [savingFeishuAdvancedSettings, setSavingFeishuAdvancedSettings] = useState(false);
  const [retryingFeishuConnector, setRetryingFeishuConnector] = useState(false);
  const [installingOfficialFeishuPlugin, setInstallingOfficialFeishuPlugin] = useState(false);
  const [feishuConnectorNotice, setFeishuConnectorNotice] = useState("");
  const [feishuConnectorError, setFeishuConnectorError] = useState("");
  const [feishuOnboardingPanelMode, setFeishuOnboardingPanelMode] =
    useState<"guided" | "skipped">("guided");
  const [feishuOnboardingSelectedPath, setFeishuOnboardingSelectedPath] = useState<
    "existing_robot" | "create_robot" | null
  >(null);
  const [feishuOnboardingSkippedSignature, setFeishuOnboardingSkippedSignature] =
    useState<string | null>(null);

  function openFeishuOnboardingPath(path: "existing_robot" | "create_robot") {
    setFeishuOnboardingSelectedPath(path);
    setFeishuOnboardingPanelMode("guided");
    setFeishuOnboardingSkippedSignature(null);
  }

  function reopenFeishuOnboarding() {
    setFeishuOnboardingPanelMode("guided");
    setFeishuOnboardingSkippedSignature(null);
  }

  function skipFeishuOnboarding(signature: string) {
    setFeishuOnboardingPanelMode("skipped");
    setFeishuOnboardingSkippedSignature(signature);
  }

  const actions = createFeishuSettingsControllerActions({
    feishuConnectorSettings,
    feishuAdvancedSettings,
    feishuInstallerInput,
    pluginChannelHosts,
    feishuSetupProgress,
    setFeishuConnectorSettings,
    setFeishuAdvancedSettings,
    setPluginChannelHosts,
    setPluginChannelHostsError,
    setValidatingFeishuCredentials,
    setFeishuCredentialProbe,
    setFeishuInstallerSession,
    setFeishuInstallerInput,
    setFeishuInstallerBusy,
    setFeishuInstallerStartingMode,
    setFeishuPairingRequests,
    setFeishuPairingRequestsError,
    setFeishuPairingActionLoading,
    setSavingFeishuConnector,
    setSavingFeishuAdvancedSettings,
    setRetryingFeishuConnector,
    setInstallingOfficialFeishuPlugin,
    setFeishuConnectorNotice,
    setFeishuConnectorError,
    setOfficialFeishuRuntimeStatus,
    loadConnectorStatuses,
    loadFeishuSetupProgress,
  });

  useEffect(() => {
    if (activeTab !== "feishu") {
      return;
    }

    void Promise.all([
      actions.loadConnectorSettings(),
      actions.loadConnectorPlatformData(),
    ]);
  }, [activeTab]);

  useEffect(() => {
    if (activeTab !== "feishu") {
      return;
    }

    const timer = window.setInterval(() => {
      void actions.loadConnectorPlatformData();
    }, 5000);

    return () => window.clearInterval(timer);
  }, [activeTab]);

  useEffect(() => {
    if (activeTab !== "feishu" || !feishuInstallerSession.running) {
      return;
    }

    const timer = window.setInterval(() => {
      void Promise.all([
        actions.loadConnectorSettings(),
        loadFeishuSetupProgress(),
      ]);
    }, 1500);

    return () => window.clearInterval(timer);
  }, [activeTab, feishuInstallerSession.running, loadFeishuSetupProgress]);

  useEffect(() => {
    if (activeTab !== "feishu") {
      return;
    }

    const completionNotice = resolveFeishuInstallerCompletionNotice(feishuInstallerSession);
    if (!completionNotice) {
      return;
    }

    const completionKey = [
      feishuInstallerSession.mode ?? "",
      feishuInstallerSession.last_output_at ?? "",
      feishuInstallerSession.last_error ?? "",
      getLatestInstallerOutputLine(feishuInstallerSession),
    ].join("|");
    if (!completionKey || handledFeishuInstallerCompletionRef.current === completionKey) {
      return;
    }
    handledFeishuInstallerCompletionRef.current = completionKey;

    void Promise.all([
      actions.loadConnectorSettings(),
      loadConnectorStatuses(),
      loadFeishuSetupProgress(),
    ]).finally(() => {
      setFeishuConnectorNotice(completionNotice);
    });
  }, [activeTab, feishuInstallerSession, loadConnectorStatuses, loadFeishuSetupProgress]);

  const viewModel = buildFeishuSettingsControllerViewModel({
    feishuConnectorSettings,
    feishuAdvancedSettings,
    pluginChannelHosts,
    feishuEnvironmentStatus,
    feishuSetupProgress,
    officialFeishuRuntimeStatus,
    feishuCredentialProbe,
    feishuInstallerSession,
    feishuInstallerInput,
    feishuInstallerBusy,
    feishuInstallerStartingMode,
    feishuPairingRequests,
    feishuPairingActionLoading,
    savingFeishuConnector,
    savingFeishuAdvancedSettings,
    retryingFeishuConnector,
    installingOfficialFeishuPlugin,
    validatingFeishuCredentials,
    feishuConnectorNotice,
    feishuConnectorError,
    feishuOnboardingPanelMode,
    feishuOnboardingSelectedPath,
    feishuOnboardingSkippedSignature,
    actions: {
      updateFeishuConnectorSettings: actions.updateFeishuConnectorSettings,
      updateFeishuAdvancedSettings: actions.updateFeishuAdvancedSettings,
      updateFeishuInstallerInput: actions.updateFeishuInstallerInput,
      openFeishuOnboardingPath,
      reopenFeishuOnboarding,
      skipFeishuOnboarding,
      handleRefreshFeishuSetup: actions.handleRefreshFeishuSetup,
      handleOpenFeishuOfficialDocs: actions.handleOpenFeishuOfficialDocs,
      handleValidateFeishuCredentials: actions.handleValidateFeishuCredentials,
      handleSaveFeishuConnector: actions.handleSaveFeishuConnector,
      handleInstallOfficialFeishuPlugin: actions.handleInstallOfficialFeishuPlugin,
      handleInstallAndStartFeishuConnector: actions.handleInstallAndStartFeishuConnector,
      handleResolveFeishuPairingRequest: actions.handleResolveFeishuPairingRequest,
      handleStartFeishuInstaller: actions.handleStartFeishuInstaller,
      handleStopFeishuInstallerSession: actions.handleStopFeishuInstallerSession,
      handleSendFeishuInstallerInput: actions.handleSendFeishuInstallerInput,
      handleSaveFeishuAdvancedSettings: actions.handleSaveFeishuAdvancedSettings,
      handleCopyFeishuDiagnostics,
    },
  });

  useEffect(() => {
    if (
      feishuOnboardingPanelMode === "skipped" &&
      feishuOnboardingSkippedSignature &&
      feishuOnboardingSkippedSignature !==
        viewModel.sections.settingsSectionProps.feishuOnboardingProgressSignature
    ) {
      setFeishuOnboardingPanelMode("guided");
      setFeishuOnboardingSkippedSignature(null);
    }
  }, [
    feishuOnboardingPanelMode,
    feishuOnboardingSkippedSignature,
    viewModel.sections.settingsSectionProps.feishuOnboardingProgressSignature,
  ]);

  async function handleCopyFeishuDiagnostics() {
    try {
      await navigator?.clipboard?.writeText?.(
        buildFeishuDiagnosticsClipboardText({
          connectorStatus: {
            running: officialFeishuRuntimeStatus?.running === true,
            lastError: officialFeishuRuntimeStatus?.last_error ?? "",
            hasInstalledOfficialFeishuPlugin: viewModel.hasInstalledOfficialFeishuPlugin,
          },
          pluginVersion:
            feishuSetupProgress?.plugin_version || viewModel.primaryPluginChannelHost?.version || "未识别",
          defaultAccountId: officialFeishuRuntimeStatus?.account_id || "未识别",
          authApproved: feishuSetupProgress?.auth_status === "approved",
          defaultRoutingEmployeeName: feishuSetupProgress?.default_routing_employee_name || "未设置",
          scopedRoutingCount: feishuSetupProgress?.scoped_routing_count ?? 0,
          pendingPairings: viewModel.pendingFeishuPairingCount,
          lastEventAt: officialFeishuRuntimeStatus?.last_event_at,
          runtimeStatus: officialFeishuRuntimeStatus,
          pluginChannelHosts: pluginChannelHosts.length,
          pluginInstalled: feishuSetupProgress?.plugin_installed === true,
        }),
      );
      setFeishuConnectorNotice("连接诊断摘要已复制");
    } catch (error) {
      setFeishuConnectorError("复制连接诊断摘要失败: " + String(error));
    }
  }

  return {
    sections: viewModel.sections,
  };
}
