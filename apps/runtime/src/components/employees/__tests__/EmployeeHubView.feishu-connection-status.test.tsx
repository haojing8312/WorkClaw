import { render, screen, waitFor } from "@testing-library/react";
import { EmployeeHubView } from "../EmployeeHubView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

function buildEmployee(id: string, employeeId: string, enabled: boolean, appId: string, appSecret: string) {
  return {
    id,
    employee_id: employeeId,
    name: employeeId,
    role_id: employeeId,
    persona: "",
    feishu_open_id: "",
    feishu_app_id: appId,
    feishu_app_secret: appSecret,
    primary_skill_id: "",
    default_work_dir: "",
    openclaw_agent_id: employeeId,
    enabled_scopes: ["feishu"],
    routing_priority: 100,
    enabled,
    is_default: false,
    skill_ids: [],
    created_at: "2026-03-01T00:00:00Z",
    updated_at: "2026-03-01T00:00:00Z",
  };
}

describe("EmployeeHubView feishu connection status", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "C:\\Users\\test\\WorkClaw\\workspace" });
      }
      if (command === "get_feishu_employee_connection_statuses") {
        return Promise.resolve({
          relay: {
            running: true,
            generation: 1,
            interval_ms: 1500,
            total_accepted: 0,
            last_error: null,
          },
          sidecar: {
            running: true,
            started_at: "2026-03-04T00:00:00Z",
            queued_events: 0,
            running_count: 1,
            items: [
              {
                employee_id: "pm",
                running: true,
                started_at: "2026-03-04T00:00:00Z",
                queued_events: 0,
                last_event_at: "2026-03-04T00:00:00Z",
                last_error: null,
                reconnect_attempts: 0,
              },
              {
                employee_id: "tech",
                running: false,
                started_at: null,
                queued_events: 0,
                last_event_at: null,
                last_error: "auth failed",
                reconnect_attempts: 3,
              },
            ],
          },
        });
      }
      if (command === "set_runtime_preferences") return Promise.resolve(null);
      if (command === "resolve_default_work_dir") return Promise.resolve("C:\\Users\\test\\WorkClaw\\workspace");
      return Promise.resolve(null);
    });
  });

  test("shows green red and gray dots by employee feishu connection state", async () => {
    render(
      <EmployeeHubView
        employees={[
          buildEmployee("emp-green", "pm", true, "cli_pm", "sec_pm"),
          buildEmployee("emp-red", "tech", true, "cli_tech", "sec_tech"),
          buildEmployee("emp-gray", "ops", false, "", ""),
        ]}
        skills={[
          {
            id: "builtin-general",
            name: "通用助手",
            description: "",
            version: "1.0.0",
            author: "",
            recommended_model: "",
            tags: [],
            created_at: "2026-03-01T00:00:00Z",
          },
        ]}
        selectedEmployeeId="emp-red"
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_feishu_employee_connection_statuses", { sidecarBaseUrl: null });
    });

    expect(screen.getByTestId("employee-connection-dot-emp-green")).toHaveClass("bg-emerald-500");
    expect(screen.getByTestId("employee-connection-dot-emp-red")).toHaveClass("bg-red-500");
    expect(screen.getByTestId("employee-connection-dot-emp-gray")).toHaveClass("bg-gray-300");
    expect(screen.getByText("auth failed")).toBeInTheDocument();
  });
});
