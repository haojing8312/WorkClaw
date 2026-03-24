import { invoke } from "@tauri-apps/api/core";
import type {
  FeishuGatewaySettings,
  FeishuPairingRequestRecord,
  FeishuSetupProgress,
  OpenClawLarkInstallerMode,
  OpenClawLarkInstallerSessionStatus,
  OpenClawPluginChannelHost,
  OpenClawPluginChannelSnapshotResult,
  OpenClawPluginFeishuAdvancedSettings,
  OpenClawPluginFeishuCredentialProbeResult,
  OpenClawPluginFeishuRuntimeStatus,
  OpenClawPluginInstallRecord,
} from "../../../types";

export function getFeishuErrorMessage(error: unknown, fallback: string) {
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

export function loadFeishuGatewaySettings() {
  return invoke<FeishuGatewaySettings>("get_feishu_gateway_settings");
}

export function normalizeFeishuGatewaySettings(
  settings: FeishuGatewaySettings | null | undefined,
): FeishuGatewaySettings {
  return {
    app_id: settings?.app_id || "",
    app_secret: settings?.app_secret || "",
    ingress_token: settings?.ingress_token || "",
    encrypt_key: settings?.encrypt_key || "",
    sidecar_base_url: settings?.sidecar_base_url || "",
  };
}

export async function saveFeishuGatewaySettings(settings: FeishuGatewaySettings) {
  await invoke("set_feishu_gateway_settings", {
    settings,
  });
  return invoke<FeishuGatewaySettings>("get_feishu_gateway_settings");
}

export function loadFeishuAdvancedSettings() {
  return invoke<OpenClawPluginFeishuAdvancedSettings>("get_openclaw_plugin_feishu_advanced_settings");
}

export function normalizeFeishuAdvancedSettings(
  settings: OpenClawPluginFeishuAdvancedSettings | null | undefined,
): OpenClawPluginFeishuAdvancedSettings {
  return {
    groups_json: settings?.groups_json || "",
    dms_json: settings?.dms_json || "",
    footer_json: settings?.footer_json || "",
    account_overrides_json: settings?.account_overrides_json || "",
    render_mode: settings?.render_mode || "",
    streaming: settings?.streaming || "",
    text_chunk_limit: settings?.text_chunk_limit || "",
    chunk_mode: settings?.chunk_mode || "",
    reply_in_thread: settings?.reply_in_thread || "",
    group_session_scope: settings?.group_session_scope || "",
    topic_session_mode: settings?.topic_session_mode || "",
    markdown_mode: settings?.markdown_mode || "",
    markdown_table_mode: settings?.markdown_table_mode || "",
    heartbeat_visibility: settings?.heartbeat_visibility || "",
    heartbeat_interval_ms: settings?.heartbeat_interval_ms || "",
    media_max_mb: settings?.media_max_mb || "",
    http_timeout_ms: settings?.http_timeout_ms || "",
    config_writes: settings?.config_writes || "",
    webhook_host: settings?.webhook_host || "",
    webhook_port: settings?.webhook_port || "",
    dynamic_agent_creation_enabled: settings?.dynamic_agent_creation_enabled || "",
    dynamic_agent_creation_workspace_template: settings?.dynamic_agent_creation_workspace_template || "",
    dynamic_agent_creation_agent_dir_template: settings?.dynamic_agent_creation_agent_dir_template || "",
    dynamic_agent_creation_max_agents: settings?.dynamic_agent_creation_max_agents || "",
  };
}

export function saveFeishuAdvancedSettings(settings: OpenClawPluginFeishuAdvancedSettings) {
  return invoke<OpenClawPluginFeishuAdvancedSettings>("set_openclaw_plugin_feishu_advanced_settings", {
    settings,
  });
}

export function loadFeishuSetupProgress() {
  return invoke<FeishuSetupProgress | null>("get_feishu_setup_progress");
}

export function loadFeishuRuntimeStatus() {
  return invoke<OpenClawPluginFeishuRuntimeStatus>("get_openclaw_plugin_feishu_runtime_status");
}

export function loadFeishuInstallerSessionStatus() {
  return invoke<OpenClawLarkInstallerSessionStatus | null>("get_openclaw_lark_installer_session_status");
}

export function loadFeishuPluginChannelHosts() {
  return invoke<OpenClawPluginChannelHost[]>("list_openclaw_plugin_channel_hosts");
}

export function loadFeishuPairingRequests() {
  return invoke<FeishuPairingRequestRecord[]>("list_feishu_pairing_requests", {
    status: null,
  });
}

export function loadFeishuPluginChannelSnapshot(pluginId: string) {
  return invoke<OpenClawPluginChannelSnapshotResult>("get_openclaw_plugin_feishu_channel_snapshot", {
    pluginId,
  });
}

export function probeFeishuCredentials(appId: string, appSecret: string) {
  return invoke<OpenClawPluginFeishuCredentialProbeResult>("probe_openclaw_plugin_feishu_credentials", {
    appId,
    appSecret,
  });
}

export function installOpenClawLarkPlugin() {
  return invoke<OpenClawPluginInstallRecord>("install_openclaw_plugin_from_npm", {
    pluginId: "openclaw-lark",
    npmSpec: "@larksuite/openclaw-lark",
  });
}

export function startFeishuInstallerSession(
  mode: OpenClawLarkInstallerMode,
  appId: string | null,
  appSecret: string | null,
) {
  return invoke<OpenClawLarkInstallerSessionStatus | null>("start_openclaw_lark_installer_session", {
    mode,
    appId,
    appSecret,
  });
}

export function sendFeishuInstallerInput(input: string) {
  return invoke<OpenClawLarkInstallerSessionStatus | null>("send_openclaw_lark_installer_input", {
    input,
  });
}

export function stopFeishuInstallerSession() {
  return invoke<OpenClawLarkInstallerSessionStatus | null>("stop_openclaw_lark_installer_session");
}

export function startFeishuRuntime(pluginId: string, accountId: string | null) {
  return invoke<OpenClawPluginFeishuRuntimeStatus | null>("start_openclaw_plugin_feishu_runtime", {
    pluginId,
    accountId,
  });
}

export function approveFeishuPairingRequest(requestId: string) {
  return invoke<FeishuPairingRequestRecord>("approve_feishu_pairing_request", {
    requestId,
    resolvedByUser: "settings-ui",
  });
}

export function denyFeishuPairingRequest(requestId: string) {
  return invoke<FeishuPairingRequestRecord>("deny_feishu_pairing_request", {
    requestId,
    resolvedByUser: "settings-ui",
  });
}
