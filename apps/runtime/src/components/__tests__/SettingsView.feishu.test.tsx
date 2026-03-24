import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
      if (command === "get_feishu_long_connection_status") {
        return Promise.resolve({
          running: false,
          started_at: null,
          queued_events: 0,
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
    expect(screen.getByText("飞书连接")).toBeInTheDocument();
    expect(screen.getByText("先完成飞书连接，再到员工详情中指定谁来接待飞书消息。")).toBeInTheDocument();
    expect(screen.getAllByText("飞书接入概览").length).toBeGreaterThan(0);
    expect(screen.getByText("员工关联入口")).toBeInTheDocument();
    expect(screen.getByText("查看飞书连接是否已启动并可接收事件。")).toBeInTheDocument();
    expect(screen.getByText("连接详情")).toBeInTheDocument();
    expect(screen.getByText("高级设置")).toBeInTheDocument();
    const connectionDetails = screen.getByText("连接详情").closest("details") ?? document.body;
    expect(within(connectionDetails).getByRole("button", { name: "重新检测" })).toBeInTheDocument();
    expect(within(connectionDetails).getByRole("button", { name: "复制诊断摘要" })).toBeInTheDocument();
    expect(screen.queryByTestId("connector-panel-wecom")).not.toBeInTheDocument();
    expect(screen.queryByText("企业微信")).not.toBeInTheDocument();
    expect(screen.queryByText("消息处理规则")).not.toBeInTheDocument();

    expect(invokeMock.mock.calls.some(([command]) => command === "list_im_routing_bindings")).toBe(false);
    expect(invokeMock.mock.calls.some(([command]) => command === "get_feishu_long_connection_status")).toBe(false);
  });
});
