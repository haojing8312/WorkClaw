import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("SettingsView connector tab", () => {
  beforeEach(() => {
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
          app_id: "",
          app_secret: "",
          ingress_token: "",
          encrypt_key: "",
          sidecar_base_url: "",
        });
      }
      if (command === "get_wecom_gateway_settings") {
        return Promise.resolve({
          corp_id: "",
          agent_id: "",
          agent_secret: "",
          sidecar_base_url: "",
        });
      }
      if (command === "get_feishu_long_connection_status") {
        return Promise.resolve({
          running: false,
          started_at: null,
          queued_events: 0,
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

      return Promise.resolve(null);
    });
  });

  test("shows connector overview copy but keeps routing data lazy-loaded", async () => {
    render(<SettingsView onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "渠道连接器" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "渠道连接器" }));
    await waitFor(() => {
      expect(screen.getByTestId("connector-panel-feishu")).toBeInTheDocument();
    });
    expect(screen.getByText("先连接消息渠道，再设置处理规则，最后用模拟结果确认命中原因。")).toBeInTheDocument();
    expect(screen.getAllByText("连接器概览").length).toBeGreaterThan(0);
    expect(screen.getAllByText("消息处理规则").length).toBeGreaterThan(0);

    expect(invokeMock.mock.calls.some(([command]) => command === "list_im_routing_bindings")).toBe(false);
  });
});
