import { useEffect, useState } from "react";
import type { SettingsTabName } from "../SettingsTabNav";
import {
  loadFeishuSetupProgress as loadFeishuSetupProgressFromService,
} from "./feishuSettingsService";
import type {
  FeishuPluginEnvironmentStatus,
  FeishuSetupProgress,
} from "../../../types";

interface UseFeishuSetupProgressControllerOptions {
  activeTab: SettingsTabName;
}

export function useFeishuSetupProgressController({
  activeTab,
}: UseFeishuSetupProgressControllerOptions) {
  const [feishuEnvironmentStatus, setFeishuEnvironmentStatus] = useState<FeishuPluginEnvironmentStatus | null>(null);
  const [feishuSetupProgress, setFeishuSetupProgress] = useState<FeishuSetupProgress | null>(null);

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
    } catch (error) {
      console.warn("加载飞书接入进度失败:", error);
      setFeishuEnvironmentStatus(null);
      setFeishuSetupProgress(null);
    }
  }

  useEffect(() => {
    if (activeTab !== "feishu") {
      return;
    }

    void loadFeishuSetupProgress();
  }, [activeTab]);

  useEffect(() => {
    if (activeTab !== "feishu") {
      return;
    }

    const timer = window.setInterval(() => {
      void loadFeishuSetupProgress();
    }, 5000);

    return () => window.clearInterval(timer);
  }, [activeTab]);

  return {
    feishuEnvironmentStatus,
    feishuSetupProgress,
    loadFeishuSetupProgress,
  };
}
