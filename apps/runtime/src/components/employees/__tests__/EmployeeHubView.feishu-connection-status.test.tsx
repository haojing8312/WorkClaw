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
        return Promise.resolve({ default_work_dir: "C:\\Users\\test\\.workclaw\\workspace" });
      }
      if (command === "get_openclaw_plugin_feishu_runtime_status") {
        return Promise.resolve({
          plugin_id: "@larksuite/openclaw-lark",
          account_id: "default",
          running: true,
          started_at: "2026-03-04T00:00:00Z",
          last_error: null,
          recent_logs: [],
        });
      }
      if (command === "set_runtime_preferences") return Promise.resolve(null);
      if (command === "list_im_routing_bindings") return Promise.resolve([]);
      if (command === "upsert_im_routing_binding") return Promise.resolve("binding-1");
      if (command === "delete_im_routing_binding") return Promise.resolve(null);
      if (command === "resolve_default_work_dir") return Promise.resolve("C:\\Users\\test\\.workclaw\\workspace");
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
      expect(invokeMock).toHaveBeenCalledWith("get_openclaw_plugin_feishu_runtime_status", {
        pluginId: "@larksuite/openclaw-lark",
        accountId: "default",
      });
    });

    expect(screen.getByTestId("employee-connection-dot-emp-green")).toHaveClass("bg-emerald-500");
    expect(screen.getByTestId("employee-connection-dot-emp-red")).toHaveClass("bg-emerald-500");
    expect(screen.getByTestId("employee-connection-dot-emp-gray")).toHaveClass("bg-gray-300");
    expect(screen.getByTestId("employee-feishu-association")).toBeInTheDocument();
    expect(screen.getByText("飞书接待")).toBeInTheDocument();
    expect(screen.queryByTestId("connector-panel-feishu")).not.toBeInTheDocument();
    expect(screen.getByText("飞书连接在设置中心统一管理。这里仅决定该员工是否接待飞书入口，以及接待哪些会话。")).toBeInTheDocument();
    expect(screen.queryByTestId("connector-panel-wecom")).not.toBeInTheDocument();
    expect(screen.queryByText("企业微信")).not.toBeInTheDocument();
  });

  test("treats legacy feishu bindings as active reception even when scopes are empty", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "C:\\Users\\test\\.workclaw\\workspace" });
      }
      if (command === "get_openclaw_plugin_feishu_runtime_status") {
        return Promise.resolve({
          plugin_id: "@larksuite/openclaw-lark",
          account_id: "default",
          running: true,
          started_at: "2026-03-04T00:00:00Z",
          last_error: null,
          recent_logs: [],
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
      if (command === "resolve_default_work_dir") return Promise.resolve("C:\\Users\\test\\.workclaw\\workspace");
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
      expect(screen.getByTestId("employee-connection-dot-emp-ops")).toHaveClass("bg-emerald-500");
    });
  });

  test("saving feishu association preserves app scope when employee scopes are empty", async () => {
    const refreshEmployees = vi.fn().mockResolvedValue(undefined);
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
        onRefreshEmployees={refreshEmployees}
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
    expect(refreshEmployees).toHaveBeenCalledTimes(1);
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
        return Promise.resolve({ default_work_dir: "C:\\Users\\test\\.workclaw\\workspace" });
      }
      if (command === "get_openclaw_plugin_feishu_runtime_status") {
        return Promise.resolve({
          plugin_id: "@larksuite/openclaw-lark",
          account_id: "default",
          running: true,
          started_at: "2026-03-04T00:00:00Z",
          last_error: null,
          recent_logs: [],
        });
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
      if (command === "resolve_default_work_dir") return Promise.resolve("C:\\Users\\test\\.workclaw\\workspace");
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
        return Promise.resolve({ default_work_dir: "C:\\Users\\test\\.workclaw\\workspace" });
      }
      if (command === "get_openclaw_plugin_feishu_runtime_status") {
        return Promise.resolve({
          plugin_id: "@larksuite/openclaw-lark",
          account_id: "default",
          running: true,
          started_at: "2026-03-04T00:00:00Z",
          last_error: null,
          recent_logs: [],
        });
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
      if (command === "resolve_default_work_dir") return Promise.resolve("C:\\Users\\test\\.workclaw\\workspace");
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
