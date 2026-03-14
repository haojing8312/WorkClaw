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
      if (command === "list_im_routing_bindings") return Promise.resolve([]);
      if (command === "upsert_im_routing_binding") return Promise.resolve("binding-1");
      if (command === "delete_im_routing_binding") return Promise.resolve(null);
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
    expect(screen.getByTestId("employee-feishu-association")).toBeInTheDocument();
    expect(screen.getByText("飞书接待")).toBeInTheDocument();
    expect(screen.queryByTestId("connector-panel-feishu")).not.toBeInTheDocument();
    expect(screen.getByText("飞书连接在设置中心统一管理。这里仅决定该员工是否接待飞书入口，以及接待哪些会话。")).toBeInTheDocument();
    expect(screen.getByTestId("connector-panel-wecom")).toBeInTheDocument();
    expect(screen.getAllByText("重连次数").length).toBeGreaterThan(0);
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

    fireEvent.click(screen.getByRole("button", { name: "重试连接" }));

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

  test("treats legacy feishu bindings as active reception even when scopes are empty", async () => {
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
            running_count: 0,
            items: [],
          },
        });
      }
      if (command === "set_runtime_preferences") return Promise.resolve(null);
      if (command === "list_im_routing_bindings") {
        return Promise.resolve([
          {
            id: "binding-legacy-ops",
            agent_id: "ops",
            channel: "feishu",
            account_id: "*",
            peer_kind: "group",
            peer_id: "",
            guild_id: "",
            team_id: "",
            role_ids: [],
            connector_meta: { connector_id: "feishu" },
            priority: 100,
            enabled: true,
            created_at: "2026-03-11T00:00:00Z",
            updated_at: "2026-03-11T00:00:00Z",
          },
        ]);
      }
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

    render(
      <EmployeeHubView
        employees={[
          {
            ...buildEmployee("emp-ops", "ops", true, "cli_ops", "sec_ops"),
            enabled_scopes: [],
          },
        ]}
        skills={[]}
        selectedEmployeeId="emp-ops"
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("employee-connection-dot-emp-ops")).toHaveClass("bg-red-500");
    });
  });

  test("saving feishu association preserves app scope when employee scopes are empty", async () => {
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
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("employee-feishu-association")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByLabelText("启用飞书接待"));
    fireEvent.click(screen.getByRole("button", { name: "保存飞书接待" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_feishu_employee_association",
        expect.objectContaining({
          input: expect.objectContaining({
            employee_db_id: "emp-scope",
            enabled: true,
            mode: "default",
            peer_kind: "group",
            peer_id: "",
            priority: 100,
          }),
        }),
      );
    });
    expect(invokeMock).not.toHaveBeenCalledWith("upsert_im_routing_binding", expect.anything());
  });

  test("supports scoped feishu reception for a specific group", async () => {
    render(
      <EmployeeHubView
        employees={[buildEmployee("emp-scoped", "ops", true, "cli_ops", "sec_ops")]}
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
        selectedEmployeeId="emp-scoped"
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("employee-feishu-association")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByLabelText("仅处理指定群聊或会话"));
    fireEvent.change(screen.getByLabelText("飞书处理范围 ID"), {
      target: { value: "chat-delivery-room" },
    });
    fireEvent.change(screen.getByLabelText("飞书处理优先级"), {
      target: { value: "66" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存飞书接待" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_feishu_employee_association",
        expect.objectContaining({
          input: expect.objectContaining({
            employee_db_id: "emp-scoped",
            enabled: true,
            mode: "scoped",
            peer_kind: "group",
            peer_id: "chat-delivery-room",
            priority: 66,
          }),
        }),
      );
    });
  });

  test("shows default receiver replacement hint and removes previous default binding on save", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "C:\\Users\\test\\WorkClaw\\workspace" });
      }
      if (command === "get_feishu_employee_connection_statuses") {
        return Promise.resolve({
          relay: { running: true, generation: 1, interval_ms: 1500, total_accepted: 0, last_error: null },
          sidecar: { running: true, started_at: "2026-03-04T00:00:00Z", queued_events: 0, running_count: 0, items: [] },
        });
      }
      if (command === "set_runtime_preferences") return Promise.resolve(null);
      if (command === "list_im_routing_bindings") {
        return Promise.resolve([
          {
            id: "binding-default-pm",
            agent_id: "pm",
            channel: "feishu",
            account_id: "*",
            peer_kind: "group",
            peer_id: "",
            guild_id: "",
            team_id: "",
            role_ids: [],
            connector_meta: { connector_id: "feishu" },
            priority: 100,
            enabled: true,
            created_at: "2026-03-11T00:00:00Z",
            updated_at: "2026-03-11T00:00:00Z",
          },
        ]);
      }
      if (command === "delete_im_routing_binding") return Promise.resolve(null);
      if (command === "upsert_im_routing_binding") return Promise.resolve("binding-tech");
      if (command === "get_wecom_gateway_settings") {
        return Promise.resolve({ corp_id: "wwcorp", agent_id: "1000002", agent_secret: "secret-x", sidecar_base_url: "" });
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

    render(
      <EmployeeHubView
        employees={[
          buildEmployee("emp-pm", "pm", true, "cli_pm", "sec_pm"),
          buildEmployee("emp-tech", "tech", true, "cli_tech", "sec_tech"),
        ]}
        skills={[]}
        selectedEmployeeId="emp-tech"
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("employee-feishu-association")).toBeInTheDocument();
    });

    expect(screen.getByText("当前默认接待员工是 pm，保存后将替换为当前员工。")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "保存飞书接待" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_feishu_employee_association",
        expect.objectContaining({
          input: expect.objectContaining({
            employee_db_id: "emp-tech",
            enabled: true,
            mode: "default",
            peer_id: "",
          }),
        }),
      );
    });
  });

  test("shows scoped conflict hint and removes conflicting binding before saving", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "C:\\Users\\test\\WorkClaw\\workspace" });
      }
      if (command === "get_feishu_employee_connection_statuses") {
        return Promise.resolve({
          relay: { running: true, generation: 1, interval_ms: 1500, total_accepted: 0, last_error: null },
          sidecar: { running: true, started_at: "2026-03-04T00:00:00Z", queued_events: 0, running_count: 0, items: [] },
        });
      }
      if (command === "set_runtime_preferences") return Promise.resolve(null);
      if (command === "list_im_routing_bindings") {
        return Promise.resolve([
          {
            id: "binding-ops-room",
            agent_id: "ops",
            channel: "feishu",
            account_id: "*",
            peer_kind: "group",
            peer_id: "chat-delivery-room",
            guild_id: "",
            team_id: "",
            role_ids: [],
            connector_meta: { connector_id: "feishu" },
            priority: 88,
            enabled: true,
            created_at: "2026-03-11T00:00:00Z",
            updated_at: "2026-03-11T00:00:00Z",
          },
        ]);
      }
      if (command === "delete_im_routing_binding") return Promise.resolve(null);
      if (command === "upsert_im_routing_binding") return Promise.resolve("binding-tech-room");
      if (command === "get_wecom_gateway_settings") {
        return Promise.resolve({ corp_id: "wwcorp", agent_id: "1000002", agent_secret: "secret-x", sidecar_base_url: "" });
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

    render(
      <EmployeeHubView
        employees={[
          buildEmployee("emp-ops", "ops", true, "cli_ops", "sec_ops"),
          buildEmployee("emp-tech", "tech", true, "cli_tech", "sec_tech"),
        ]}
        skills={[]}
        selectedEmployeeId="emp-tech"
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("employee-feishu-association")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByLabelText("仅处理指定群聊或会话"));
    fireEvent.change(screen.getByLabelText("飞书处理范围 ID"), {
      target: { value: "chat-delivery-room" },
    });

    expect(screen.getByText("群聊/会话 chat-delivery-room 当前由 ops 处理，保存后将改为当前员工接待。")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "保存飞书接待" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_feishu_employee_association",
        expect.objectContaining({
          input: expect.objectContaining({
            employee_db_id: "emp-tech",
            enabled: true,
            mode: "scoped",
            peer_id: "chat-delivery-room",
          }),
        }),
      );
    });
  });
});
