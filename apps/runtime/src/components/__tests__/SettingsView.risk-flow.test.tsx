import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("SettingsView risk flow", () => {
  beforeEach(() => {
    let employees = [
      {
        id: "emp-1",
        name: "张三",
        role_id: "project_manager",
        persona: "",
        feishu_open_id: "",
        feishu_app_id: "",
        feishu_app_secret: "",
        primary_skill_id: "",
        default_work_dir: "",
        enabled: true,
        is_default: false,
        skill_ids: [],
        created_at: "2026-03-01T00:00:00Z",
        updated_at: "2026-03-01T00:00:00Z",
      },
    ];

    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_model_configs") return Promise.resolve([]);
      if (command === "list_mcp_servers") return Promise.resolve([]);
      if (command === "list_search_configs") return Promise.resolve([]);
      if (command === "get_routing_settings") {
        return Promise.resolve({ max_call_depth: 4, node_timeout_seconds: 60, retry_count: 0 });
      }
      if (command === "list_builtin_provider_plugins") return Promise.resolve([]);
      if (command === "list_provider_configs") return Promise.resolve([]);
      if (command === "get_capability_routing_policy") return Promise.resolve(null);
      if (command === "list_capability_route_templates") return Promise.resolve([]);

      if (command === "get_feishu_gateway_settings") {
        return Promise.resolve({
          app_id: "cli_xxx",
          app_secret: "secret_xxx",
          ingress_token: "",
          encrypt_key: "",
          sidecar_base_url: "http://localhost:8765",
        });
      }
      if (command === "get_feishu_long_connection_status") {
        return Promise.resolve({ running: true, started_at: "2026-03-02T00:00:00Z", queued_events: 0 });
      }
      if (command === "get_feishu_event_relay_status") {
        return Promise.resolve({
          running: true,
          generation: 1,
          interval_ms: 1500,
          total_accepted: 3,
          last_error: null,
        });
      }
      if (command === "list_feishu_chats") return Promise.resolve({ items: [], has_more: false, page_token: "" });
      if (command === "list_recent_im_threads") return Promise.resolve([]);
      if (command === "list_agent_employees") return Promise.resolve(employees);
      if (command === "list_skills") return Promise.resolve([]);
      if (command === "get_runtime_preferences") return Promise.resolve({ default_work_dir: "" });
      if (command === "delete_agent_employee") {
        employees = [];
        return Promise.resolve(null);
      }
      return Promise.resolve(null);
    });
  });

  test("delete employee requires high-risk confirmation", async () => {
    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "飞书协作" }));

    await waitFor(() => {
      expect(screen.getByText("智能体员工")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "删除员工" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "删除员工" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    const deleteCallsAfterCancel = invokeMock.mock.calls.filter(([command]) => command === "delete_agent_employee");
    expect(deleteCallsAfterCancel).toHaveLength(0);

    fireEvent.click(screen.getByRole("button", { name: "删除员工" }));
    fireEvent.click(screen.getByRole("button", { name: "确认删除" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("delete_agent_employee", { employeeId: "emp-1" });
    });
  });
});
