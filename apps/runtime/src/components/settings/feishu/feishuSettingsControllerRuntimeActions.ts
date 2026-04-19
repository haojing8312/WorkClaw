import type {
  OpenClawLarkInstallerMode,
  OpenClawPluginFeishuRuntimeStatus,
} from "../../../types";
import { DEFAULT_FEISHU_INSTALLER_SESSION } from "./useFeishuInstallerSessionController";
import {
  extractFeishuRuntimeStatusFromEntry,
  setImChannelHostRunning,
} from "../channels/channelRegistryService";
import {
  saveFeishuGatewaySettings as saveFeishuGatewaySettingsFromService,
  startFeishuInstallerSession as startFeishuInstallerSessionFromService,
} from "./feishuSettingsService";
import type { FeishuSettingsControllerActionDeps } from "./feishuSettingsControllerActionTypes";

interface FeishuSettingsControllerRuntimeActionHelpers {
  maybeInstallOfficialPlugin: () => Promise<unknown> | null;
  refreshFeishuSetupData: () => Promise<void>;
  applyOfficialFeishuRuntimeStatus: (
    status: OpenClawPluginFeishuRuntimeStatus | null | undefined,
    options?: { showStartErrorNotice?: boolean },
  ) => void;
}

export function createFeishuSettingsControllerRuntimeActions(
  deps: FeishuSettingsControllerActionDeps,
  helpers: FeishuSettingsControllerRuntimeActionHelpers,
) {
  async function handleStartFeishuInstaller(mode: OpenClawLarkInstallerMode) {
    deps.setFeishuInstallerBusy(true);
    deps.setFeishuInstallerStartingMode(mode);
    deps.setFeishuConnectorNotice("");
    deps.setFeishuConnectorError("");
    try {
      const installPromise = helpers.maybeInstallOfficialPlugin();
      if (installPromise) {
        await installPromise;
      }
      const status = await startFeishuInstallerSessionFromService(
        mode,
        mode === "link" ? deps.feishuConnectorSettings.app_id.trim() : null,
        mode === "link" ? deps.feishuConnectorSettings.app_secret.trim() : null,
      );
      deps.setFeishuInstallerSession(status ?? DEFAULT_FEISHU_INSTALLER_SESSION);
      deps.setFeishuInstallerInput("");
      await helpers.refreshFeishuSetupData();
      deps.setFeishuConnectorNotice(mode === "create" ? "已启动飞书官方创建机器人向导" : "已启动飞书官方绑定机器人向导");
    } catch (error) {
      deps.setFeishuConnectorError(
        `${mode === "create" ? "启动飞书官方创建机器人向导" : "启动飞书官方绑定机器人向导"}失败: ${String(error)}`,
      );
    } finally {
      deps.setFeishuInstallerBusy(false);
      deps.setFeishuInstallerStartingMode(null);
    }
  }

  async function handleRetryFeishuConnector() {
    deps.setRetryingFeishuConnector(true);
    deps.setFeishuConnectorNotice("");
    deps.setFeishuConnectorError("");
    try {
      const registryEntry = await setImChannelHostRunning("feishu", true);
      const runtimeStatus = extractFeishuRuntimeStatusFromEntry(registryEntry);
      if (runtimeStatus) {
        helpers.applyOfficialFeishuRuntimeStatus(runtimeStatus, {
          showStartErrorNotice: true,
        });
      } else {
        await deps.loadConnectorStatuses();
      }
      await helpers.refreshFeishuSetupData();
      deps.setFeishuConnectorNotice(
        runtimeStatus ? (runtimeStatus.running ? "已触发飞书官方插件启动" : "已刷新飞书官方插件状态") : "已触发飞书官方插件启动",
      );
    } catch (error) {
      deps.setFeishuConnectorError("刷新飞书官方插件状态失败: " + String(error));
    } finally {
      deps.setRetryingFeishuConnector(false);
    }
  }

  async function handleInstallAndStartFeishuConnector() {
    deps.setRetryingFeishuConnector(true);
    deps.setFeishuConnectorNotice("");
    deps.setFeishuConnectorError("");
    try {
      if (!deps.feishuConnectorSettings.app_id.trim() || !deps.feishuConnectorSettings.app_secret.trim()) {
        deps.setFeishuConnectorError("请先填写并保存已有机器人的 App ID 和 App Secret");
        return;
      }

      const saved = await saveFeishuGatewaySettingsFromService(deps.feishuConnectorSettings);
      deps.setFeishuConnectorSettings(saved);

      const installPromise = helpers.maybeInstallOfficialPlugin();
      if (installPromise) {
        await installPromise;
      }

      const registryEntry = await setImChannelHostRunning("feishu", true);
      const runtimeStatus = extractFeishuRuntimeStatusFromEntry(registryEntry);
      if (runtimeStatus) {
        helpers.applyOfficialFeishuRuntimeStatus(runtimeStatus, { showStartErrorNotice: true });
      }
      await deps.loadConnectorStatuses();
      await helpers.refreshFeishuSetupData();
      deps.setFeishuConnectorNotice(runtimeStatus?.running ? "飞书连接组件已启动" : "已尝试启动飞书连接组件");
    } catch (error) {
      deps.setFeishuConnectorError("安装并启动飞书连接失败: " + String(error));
    } finally {
      deps.setRetryingFeishuConnector(false);
    }
  }

  return {
    handleStartFeishuInstaller,
    handleRetryFeishuConnector,
    handleInstallAndStartFeishuConnector,
  };
}
