import { openExternalUrl } from "../../../utils/openExternalUrl";
import type {
  FeishuGatewaySettings,
  OpenClawPluginFeishuAdvancedSettings,
} from "../../../types";
import {
  approveFeishuPairingRequest as approveFeishuPairingRequestFromService,
  denyFeishuPairingRequest as denyFeishuPairingRequestFromService,
  getFeishuErrorMessage,
  installOpenClawLarkPlugin as installOpenClawLarkPluginFromService,
  probeFeishuCredentials as probeFeishuCredentialsFromService,
  saveFeishuGatewaySettings as saveFeishuGatewaySettingsFromService,
  saveFeishuAdvancedSettings as saveFeishuAdvancedSettingsFromService,
  sendFeishuInstallerInput as sendFeishuInstallerInputFromService,
  stopFeishuInstallerSession as stopFeishuInstallerSessionFromService,
} from "./feishuSettingsService";
import type { FeishuSettingsControllerActionDeps } from "./feishuSettingsControllerActionTypes";
import { createFeishuSettingsControllerLoaders } from "./feishuSettingsControllerLoaders";
import { createFeishuSettingsControllerRuntimeActions } from "./feishuSettingsControllerRuntimeActions";
import { DEFAULT_FEISHU_INSTALLER_SESSION } from "./useFeishuInstallerSessionController";

const FEISHU_OFFICIAL_PLUGIN_DOC_URL =
  "https://bytedance.larkoffice.com/docx/MFK7dDFLFoVlOGxWCv5cTXKmnMh#M0usd9GLwoiBxtx1UyjcpeMhnRe";

export function createFeishuSettingsControllerActions(deps: FeishuSettingsControllerActionDeps) {
  const {
    loadConnectorSettings,
    loadConnectorPlatformData,
    applyOfficialFeishuRuntimeStatus,
  } = createFeishuSettingsControllerLoaders(deps);

  function getPrimaryFeishuPluginId() {
    return deps.pluginChannelHosts.find((host) => host.channel === "feishu")?.plugin_id || "openclaw-lark";
  }

  function maybeInstallOfficialPlugin() {
    if (!deps.pluginChannelHosts.some((host) => host.status === "ready") && !deps.feishuSetupProgress?.plugin_installed) {
      return installOpenClawLarkPluginFromService();
    }
    return null;
  }

  async function refreshFeishuSetupData() {
    await loadConnectorPlatformData();
    await deps.loadFeishuSetupProgress();
  }

  const {
    handleStartFeishuInstaller,
    handleRetryFeishuConnector,
    handleInstallAndStartFeishuConnector,
  } = createFeishuSettingsControllerRuntimeActions(deps, {
    getPrimaryFeishuPluginId,
    maybeInstallOfficialPlugin,
    refreshFeishuSetupData,
    applyOfficialFeishuRuntimeStatus,
  });

  function updateFeishuConnectorSettings(patch: Partial<FeishuGatewaySettings>) {
    deps.setFeishuConnectorSettings((state) => ({
      ...state,
      ...patch,
    }));
  }

  function updateFeishuAdvancedSettings(patch: Partial<OpenClawPluginFeishuAdvancedSettings>) {
    deps.setFeishuAdvancedSettings((state) => ({
      ...state,
      ...patch,
    }));
  }

  function updateFeishuInstallerInput(value: string) {
    deps.setFeishuInstallerInput(value);
  }

  async function handleValidateFeishuCredentials() {
    const appId = deps.feishuConnectorSettings.app_id.trim();
    const appSecret = deps.feishuConnectorSettings.app_secret.trim();
    if (!appId || !appSecret) {
      deps.setFeishuConnectorError("请先填写已有机器人的 App ID 和 App Secret");
      return;
    }

    deps.setValidatingFeishuCredentials(true);
    deps.setFeishuConnectorNotice("");
    deps.setFeishuConnectorError("");
    try {
      const probe = await probeFeishuCredentialsFromService(appId, appSecret);
      if (!probe.ok) {
        deps.setFeishuCredentialProbe(null);
        deps.setFeishuConnectorError(`已有机器人校验失败: ${probe.error || "无法获取机器人信息"}`);
        return;
      }
      deps.setFeishuCredentialProbe(probe);
      const botLabel = probe.bot_name?.trim() ? `（${probe.bot_name.trim()}）` : "";
      deps.setFeishuConnectorNotice(`机器人信息验证成功${botLabel}`);
    } catch (error) {
      deps.setFeishuCredentialProbe(null);
      deps.setFeishuConnectorError("验证机器人信息失败: " + String(error));
    } finally {
      deps.setValidatingFeishuCredentials(false);
    }
  }

  async function handleSaveFeishuConnector() {
    deps.setSavingFeishuConnector(true);
    deps.setFeishuConnectorNotice("");
    deps.setFeishuConnectorError("");
    try {
      const saved = await saveFeishuGatewaySettingsFromService(deps.feishuConnectorSettings);
      deps.setFeishuConnectorSettings(saved);
      await deps.loadConnectorStatuses();
      await loadConnectorPlatformData();
      await deps.loadFeishuSetupProgress();
      deps.setFeishuConnectorNotice("飞书官方插件配置已保存");
    } catch (error) {
      deps.setFeishuConnectorError("保存飞书官方插件配置失败: " + String(error));
    } finally {
      deps.setSavingFeishuConnector(false);
    }
  }

  async function handleSaveFeishuAdvancedSettings() {
    deps.setSavingFeishuAdvancedSettings(true);
    deps.setFeishuConnectorNotice("");
    deps.setFeishuConnectorError("");
    try {
      const saved = await saveFeishuAdvancedSettingsFromService(deps.feishuAdvancedSettings);
      deps.setFeishuAdvancedSettings(saved);
      await refreshFeishuSetupData();
      deps.setFeishuConnectorNotice("飞书高级配置已保存");
    } catch (error) {
      deps.setFeishuConnectorError("保存飞书高级配置失败: " + String(error));
    } finally {
      deps.setSavingFeishuAdvancedSettings(false);
    }
  }

  async function handleSendFeishuInstallerInput() {
    const input = deps.feishuInstallerInput.trim();
    if (!input) {
      return;
    }
    deps.setFeishuInstallerBusy(true);
    deps.setFeishuConnectorError("");
    try {
      const status = await sendFeishuInstallerInputFromService(input);
      deps.setFeishuInstallerSession(status ?? DEFAULT_FEISHU_INSTALLER_SESSION);
      deps.setFeishuInstallerInput("");
    } catch (error) {
      deps.setFeishuConnectorError("发送安装向导输入失败: " + String(error));
    } finally {
      deps.setFeishuInstallerBusy(false);
    }
  }

  async function handleStopFeishuInstallerSession() {
    deps.setFeishuInstallerBusy(true);
    deps.setFeishuConnectorError("");
    try {
      const status = await stopFeishuInstallerSessionFromService();
      deps.setFeishuInstallerSession(status ?? DEFAULT_FEISHU_INSTALLER_SESSION);
      deps.setFeishuConnectorNotice("已停止飞书官方安装向导");
    } catch (error) {
      deps.setFeishuConnectorError("停止飞书官方安装向导失败: " + String(error));
    } finally {
      deps.setFeishuInstallerBusy(false);
    }
  }

  async function handleInstallOfficialFeishuPlugin() {
    deps.setInstallingOfficialFeishuPlugin(true);
    deps.setFeishuConnectorNotice("");
    deps.setFeishuConnectorError("");
    try {
      await installOpenClawLarkPluginFromService();
      await refreshFeishuSetupData();
      deps.setFeishuConnectorNotice("飞书官方插件已安装");
    } catch (error) {
      deps.setFeishuConnectorError("安装飞书官方插件失败: " + String(error));
    } finally {
      deps.setInstallingOfficialFeishuPlugin(false);
    }
  }

  async function handleResolveFeishuPairingRequest(requestId: string, action: "approve" | "deny") {
    deps.setFeishuPairingActionLoading(action);
    deps.setFeishuConnectorNotice("");
    deps.setFeishuConnectorError("");
    try {
      if (action === "approve") {
        await approveFeishuPairingRequestFromService(requestId);
      } else {
        await denyFeishuPairingRequestFromService(requestId);
      }
      await refreshFeishuSetupData();
      deps.setFeishuConnectorNotice(action === "approve" ? "已批准飞书接入请求" : "已拒绝飞书接入请求");
    } catch (error) {
      deps.setFeishuConnectorError(`${action === "approve" ? "批准" : "拒绝"}飞书接入请求失败: ${String(error)}`);
    } finally {
      deps.setFeishuPairingActionLoading(null);
    }
  }

  async function handleOpenFeishuOfficialDocs() {
    try {
      await openExternalUrl(FEISHU_OFFICIAL_PLUGIN_DOC_URL);
    } catch (error) {
      deps.setFeishuConnectorError(getFeishuErrorMessage(error, "打开官方文档失败，请稍后重试"));
    }
  }

  async function handleRefreshFeishuSetup() {
    deps.setRetryingFeishuConnector(true);
    deps.setFeishuConnectorNotice("");
    deps.setFeishuConnectorError("");
    try {
      await Promise.all([
        loadConnectorSettings(),
        deps.loadConnectorStatuses(),
        loadConnectorPlatformData(),
        deps.loadFeishuSetupProgress(),
      ]);
    } catch (error) {
      deps.setFeishuConnectorError("刷新飞书接入状态失败: " + String(error));
    } finally {
      deps.setRetryingFeishuConnector(false);
    }
  }

  return {
    applyOfficialFeishuRuntimeStatus,
    updateFeishuConnectorSettings,
    updateFeishuAdvancedSettings,
    updateFeishuInstallerInput,
    loadConnectorSettings,
    loadConnectorPlatformData,
    handleValidateFeishuCredentials,
    handleSaveFeishuConnector,
    handleSaveFeishuAdvancedSettings,
    handleStartFeishuInstaller,
    handleSendFeishuInstallerInput,
    handleStopFeishuInstallerSession,
    handleRetryFeishuConnector,
    handleInstallOfficialFeishuPlugin,
    handleResolveFeishuPairingRequest,
    handleInstallAndStartFeishuConnector,
    handleOpenFeishuOfficialDocs,
    handleRefreshFeishuSetup,
  };
}
