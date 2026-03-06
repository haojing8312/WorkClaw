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

function createRuntimePreferences(overrides?: Record<string, unknown>) {
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
    ...overrides,
  };
}

function createUpdaterState(overrides?: Record<string, unknown>) {
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
    ...overrides,
  };
}

describe("SettingsView updater", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    useAppUpdaterMock.mockReset();
    useAppUpdaterMock.mockReturnValue(createUpdaterState());
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "list_model_configs") return Promise.resolve([]);
      if (command === "list_mcp_servers") return Promise.resolve([]);
      if (command === "list_search_configs") return Promise.resolve([]);
      if (command === "list_provider_configs") return Promise.resolve([]);
      if (command === "get_runtime_preferences") {
        return Promise.resolve(createRuntimePreferences());
      }
      if (command === "set_runtime_preferences") {
        return Promise.resolve(createRuntimePreferences(payload?.input));
      }
      return Promise.resolve(null);
    });
  });

  test("renders updater controls and runs a manual update check", async () => {
    const updaterState = createUpdaterState();
    useAppUpdaterMock.mockReturnValue(updaterState);

    render(<SettingsView onClose={() => {}} />);

    expect(await screen.findByText("软件更新")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "检查更新" }));

    await waitFor(() => {
      expect(updaterState.checkForUpdates).toHaveBeenCalledWith({ manual: true });
    });
  });

  test("saves auto update preference separately from translation preferences", async () => {
    render(<SettingsView onClose={() => {}} />);

    const autoUpdateToggle = await screen.findByRole("checkbox", { name: "自动检查更新" });
    fireEvent.click(autoUpdateToggle);
    fireEvent.click(screen.getByRole("button", { name: "保存更新设置" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_runtime_preferences", {
        input: expect.objectContaining({
          auto_update_enabled: false,
          update_channel: "stable",
        }),
      });
    });
  });

  test("shows update available state and can start downloading", async () => {
    const updaterState = createUpdaterState({
      status: "update_available",
      availableUpdate: {
        currentVersion: "0.2.3",
        version: "0.2.4",
        date: "2026-03-06T12:00:00.000Z",
        body: "新增自动更新入口",
        rawJson: { version: "0.2.4" },
      },
      canDismiss: true,
      canDownload: true,
    });
    useAppUpdaterMock.mockReturnValue(updaterState);

    render(<SettingsView onClose={() => {}} />);

    expect(await screen.findByText("发现新版本 v0.2.4")).toBeInTheDocument();
    expect(screen.getByText("新增自动更新入口")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "下载更新" }));
    expect(updaterState.downloadUpdate).toHaveBeenCalledTimes(1);
  });

  test("renders downloading progress clearly", async () => {
    useAppUpdaterMock.mockReturnValue(
      createUpdaterState({
        status: "downloading",
        isWorking: true,
        downloadProgress: {
          contentLength: 100,
          downloadedBytes: 25,
          percent: 25,
        },
      }),
    );

    render(<SettingsView onClose={() => {}} />);

    expect(await screen.findByText("正在下载更新")).toBeInTheDocument();
    expect(screen.getByText("已下载 25%")).toBeInTheDocument();
  });

  test("renders downloading and failed states with clear guidance", async () => {
    useAppUpdaterMock.mockReturnValue(
      createUpdaterState({
        status: "failed",
        error: "暂时无法连接更新服务",
      }),
    );

    render(<SettingsView onClose={() => {}} />);

    expect(await screen.findByText("更新失败")).toBeInTheDocument();
    expect(screen.getByText("暂时无法连接更新服务")).toBeInTheDocument();
  });
});
