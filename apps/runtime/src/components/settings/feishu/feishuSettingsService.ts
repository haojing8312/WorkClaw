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

export function loadFeishuGatewaySettings() {
  return invoke<FeishuGatewaySettings>("get_feishu_gateway_settings");
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
