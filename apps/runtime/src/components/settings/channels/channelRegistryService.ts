import { invoke } from "@tauri-apps/api/core";
import type {
  ImChannelRegistryEntry,
  OpenClawPluginFeishuRuntimeStatus,
  WecomConnectorStatus,
  WecomGatewaySettings,
} from "../../../types";

export function loadImChannelRegistry() {
  return invoke<ImChannelRegistryEntry[]>("list_im_channel_registry");
}

export function findImChannelRegistryEntry(
  entries: ImChannelRegistryEntry[] | null | undefined,
  channel: string,
) {
  const normalizedChannel = channel.trim().toLowerCase();
  return (
    (Array.isArray(entries) ? entries : []).find(
      (entry) => entry.channel.trim().toLowerCase() === normalizedChannel,
    ) ?? null
  );
}

export function extractFeishuRegistryEntry(entries: ImChannelRegistryEntry[] | null | undefined) {
  return findImChannelRegistryEntry(entries, "feishu");
}

export function extractWecomRegistryEntry(entries: ImChannelRegistryEntry[] | null | undefined) {
  return findImChannelRegistryEntry(entries, "wecom");
}

export function extractFeishuRuntimeStatusFromEntry(
  entry: ImChannelRegistryEntry | null | undefined,
) {
  return (entry?.runtime_status as OpenClawPluginFeishuRuntimeStatus | null | undefined) ?? null;
}

export function extractWecomRuntimeStatusFromEntry(
  entry: ImChannelRegistryEntry | null | undefined,
) {
  return (entry?.runtime_status as WecomConnectorStatus | null | undefined) ?? null;
}

export function loadWecomGatewaySettings() {
  return invoke<WecomGatewaySettings>("get_wecom_gateway_settings");
}

export async function saveWecomGatewaySettings(settings: WecomGatewaySettings) {
  await invoke("set_wecom_gateway_settings", { settings });
  return loadWecomGatewaySettings();
}

export function startWecomConnector(settings?: Partial<WecomGatewaySettings>) {
  return invoke<string>("start_wecom_connector", {
    sidecarBaseUrl: settings?.sidecar_base_url || null,
    corpId: settings?.corp_id || null,
    agentId: settings?.agent_id || null,
    agentSecret: settings?.agent_secret || null,
  });
}

export function setImChannelHostRunning(channel: string, desiredRunning: boolean) {
  return invoke<ImChannelRegistryEntry>("set_im_channel_host_running", {
    channel,
    desiredRunning,
  });
}
