import { useEffect, useRef, useState } from "react";
import { openExternalUrl } from "../../../utils/openExternalUrl";
import { type SettingsTabName } from "../SettingsTabNav";
import {
  buildFeishuOnboardingState,
  buildFeishuDiagnosticsClipboardText,
  extractFeishuInstallerQrBlock,
  formatCompactDateTime,
  getFeishuAuthorizationAction,
  getFeishuConnectionDetailSummary,
  getFeishuEnvironmentLabel,
  getFeishuRoutingStatus,
  getFeishuSetupSummary,
  getLatestInstallerOutputLine,
  resolveFeishuAuthorizationInlineError,
  resolveFeishuConnectorStatus,
  resolveFeishuGuidedInlineError,
  resolveFeishuGuidedInlineNotice,
  resolveFeishuInstallerFlowLabel,
  resolveFeishuInstallerCompletionNotice,
  resolveFeishuOnboardingPanelDisplay,
  sanitizeFeishuInstallerDisplayLines,
  shouldShowFeishuInstallerGuidedPanel,
  summarizeConnectorIssue,
  summarizeOfficialFeishuRuntimeLogs,
} from "./feishuSelectors";
import {
  approveFeishuPairingRequest as approveFeishuPairingRequestFromService,
  denyFeishuPairingRequest as denyFeishuPairingRequestFromService,
  getFeishuErrorMessage,
  installOpenClawLarkPlugin as installOpenClawLarkPluginFromService,
  loadFeishuAdvancedSettings as loadFeishuAdvancedSettingsFromService,
  loadFeishuGatewaySettings as loadFeishuGatewaySettingsFromService,
  loadFeishuInstallerSessionStatus as loadFeishuInstallerSessionStatusFromService,
  loadFeishuPairingRequests as loadFeishuPairingRequestsFromService,
  loadFeishuPluginChannelHosts as loadFeishuPluginChannelHostsFromService,
  loadFeishuPluginChannelSnapshot as loadFeishuPluginChannelSnapshotFromService,
  loadFeishuRuntimeStatus as loadFeishuRuntimeStatusFromService,
  loadFeishuSetupProgress as loadFeishuSetupProgressFromService,
  normalizeFeishuAdvancedSettings,
  normalizeFeishuGatewaySettings,
  probeFeishuCredentials as probeFeishuCredentialsFromService,
  saveFeishuAdvancedSettings as saveFeishuAdvancedSettingsFromService,
  saveFeishuGatewaySettings as saveFeishuGatewaySettingsFromService,
  sendFeishuInstallerInput as sendFeishuInstallerInputFromService,
  startFeishuInstallerSession as startFeishuInstallerSessionFromService,
  startFeishuRuntime as startFeishuRuntimeFromService,
  stopFeishuInstallerSession as stopFeishuInstallerSessionFromService,
} from "./feishuSettingsService";
import type {
  FeishuGatewaySettings,
  FeishuPairingRequestRecord,
  FeishuPluginEnvironmentStatus,
  FeishuSetupProgress,
  OpenClawLarkInstallerMode,
  OpenClawLarkInstallerSessionStatus,
  OpenClawPluginChannelHost,
  OpenClawPluginChannelSnapshotResult,
  OpenClawPluginFeishuAdvancedSettings,
  OpenClawPluginFeishuCredentialProbeResult,
  OpenClawPluginFeishuRuntimeStatus,
} from "../../../types";

const FEISHU_OFFICIAL_PLUGIN_DOC_URL =
  "https://bytedance.larkoffice.com/docx/MFK7dDFLFoVlOGxWCv5cTXKmnMh#M0usd9GLwoiBxtx1UyjcpeMhnRe";

const DEFAULT_FEISHU_INSTALLER_SESSION: OpenClawLarkInstallerSessionStatus = {
  running: false,
  mode: null,
  started_at: null,
  last_output_at: null,
  last_error: null,
  prompt_hint: null,
  recent_output: [],
};

interface UseFeishuSettingsControllerOptions {
  activeTab: SettingsTabName;
}

export function useFeishuSettingsController({
  activeTab,
}: UseFeishuSettingsControllerOptions) {
  const [feishuConnectorSettings, setFeishuConnectorSettings] = useState<FeishuGatewaySettings>({
    app_id: "",
    app_secret: "",
    ingress_token: "",
    encrypt_key: "",
    sidecar_base_url: "",
  });
  const [feishuAdvancedSettings, setFeishuAdvancedSettings] = useState<OpenClawPluginFeishuAdvancedSettings>({
    groups_json: "",
    dms_json: "",
    footer_json: "",
    account_overrides_json: "",
    render_mode: "",
    streaming: "",
    text_chunk_limit: "",
    chunk_mode: "",
    reply_in_thread: "",
    group_session_scope: "",
    topic_session_mode: "",
    markdown_mode: "",
    markdown_table_mode: "",
    heartbeat_visibility: "",
    heartbeat_interval_ms: "",
    media_max_mb: "",
    http_timeout_ms: "",
    config_writes: "",
    webhook_host: "",
    webhook_port: "",
    dynamic_agent_creation_enabled: "",
    dynamic_agent_creation_workspace_template: "",
    dynamic_agent_creation_agent_dir_template: "",
    dynamic_agent_creation_max_agents: "",
  });
  const [officialFeishuRuntimeStatus, setOfficialFeishuRuntimeStatus] =
    useState<OpenClawPluginFeishuRuntimeStatus | null>(null);
  const [pluginChannelHosts, setPluginChannelHosts] = useState<OpenClawPluginChannelHost[]>([]);
  const [pluginChannelSnapshots, setPluginChannelSnapshots] =
    useState<Record<string, OpenClawPluginChannelSnapshotResult>>({});
  const [pluginChannelHostsError, setPluginChannelHostsError] = useState("");
  const [pluginChannelSnapshotsError, setPluginChannelSnapshotsError] = useState("");
  const [feishuEnvironmentStatus, setFeishuEnvironmentStatus] = useState<FeishuPluginEnvironmentStatus | null>(null);
  const [feishuSetupProgress, setFeishuSetupProgress] = useState<FeishuSetupProgress | null>(null);
  const [validatingFeishuCredentials, setValidatingFeishuCredentials] = useState(false);
  const [feishuCredentialProbe, setFeishuCredentialProbe] =
    useState<OpenClawPluginFeishuCredentialProbeResult | null>(null);
  const [feishuInstallerSession, setFeishuInstallerSession] = useState<OpenClawLarkInstallerSessionStatus>(
    DEFAULT_FEISHU_INSTALLER_SESSION,
  );
  const [feishuInstallerInput, setFeishuInstallerInput] = useState("");
  const [feishuInstallerBusy, setFeishuInstallerBusy] = useState(false);
  const [feishuInstallerStartingMode, setFeishuInstallerStartingMode] = useState<OpenClawLarkInstallerMode | null>(null);
  const handledFeishuInstallerCompletionRef = useRef("");
  const [feishuPairingRequests, setFeishuPairingRequests] = useState<FeishuPairingRequestRecord[]>([]);
  const [feishuPairingRequestsError, setFeishuPairingRequestsError] = useState("");
  const [feishuPairingActionLoading, setFeishuPairingActionLoading] = useState<"approve" | "deny" | null>(null);
  const [savingFeishuConnector, setSavingFeishuConnector] = useState(false);
  const [savingFeishuAdvancedSettings, setSavingFeishuAdvancedSettings] = useState(false);
  const [retryingFeishuConnector, setRetryingFeishuConnector] = useState(false);
  const [installingOfficialFeishuPlugin, setInstallingOfficialFeishuPlugin] = useState(false);
  const [feishuConnectorNotice, setFeishuConnectorNotice] = useState("");
  const [feishuConnectorError, setFeishuConnectorError] = useState("");
  const [feishuOnboardingPanelMode, setFeishuOnboardingPanelMode] = useState<"guided" | "skipped">("guided");
  const [feishuOnboardingSelectedPath, setFeishuOnboardingSelectedPath] = useState<
    "existing_robot" | "create_robot" | null
  >(null);
  const [feishuOnboardingSkippedSignature, setFeishuOnboardingSkippedSignature] = useState<string | null>(null);

  useEffect(() => {
    if (activeTab !== "feishu") {
      return;
    }

    void Promise.all([
      loadConnectorSettings(),
      loadConnectorStatuses(),
      loadConnectorPlatformData(),
      loadFeishuSetupProgress(),
      loadFeishuInstallerSessionStatus(),
    ]);
  }, [activeTab]);

  useEffect(() => {
    if (activeTab !== "feishu") {
      return;
    }

    const timer = window.setInterval(() => {
      void Promise.all([
        loadConnectorStatuses(),
        loadConnectorPlatformData(),
        loadFeishuSetupProgress(),
      ]);
    }, 5000);

    return () => window.clearInterval(timer);
  }, [activeTab]);

  useEffect(() => {
    if (activeTab !== "feishu" || !feishuInstallerSession.running) {
      return;
    }

    const timer = window.setInterval(() => {
      void Promise.all([
        loadFeishuInstallerSessionStatus(),
        loadConnectorSettings(),
        loadFeishuSetupProgress(),
      ]);
    }, 1500);

    return () => window.clearInterval(timer);
  }, [activeTab, feishuInstallerSession.running]);

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
      loadConnectorSettings(),
      loadConnectorStatuses(),
      loadFeishuSetupProgress(),
    ]).finally(() => {
      setFeishuConnectorNotice(completionNotice);
    });
  }, [activeTab, feishuInstallerSession]);

  useEffect(() => {
    const onboardingState = buildFeishuOnboardingState({
      summaryState: feishuSetupProgress?.summary_state ?? null,
      setupProgress: feishuSetupProgress,
      installerMode: feishuInstallerSession?.mode ?? null,
    });
    const progressSignature = [
      onboardingState.currentStep,
      onboardingState.mode,
      onboardingState.canContinue ? "continue" : "blocked",
      onboardingState.skipped ? "backend-skipped" : "active",
    ].join("|");

    if (
      feishuOnboardingPanelMode === "skipped" &&
      feishuOnboardingSkippedSignature &&
      feishuOnboardingSkippedSignature !== progressSignature
    ) {
      setFeishuOnboardingPanelMode("guided");
      setFeishuOnboardingSkippedSignature(null);
    }
  }, [feishuOnboardingPanelMode, feishuOnboardingSkippedSignature, feishuSetupProgress, feishuInstallerSession]);

  async function loadConnectorSettings() {
    try {
      const [feishuSettings, feishuAdvanced] = await Promise.all([
        loadFeishuGatewaySettingsFromService(),
        loadFeishuAdvancedSettingsFromService(),
      ]);
      setFeishuConnectorSettings(normalizeFeishuGatewaySettings(feishuSettings));
      setFeishuAdvancedSettings(normalizeFeishuAdvancedSettings(feishuAdvanced));
    } catch (error) {
      console.warn("加载渠道连接器配置失败:", error);
    }
  }

  async function loadFeishuSetupProgress() {
    try {
      const progress = await loadFeishuSetupProgressFromService();
      if (progress) {
        setFeishuEnvironmentStatus(progress.environment ?? null);
        setFeishuSetupProgress(progress);
      } else {
        setFeishuEnvironmentStatus(null);
        setFeishuSetupProgress(null);
      }
    } catch (error) {
      console.warn("加载飞书接入进度失败:", error);
      setFeishuEnvironmentStatus(null);
      setFeishuSetupProgress(null);
    }
  }

  async function loadConnectorStatuses() {
    try {
      const runtimeStatus = await loadFeishuRuntimeStatusFromService();
      setOfficialFeishuRuntimeStatus(runtimeStatus);
    } catch (error) {
      console.warn("加载渠道连接器状态失败:", error);
      setOfficialFeishuRuntimeStatus(null);
    }
  }

  async function loadFeishuInstallerSessionStatus() {
    try {
      const status = await loadFeishuInstallerSessionStatusFromService();
      setFeishuInstallerSession(status ?? DEFAULT_FEISHU_INSTALLER_SESSION);
    } catch (error) {
      console.warn("加载飞书官方安装向导状态失败:", error);
    }
  }

  async function loadConnectorPlatformData() {
    const [hostsResult, pairingResult] = await Promise.allSettled([
      loadFeishuPluginChannelHostsFromService(),
      loadFeishuPairingRequestsFromService(),
    ]);

    const normalizedHosts =
      hostsResult.status === "fulfilled"
        ? (Array.isArray(hostsResult.value) ? hostsResult.value : []).filter(
            (host) =>
              host.channel === "feishu" ||
              host.plugin_id === "openclaw-lark" ||
              host.npm_spec === "@larksuite/openclaw-lark" ||
              host.display_name.toLowerCase().includes("feishu") ||
              host.display_name.toLowerCase().includes("lark"),
          )
        : [];
    if (hostsResult.status !== "fulfilled") {
      console.warn("加载官方插件宿主失败:", hostsResult.reason);
    }
    setPluginChannelHosts(normalizedHosts);
    setPluginChannelHostsError(hostsResult.status === "fulfilled" ? "" : "官方插件状态暂时不可用");

    if (pairingResult.status !== "fulfilled") {
      console.warn("加载飞书配对请求失败:", pairingResult.reason);
    }
    setFeishuPairingRequests(pairingResult.status === "fulfilled" && Array.isArray(pairingResult.value) ? pairingResult.value : []);
    setFeishuPairingRequestsError(pairingResult.status === "fulfilled" ? "" : "配对记录加载失败");

    if (normalizedHosts.length === 0) {
      setPluginChannelSnapshots({});
      setPluginChannelSnapshotsError("");
      return;
    }

    const snapshotResults = await Promise.allSettled(
      normalizedHosts.map((host) => loadFeishuPluginChannelSnapshotFromService(host.plugin_id)),
    );
    const nextSnapshots: Record<string, OpenClawPluginChannelSnapshotResult> = {};
    for (const result of snapshotResults) {
      if (result.status !== "fulfilled") {
        continue;
      }
      nextSnapshots[result.value.snapshot.channelId || result.value.entryPath] = result.value;
    }
    setPluginChannelSnapshots(nextSnapshots);
    setPluginChannelSnapshotsError(snapshotResults.some((result) => result.status !== "fulfilled") ? "部分账号快照暂时不可用" : "");
  }

  function applyOfficialFeishuRuntimeStatus(
    status: OpenClawPluginFeishuRuntimeStatus | null | undefined,
    options?: { showStartErrorNotice?: boolean },
  ) {
    if (!status) {
      return;
    }
    setOfficialFeishuRuntimeStatus(status);
    if (options?.showStartErrorNotice && !status.running && status.last_error?.trim()) {
      setFeishuConnectorError(`官方插件启动失败: ${status.last_error.trim()}`);
    }
  }

  function updateFeishuConnectorSettings(patch: Partial<FeishuGatewaySettings>) {
    setFeishuConnectorSettings((state) => ({
      ...state,
      ...patch,
    }));
  }

  function updateFeishuAdvancedSettings(patch: Partial<OpenClawPluginFeishuAdvancedSettings>) {
    setFeishuAdvancedSettings((state) => ({
      ...state,
      ...patch,
    }));
  }

  function updateFeishuInstallerInput(value: string) {
    setFeishuInstallerInput(value);
  }

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

  async function handleValidateFeishuCredentials() {
    const appId = feishuConnectorSettings.app_id.trim();
    const appSecret = feishuConnectorSettings.app_secret.trim();
    if (!appId || !appSecret) {
      setFeishuConnectorError("请先填写已有机器人的 App ID 和 App Secret");
      return;
    }

    setValidatingFeishuCredentials(true);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      const probe = await probeFeishuCredentialsFromService(appId, appSecret);
      if (!probe.ok) {
        setFeishuCredentialProbe(null);
        setFeishuConnectorError(`已有机器人校验失败: ${probe.error || "无法获取机器人信息"}`);
        return;
      }
      setFeishuCredentialProbe(probe);
      const botLabel = probe.bot_name?.trim() ? `（${probe.bot_name.trim()}）` : "";
      setFeishuConnectorNotice(`机器人信息验证成功${botLabel}`);
    } catch (error) {
      setFeishuCredentialProbe(null);
      setFeishuConnectorError("验证机器人信息失败: " + String(error));
    } finally {
      setValidatingFeishuCredentials(false);
    }
  }

  async function handleSaveFeishuConnector() {
    setSavingFeishuConnector(true);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      const saved = await saveFeishuGatewaySettingsFromService(feishuConnectorSettings);
      setFeishuConnectorSettings(saved);
      await loadConnectorStatuses();
      await loadConnectorPlatformData();
      await loadFeishuSetupProgress();
      setFeishuConnectorNotice("飞书官方插件配置已保存");
    } catch (error) {
      setFeishuConnectorError("保存飞书官方插件配置失败: " + String(error));
    } finally {
      setSavingFeishuConnector(false);
    }
  }

  async function handleSaveFeishuAdvancedSettings() {
    setSavingFeishuAdvancedSettings(true);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      const saved = await saveFeishuAdvancedSettingsFromService(feishuAdvancedSettings);
      setFeishuAdvancedSettings(saved);
      await loadConnectorPlatformData();
      await loadFeishuSetupProgress();
      setFeishuConnectorNotice("飞书高级配置已保存");
    } catch (error) {
      setFeishuConnectorError("保存飞书高级配置失败: " + String(error));
    } finally {
      setSavingFeishuAdvancedSettings(false);
    }
  }

  async function handleStartFeishuInstaller(mode: OpenClawLarkInstallerMode) {
    setFeishuInstallerBusy(true);
    setFeishuInstallerStartingMode(mode);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      if (!pluginChannelHosts.some((host) => host.status === "ready") && !feishuSetupProgress?.plugin_installed) {
        await installOpenClawLarkPluginFromService();
      }
      const status = await startFeishuInstallerSessionFromService(
        mode,
        mode === "link" ? feishuConnectorSettings.app_id.trim() : null,
        mode === "link" ? feishuConnectorSettings.app_secret.trim() : null,
      );
      setFeishuInstallerSession(status ?? DEFAULT_FEISHU_INSTALLER_SESSION);
      setFeishuInstallerInput("");
      await loadConnectorPlatformData();
      await loadFeishuSetupProgress();
      setFeishuConnectorNotice(mode === "create" ? "已启动飞书官方创建机器人向导" : "已启动飞书官方绑定机器人向导");
    } catch (error) {
      setFeishuConnectorError(
        `${mode === "create" ? "启动飞书官方创建机器人向导" : "启动飞书官方绑定机器人向导"}失败: ${String(error)}`,
      );
    } finally {
      setFeishuInstallerBusy(false);
      setFeishuInstallerStartingMode(null);
    }
  }

  async function handleSendFeishuInstallerInput() {
    const input = feishuInstallerInput.trim();
    if (!input) return;
    setFeishuInstallerBusy(true);
    setFeishuConnectorError("");
    try {
      const status = await sendFeishuInstallerInputFromService(input);
      setFeishuInstallerSession(status ?? DEFAULT_FEISHU_INSTALLER_SESSION);
      setFeishuInstallerInput("");
    } catch (error) {
      setFeishuConnectorError("发送安装向导输入失败: " + String(error));
    } finally {
      setFeishuInstallerBusy(false);
    }
  }

  async function handleStopFeishuInstallerSession() {
    setFeishuInstallerBusy(true);
    setFeishuConnectorError("");
    try {
      const status = await stopFeishuInstallerSessionFromService();
      setFeishuInstallerSession(status ?? DEFAULT_FEISHU_INSTALLER_SESSION);
      setFeishuConnectorNotice("已停止飞书官方安装向导");
    } catch (error) {
      setFeishuConnectorError("停止飞书官方安装向导失败: " + String(error));
    } finally {
      setFeishuInstallerBusy(false);
    }
  }

  async function handleRetryFeishuConnector() {
    setRetryingFeishuConnector(true);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      const runtimeStatus = await startFeishuRuntimeFromService(
        pluginChannelHosts.find((host) => host.channel === "feishu")?.plugin_id || "openclaw-lark",
        null,
      );
      if (runtimeStatus) {
        applyOfficialFeishuRuntimeStatus(runtimeStatus, { showStartErrorNotice: true });
      } else {
        await loadConnectorStatuses();
      }
      await loadConnectorPlatformData();
      await loadFeishuSetupProgress();
      setFeishuConnectorNotice(
        runtimeStatus ? (runtimeStatus.running ? "已触发飞书官方插件启动" : "已刷新飞书官方插件状态") : "已触发飞书官方插件启动",
      );
    } catch (error) {
      setFeishuConnectorError("刷新飞书官方插件状态失败: " + String(error));
    } finally {
      setRetryingFeishuConnector(false);
    }
  }

  async function handleInstallOfficialFeishuPlugin() {
    setInstallingOfficialFeishuPlugin(true);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      await installOpenClawLarkPluginFromService();
      await loadConnectorPlatformData();
      await loadFeishuSetupProgress();
      setFeishuConnectorNotice("飞书官方插件已安装");
    } catch (error) {
      setFeishuConnectorError("安装飞书官方插件失败: " + String(error));
    } finally {
      setInstallingOfficialFeishuPlugin(false);
    }
  }

  async function handleResolveFeishuPairingRequest(requestId: string, action: "approve" | "deny") {
    setFeishuPairingActionLoading(action);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      if (action === "approve") {
        await approveFeishuPairingRequestFromService(requestId);
      } else {
        await denyFeishuPairingRequestFromService(requestId);
      }
      await loadConnectorPlatformData();
      await loadFeishuSetupProgress();
      setFeishuConnectorNotice(action === "approve" ? "已批准飞书接入请求" : "已拒绝飞书接入请求");
    } catch (error) {
      setFeishuConnectorError(`${action === "approve" ? "批准" : "拒绝"}飞书接入请求失败: ${String(error)}`);
    } finally {
      setFeishuPairingActionLoading(null);
    }
  }

  async function handleInstallAndStartFeishuConnector() {
    setRetryingFeishuConnector(true);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      if (!feishuConnectorSettings.app_id.trim() || !feishuConnectorSettings.app_secret.trim()) {
        setFeishuConnectorError("请先填写并保存已有机器人的 App ID 和 App Secret");
        return;
      }

      const saved = await saveFeishuGatewaySettingsFromService(feishuConnectorSettings);
      setFeishuConnectorSettings(saved);

      if (!pluginChannelHosts.some((host) => host.status === "ready") && !feishuSetupProgress?.plugin_installed) {
        await installOpenClawLarkPluginFromService();
      }

      const runtimeStatus = await startFeishuRuntimeFromService(
        pluginChannelHosts.find((host) => host.channel === "feishu")?.plugin_id || "openclaw-lark",
        null,
      );
      if (runtimeStatus) {
        applyOfficialFeishuRuntimeStatus(runtimeStatus, { showStartErrorNotice: true });
      }
      await loadConnectorStatuses();
      await loadConnectorPlatformData();
      await loadFeishuSetupProgress();
      setFeishuConnectorNotice(runtimeStatus?.running ? "飞书连接组件已启动" : "已尝试启动飞书连接组件");
    } catch (error) {
      setFeishuConnectorError("安装并启动飞书连接失败: " + String(error));
    } finally {
      setRetryingFeishuConnector(false);
    }
  }

  async function handleOpenFeishuOfficialDocs() {
    try {
      await openExternalUrl(FEISHU_OFFICIAL_PLUGIN_DOC_URL);
    } catch (error) {
      setFeishuConnectorError(getFeishuErrorMessage(error, "打开官方文档失败，请稍后重试"));
    }
  }

  async function handleCopyFeishuDiagnostics() {
    try {
      await navigator?.clipboard?.writeText?.(
        buildFeishuDiagnosticsClipboardText({
          connectorStatus: {
            running: officialFeishuRuntimeStatus?.running === true,
            lastError: officialFeishuRuntimeStatus?.last_error ?? "",
            hasInstalledOfficialFeishuPlugin:
              pluginChannelHosts.length > 0 || feishuSetupProgress?.plugin_installed === true,
          },
          pluginVersion: feishuSetupProgress?.plugin_version || pluginChannelHosts[0]?.version || "未识别",
          defaultAccountId: Object.values(pluginChannelSnapshots)[0]?.snapshot.defaultAccountId || "未识别",
          authApproved: feishuSetupProgress?.auth_status === "approved",
          defaultRoutingEmployeeName: feishuSetupProgress?.default_routing_employee_name || "未设置",
          scopedRoutingCount: feishuSetupProgress?.scoped_routing_count ?? 0,
          lastEventAt: officialFeishuRuntimeStatus?.last_event_at,
          runtimeStatus: officialFeishuRuntimeStatus,
          pluginChannelHosts: pluginChannelHosts.length,
          pluginInstalled: feishuSetupProgress?.plugin_installed === true,
        }),
      );
      setFeishuConnectorNotice("连接诊断摘要已复制");
    } catch (error) {
      setFeishuConnectorError(getFeishuErrorMessage(error, "复制连接诊断摘要失败，请稍后重试"));
    }
  }

  async function handleRefreshFeishuSetup() {
    setRetryingFeishuConnector(true);
    setFeishuConnectorNotice("");
    setFeishuConnectorError("");
    try {
      await Promise.all([
        loadConnectorSettings(),
        loadConnectorStatuses(),
        loadConnectorPlatformData(),
        loadFeishuSetupProgress(),
      ]);
    } catch (error) {
      setFeishuConnectorError("刷新飞书接入状态失败: " + String(error));
    } finally {
      setRetryingFeishuConnector(false);
    }
  }

  const primaryPluginChannelHost =
    pluginChannelHosts.find((host) => host.channel === "feishu") ?? pluginChannelHosts[0] ?? null;
  const primaryPluginChannelSnapshot =
    (primaryPluginChannelHost ? pluginChannelSnapshots[primaryPluginChannelHost.channel] : null) ??
    Object.values(pluginChannelSnapshots)[0] ??
    null;
  const hasInstalledOfficialFeishuPlugin =
    pluginChannelHosts.length > 0 || feishuSetupProgress?.plugin_installed === true;
  const runtimeRunning =
    officialFeishuRuntimeStatus?.running === true || feishuSetupProgress?.runtime_running === true;
  const pendingFeishuPairingCount =
    feishuSetupProgress?.pending_pairings ??
    feishuPairingRequests.filter((request) => request.status === "pending").length;
  const pendingFeishuPairingRequest =
    feishuPairingRequests.find((request) => request.status === "pending") ?? null;
  const feishuOnboardingState = buildFeishuOnboardingState({
    summaryState: feishuSetupProgress?.summary_state ?? null,
    setupProgress: feishuSetupProgress,
    installerMode: feishuInstallerSession?.mode ?? null,
  });
  const feishuOnboardingProgressSignature = [
    feishuOnboardingState.currentStep,
    feishuOnboardingState.mode,
    feishuOnboardingState.canContinue ? "continue" : "blocked",
    feishuOnboardingState.skipped ? "backend-skipped" : "active",
  ].join("|");
  const feishuOnboardingIsSkipped =
    feishuOnboardingState.skipped ||
    (feishuOnboardingPanelMode === "skipped" && feishuOnboardingSkippedSignature === feishuOnboardingProgressSignature);
  const feishuOnboardingBackendBranch =
    feishuOnboardingState.currentStep === "existing_robot" || feishuOnboardingState.currentStep === "create_robot"
      ? feishuOnboardingState.currentStep
      : null;
  const feishuOnboardingEffectiveBranch = feishuOnboardingSelectedPath ?? feishuOnboardingBackendBranch;
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
    feishuInstallerSession,
  );
  const feishuGuidedInlineError = resolveFeishuGuidedInlineError(
    feishuConnectorError,
    feishuOnboardingHeaderStep,
    feishuOnboardingEffectiveBranch,
  );
  const feishuGuidedInlineNotice = resolveFeishuGuidedInlineNotice(
    feishuConnectorNotice,
    feishuOnboardingHeaderStep,
    feishuOnboardingEffectiveBranch,
  );
  const feishuAuthorizationInlineError = resolveFeishuAuthorizationInlineError(feishuConnectorError);
  const feishuInstallerDisplayMode =
    feishuInstallerSession.mode ??
    (feishuOnboardingEffectiveBranch === "create_robot" ? "create" : feishuOnboardingEffectiveBranch ? "link" : null);
  const feishuInstallerFlowLabel = resolveFeishuInstallerFlowLabel(feishuInstallerDisplayMode);
  const feishuInstallerQrBlock = extractFeishuInstallerQrBlock(feishuInstallerSession.recent_output ?? []);
  const feishuInstallerDisplayLines = sanitizeFeishuInstallerDisplayLines(feishuInstallerSession.recent_output ?? []);
  const feishuInstallerStartupHint =
    feishuInstallerBusy && feishuInstallerStartingMode
      ? `正在启动${resolveFeishuInstallerFlowLabel(feishuInstallerStartingMode)}，请稍候...`
      : feishuInstallerSession.prompt_hint || null;
  const feishuAuthorizationAction = getFeishuAuthorizationAction({
    runtimeRunning,
    pluginInstalled: feishuSetupProgress?.plugin_installed === true,
  });
  const feishuRoutingStatus = getFeishuRoutingStatus({
    authApproved: feishuSetupProgress?.auth_status === "approved",
    defaultRoutingEmployeeName: feishuSetupProgress?.default_routing_employee_name,
    scopedRoutingCount: feishuSetupProgress?.scoped_routing_count,
  });
  const feishuSetupSummary = getFeishuSetupSummary({
    skipped: feishuOnboardingState.skipped,
    summaryState: feishuSetupProgress?.summary_state ?? null,
    runtimeRunning,
    authApproved: feishuSetupProgress?.auth_status === "approved",
    runtimeLastError: feishuSetupProgress?.runtime_last_error,
    officialRuntimeLastError: officialFeishuRuntimeStatus?.last_error,
  });
  const feishuConnectorStatus = resolveFeishuConnectorStatus({
    running: runtimeRunning,
    lastError: officialFeishuRuntimeStatus?.last_error ?? "",
    hasInstalledOfficialFeishuPlugin,
  });
  const feishuConnectionDetailSummary = getFeishuConnectionDetailSummary({
    connectorStatus: feishuConnectorStatus,
    runtimeRunning,
    authApproved: feishuSetupProgress?.auth_status === "approved",
    defaultRoutingEmployeeName: feishuSetupProgress?.default_routing_employee_name,
    scopedRoutingCount: feishuSetupProgress?.scoped_routing_count,
  });

  const settingsSectionProps = {
    feishuConnectorSettings,
    onUpdateFeishuConnectorSettings: updateFeishuConnectorSettings,
    feishuEnvironmentStatus,
    feishuSetupProgress,
    validatingFeishuCredentials,
    feishuCredentialProbe,
    feishuInstallerSession,
    feishuInstallerInput,
    onUpdateFeishuInstallerInput: updateFeishuInstallerInput,
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
    onOpenFeishuOnboardingPath: openFeishuOnboardingPath,
    onReopenFeishuOnboarding: reopenFeishuOnboarding,
    onSkipFeishuOnboarding: skipFeishuOnboarding,
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
    feishuRoutingActionAvailable: feishuSetupProgress?.auth_status === "approved",
    feishuOnboardingPrimaryActionLabel: feishuOnboardingIsSkipped
      ? "重新打开引导"
      : feishuOnboardingHeaderStep === "environment"
        ? retryingFeishuConnector
          ? "检测中..."
          : "重新检测环境"
        : feishuOnboardingHeaderStep === "plugin"
          ? installingOfficialFeishuPlugin
            ? "安装中..."
            : "安装官方插件"
          : feishuOnboardingHeaderStep === "create_robot"
            ? feishuInstallerBusy && feishuInstallerStartingMode === "create"
              ? "启动中..."
              : feishuOnboardingPanelDisplay.primaryActionLabel
              : feishuOnboardingHeaderStep === "authorize"
                ? retryingFeishuConnector || installingOfficialFeishuPlugin
                  ? feishuAuthorizationAction.busyLabel
                  : feishuSetupProgress?.plugin_installed
                    ? "启动连接"
                    : "安装并启动"
              : feishuOnboardingHeaderStep === "approve_pairing"
                ? feishuPairingActionLoading === "approve"
                  ? "批准中..."
                  : "批准这次接入"
                : feishuOnboardingHeaderStep === "routing"
                  ? feishuRoutingStatus.actionLabel
                  : feishuOnboardingPanelDisplay.primaryActionLabel,
    feishuOnboardingPrimaryActionDisabled:
      feishuOnboardingHeaderStep === "environment"
        ? retryingFeishuConnector
        : feishuOnboardingHeaderStep === "plugin"
          ? installingOfficialFeishuPlugin
          : feishuOnboardingHeaderStep === "existing_robot"
            ? validatingFeishuCredentials
            : feishuOnboardingHeaderStep === "create_robot"
              ? feishuInstallerBusy
              : feishuOnboardingHeaderStep === "approve_pairing"
                ? !pendingFeishuPairingRequest || feishuPairingActionLoading !== null
                : feishuOnboardingHeaderStep === "authorize"
                  ? retryingFeishuConnector || installingOfficialFeishuPlugin
                  : feishuOnboardingHeaderStep === "routing"
                    ? false
                  : false,
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
  };

  const advancedConsoleSectionProps = {
    feishuConnectorSettings,
    onUpdateFeishuConnectorSettings: updateFeishuConnectorSettings,
    feishuEnvironmentStatus,
    feishuSetupProgress,
    officialFeishuRuntimeStatus,
    feishuCredentialProbe,
    validatingFeishuCredentials,
    savingFeishuConnector,
    retryingFeishuConnector,
    installingOfficialFeishuPlugin,
    feishuInstallerSession,
    feishuInstallerInput,
    onUpdateFeishuInstallerInput: updateFeishuInstallerInput,
    feishuInstallerBusy,
    feishuInstallerStartingMode,
    feishuPairingActionLoading,
    pendingFeishuPairingCount,
    pendingFeishuPairingRequest,
    feishuOnboardingEffectiveBranch,
    feishuAuthorizationInlineError,
    feishuOnboardingHeaderStep,
    feishuInstallerDisplayMode,
    feishuInstallerStartupHint,
    feishuAuthorizationAction,
    feishuRoutingStatus,
    getFeishuEnvironmentLabel,
    formatCompactDateTime,
    handleValidateFeishuCredentials,
    handleSaveFeishuConnector,
    handleInstallAndStartFeishuConnector,
    handleRefreshFeishuSetup,
    handleResolveFeishuPairingRequest,
    handleStartFeishuInstaller,
    handleStopFeishuInstallerSession,
    handleSendFeishuInstallerInput,
  };

  const advancedSectionProps = {
    connectionDetailSummary: feishuConnectionDetailSummary,
    feishuAdvancedSettings,
    onUpdateFeishuAdvancedSettings: updateFeishuAdvancedSettings,
    connectionStatusLabel: feishuConnectorStatus.label,
    pluginVersionLabel: feishuSetupProgress?.plugin_version || primaryPluginChannelHost?.version || "未识别",
    currentAccountLabel: primaryPluginChannelSnapshot?.snapshot.defaultAccountId || "未识别",
    pendingPairingCount: pendingFeishuPairingCount,
    lastEventAtLabel: formatCompactDateTime(officialFeishuRuntimeStatus?.last_event_at),
    recentIssueLabel: summarizeConnectorIssue(feishuConnectorStatus.error),
    runtimeLogsLabel: summarizeOfficialFeishuRuntimeLogs(officialFeishuRuntimeStatus),
    retryingFeishuConnector,
    savingFeishuAdvancedSettings,
    onRefreshFeishuSetup: handleRefreshFeishuSetup,
    onSaveFeishuAdvancedSettings: handleSaveFeishuAdvancedSettings,
    onCopyDiagnostics: handleCopyFeishuDiagnostics,
  };

  return {
    sections: {
      settingsSectionProps,
      advancedConsoleSectionProps,
      advancedSectionProps,
    },
  };
}
