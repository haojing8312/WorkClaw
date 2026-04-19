import { act, renderHook } from "@testing-library/react";
import { useFeishuRuntimeStatusController } from "../useFeishuRuntimeStatusController";
import type { SettingsTabName } from "../../SettingsTabNav";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

function installInvokeMock() {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === "list_im_channel_registry") {
      return Promise.resolve([
        {
          channel: "feishu",
          runtime_status: {
            plugin_id: "openclaw-lark",
            account_id: "default",
            running: true,
            started_at: "2026-03-24T12:00:00Z",
            last_stop_at: null,
            last_event_at: "2026-03-24T12:00:01Z",
            last_error: null,
            pid: 1234,
            port: 5174,
            recent_logs: [],
          },
        },
      ]);
    }
    return Promise.resolve(null);
  });
}

describe("useFeishuRuntimeStatusController", () => {
  beforeEach(() => {
    installInvokeMock();
  });

  test("loads runtime status on mount", async () => {
    const { result } = renderHook(({ activeTab }) => useFeishuRuntimeStatusController({ activeTab }), {
      initialProps: { activeTab: "feishu" as SettingsTabName },
    });

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.officialFeishuRuntimeStatus?.running).toBe(true);
    expect(invokeMock).toHaveBeenCalledWith("list_im_channel_registry");
  });
});
