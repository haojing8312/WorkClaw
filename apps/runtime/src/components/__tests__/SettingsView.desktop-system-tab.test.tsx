import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();
const openDialogMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => openDialogMock(...args),
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
    openDialogMock.mockReset();
    openDialogMock.mockResolvedValue(null);
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
          runtime_root_dir: "C:\\Users\\me\\.workclaw",
          pending_runtime_root_dir: null,
          last_runtime_migration_status: null,
          last_runtime_migration_message: null,
        });
      }
      if (command === "schedule_desktop_runtime_root_migration") {
        return Promise.resolve(null);
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
    expect(screen.getByText("WorkClaw 数据根目录")).toBeInTheDocument();
    expect(screen.queryByText("应用数据目录")).not.toBeInTheDocument();
    expect(screen.queryByText("默认工作目录")).not.toBeInTheDocument();
    expect(screen.queryByTestId("settings-model-provider-preset")).not.toBeInTheDocument();
  });

  test("preserves desktop edits when switching away and back", async () => {
    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(await screen.findByRole("button", { name: "桌面 / 系统" }));

    const languageSelect = await screen.findByLabelText("默认语言");
    fireEvent.change(languageSelect, { target: { value: "en-US" } });
    expect((languageSelect as HTMLSelectElement).value).toBe("en-US");

    fireEvent.click(screen.getByRole("button", { name: "模型连接" }));
    expect(screen.queryByLabelText("默认语言")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "桌面 / 系统" }));
    expect(await screen.findByLabelText("默认语言")).toHaveValue("en-US");
  });

  test("selects a new runtime root and schedules migration on restart", async () => {
    openDialogMock.mockResolvedValue("D:\\WorkClawData");
    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(await screen.findByRole("button", { name: "桌面 / 系统" }));
    fireEvent.click(await screen.findByRole("button", { name: "选择目录" }));

    expect(await screen.findByText("准备迁移到新的数据根目录")).toBeInTheDocument();
    expect(screen.getByText("D:\\WorkClawData")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "迁移并重启" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("schedule_desktop_runtime_root_migration", {
        targetRoot: "D:\\WorkClawData",
      });
    });
  });
});
