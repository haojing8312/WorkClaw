import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("SettingsView feishu console", () => {
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
        return Promise.resolve({ running: true, generation: 1, interval_ms: 1500, total_accepted: 3, last_error: null });
      }
      if (command === "list_feishu_chats") {
        return Promise.resolve({ items: [{ chat_id: "oc_1", name: "群聊1" }], has_more: false, page_token: "" });
      }
      if (command === "list_recent_im_threads") {
        return Promise.resolve([{ thread_id: "oc_1", source: "feishu", last_text_preview: "hello", last_seen_at: "2026-03-02T00:00:00Z" }]);
      }
      if (command === "list_skills") {
        return Promise.resolve([
          { id: "builtin-general", name: "通用助手", description: "", version: "1.0.0", author: "", recommended_model: "", tags: [], created_at: "2026-01-01T00:00:00Z" },
          { id: "local-project-manager", name: "项目经理助手", description: "", version: "1.0.0", author: "", recommended_model: "", tags: [], created_at: "2026-01-01T00:00:00Z" },
        ]);
      }
      if (command === "set_feishu_gateway_settings") return Promise.resolve(null);

      return Promise.resolve(null);
    });
  });

  test("loads feishu tab data and saves settings", async () => {
    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "飞书协作" }));

    await waitFor(() => {
      expect(screen.getByText("飞书网关配置")).toBeInTheDocument();
      expect(screen.getByText("连接状态")).toBeInTheDocument();
      expect(screen.getByText("最近会话（用于绑定角色）")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "保存配置" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_feishu_gateway_settings", expect.anything());
    });
  });
});
