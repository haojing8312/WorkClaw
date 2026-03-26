import { useEffect, useState } from "react";
import { type SettingsTabName } from "../SettingsTabNav";
import {
  loadFeishuInstallerSessionStatus as loadFeishuInstallerSessionStatusFromService,
} from "./feishuSettingsService";
import type {
  OpenClawLarkInstallerSessionStatus,
} from "../../../types";

export const DEFAULT_FEISHU_INSTALLER_SESSION: OpenClawLarkInstallerSessionStatus = {
  running: false,
  mode: null,
  started_at: null,
  last_output_at: null,
  last_error: null,
  prompt_hint: null,
  recent_output: [],
};

interface UseFeishuInstallerSessionControllerOptions {
  activeTab: SettingsTabName;
}

export function useFeishuInstallerSessionController({
  activeTab,
}: UseFeishuInstallerSessionControllerOptions) {
  const [feishuInstallerSession, setFeishuInstallerSession] = useState<OpenClawLarkInstallerSessionStatus>(
    DEFAULT_FEISHU_INSTALLER_SESSION,
  );

  async function loadFeishuInstallerSessionStatus() {
    try {
      const status = await loadFeishuInstallerSessionStatusFromService();
      setFeishuInstallerSession(status ?? DEFAULT_FEISHU_INSTALLER_SESSION);
    } catch (error) {
      console.warn("加载飞书官方安装向导状态失败:", error);
    }
  }

  useEffect(() => {
    if (activeTab !== "feishu") {
      return;
    }

    void loadFeishuInstallerSessionStatus();
  }, [activeTab]);

  useEffect(() => {
    if (activeTab !== "feishu" || !feishuInstallerSession.running) {
      return;
    }

    const timer = window.setInterval(() => {
      void loadFeishuInstallerSessionStatus();
    }, 1500);

    return () => window.clearInterval(timer);
  }, [activeTab, feishuInstallerSession.running]);

  return {
    feishuInstallerSession,
    setFeishuInstallerSession,
    loadFeishuInstallerSessionStatus,
  };
}
