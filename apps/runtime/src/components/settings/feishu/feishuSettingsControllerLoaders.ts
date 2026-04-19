import type {
  ImChannelRegistryEntry,
  OpenClawPluginChannelHost,
  OpenClawPluginFeishuRuntimeStatus,
} from "../../../types";
import { loadImChannelRegistry } from "../channels/channelRegistryService";
import type { FeishuSettingsControllerActionDeps } from "./feishuSettingsControllerActionTypes";
import {
  loadFeishuAdvancedSettings as loadFeishuAdvancedSettingsFromService,
  loadFeishuGatewaySettings as loadFeishuGatewaySettingsFromService,
  loadFeishuPairingRequests as loadFeishuPairingRequestsFromService,
  normalizeFeishuAdvancedSettings,
  normalizeFeishuGatewaySettings,
} from "./feishuSettingsService";

function normalizeFeishuHosts(hosts: OpenClawPluginChannelHost[]) {
  return hosts.filter(
    (host) =>
      host.channel === "feishu" ||
      host.plugin_id === "openclaw-lark" ||
      host.npm_spec === "@larksuite/openclaw-lark" ||
      host.display_name.toLowerCase().includes("feishu") ||
      host.display_name.toLowerCase().includes("lark"),
  );
}

function extractFeishuHostsFromRegistry(entries: ImChannelRegistryEntry[]) {
  return normalizeFeishuHosts(
    entries
      .filter((entry) => entry.channel === "feishu")
      .map((entry) => entry.plugin_host)
      .filter((host): host is OpenClawPluginChannelHost => Boolean(host)),
  );
}

export function createFeishuSettingsControllerLoaders(deps: FeishuSettingsControllerActionDeps) {
  async function loadConnectorSettings() {
    try {
      const [feishuSettings, feishuAdvanced] = await Promise.all([
        loadFeishuGatewaySettingsFromService(),
        loadFeishuAdvancedSettingsFromService(),
      ]);
      deps.setFeishuConnectorSettings(normalizeFeishuGatewaySettings(feishuSettings));
      deps.setFeishuAdvancedSettings(normalizeFeishuAdvancedSettings(feishuAdvanced));
    } catch (error) {
      console.warn("加载渠道连接器配置失败:", error);
    }
  }

  async function loadConnectorPlatformData() {
    const [registryResult, pairingResult] = await Promise.allSettled([
      loadImChannelRegistry(),
      loadFeishuPairingRequestsFromService(),
    ]);

    const normalizedHosts =
      registryResult.status === "fulfilled"
        ? extractFeishuHostsFromRegistry(Array.isArray(registryResult.value) ? registryResult.value : [])
        : [];
    if (registryResult.status !== "fulfilled") {
      console.warn("加载飞书渠道宿主总览失败:", registryResult.reason);
    }
    deps.setPluginChannelHosts(normalizedHosts);
    deps.setPluginChannelHostsError(registryResult.status === "fulfilled" ? "" : "飞书宿主状态暂时不可用");

    if (pairingResult.status !== "fulfilled") {
      console.warn("加载飞书配对请求失败:", pairingResult.reason);
    }
    deps.setFeishuPairingRequests(
      pairingResult.status === "fulfilled" && Array.isArray(pairingResult.value) ? pairingResult.value : [],
    );
    deps.setFeishuPairingRequestsError(pairingResult.status === "fulfilled" ? "" : "配对记录加载失败");
  }

  function applyOfficialFeishuRuntimeStatus(
    status: OpenClawPluginFeishuRuntimeStatus | null | undefined,
    options?: { showStartErrorNotice?: boolean },
  ) {
    if (!status) {
      return;
    }
    deps.setOfficialFeishuRuntimeStatus(status);
    if (options?.showStartErrorNotice && !status.running && status.last_error?.trim()) {
      deps.setFeishuConnectorError(`官方插件启动失败: ${status.last_error.trim()}`);
    }
  }

  return {
    loadConnectorSettings,
    loadConnectorPlatformData,
    applyOfficialFeishuRuntimeStatus,
  };
}
