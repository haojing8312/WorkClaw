import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
      if (command === "get_wecom_gateway_settings") {
        return Promise.resolve({
          corp_id: "wwcorp",
          agent_id: "1000002",
          agent_secret: "secret-x",
          sidecar_base_url: "",
        });
      }
      if (command === "get_wecom_connector_status") {
        return Promise.resolve({
          running: false,
          started_at: null,
          last_error: null,
          reconnect_attempts: 0,
          queue_depth: 0,
          instance_id: "wecom:wecom-main",
        });
      }
      if (command === "set_wecom_gateway_settings") return Promise.resolve(null);
      if (command === "start_wecom_connector") return Promise.resolve("wecom:wecom-main");
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
    expect(screen.getByTestId("connector-panel-feishu")).toBeInTheDocument();
    expect(screen.getByTestId("connector-diagnostics-feishu")).toBeInTheDocument();
    expect(screen.getByText("渠道连接器 / 飞书")).toBeInTheDocument();
    expect(screen.getByTestId("connector-panel-wecom")).toBeInTheDocument();
    expect(screen.getByText("渠道连接器 / 企业微信")).toBeInTheDocument();
    expect(screen.getAllByText("重连次数").length).toBeGreaterThan(0);
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("auth failed")).toBeInTheDocument();
  });

  test("allows saving and retrying wecom connector from employee hub", async () => {
    render(
      <EmployeeHubView
        employees={[buildEmployee("emp-red", "tech", true, "cli_tech", "sec_tech")]}
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
      expect(screen.getByPlaceholderText("企业微信 Corp ID")).toHaveValue("wwcorp");
    });

    fireEvent.change(screen.getByPlaceholderText("企业微信 Corp ID"), {
      target: { value: "wwcorp-updated" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存企业微信连接器" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "set_wecom_gateway_settings",
        expect.objectContaining({
          settings: expect.objectContaining({
            corp_id: "wwcorp-updated",
            agent_id: "1000002",
          }),
        }),
      );
    });

    fireEvent.click(screen.getAllByRole("button", { name: "重试连接" })[1]);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "start_wecom_connector",
        expect.objectContaining({
          corpId: "wwcorp-updated",
          agentId: "1000002",
          agentSecret: "secret-x",
        }),
      );
    });
  });

  test("saving feishu config preserves app scope when employee scopes are empty", async () => {
    const onSaveEmployee = vi.fn().mockResolvedValue(undefined);
    const employee = {
      ...buildEmployee("emp-scope", "scope-user", true, "cli_scope", "sec_scope"),
      enabled_scopes: [],
    };

    render(
      <EmployeeHubView
        employees={[employee]}
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
        selectedEmployeeId="emp-scope"
        onSelectEmployee={() => {}}
        onSaveEmployee={onSaveEmployee}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "保存连接器配置" }));

    await waitFor(() => {
      expect(onSaveEmployee).toHaveBeenCalledWith(
        expect.objectContaining({
          enabled_scopes: ["app"],
        }),
      );
    });
  });
});
