import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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

describe("SettingsView data retention", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    useAppUpdaterMock.mockReset();
    useAppUpdaterMock.mockReturnValue(createUpdaterState());
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_model_configs") return Promise.resolve([]);
      if (command === "list_mcp_servers") return Promise.resolve([]);
      if (command === "list_search_configs") return Promise.resolve([]);
      if (command === "list_provider_configs") return Promise.resolve([]);
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
      if (command === "clear_desktop_cache_and_logs") {
        return Promise.resolve({
          removed_files: 12,
          removed_dirs: 3,
        });
      }
      if (command === "export_desktop_environment_summary") {
        return Promise.resolve("# WorkClaw Environment Summary");
      }
      if (command === "open_desktop_path") {
        return Promise.resolve(null);
      }
      return Promise.resolve(null);
    });
  });

  test("shows data paths, uninstall guidance and maintenance actions", async () => {
    render(<SettingsView onClose={() => {}} />);

    expect(await screen.findByText("数据与卸载")).toBeInTheDocument();
    expect(screen.getByText("C:\\Users\\me\\AppData\\Roaming\\WorkClaw")).toBeInTheDocument();
    expect(screen.getByText("C:\\Users\\me\\AppData\\Local\\WorkClaw\\cache")).toBeInTheDocument();
    expect(screen.getByText("E:\\workspace")).toBeInTheDocument();
    expect(screen.getByText("卸载程序不会删除你的工作目录")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "打开应用数据目录" }));
    fireEvent.click(screen.getByRole("button", { name: "清理缓存与日志" }));
    fireEvent.click(screen.getByRole("button", { name: "导出环境摘要" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("open_desktop_path", {
        path: "C:\\Users\\me\\AppData\\Roaming\\WorkClaw",
      });
      expect(invokeMock).toHaveBeenCalledWith("clear_desktop_cache_and_logs");
      expect(invokeMock).toHaveBeenCalledWith("export_desktop_environment_summary");
    });
  });
});
