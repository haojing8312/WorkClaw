import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { EmployeeHubView } from "../EmployeeHubView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("EmployeeHubView browser setup panel", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "C:\\Users\\test\\WorkClaw\\workspace" });
      }
      if (command === "get_feishu_employee_connection_statuses") {
        return Promise.resolve({ relay: null, sidecar: null });
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
      return Promise.resolve(null);
    });
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
    fireEvent.click(screen.getByRole("button", { name: "启动飞书浏览器配置" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("start_feishu_browser_setup", { provider: "feishu" });
    });

    expect(screen.getByText("请先登录飞书")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "打开浏览器" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("open_external_url", {
        url: "https://open.feishu.cn/?workclaw_session_id=sess-1",
      });
    });
  });
});
