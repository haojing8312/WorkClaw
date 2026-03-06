import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();
const useAppUpdaterMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("../../hooks/useAppUpdater", () => ({
  useAppUpdater: (...args: unknown[]) => useAppUpdaterMock(...args),
}));

function createRuntimePreferences() {
  return {
    default_work_dir: "E:\\workspace",
    default_language: "zh-CN",
    immersive_translation_enabled: true,
    immersive_translation_display: "translated_only",
    immersive_translation_trigger: "auto",
    translation_engine: "model_then_free",
    translation_model_id: "",
    auto_update_enabled: true,
    update_channel: "stable",
    dismissed_update_version: "",
    last_update_check_at: "",
    launch_at_login: false,
    launch_minimized: false,
    close_to_tray: true,
  };
}

function createUpdaterState() {
  return {
    status: "idle",
    error: "",
    availableUpdate: null,
    downloadProgress: {
      contentLength: null,
      downloadedBytes: 0,
      percent: null,
    },
    dismissedVersion: "",
    lastCheckedAt: "",
    isWorking: false,
    canDismiss: false,
    canDownload: false,
    canInstall: false,
    checkForUpdates: vi.fn(async () => null),
    dismissUpdate: vi.fn(),
    downloadUpdate: vi.fn(async () => undefined),
    installUpdate: vi.fn(async () => undefined),
    resetFailure: vi.fn(),
  };
}

describe("SettingsView desktop/system tab", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    useAppUpdaterMock.mockReset();
    useAppUpdaterMock.mockReturnValue(createUpdaterState());
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_model_configs") {
        return Promise.resolve([]);
      }
      if (command === "list_search_configs") {
        return Promise.resolve([]);
      }
      if (command === "list_mcp_servers" || command === "list_provider_configs") {
        return Promise.resolve([]);
      }
      if (command === "get_runtime_preferences") {
        return Promise.resolve(createRuntimePreferences());
      }
      if (command === "get_desktop_lifecycle_paths") {
        return Promise.resolve({
          app_data_dir: "C:\\Users\\me\\AppData\\Roaming\\WorkClaw",
          cache_dir: "C:\\Users\\me\\AppData\\Local\\WorkClaw\\cache",
          log_dir: "C:\\Users\\me\\AppData\\Local\\WorkClaw\\logs",
          default_work_dir: "E:\\workspace",
        });
      }
      return Promise.resolve(null);
    });
  });

  test("keeps desktop/system preferences outside the model tab", async () => {
    render(<SettingsView onClose={() => {}} />);

    await screen.findByTestId("settings-model-provider-preset");
    expect(screen.queryByRole("button", { name: "保存语言与翻译设置" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "保存更新设置" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "清理缓存与日志" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "桌面 / 系统" }));

    expect(await screen.findByRole("button", { name: "保存语言与翻译设置" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存更新设置" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "清理缓存与日志" })).toBeInTheDocument();
    expect(screen.queryByTestId("settings-model-provider-preset")).not.toBeInTheDocument();
  });
});
