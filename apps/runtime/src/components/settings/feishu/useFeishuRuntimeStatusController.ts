import { useEffect, useState } from "react";
import { type SettingsTabName } from "../SettingsTabNav";
import {
  loadFeishuRuntimeStatus as loadFeishuRuntimeStatusFromService,
} from "./feishuSettingsService";
import type {
  OpenClawPluginFeishuRuntimeStatus,
} from "../../../types";

interface UseFeishuRuntimeStatusControllerOptions {
  activeTab: SettingsTabName;
}

export function useFeishuRuntimeStatusController({
  activeTab,
}: UseFeishuRuntimeStatusControllerOptions) {
  const [officialFeishuRuntimeStatus, setOfficialFeishuRuntimeStatus] =
    useState<OpenClawPluginFeishuRuntimeStatus | null>(null);

  async function loadConnectorStatuses() {
    try {
      const runtimeStatus = await loadFeishuRuntimeStatusFromService();
      setOfficialFeishuRuntimeStatus(runtimeStatus);
    } catch (error) {
      console.warn("加载渠道连接器状态失败:", error);
      setOfficialFeishuRuntimeStatus(null);
    }
  }

  useEffect(() => {
    if (activeTab !== "feishu") {
      return;
    }

    void loadConnectorStatuses();
  }, [activeTab]);

  useEffect(() => {
    if (activeTab !== "feishu") {
      return;
    }

    const timer = window.setInterval(() => {
      void loadConnectorStatuses();
    }, 5000);

    return () => window.clearInterval(timer);
  }, [activeTab]);

  return {
    officialFeishuRuntimeStatus,
    setOfficialFeishuRuntimeStatus,
    loadConnectorStatuses,
  };
}
