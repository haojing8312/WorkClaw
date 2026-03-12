import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
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
    launch_at_login: false,
    launch_minimized: false,
    close_to_tray: true,
  };
}

describe("SettingsView desktop/system tab", () => {
  beforeEach(() => {
    invokeMock.mockReset();
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
    expect(screen.queryByRole("button", { name: "清理缓存与日志" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "桌面 / 系统" }));

    expect(await screen.findByRole("button", { name: "保存语言与翻译设置" })).toBeInTheDocument();
    expect(screen.queryByText("软件更新")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "检查更新" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "清理缓存与日志" })).toBeInTheDocument();
    expect(screen.queryByTestId("settings-model-provider-preset")).not.toBeInTheDocument();
  });
});
