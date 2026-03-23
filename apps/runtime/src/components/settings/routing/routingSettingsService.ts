import { invoke } from "@tauri-apps/api/core";

export interface RoutingSettings {
  max_call_depth: number;
  node_timeout_seconds: number;
  retry_count: number;
}

export async function loadRoutingSettings() {
  return invoke<RoutingSettings>("get_routing_settings");
}

export async function saveRoutingSettings(settings: RoutingSettings) {
  await invoke("set_routing_settings", {
    settings: {
      max_call_depth: Math.max(2, Math.min(8, settings.max_call_depth)),
      node_timeout_seconds: Math.max(5, Math.min(600, settings.node_timeout_seconds)),
      retry_count: Math.max(0, Math.min(2, settings.retry_count)),
    },
  });
}
