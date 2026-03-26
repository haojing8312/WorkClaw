import { act, cleanup, renderHook } from "@testing-library/react";
import { useFeishuSetupProgressController } from "../useFeishuSetupProgressController";
import type { SettingsTabName } from "../../SettingsTabNav";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

function installInvokeMock() {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === "get_feishu_setup_progress") {
      return Promise.resolve({
        environment: {
          node_available: true,
          npm_available: true,
          node_version: "v22.0.0",
          npm_version: "10.0.0",
          can_install_plugin: true,
          can_start_runtime: true,
          error: null,
        },
        credentials_configured: true,
        plugin_installed: true,
        plugin_version: "2026.3.17",
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "pending",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "awaiting_auth",
      });
    }
    return Promise.resolve(null);
  });
}

describe("useFeishuSetupProgressController", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    installInvokeMock();
  });

  afterEach(() => {
    cleanup();
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  test("loads setup progress on mount and stops polling after leaving the tab", async () => {
    const { rerender, result } = renderHook(({ activeTab }) => useFeishuSetupProgressController({ activeTab }), {
      initialProps: { activeTab: "feishu" as SettingsTabName },
    });

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.feishuSetupProgress?.summary_state).toBe("awaiting_auth");

    const beforePollCount = invokeMock.mock.calls.filter(([command]) => command === "get_feishu_setup_progress").length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });

    const currentCount = invokeMock.mock.calls.filter(([command]) => command === "get_feishu_setup_progress").length;
    expect(currentCount).toBeGreaterThan(beforePollCount);

    rerender({ activeTab: "models" as SettingsTabName });
    const afterLeaveCount = invokeMock.mock.calls.filter(([command]) => command === "get_feishu_setup_progress").length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(10000);
    });

    expect(
      invokeMock.mock.calls.filter(([command]) => command === "get_feishu_setup_progress").length,
    ).toBe(afterLeaveCount);
  });
});
