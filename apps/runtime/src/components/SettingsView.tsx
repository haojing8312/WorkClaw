import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openExternalUrl } from "../utils/openExternalUrl";
import { SettingsShell } from "./settings/SettingsShell";
import { ModelsSettingsSection } from "./settings/models/ModelsSettingsSection";
import { DesktopSettingsSection } from "./settings/desktop/DesktopSettingsSection";
import { SearchSettingsSection } from "./settings/search/SearchSettingsSection";
import { McpSettingsSection } from "./settings/mcp/McpSettingsSection";
import { RoutingSettingsSection } from "./settings/routing/RoutingSettingsSection";
import { FeishuSettingsSection } from "./settings/feishu/FeishuSettingsSection";
import { FeishuAdvancedSection } from "./settings/feishu/FeishuAdvancedSection";
import { SettingsTabNav, type SettingsTabName } from "./settings/SettingsTabNav";
import {
  listModelConfigs,
  listProviderConfigs,
  syncModelConnections,
} from "./settings/models/modelSettingsService";
import {
  buildFeishuDiagnosticSummary,
  buildFeishuOnboardingState,
  extractFeishuInstallerQrBlock,
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
  resolveFeishuInstallerCompletionNotice,
  resolveFeishuInstallerFlowLabel,
  resolveFeishuOnboardingPanelDisplay,
  sanitizeFeishuInstallerDisplayLines,
  shouldShowFeishuInstallerGuidedPanel,
  summarizeConnectorIssue,
  summarizeOfficialFeishuRuntimeLogs,
} from "./settings/feishu/feishuSelectors";
import {
  approveFeishuPairingRequest as approveFeishuPairingRequestFromService,
  denyFeishuPairingRequest as denyFeishuPairingRequestFromService,
  installOpenClawLarkPlugin as installOpenClawLarkPluginFromService,
  loadFeishuAdvancedSettings as loadFeishuAdvancedSettingsFromService,
  loadFeishuGatewaySettings as loadFeishuGatewaySettingsFromService,
  loadFeishuInstallerSessionStatus as loadFeishuInstallerSessionStatusFromService,
  loadFeishuPairingRequests as loadFeishuPairingRequestsFromService,
  loadFeishuPluginChannelHosts as loadFeishuPluginChannelHostsFromService,
  loadFeishuPluginChannelSnapshot as loadFeishuPluginChannelSnapshotFromService,
  loadFeishuRuntimeStatus as loadFeishuRuntimeStatusFromService,
  loadFeishuSetupProgress as loadFeishuSetupProgressFromService,
  probeFeishuCredentials as probeFeishuCredentialsFromService,
  saveFeishuAdvancedSettings as saveFeishuAdvancedSettingsFromService,
  saveFeishuGatewaySettings as saveFeishuGatewaySettingsFromService,
  sendFeishuInstallerInput as sendFeishuInstallerInputFromService,
  startFeishuInstallerSession as startFeishuInstallerSessionFromService,
  startFeishuRuntime as startFeishuRuntimeFromService,
  stopFeishuInstallerSession as stopFeishuInstallerSessionFromService,
} from "./settings/feishu/feishuSettingsService";
export { buildFeishuOnboardingState } from "./settings/feishu/feishuSelectors";
export type {
  FeishuOnboardingInput,
  FeishuOnboardingState,
  FeishuOnboardingStep,
} from "./settings/feishu/feishuSelectors";
import {
  CapabilityRouteTemplateInfo,
  CapabilityRoutingPolicy,
  FeishuPairingRequestRecord,
  FeishuPluginEnvironmentStatus,
  FeishuSetupProgress,
  FeishuGatewaySettings,
  OpenClawPluginFeishuAdvancedSettings,
  OpenClawPluginChannelHost,
  OpenClawPluginChannelSnapshotResult,
  OpenClawPluginFeishuCredentialProbeResult,
  OpenClawPluginFeishuRuntimeStatus,
  OpenClawLarkInstallerMode,
  OpenClawLarkInstallerSessionStatus,
  ModelConfig,
  ProviderConfig,
  ProviderHealthInfo,
  RouteAttemptLog,
  RouteAttemptStat,
} from "../types";

function getErrorMessage(error: unknown, fallback: string): string {
  if (typeof error === "string") {
    return error || fallback;
  }
  if (error instanceof Error) {
    return error.message || fallback;
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message?: unknown }).message === "string"
  ) {
    return (error as { message: string }).message || fallback;
  }
  return fallback;
}

interface Props {
  onClose: () => void;
  onOpenEmployees?: () => void;
  initialTab?: SettingsTabName;
  showDevModelSetupTools?: boolean;
  onDevResetFirstUseOnboarding?: () => void;
  onDevOpenQuickModelSetup?: () => void;
}

const ROUTING_CAPABILITIES = [
  { label: "对话 Chat", value: "chat" },
  { label: "视觉 Vision", value: "vision" },
  { label: "生图 Image", value: "image_gen" },
  { label: "语音转写 STT", value: "audio_stt" },
  { label: "语音合成 TTS", value: "audio_tts" },
];

// 普通用户模式：仅保留关键入口，其他能力后台自动处理
const SHOW_CAPABILITY_ROUTING_SETTINGS = false;
const SHOW_HEALTH_SETTINGS = false;
const SHOW_MCP_SETTINGS = true;
const SHOW_AUTO_ROUTING_SETTINGS = false;

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

export function SettingsView({
  onClose,
  onOpenEmployees,
  initialTab = "models",
  showDevModelSetupTools = false,
  onDevResetFirstUseOnboarding,
  onDevOpenQuickModelSetup,
}: Props) {
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [activeTab, setActiveTab] = useState<SettingsTabName>(initialTab);

  const [providers, setProviders] = useState<ProviderConfig[]>([]);

  const [selectedCapability, setSelectedCapability] = useState("chat");
  const [chatRoutingPolicy, setChatRoutingPolicy] = useState<CapabilityRoutingPolicy>({
    capability: "chat",
    primary_provider_id: "",
    primary_model: "",
    fallback_chain_json: "[]",
    timeout_ms: 60000,
    retry_count: 0,
    enabled: true,
  });
  const [policySaveState, setPolicySaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [policyError, setPolicyError] = useState("");
  const [chatPrimaryModels, setChatPrimaryModels] = useState<string[]>([]);
  const [chatFallbackRows, setChatFallbackRows] = useState<Array<{ provider_id: string; model: string }>>([]);
  const [routeTemplates, setRouteTemplates] = useState<CapabilityRouteTemplateInfo[]>([]);
  const [selectedRouteTemplateId, setSelectedRouteTemplateId] = useState("china-first-p0");

  const [healthResult, setHealthResult] = useState<ProviderHealthInfo | null>(null);
  const [allHealthResults, setAllHealthResults] = useState<ProviderHealthInfo[]>([]);
  const [healthLoading, setHealthLoading] = useState(false);
  const [healthProviderId, setHealthProviderId] = useState("");
  const [routeLogs, setRouteLogs] = useState<RouteAttemptLog[]>([]);
  const [routeLogsLoading, setRouteLogsLoading] = useState(false);
  const [routeLogsOffset, setRouteLogsOffset] = useState(0);
  const [routeLogsHasMore, setRouteLogsHasMore] = useState(false);
  const [routeLogsSessionId, setRouteLogsSessionId] = useState("");
  const [routeLogsCapabilityFilter, setRouteLogsCapabilityFilter] = useState("all");
  const [routeLogsResultFilter, setRouteLogsResultFilter] = useState("all");
  const [routeLogsErrorKindFilter, setRouteLogsErrorKindFilter] = useState("all");
  const [routeLogsExporting, setRouteLogsExporting] = useState(false);
  const [routeStats, setRouteStats] = useState<RouteAttemptStat[]>([]);
  const [routeStatsLoading, setRouteStatsLoading] = useState(false);
  const [routeStatsCapability, setRouteStatsCapability] = useState("all");
  const [routeStatsHours, setRouteStatsHours] = useState(24);
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
  const [pluginChannelSnapshots, setPluginChannelSnapshots] = useState<Record<string, OpenClawPluginChannelSnapshotResult>>({});
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

  useEffect(() => {
    setActiveTab(initialTab);
  }, [initialTab]);

  useEffect(() => {
    void loadSharedModelData();
    if (SHOW_CAPABILITY_ROUTING_SETTINGS) {
      loadCapabilityRoutingPolicy("chat");
      loadRouteTemplates("chat");
    }
  }, []);

  useEffect(() => {
    if (chatRoutingPolicy.primary_provider_id) {
      loadChatPrimaryModels(chatRoutingPolicy.primary_provider_id, selectedCapability);
    }
  }, [chatRoutingPolicy.primary_provider_id, selectedCapability]);

  useEffect(() => {
    if (SHOW_HEALTH_SETTINGS && activeTab === "health") {
      loadRecentRouteLogs(false);
      loadRouteStats();
    }
  }, [activeTab]);

  async function loadSharedModelData() {
    try {
      const list = await listModelConfigs();
      setModels(list);
      await syncModelConnections(list);
      const providerList = await listProviderConfigs();
      const ids = new Set(list.map((model) => model.id));
      const aligned = providerList.filter((provider) => ids.has(provider.id));
      setProviders(aligned);
      if (aligned.length === 0) {
        setHealthProviderId("");
      } else if (!healthProviderId || !aligned.some((provider) => provider.id === healthProviderId)) {
        setHealthProviderId(aligned[0].id);
      }
    } catch (error) {
      console.warn("加载模型共享数据失败:", error);
    }
  }

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

  async function loadCapabilityRoutingPolicy(capability: string) {
    try {
      const policy = await invoke<CapabilityRoutingPolicy | null>("get_capability_routing_policy", {
        capability,
      });
      if (policy) {
        setChatRoutingPolicy(policy);
        try {
          const parsed = JSON.parse(policy.fallback_chain_json || "[]");
          if (Array.isArray(parsed)) {
            setChatFallbackRows(
              parsed.map((item) => ({
                provider_id: String(item?.provider_id || ""),
                model: String(item?.model || ""),
              })),
            );
          }
        } catch {
          setChatFallbackRows([]);
        }
      } else {
        setChatRoutingPolicy({
          capability,
          primary_provider_id: "",
          primary_model: "",
          fallback_chain_json: "[]",
          timeout_ms: 60000,
          retry_count: 0,
          enabled: true,
        });
        setChatFallbackRows([]);
      }
    } catch (e) {
      setPolicyError("加载聊天路由策略失败: " + String(e));
    }
  }

  async function loadRouteTemplates(capability: string) {
    try {
      const list = await invoke<CapabilityRouteTemplateInfo[]>("list_capability_route_templates", {
        capability,
      });
      setRouteTemplates(list);
      if (list.length > 0 && !list.some((x) => x.template_id === selectedRouteTemplateId)) {
        setSelectedRouteTemplateId(list[0].template_id);
      }
    } catch {
      setRouteTemplates([]);
    }
  }

  async function loadConnectorSettings() {
    try {
      const [feishuSettings, feishuAdvanced] = await Promise.all([
        loadFeishuGatewaySettingsFromService(),
        loadFeishuAdvancedSettingsFromService(),
      ]);
      setFeishuConnectorSettings({
        app_id: feishuSettings?.app_id || "",
        app_secret: feishuSettings?.app_secret || "",
        ingress_token: feishuSettings?.ingress_token || "",
        encrypt_key: feishuSettings?.encrypt_key || "",
        sidecar_base_url: feishuSettings?.sidecar_base_url || "",
      });
      setFeishuAdvancedSettings({
        groups_json: feishuAdvanced?.groups_json || "",
        dms_json: feishuAdvanced?.dms_json || "",
        footer_json: feishuAdvanced?.footer_json || "",
        account_overrides_json: feishuAdvanced?.account_overrides_json || "",
        render_mode: feishuAdvanced?.render_mode || "",
        streaming: feishuAdvanced?.streaming || "",
        text_chunk_limit: feishuAdvanced?.text_chunk_limit || "",
        chunk_mode: feishuAdvanced?.chunk_mode || "",
        reply_in_thread: feishuAdvanced?.reply_in_thread || "",
        group_session_scope: feishuAdvanced?.group_session_scope || "",
        topic_session_mode: feishuAdvanced?.topic_session_mode || "",
        markdown_mode: feishuAdvanced?.markdown_mode || "",
        markdown_table_mode: feishuAdvanced?.markdown_table_mode || "",
        heartbeat_visibility: feishuAdvanced?.heartbeat_visibility || "",
        heartbeat_interval_ms: feishuAdvanced?.heartbeat_interval_ms || "",
        media_max_mb: feishuAdvanced?.media_max_mb || "",
        http_timeout_ms: feishuAdvanced?.http_timeout_ms || "",
        config_writes: feishuAdvanced?.config_writes || "",
        webhook_host: feishuAdvanced?.webhook_host || "",
        webhook_port: feishuAdvanced?.webhook_port || "",
        dynamic_agent_creation_enabled: feishuAdvanced?.dynamic_agent_creation_enabled || "",
        dynamic_agent_creation_workspace_template:
          feishuAdvanced?.dynamic_agent_creation_workspace_template || "",
        dynamic_agent_creation_agent_dir_template:
          feishuAdvanced?.dynamic_agent_creation_agent_dir_template || "",
        dynamic_agent_creation_max_agents:
          feishuAdvanced?.dynamic_agent_creation_max_agents || "",
      });
    } catch (e) {
      console.warn("加载渠道连接器配置失败:", e);
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
    } catch (e) {
      console.warn("加载飞书接入进度失败:", e);
      setFeishuEnvironmentStatus(null);
      setFeishuSetupProgress(null);
    }
  }

  async function loadConnectorStatuses() {
    try {
      const runtimeStatus = await loadFeishuRuntimeStatusFromService();
      setOfficialFeishuRuntimeStatus(runtimeStatus);
    } catch (e) {
      console.warn("加载渠道连接器状态失败:", e);
      setOfficialFeishuRuntimeStatus(null);
    }
  }

  async function loadFeishuInstallerSessionStatus() {
    try {
      const status = await loadFeishuInstallerSessionStatusFromService();
      setFeishuInstallerSession(status ?? DEFAULT_FEISHU_INSTALLER_SESSION);
    } catch (e) {
      console.warn("加载飞书官方安装向导状态失败:", e);
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
      normalizedHosts.map((host) =>
        loadFeishuPluginChannelSnapshotFromService(host.plugin_id),
      ),
    );
    const nextSnapshots: Record<string, OpenClawPluginChannelSnapshotResult> = {};
    for (const result of snapshotResults) {
      if (result.status !== "fulfilled") {
        continue;
      }
      nextSnapshots[result.value.snapshot.channelId || result.value.entryPath] = result.value;
    }
    setPluginChannelSnapshots(nextSnapshots);
    setPluginChannelSnapshotsError(
      snapshotResults.some((result) => result.status !== "fulfilled") ? "部分账号快照暂时不可用" : "",
    );
  }

  function formatCompactDateTime(value: string | null | undefined) {
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

  const primaryPluginChannelHost = pluginChannelHosts.find((host) => host.channel === "feishu") ?? pluginChannelHosts[0] ?? null;
  const primaryPluginChannelSnapshot =
    (primaryPluginChannelHost ? pluginChannelSnapshots[primaryPluginChannelHost.channel] : null) ??
    Object.values(pluginChannelSnapshots)[0] ??
    null;
  const hasReadyOfficialFeishuPlugin =
    pluginChannelHosts.some((host) => host.status === "ready") ||
    (feishuSetupProgress?.plugin_installed === true && officialFeishuRuntimeStatus?.running === true);
  const hasErroredOfficialFeishuPlugin = pluginChannelHosts.some((host) => host.status === "error");
  const hasInstalledOfficialFeishuPlugin =
    pluginChannelHosts.length > 0 || feishuSetupProgress?.plugin_installed === true;
  const pendingFeishuPairingCount = feishuSetupProgress?.pending_pairings ?? feishuPairingRequests.filter((request) => request.status === "pending").length;
  const pendingFeishuPairingRequest =
    feishuPairingRequests.find((request) => request.status === "pending") ?? null;
  const feishuOnboardingState = buildFeishuOnboardingState({
    summaryState: feishuSetupProgress?.summary_state ?? null,
    setupProgress: feishuSetupProgress,
    installerMode: feishuInstallerSession?.mode ?? null,
  });
  const [feishuOnboardingPanelMode, setFeishuOnboardingPanelMode] = useState<"guided" | "skipped">("guided");
  const [feishuOnboardingSelectedPath, setFeishuOnboardingSelectedPath] = useState<
    "existing_robot" | "create_robot" | null
  >(null);
  const [feishuOnboardingSkippedSignature, setFeishuOnboardingSkippedSignature] = useState<string | null>(null);
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
  const feishuInstallerDisplayMode = feishuInstallerSession?.mode ?? feishuInstallerStartingMode;
  const feishuInstallerFlowLabel = resolveFeishuInstallerFlowLabel(feishuInstallerDisplayMode);
  const feishuInstallerQrBlock = extractFeishuInstallerQrBlock(feishuInstallerSession?.recent_output ?? []);
  const feishuInstallerDisplayLines = sanitizeFeishuInstallerDisplayLines(feishuInstallerSession?.recent_output ?? []);
  const feishuInstallerStartupHint = feishuInstallerBusy && feishuInstallerStartingMode
    ? `正在启动${resolveFeishuInstallerFlowLabel(feishuInstallerStartingMode)}，请稍候...`
    : null;
  useEffect(() => {
    if (
      feishuOnboardingPanelMode === "skipped" &&
      feishuOnboardingSkippedSignature &&
      feishuOnboardingSkippedSignature !== feishuOnboardingProgressSignature
    ) {
      setFeishuOnboardingPanelMode("guided");
      setFeishuOnboardingSkippedSignature(null);
    }
  }, [feishuOnboardingPanelMode, feishuOnboardingProgressSignature, feishuOnboardingSkippedSignature]);
  const feishuConnectorStatus = resolveFeishuConnectorStatus({
    running: officialFeishuRuntimeStatus?.running === true,
    lastError: officialFeishuRuntimeStatus?.last_error ?? "",
    hasInstalledOfficialFeishuPlugin,
  });
  const feishuAuthorizationAction = getFeishuAuthorizationAction({
    runtimeRunning: officialFeishuRuntimeStatus?.running === true,
    pluginInstalled: feishuSetupProgress?.plugin_installed === true,
  });
  const feishuRoutingStatus = getFeishuRoutingStatus({
    authApproved: feishuSetupProgress?.auth_status === "approved",
    defaultRoutingEmployeeName: feishuSetupProgress?.default_routing_employee_name,
    scopedRoutingCount: feishuSetupProgress?.scoped_routing_count ?? 0,
  });
  const feishuSetupSummary = getFeishuSetupSummary({
    skipped: feishuOnboardingState.skipped,
    summaryState: feishuSetupProgress?.summary_state,
    runtimeRunning: feishuSetupProgress?.runtime_running === true,
    authApproved: feishuSetupProgress?.auth_status === "approved",
    runtimeLastError: feishuSetupProgress?.runtime_last_error,
    officialRuntimeLastError: officialFeishuRuntimeStatus?.last_error,
  });
  const feishuConnectionDetailSummary = getFeishuConnectionDetailSummary({
    connectorStatus: feishuConnectorStatus,
    runtimeRunning: officialFeishuRuntimeStatus?.running === true,
    authApproved: feishuSetupProgress?.auth_status === "approved",
    defaultRoutingEmployeeName: feishuSetupProgress?.default_routing_employee_name,
    scopedRoutingCount: feishuSetupProgress?.scoped_routing_count ?? 0,
  });
  const feishuRoutingActionAvailable = Boolean(onOpenEmployees);
  const feishuOnboardingPrimaryActionLabel = feishuOnboardingIsSkipped
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
          : feishuAuthorizationAction.label
        : feishuOnboardingHeaderStep === "approve_pairing"
          ? feishuPairingActionLoading === "approve"
            ? "批准中..."
            : "批准这次接入"
        : feishuOnboardingHeaderStep === "routing"
          ? feishuRoutingActionAvailable
            ? feishuRoutingStatus.actionLabel
            : "请从员工中心继续"
          : feishuOnboardingPanelDisplay.primaryActionLabel;
  const feishuOnboardingPrimaryActionDisabled = feishuOnboardingHeaderStep === "existing_robot"
    ? validatingFeishuCredentials
    : feishuOnboardingHeaderStep === "create_robot"
      ? feishuInstallerBusy
      : feishuOnboardingHeaderStep === "environment"
        ? retryingFeishuConnector
        : feishuOnboardingHeaderStep === "plugin"
          ? installingOfficialFeishuPlugin
        : feishuOnboardingHeaderStep === "authorize"
          ? retryingFeishuConnector || installingOfficialFeishuPlugin
          : feishuOnboardingHeaderStep === "approve_pairing"
            ? feishuPairingActionLoading !== null || !pendingFeishuPairingRequest
          : feishuOnboardingHeaderStep === "routing"
            ? !feishuRoutingActionAvailable
          : false;

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

  async function loadChatPrimaryModels(providerId: string, capability: string) {
    if (!providerId) {
      setChatPrimaryModels([]);
      return;
    }
    try {
      const models = await invoke<string[]>("list_provider_models", {
        providerId,
        capability,
      });
      setChatPrimaryModels(models);
    } catch {
      setChatPrimaryModels([]);
    }
  }

  async function handleSaveChatPolicy() {
    setPolicySaveState("saving");
    setPolicyError("");
    try {
      const policyToSave = {
        ...chatRoutingPolicy,
        capability: selectedCapability,
        fallback_chain_json: JSON.stringify(chatFallbackRows),
      };
      await invoke("set_capability_routing_policy", { policy: policyToSave });
      setPolicySaveState("saved");
      setTimeout(() => setPolicySaveState("idle"), 1200);
    } catch (e) {
      setPolicySaveState("error");
      setPolicyError("保存聊天路由策略失败: " + String(e));
    }
  }

  async function handleCheckProviderHealth() {
    if (!healthProviderId) return;
    setHealthLoading(true);
    try {
      const result = await invoke<ProviderHealthInfo>("test_provider_health", {
        providerId: healthProviderId,
      });
      setHealthResult(result);
    } catch (e) {
      setHealthResult({
        provider_id: healthProviderId,
        ok: false,
        protocol_type: "",
        message: String(e),
      });
    } finally {
      setHealthLoading(false);
    }
  }

  async function handleCheckAllProviderHealth() {
    setHealthLoading(true);
    try {
      const results = await invoke<ProviderHealthInfo[]>("test_all_provider_health");
      setAllHealthResults(results);
      if (results.length > 0) {
        setHealthResult(results[0]);
      }
    } catch (e) {
      setAllHealthResults([
        {
          provider_id: "",
          ok: false,
          protocol_type: "",
          message: String(e),
        },
      ]);
    } finally {
      setHealthLoading(false);
    }
  }

  async function loadRecentRouteLogs(append: boolean) {
    setRouteLogsLoading(true);
    try {
      const logs = await invoke<RouteAttemptLog[]>("list_recent_route_attempt_logs", {
        sessionId: routeLogsSessionId.trim() || null,
        limit: 50,
        offset: append ? routeLogsOffset : 0,
      });
      setRouteLogs((prev) => (append ? [...prev, ...logs] : logs));
      setRouteLogsOffset((prev) => (append ? prev + logs.length : logs.length));
      setRouteLogsHasMore(logs.length === 50);
    } catch {
      if (!append) {
        setRouteLogs([]);
        setRouteLogsOffset(0);
        setRouteLogsHasMore(false);
      }
    } finally {
      setRouteLogsLoading(false);
    }
  }

  async function loadRouteStats() {
    setRouteStatsLoading(true);
    try {
      const stats = await invoke<RouteAttemptStat[]>("list_route_attempt_stats", {
        hours: routeStatsHours,
        capability: routeStatsCapability === "all" ? null : routeStatsCapability,
      });
      setRouteStats(stats);
    } catch {
      setRouteStats([]);
    } finally {
      setRouteStatsLoading(false);
    }
  }

  async function handleExportRouteLogsCsv() {
    setRouteLogsExporting(true);
    try {
      const csv = await invoke<string>("export_route_attempt_logs_csv", {
        sessionId: routeLogsSessionId.trim() || null,
        hours: routeStatsHours,
        capability: routeLogsCapabilityFilter === "all" ? null : routeLogsCapabilityFilter,
        resultFilter: routeLogsResultFilter === "all" ? null : routeLogsResultFilter,
        errorKind: routeLogsErrorKindFilter === "all" ? null : routeLogsErrorKindFilter,
      });
      const dir = await invoke<string | null>("select_directory", { defaultPath: "" });
      if (dir) {
        const stamp = new Date().toISOString().replace(/:/g, "-").replace(/\..+/, "");
        const path = `${dir}\\route-attempt-logs-${stamp}.csv`;
        await invoke("write_export_file", { path, content: csv });
      }
      if (navigator?.clipboard?.writeText) {
        await navigator.clipboard.writeText(csv);
      }
    } finally {
      setRouteLogsExporting(false);
    }
  }

  function getCapabilityRecommendedDefaults(capability: string): { timeout_ms: number; retry_count: number } {
    switch (capability) {
      case "vision":
        return { timeout_ms: 90000, retry_count: 1 };
      case "image_gen":
        return { timeout_ms: 120000, retry_count: 1 };
      case "audio_stt":
        return { timeout_ms: 90000, retry_count: 1 };
      case "audio_tts":
        return { timeout_ms: 60000, retry_count: 1 };
      default:
        return { timeout_ms: 60000, retry_count: 1 };
    }
  }

  const filteredRouteLogs = routeLogs.filter((log) => {
    if (routeLogsCapabilityFilter !== "all" && log.capability !== routeLogsCapabilityFilter) return false;
    if (routeLogsResultFilter === "success" && !log.success) return false;
    if (routeLogsResultFilter === "failed" && log.success) return false;
    if (routeLogsErrorKindFilter !== "all" && log.error_kind !== routeLogsErrorKindFilter) return false;
    return true;
  });

  function addFallbackRow() {
    setChatFallbackRows((rows) => [...rows, { provider_id: "", model: "" }]);
  }

  function updateFallbackRow(index: number, patch: Partial<{ provider_id: string; model: string }>) {
    setChatFallbackRows((rows) => rows.map((row, i) => (i === index ? { ...row, ...patch } : row)));
  }

  function removeFallbackRow(index: number) {
    setChatFallbackRows((rows) => rows.filter((_, i) => i !== index));
  }

  async function handleApplyRouteTemplate() {
    try {
      const policy = await invoke<CapabilityRoutingPolicy>("apply_capability_route_template", {
        capability: selectedCapability,
        templateId: selectedRouteTemplateId,
      });
      setChatRoutingPolicy(policy);
      const parsed = JSON.parse(policy.fallback_chain_json || "[]");
      if (Array.isArray(parsed)) {
        setChatFallbackRows(
          parsed.map((item) => ({
            provider_id: String(item?.provider_id || ""),
            model: String(item?.model || ""),
          })),
        );
      } else {
        setChatFallbackRows([]);
      }
    } catch (e) {
      const raw = String(e);
      const enabledKeys = Array.from(new Set(providers.filter((p) => p.enabled).map((p) => p.provider_key)));
      const enabledText = enabledKeys.length > 0 ? enabledKeys.join(", ") : "无";
      let missingText = "";
      const match = raw.match(/需要其一）:\s*(\[[^\]]+\])/);
      if (match?.[1]) {
        missingText = `；缺少服务标识（任选其一）: ${match[1]}`;
      }
      setPolicyError(`应用路由模板失败: ${raw}${missingText}；当前已启用: ${enabledText}。请先到“模型连接”补齐并启用。`);
    }
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
      const saved = await saveFeishuAdvancedSettingsFromService({
        groups_json: feishuAdvancedSettings.groups_json,
        dms_json: feishuAdvancedSettings.dms_json,
        footer_json: feishuAdvancedSettings.footer_json,
        account_overrides_json: feishuAdvancedSettings.account_overrides_json,
        render_mode: feishuAdvancedSettings.render_mode,
        streaming: feishuAdvancedSettings.streaming,
        text_chunk_limit: feishuAdvancedSettings.text_chunk_limit,
        chunk_mode: feishuAdvancedSettings.chunk_mode,
        reply_in_thread: feishuAdvancedSettings.reply_in_thread,
        group_session_scope: feishuAdvancedSettings.group_session_scope,
        topic_session_mode: feishuAdvancedSettings.topic_session_mode,
        markdown_mode: feishuAdvancedSettings.markdown_mode,
        markdown_table_mode: feishuAdvancedSettings.markdown_table_mode,
        heartbeat_visibility: feishuAdvancedSettings.heartbeat_visibility,
        heartbeat_interval_ms: feishuAdvancedSettings.heartbeat_interval_ms,
        media_max_mb: feishuAdvancedSettings.media_max_mb,
        http_timeout_ms: feishuAdvancedSettings.http_timeout_ms,
        config_writes: feishuAdvancedSettings.config_writes,
        webhook_host: feishuAdvancedSettings.webhook_host,
        webhook_port: feishuAdvancedSettings.webhook_port,
        dynamic_agent_creation_enabled: feishuAdvancedSettings.dynamic_agent_creation_enabled,
        dynamic_agent_creation_workspace_template:
          feishuAdvancedSettings.dynamic_agent_creation_workspace_template,
        dynamic_agent_creation_agent_dir_template:
          feishuAdvancedSettings.dynamic_agent_creation_agent_dir_template,
        dynamic_agent_creation_max_agents:
          feishuAdvancedSettings.dynamic_agent_creation_max_agents,
      });
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
      if (!hasInstalledOfficialFeishuPlugin) {
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
      const runtimeStatus = await startFeishuRuntimeFromService(primaryPluginChannelHost?.plugin_id || "openclaw-lark", null);
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

      if (!hasInstalledOfficialFeishuPlugin) {
        await installOpenClawLarkPluginFromService();
      }

      const runtimeStatus = await startFeishuRuntimeFromService(
        primaryPluginChannelHost?.plugin_id || "openclaw-lark",
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
      setFeishuConnectorError(getErrorMessage(error, "打开官方文档失败，请稍后重试"));
    }
  }

  async function handleCopyFeishuDiagnostics() {
    try {
      await navigator?.clipboard?.writeText?.(
        buildFeishuDiagnosticSummary({
          connectorStatus: feishuConnectorStatus,
          pluginVersion: feishuSetupProgress?.plugin_version || primaryPluginChannelHost?.version || "未识别",
          defaultAccountId: primaryPluginChannelSnapshot?.snapshot.defaultAccountId || "未识别",
          authApproved: feishuSetupProgress?.auth_status === "approved",
          defaultRoutingEmployeeName: feishuSetupProgress?.default_routing_employee_name || "未设置",
          scopedRoutingCount: feishuSetupProgress?.scoped_routing_count ?? 0,
          lastEventAtLabel: formatCompactDateTime(officialFeishuRuntimeStatus?.last_event_at),
          connectionDetailSummary: feishuConnectionDetailSummary,
          recentLogsSummary: summarizeOfficialFeishuRuntimeLogs(officialFeishuRuntimeStatus),
        }),
      );
      setFeishuConnectorNotice("连接诊断摘要已复制");
    } catch (error) {
      setFeishuConnectorError(getErrorMessage(error, "复制连接诊断摘要失败，请稍后重试"));
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

  const inputCls = "sm-input w-full text-sm py-1.5";
  const labelCls = "sm-field-label";
  const feishuSectionProps = {
    onOpenEmployees,
    feishuConnectorSettings,
    setFeishuConnectorSettings,
    feishuEnvironmentStatus,
    feishuSetupProgress,
    validatingFeishuCredentials,
    feishuCredentialProbe,
    feishuInstallerSession,
    feishuInstallerInput,
    setFeishuInstallerInput,
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
    setFeishuOnboardingPanelMode,
    feishuOnboardingSelectedPath,
    setFeishuOnboardingSelectedPath,
    feishuOnboardingSkippedSignature,
    setFeishuOnboardingSkippedSignature,
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
  };
  // 眼睛图标：显示状态（可见）
  function EyeOpenIcon() {
    return (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M2.036 12.322a1.012 1.012 0 010-.639C3.423 7.51 7.36 4.5 12 4.5c4.638 0 8.573 3.007 9.963 7.178.07.207.07.431 0 .639C20.577 16.49 16.64 19.5 12 19.5c-4.638 0-8.573-3.007-9.963-7.178z" />
        <path strokeLinecap="round" strokeLinejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
      </svg>
    );
  }

  // 眼睛图标：隐藏状态（划线）
  function EyeSlashIcon() {
    return (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M3.98 8.223A10.477 10.477 0 001.934 12C3.226 16.338 7.244 19.5 12 19.5c.993 0 1.953-.138 2.863-.395M6.228 6.228A10.45 10.45 0 0112 4.5c4.756 0 8.773 3.162 10.065 7.498a10.523 10.523 0 01-4.293 5.774M6.228 6.228L3 3m3.228 3.228l3.65 3.65m7.894 7.894L21 21m-3.228-3.228l-3.65-3.65m0 0a3 3 0 10-4.243-4.243m4.242 4.242L9.88 9.88" />
      </svg>
    );
  }

  return (
    <SettingsShell
      onClose={onClose}
      tabs={
        <SettingsTabNav
          activeTab={activeTab}
          onSelectTab={setActiveTab}
          showCapabilityRoutingSettings={SHOW_CAPABILITY_ROUTING_SETTINGS}
          showHealthSettings={SHOW_HEALTH_SETTINGS}
          showMcpSettings={SHOW_MCP_SETTINGS}
          showAutoRoutingSettings={SHOW_AUTO_ROUTING_SETTINGS}
        />
      }
    >

      {activeTab === "models" && (
        <div className="space-y-4">
          <ModelsSettingsSection
            models={models}
            providers={providers}
            onModelsChange={setModels}
            onProvidersChange={setProviders}
            showDevModelSetupTools={showDevModelSetupTools}
            onDevResetFirstUseOnboarding={onDevResetFirstUseOnboarding}
            onDevOpenQuickModelSetup={onDevOpenQuickModelSetup}
          />
        </div>
      )}
      <DesktopSettingsSection models={models} visible={activeTab === "desktop"} />

      {SHOW_CAPABILITY_ROUTING_SETTINGS && activeTab === "capabilities" && (
        <div className="bg-white rounded-lg p-4 space-y-3">
          <div className="text-xs font-medium text-gray-500 mb-2">能力路由</div>
          <div>
            <label className={labelCls}>能力类型</label>
            <select
              className={inputCls}
              value={selectedCapability}
              onChange={(e) => {
                const capability = e.target.value;
                setSelectedCapability(capability);
                loadCapabilityRoutingPolicy(capability);
                loadRouteTemplates(capability);
              }}
            >
              {ROUTING_CAPABILITIES.map((c) => (
                <option key={c.value} value={c.value}>{c.label}</option>
              ))}
            </select>
          </div>
          <div>
            <label className={labelCls}>主连接</label>
            <select
              className={inputCls}
              value={chatRoutingPolicy.primary_provider_id}
              onChange={(e) => {
                const providerId = e.target.value;
                setChatRoutingPolicy((s) => ({ ...s, primary_provider_id: providerId }));
                loadChatPrimaryModels(providerId, selectedCapability);
              }}
            >
              <option value="">请选择</option>
              {providers.map((p) => (
                <option key={p.id} value={p.id}>{p.display_name}</option>
              ))}
            </select>
          </div>
          <div>
            <label className={labelCls}>主模型</label>
            <input
              className={inputCls}
              list="chat-primary-models"
              value={chatRoutingPolicy.primary_model}
              onChange={(e) => setChatRoutingPolicy((s) => ({ ...s, primary_model: e.target.value }))}
              placeholder="例如: deepseek-chat / qwen3.5-plus / kimi-k2"
            />
            {chatPrimaryModels.length > 0 && (
              <datalist id="chat-primary-models">
                {chatPrimaryModels.map((model) => (
                  <option key={model} value={model} />
                ))}
              </datalist>
            )}
          </div>
          <div>
            <label className={labelCls}>Fallback 链</label>
            <div className="space-y-2">
              {chatFallbackRows.map((row, index) => (
                <div key={index} className="grid grid-cols-[1fr_1fr_auto] gap-2">
                  <select
                    className={inputCls}
                    value={row.provider_id}
                    onChange={(e) => updateFallbackRow(index, { provider_id: e.target.value })}
                  >
                    <option value="">选择连接</option>
                    {providers.map((p) => (
                      <option key={p.id} value={p.id}>{p.display_name}</option>
                    ))}
                  </select>
                  <input
                    className={inputCls}
                    value={row.model}
                    onChange={(e) => updateFallbackRow(index, { model: e.target.value })}
                    placeholder="模型名"
                  />
                  <button
                    onClick={() => removeFallbackRow(index)}
                    className="px-2 text-xs text-red-500 hover:text-red-600"
                  >
                    删除
                  </button>
                </div>
              ))}
              <button
                onClick={addFallbackRow}
                className="text-xs text-blue-500 hover:text-blue-600"
              >
                + 添加回退节点
              </button>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-2">
            <div>
              <label className={labelCls}>超时(ms)</label>
              <input
                className={inputCls}
                type="number"
                value={chatRoutingPolicy.timeout_ms}
                onChange={(e) => setChatRoutingPolicy((s) => ({ ...s, timeout_ms: Number(e.target.value || 60000) }))}
              />
            </div>
            <div>
              <label className={labelCls}>重试次数</label>
              <input
                className={inputCls}
                type="number"
                value={chatRoutingPolicy.retry_count}
                onChange={(e) => setChatRoutingPolicy((s) => ({ ...s, retry_count: Number(e.target.value || 0) }))}
              />
            </div>
          </div>
          <button
            onClick={() => {
              const defaults = getCapabilityRecommendedDefaults(selectedCapability);
              setChatRoutingPolicy((s) => ({
                ...s,
                timeout_ms: defaults.timeout_ms,
                retry_count: defaults.retry_count,
              }));
            }}
            className="w-full bg-gray-100 hover:bg-gray-200 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            应用推荐超时/重试配置
          </button>
          <div className="grid grid-cols-[1fr_auto] gap-2">
            <select
              className={inputCls}
              value={selectedRouteTemplateId}
              onChange={(e) => setSelectedRouteTemplateId(e.target.value)}
            >
              {routeTemplates.length === 0 && <option value="">暂无模板</option>}
              {routeTemplates.map((tpl) => (
                <option key={`${tpl.template_id}-${tpl.capability}`} value={tpl.template_id}>
                  {tpl.name}
                </option>
              ))}
            </select>
            <button
              onClick={handleApplyRouteTemplate}
              disabled={!selectedRouteTemplateId}
              className="bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm px-3 py-1.5 rounded-lg transition-all active:scale-[0.97]"
            >
              应用模板
            </button>
          </div>
          <label className="flex items-center gap-2 text-xs text-gray-600">
            <input
              type="checkbox"
              checked={chatRoutingPolicy.enabled}
              onChange={(e) => setChatRoutingPolicy((s) => ({ ...s, enabled: e.target.checked }))}
            />
            启用当前能力路由
          </label>
          {policyError && <div className="bg-red-50 text-red-600 text-xs px-2 py-1 rounded">{policyError}</div>}
          {policySaveState === "saved" && <div className="bg-green-50 text-green-600 text-xs px-2 py-1 rounded">已保存</div>}
          <button
            onClick={handleSaveChatPolicy}
            disabled={policySaveState === "saving"}
            className="w-full bg-blue-500 hover:bg-blue-600 disabled:opacity-50 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {policySaveState === "saving" ? "保存中..." : "保存能力路由策略"}
          </button>
        </div>
      )}

      {SHOW_HEALTH_SETTINGS && activeTab === "health" && (
        <div className="bg-white rounded-lg p-4 space-y-3">
          <div className="text-xs font-medium text-gray-500 mb-2">连接健康检查</div>
          <div>
            <label className={labelCls}>选择连接</label>
            <select
              className={inputCls}
              value={healthProviderId}
              onChange={(e) => setHealthProviderId(e.target.value)}
            >
              <option value="">请选择</option>
              {providers.map((p) => (
                <option key={p.id} value={p.id}>{p.display_name}</option>
              ))}
            </select>
          </div>
          <button
            onClick={handleCheckProviderHealth}
            disabled={!healthProviderId || healthLoading}
            className="w-full bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {healthLoading ? "检测中..." : "执行健康检查"}
          </button>
          <button
            onClick={handleCheckAllProviderHealth}
            disabled={healthLoading}
            className="w-full bg-blue-500 hover:bg-blue-600 disabled:opacity-50 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
          >
            {healthLoading ? "检测中..." : "一键巡检全部连接"}
          </button>
          {healthResult && (
            <div className={"text-xs px-2 py-2 rounded " + (healthResult.ok ? "bg-green-50 text-green-700" : "bg-red-50 text-red-700")}>
              <div>状态: {healthResult.ok ? "正常" : "异常"}</div>
              <div>协议: {healthResult.protocol_type || "-"}</div>
              <div className="break-all">详情: {healthResult.message}</div>
            </div>
          )}
          {allHealthResults.length > 0 && (
            <div className="space-y-2">
              {allHealthResults.map((r, idx) => (
                <div
                  key={`${r.provider_id}-${idx}`}
                  className={"text-xs px-2 py-2 rounded " + (r.ok ? "bg-green-50 text-green-700" : "bg-red-50 text-red-700")}
                >
                  <div>连接ID: {r.provider_id || "-"}</div>
                  <div>状态: {r.ok ? "正常" : "异常"}</div>
                  <div>协议: {r.protocol_type || "-"}</div>
                  <div className="break-all">详情: {r.message}</div>
                </div>
              ))}
            </div>
          )}
          <div className="pt-2 border-t border-gray-100">
            <div className="mb-3">
              <div className="flex items-center justify-between mb-2">
                <div className="text-xs font-medium text-gray-500">路由统计</div>
                <button
                  onClick={loadRouteStats}
                  disabled={routeStatsLoading}
                  className="text-xs text-blue-500 hover:text-blue-600 disabled:opacity-50"
                >
                  {routeStatsLoading ? "刷新中..." : "刷新"}
                </button>
              </div>
              <div className="flex gap-2 mb-2">
                <select
                  className={inputCls}
                  value={String(routeStatsHours)}
                  onChange={(e) => setRouteStatsHours(Number(e.target.value || 24))}
                >
                  <option value="1">最近 1h</option>
                  <option value="24">最近 24h</option>
                  <option value="168">最近 7d</option>
                </select>
                <select
                  className={inputCls}
                  value={routeStatsCapability}
                  onChange={(e) => setRouteStatsCapability(e.target.value)}
                >
                  <option value="all">全部能力</option>
                  <option value="chat">chat</option>
                  <option value="vision">vision</option>
                  <option value="image_gen">image_gen</option>
                  <option value="audio_stt">audio_stt</option>
                  <option value="audio_tts">audio_tts</option>
                </select>
                <button
                  onClick={loadRouteStats}
                  disabled={routeStatsLoading}
                  className="bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-xs px-3 rounded"
                >
                  应用
                </button>
              </div>
              {routeStats.length === 0 ? (
                <div className="text-xs text-gray-400">暂无统计数据</div>
              ) : (
                <div className="space-y-1">
                  {routeStats.slice(0, 8).map((stat, idx) => (
                    <div key={`${stat.capability}-${stat.error_kind}-${idx}`} className="text-xs bg-gray-50 border border-gray-100 rounded px-2 py-1 text-gray-700">
                      {stat.capability} · {stat.success ? "success" : stat.error_kind || "unknown"} · {stat.count}
                    </div>
                  ))}
                </div>
              )}
            </div>
            <div className="flex items-center justify-between mb-2">
              <div className="text-xs font-medium text-gray-500">最近路由日志</div>
              <button
                onClick={() => {
                  setRouteLogsOffset(0);
                  loadRecentRouteLogs(false);
                }}
                disabled={routeLogsLoading}
                className="text-xs text-blue-500 hover:text-blue-600 disabled:opacity-50"
              >
                {routeLogsLoading ? "刷新中..." : "刷新"}
              </button>
            </div>
            <button
              onClick={handleExportRouteLogsCsv}
              disabled={routeLogsExporting}
              className="w-full mb-2 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-xs py-1.5 rounded"
            >
              {routeLogsExporting ? "导出中..." : "导出日志 CSV（保存文件并复制到剪贴板）"}
            </button>
            <div className="grid grid-cols-2 gap-2 mb-2">
              <input
                className={inputCls}
                placeholder="按 Session ID 过滤（可选）"
                value={routeLogsSessionId}
                onChange={(e) => setRouteLogsSessionId(e.target.value)}
              />
              <button
                onClick={() => {
                  setRouteLogsOffset(0);
                  loadRecentRouteLogs(false);
                }}
                disabled={routeLogsLoading}
                className="bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-xs py-1.5 rounded"
              >
                应用过滤
              </button>
              <select
                className={inputCls}
                value={routeLogsCapabilityFilter}
                onChange={(e) => setRouteLogsCapabilityFilter(e.target.value)}
              >
                <option value="all">能力: 全部</option>
                <option value="chat">chat</option>
                <option value="vision">vision</option>
                <option value="image_gen">image_gen</option>
                <option value="audio_stt">audio_stt</option>
                <option value="audio_tts">audio_tts</option>
              </select>
              <select
                className={inputCls}
                value={routeLogsResultFilter}
                onChange={(e) => setRouteLogsResultFilter(e.target.value)}
              >
                <option value="all">结果: 全部</option>
                <option value="success">成功</option>
                <option value="failed">失败</option>
              </select>
              <select
                className={inputCls}
                value={routeLogsErrorKindFilter}
                onChange={(e) => setRouteLogsErrorKindFilter(e.target.value)}
              >
                <option value="all">错误类型: 全部</option>
                <option value="auth">auth</option>
                <option value="rate_limit">rate_limit</option>
                <option value="timeout">timeout</option>
                <option value="network">network</option>
                <option value="unknown">unknown</option>
              </select>
            </div>
            {filteredRouteLogs.length === 0 ? (
              <div className="text-xs text-gray-400">暂无路由日志</div>
            ) : (
              <div className="space-y-2 max-h-72 overflow-y-auto pr-1">
                {filteredRouteLogs.map((log, idx) => (
                  <div
                    key={`${log.created_at}-${idx}`}
                    className={"text-xs rounded px-2 py-2 border " + (log.success ? "bg-green-50 border-green-100 text-green-700" : "bg-red-50 border-red-100 text-red-700")}
                  >
                    <div>{log.created_at}</div>
                    <div>能力: {log.capability} · 协议: {log.api_format}</div>
                    <div>模型: {log.model_name}</div>
                    <div>尝试: #{log.attempt_index} / 重试: {log.retry_index}</div>
                    <div className="flex gap-2 mt-1">
                      <button
                        onClick={() => setRouteLogsSessionId(log.session_id)}
                        className="text-[11px] text-blue-600 hover:text-blue-700"
                      >
                        按此 Session 过滤
                      </button>
                      <button
                        onClick={() => navigator?.clipboard?.writeText?.(log.session_id)}
                        className="text-[11px] text-blue-600 hover:text-blue-700"
                      >
                        复制 Session ID
                      </button>
                      {!log.success && log.error_message && (
                        <button
                          onClick={() => navigator?.clipboard?.writeText?.(log.error_message)}
                          className="text-[11px] text-blue-600 hover:text-blue-700"
                        >
                          复制错误详情
                        </button>
                      )}
                    </div>
                    <div>结果: {log.success ? "成功" : `失败 (${log.error_kind || "unknown"})`}</div>
                    {!log.success && log.error_message && (
                      <div className="break-all">错误: {log.error_message}</div>
                    )}
                  </div>
                ))}
              </div>
            )}
            {routeLogsHasMore && (
              <button
                onClick={() => loadRecentRouteLogs(true)}
                disabled={routeLogsLoading}
                className="w-full mt-2 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-xs py-1.5 rounded"
              >
                {routeLogsLoading ? "加载中..." : "加载更多"}
              </button>
            )}
          </div>
        </div>
      )}

      {SHOW_MCP_SETTINGS && activeTab === "mcp" && <McpSettingsSection />}

      {activeTab === "search" && <SearchSettingsSection />}

      {activeTab === "feishu" && (
        <div className="space-y-3">
          <FeishuSettingsSection {...feishuSectionProps} />

          <details className="rounded-lg border border-gray-200 bg-white p-4">
            <summary className="cursor-pointer text-sm font-medium text-gray-900">高级设置与控制台</summary>
            <div className="mt-4 space-y-3">
              <div className="rounded-lg border border-gray-200 bg-white p-4 space-y-3">
            <div>
              <div className="text-sm font-medium text-gray-900">检查运行环境</div>
              <div className="text-xs text-gray-500 mt-1">不内置运行环境；如果电脑未安装 Node.js，系统会在这里提示你先完成安装。</div>
            </div>
            <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-3">
                <div className="text-[11px] text-gray-500">Node.js</div>
                <div className="mt-1 text-sm font-medium text-gray-900">
                  {getFeishuEnvironmentLabel(Boolean(feishuEnvironmentStatus?.node_available), "未检测到")}
                </div>
                <div className="mt-1 text-[11px] text-gray-500">{feishuEnvironmentStatus?.node_version || "请安装 Node.js LTS"}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-3">
                <div className="text-[11px] text-gray-500">npm</div>
                <div className="mt-1 text-sm font-medium text-gray-900">
                  {getFeishuEnvironmentLabel(Boolean(feishuEnvironmentStatus?.npm_available), "未检测到")}
                </div>
                <div className="mt-1 text-[11px] text-gray-500">{feishuEnvironmentStatus?.npm_version || "安装 Node.js 后通常会一起提供"}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-3">
                <div className="text-[11px] text-gray-500">飞书连接组件运行条件</div>
                <div className="mt-1 text-sm font-medium text-gray-900">
                  {feishuEnvironmentStatus?.can_start_runtime ? "已准备好" : "暂未满足"}
                </div>
                <div className="mt-1 text-[11px] text-gray-500">{feishuEnvironmentStatus?.error || "完成环境检查后即可继续后续步骤"}</div>
              </div>
            </div>
            {!feishuEnvironmentStatus?.can_start_runtime ? (
              <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
                当前电脑还没有安装飞书连接所需环境。请先安装 Node.js LTS，完成后重新打开 WorkClaw 或回到这里点击“重新检测”。
              </div>
            ) : null}
          </div>

          {feishuOnboardingEffectiveBranch !== "create_robot" ? (
            <div className="rounded-lg border border-gray-200 bg-white p-4 space-y-3">
              <div>
                <div className="text-sm font-medium text-gray-900">绑定已有机器人</div>
                <div className="text-xs text-gray-500 mt-1">这里只需要填写已有机器人的 App ID 和 App Secret；当前版本不再展示 webhook 相关配置。</div>
              </div>
              <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                <label className="space-y-1">
                  <div className="text-[11px] font-medium text-gray-700">App ID</div>
                  <input
                    value={feishuConnectorSettings.app_id}
                    onChange={(event) => setFeishuConnectorSettings((state) => ({ ...state, app_id: event.target.value }))}
                    className="w-full rounded border border-gray-200 bg-gray-50 px-3 py-2 text-sm text-gray-900"
                    placeholder="cli_xxx"
                  />
                </label>
                <label className="space-y-1">
                  <div className="text-[11px] font-medium text-gray-700">App Secret</div>
                  <input
                    type="password"
                    value={feishuConnectorSettings.app_secret}
                    onChange={(event) => setFeishuConnectorSettings((state) => ({ ...state, app_secret: event.target.value }))}
                    className="w-full rounded border border-gray-200 bg-gray-50 px-3 py-2 text-sm text-gray-900"
                    placeholder="填写机器人的 App Secret"
                  />
                </label>
              </div>
              {feishuCredentialProbe?.ok ? (
                <div className="rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-2 text-xs text-emerald-800">
                  已识别机器人
                  {feishuCredentialProbe.bot_name ? `：${feishuCredentialProbe.bot_name}` : ""}。
                  {feishuCredentialProbe.bot_open_id ? ` open_id：${feishuCredentialProbe.bot_open_id}` : ""}
                </div>
              ) : null}
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() => void handleValidateFeishuCredentials()}
                  disabled={validatingFeishuCredentials}
                  className="h-8 px-3 rounded border border-blue-200 bg-white text-xs text-blue-700 hover:bg-blue-50 disabled:bg-gray-100"
                >
                  {validatingFeishuCredentials ? "验证中..." : "验证机器人信息"}
                </button>
                <button
                  type="button"
                  onClick={() => void handleSaveFeishuConnector()}
                  disabled={savingFeishuConnector}
                  className="h-8 px-3 rounded bg-blue-600 text-xs text-white hover:bg-blue-700 disabled:bg-blue-300"
                >
                  {savingFeishuConnector ? "保存中..." : "保存并继续"}
                </button>
              </div>
            </div>
          ) : null}

          <div data-testid="feishu-authorization-step" className="rounded-lg border border-gray-200 bg-white p-4 space-y-3">
            <div>
              <div className="text-sm font-medium text-gray-900">
                {pendingFeishuPairingCount > 0 ? "批准飞书接入请求" : "完成飞书授权"}
              </div>
              <div className="text-xs text-gray-500 mt-1">
                {pendingFeishuPairingCount > 0
                  ? "飞书里的机器人已经发来了接入请求。请先在这里批准这次接入，再继续后续配置。"
                  : "安装并启动后，请回到飞书中的机器人会话按提示完成授权，然后回到这里刷新状态。"}
              </div>
            </div>
            <div className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3 text-xs text-gray-700 space-y-1">
              {pendingFeishuPairingCount > 0 ? (
                <>
                  <div>1. 飞书里已经生成了 pairing request</div>
                  <div>2. 在这里点击“批准这次接入”</div>
                  <div>3. 批准后再继续配置接待员工</div>
                </>
              ) : (
                <>
                  <div>1. 在飞书中打开机器人会话</div>
                  <div>2. 按提示完成授权</div>
                  <div>3. 如果机器人提示 access not configured，下一步回来批准接入请求</div>
                </>
              )}
            </div>
            <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">连接组件</div>
                <div className="text-sm font-medium text-gray-900">{feishuSetupProgress?.plugin_installed ? "已安装" : "未安装"}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">运行状态</div>
                <div className="text-sm font-medium text-gray-900">{officialFeishuRuntimeStatus?.running ? "运行中" : "未启动"}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">授权状态</div>
                <div className="text-sm font-medium text-gray-900">
                  {pendingFeishuPairingCount > 0
                    ? "待批准接入"
                    : feishuSetupProgress?.auth_status === "approved"
                      ? "已完成"
                      : "待完成"}
                </div>
              </div>
            </div>
            {pendingFeishuPairingRequest ? (
              <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-3 text-xs text-amber-900 space-y-1">
                <div>发送者：{pendingFeishuPairingRequest.sender_id}</div>
                <div>Pairing Code：{pendingFeishuPairingRequest.code || "未返回"}</div>
                <div>发起时间：{formatCompactDateTime(pendingFeishuPairingRequest.created_at)}</div>
              </div>
            ) : null}
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => void handleInstallAndStartFeishuConnector()}
                disabled={retryingFeishuConnector || installingOfficialFeishuPlugin}
                className="h-8 px-3 rounded bg-indigo-600 text-xs text-white hover:bg-indigo-700 disabled:bg-indigo-300"
              >
                {retryingFeishuConnector || installingOfficialFeishuPlugin
                  ? feishuAuthorizationAction.busyLabel
                  : feishuAuthorizationAction.label}
              </button>
              <button
                type="button"
                onClick={() => void handleRefreshFeishuSetup()}
                disabled={retryingFeishuConnector}
                className="h-8 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
              >
                刷新授权状态
              </button>
              {pendingFeishuPairingRequest ? (
                <>
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
                </>
              ) : null}
              <button
                type="button"
                onClick={() => void handleStartFeishuInstaller("create")}
                disabled={feishuInstallerBusy}
                className="h-8 px-3 rounded border border-indigo-200 bg-white text-xs text-indigo-700 hover:bg-indigo-50 disabled:bg-gray-100"
                >
                  {feishuInstallerBusy && feishuInstallerStartingMode === "create" ? "启动中..." : "新建机器人向导（高级）"}
              </button>
            </div>
            {feishuAuthorizationInlineError && feishuOnboardingHeaderStep !== "authorize" ? (
              <div className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700">
                {feishuAuthorizationInlineError}
              </div>
            ) : null}
            <details
              className="rounded-lg border border-gray-100 bg-gray-50 p-3"
              open={feishuInstallerSession.running || feishuInstallerSession.recent_output.length > 0}
            >
              <summary className="cursor-pointer text-xs font-medium text-gray-700">查看安装向导输出</summary>
              <div className="mt-3 space-y-3">
                <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
                  <div className="rounded border border-gray-200 bg-white px-3 py-2">
                    <div className="text-[11px] text-gray-500">向导状态</div>
                    <div className="text-sm font-medium text-gray-900">
                      {feishuInstallerBusy && feishuInstallerStartingMode ? "正在启动" : feishuInstallerSession.running ? "运行中" : "未运行"}
                    </div>
                  </div>
                  <div className="rounded border border-gray-200 bg-white px-3 py-2">
                    <div className="text-[11px] text-gray-500">当前模式</div>
                    <div className="text-sm font-medium text-gray-900">
                      {feishuInstallerDisplayMode === "create"
                        ? "新建机器人"
                        : feishuInstallerDisplayMode === "link"
                          ? "绑定已有机器人"
                          : "未启动"}
                    </div>
                  </div>
                  <div className="rounded border border-gray-200 bg-white px-3 py-2">
                    <div className="text-[11px] text-gray-500">提示</div>
                    <div className="text-sm font-medium text-gray-900">
                      {feishuInstallerStartupHint || feishuInstallerSession.prompt_hint || "暂无"}
                    </div>
                  </div>
                </div>
                <div className="rounded-lg border border-gray-900 bg-[#050816] px-3 py-3 text-xs text-gray-100">
                  <pre className="max-h-72 overflow-auto whitespace-pre-wrap break-all font-mono">
                    {feishuInstallerSession.recent_output.length > 0
                      ? feishuInstallerSession.recent_output.join("\n")
                      : feishuInstallerStartupHint || "暂无安装向导输出"}
                  </pre>
                </div>
                <div className="flex flex-col gap-2 md:flex-row">
                  <input
                    value={feishuInstallerInput}
                    onChange={(event) => setFeishuInstallerInput(event.target.value)}
                    placeholder="需要时手动输入，例如 App ID、App Secret 或回车"
                    className="flex-1 rounded border border-gray-200 bg-white px-3 py-2 text-xs text-gray-900"
                  />
                  <button
                    type="button"
                    onClick={() => void handleSendFeishuInstallerInput()}
                    disabled={feishuInstallerBusy || !feishuInstallerInput.trim()}
                    className="h-9 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
                  >
                    发送输入
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleStopFeishuInstallerSession()}
                    disabled={feishuInstallerBusy || !feishuInstallerSession.running}
                    className="h-9 px-3 rounded border border-red-200 bg-white text-xs text-red-700 hover:bg-red-50 disabled:bg-gray-100"
                  >
                    停止向导
                  </button>
                </div>
                <div className="text-[11px] text-gray-500">
                  如果你的电脑已安装 OpenClaw，当前向导也会优先命中 WorkClaw 内置的受控 openclaw shim，不会写到外部 OpenClaw 配置。
                </div>
              </div>
            </details>
          </div>

          <div className="rounded-lg border border-gray-200 bg-white p-4 space-y-3">
            <div>
              <div className="text-sm font-medium text-gray-900">接待设置</div>
              <div className="text-xs text-gray-500 mt-1">飞书接通后，还需要指定默认接待员工或配置群聊范围，消息才会稳定落到正确员工。</div>
            </div>
            <div className="rounded-lg border border-blue-100 bg-blue-50 px-3 py-3">
              <div className="text-sm font-medium text-blue-950">{feishuRoutingStatus.label}</div>
              <div className="mt-1 text-xs text-blue-900">{feishuRoutingStatus.description}</div>
            </div>
            <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">授权状态</div>
                <div className="text-sm font-medium text-gray-900">{feishuSetupProgress?.auth_status === "approved" ? "已完成" : "待完成"}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">默认接待员工</div>
                <div className="text-sm font-medium text-gray-900">{feishuSetupProgress?.default_routing_employee_name || "未设置"}</div>
              </div>
              <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
                <div className="text-[11px] text-gray-500">群聊范围规则</div>
                <div className="text-sm font-medium text-gray-900">{feishuSetupProgress?.scoped_routing_count ?? 0} 条</div>
              </div>
            </div>
            <div className="rounded-lg border border-blue-100 bg-blue-50 px-3 py-2 text-xs text-blue-800">
              接待员工的具体配置入口在员工详情页。完成当前接入后，请关闭设置窗口并前往员工详情中的“飞书接待”继续配置。
            </div>
          <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => onOpenEmployees?.()}
                className="h-8 px-3 rounded border border-blue-200 bg-white text-xs text-blue-700 hover:bg-blue-50"
              >
                {feishuRoutingStatus.actionLabel}
              </button>
            </div>
          </div>
          </div>
        </details>

          <FeishuAdvancedSection
            connectionDetailSummary={feishuConnectionDetailSummary}
            feishuAdvancedSettings={feishuAdvancedSettings}
            setFeishuAdvancedSettings={setFeishuAdvancedSettings}
            connectionStatusLabel={feishuConnectorStatus.label}
            pluginVersionLabel={feishuSetupProgress?.plugin_version || primaryPluginChannelHost?.version || "未识别"}
            currentAccountLabel={primaryPluginChannelSnapshot?.snapshot.defaultAccountId || "未识别"}
            pendingPairingCount={pendingFeishuPairingCount}
            lastEventAtLabel={formatCompactDateTime(officialFeishuRuntimeStatus?.last_event_at)}
            recentIssueLabel={summarizeConnectorIssue(feishuConnectorStatus.error)}
            runtimeLogsLabel={summarizeOfficialFeishuRuntimeLogs(officialFeishuRuntimeStatus)}
            retryingFeishuConnector={retryingFeishuConnector}
            savingFeishuAdvancedSettings={savingFeishuAdvancedSettings}
            onRefreshFeishuSetup={handleRefreshFeishuSetup}
            onSaveFeishuAdvancedSettings={handleSaveFeishuAdvancedSettings}
            onCopyDiagnostics={handleCopyFeishuDiagnostics}
          />
        </div>
      )}


      {SHOW_AUTO_ROUTING_SETTINGS && activeTab === "routing" && <RoutingSettingsSection />}

    </SettingsShell>
  );
}
