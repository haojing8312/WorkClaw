import { act, cleanup, renderHook } from "@testing-library/react";
import { useFeishuInstallerSessionController } from "../useFeishuInstallerSessionController";
import type { SettingsTabName } from "../../SettingsTabNav";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

function installInvokeMock() {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === "get_openclaw_lark_installer_session_status") {
      return Promise.resolve({
        running: true,
        mode: "link",
        started_at: "2026-03-24T12:00:00Z",
        last_output_at: "2026-03-24T12:00:01Z",
        last_error: null,
        prompt_hint: null,
        recent_output: [],
      });
    }
    return Promise.resolve(null);
  });
}

describe("useFeishuInstallerSessionController", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    installInvokeMock();
  });

  afterEach(() => {
    cleanup();
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  test("loads installer session on mount and stops polling after leaving the tab", async () => {
    const { rerender, result } = renderHook(({ activeTab }) => useFeishuInstallerSessionController({ activeTab }), {
      initialProps: { activeTab: "feishu" as SettingsTabName },
    });

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.feishuInstallerSession.running).toBe(true);

    const beforePollCount = invokeMock.mock.calls.filter(([command]) => command === "get_openclaw_lark_installer_session_status").length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
    });

    const currentCount = invokeMock.mock.calls.filter(([command]) => command === "get_openclaw_lark_installer_session_status").length;
    expect(currentCount).toBeGreaterThan(beforePollCount);

    rerender({ activeTab: "models" as SettingsTabName });
    const afterLeaveCount = invokeMock.mock.calls.filter(([command]) => command === "get_openclaw_lark_installer_session_status").length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });

    expect(
      invokeMock.mock.calls.filter(([command]) => command === "get_openclaw_lark_installer_session_status").length,
    ).toBe(afterLeaveCount);
  });
});
