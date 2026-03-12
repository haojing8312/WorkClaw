import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { EmployeeHubView } from "../EmployeeHubView";
import type { BrowserBridgeInstallStatus } from "../../../types";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("EmployeeHubView browser setup panel", () => {
  beforeEach(() => {
    const installStatus: BrowserBridgeInstallStatus = {
      state: "not_installed",
      chrome_found: true,
      native_host_installed: false,
      extension_dir_ready: false,
      bridge_connected: false,
      last_error: null,
    };
    expect(installStatus.state).toBe("not_installed");

    invokeMock.mockReset();
    const sessionSnapshots = [
      {
        session_id: "sess-1",
        provider: "feishu",
        step: "BIND_LOCAL",
        app_id: "cli_a",
        app_secret_present: true,
      },
      {
        session_id: "sess-1",
        provider: "feishu",
        step: "ENABLE_LONG_CONNECTION",
        app_id: "cli_a",
        app_secret_present: true,
      },
    ];
    invokeMock.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "C:\\Users\\test\\WorkClaw\\workspace" });
      }
      if (command === "get_feishu_employee_connection_statuses") {
        return Promise.resolve({ relay: null, sidecar: null });
      }
      if (command === "get_browser_bridge_install_status") {
        return Promise.resolve({
          state: "not_installed",
          chrome_found: true,
          native_host_installed: false,
          extension_dir_ready: false,
          bridge_connected: false,
          last_error: null,
        } satisfies BrowserBridgeInstallStatus);
      }
      if (command === "start_feishu_browser_setup") {
        expect(payload).toMatchObject({ provider: "feishu" });
        return Promise.resolve({
          session_id: "sess-1",
          provider: "feishu",
          step: "LOGIN_REQUIRED",
          app_id: null,
          app_secret_present: false,
        });
      }
      if (command === "get_feishu_browser_setup_session") {
        expect(payload).toMatchObject({ sessionId: "sess-1" });
        return Promise.resolve(
          sessionSnapshots.shift() ?? sessionSnapshots[sessionSnapshots.length - 1],
        );
      }
      return Promise.resolve(null);
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("starts feishu browser setup from settings tab", async () => {
    render(
      <EmployeeHubView
        employees={[]}
        skills={[]}
        selectedEmployeeId={null}
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    fireEvent.click(screen.getByRole("tab", { name: "设置" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_browser_bridge_install_status");
    });

    expect(screen.getByText("浏览器桥接安装")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "安装浏览器桥接" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "启动飞书浏览器配置" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("start_feishu_browser_setup", { provider: "feishu" });
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("open_external_url", {
        url: "https://open.feishu.cn/?workclaw_session_id=sess-1",
      });
    });

    expect(screen.getByText("请先登录飞书")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "打开浏览器" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("open_external_url", {
        url: "https://open.feishu.cn/?workclaw_session_id=sess-1",
      });
    });
  });

  test("polls browser setup session until terminal step", async () => {
    vi.useFakeTimers();

    render(
      <EmployeeHubView
        employees={[]}
        skills={[]}
        selectedEmployeeId={null}
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    fireEvent.click(screen.getByRole("tab", { name: "设置" }));
    fireEvent.click(screen.getByRole("button", { name: "启动飞书浏览器配置" }));

    await act(async () => {
      await Promise.resolve();
    });

    expect(screen.getByText("请先登录飞书")).toBeInTheDocument();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });

    expect(screen.getByText("当前步骤：BIND_LOCAL")).toBeInTheDocument();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });

    expect(screen.getByText("当前步骤：ENABLE_LONG_CONNECTION")).toBeInTheDocument();

    const pollCallsBeforeStop = invokeMock.mock.calls.filter(
      ([command]) => command === "get_feishu_browser_setup_session",
    ).length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(15000);
    });

    const pollCallsAfterStop = invokeMock.mock.calls.filter(
      ([command]) => command === "get_feishu_browser_setup_session",
    ).length;

    expect(pollCallsAfterStop).toBe(pollCallsBeforeStop);
  });
});
