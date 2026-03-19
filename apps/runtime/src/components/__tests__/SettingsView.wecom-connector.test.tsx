import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("SettingsView connector visibility", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string, payload?: { instanceId?: string }) => {
      if (command === "list_model_configs") return Promise.resolve([]);
      if (command === "list_mcp_servers") return Promise.resolve([]);
      if (command === "list_search_configs") return Promise.resolve([]);
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          default_work_dir: "",
          default_language: "zh-CN",
          immersive_translation_enabled: true,
          immersive_translation_display: "translated_only",
          immersive_translation_trigger: "auto",
          translation_engine: "model_then_free",
          translation_model_id: "",
          launch_at_login: false,
          launch_minimized: false,
          close_to_tray: true,
        });
      }
      if (command === "get_desktop_lifecycle_paths") {
        return Promise.resolve({
          app_data_dir: "",
          cache_dir: "",
          log_dir: "",
          default_work_dir: "",
        });
      }
      if (command === "get_routing_settings") {
        return Promise.resolve({ max_call_depth: 4, node_timeout_seconds: 60, retry_count: 0 });
      }
      if (command === "list_builtin_provider_plugins") return Promise.resolve([]);
      if (command === "list_provider_configs") return Promise.resolve([]);
      if (command === "get_capability_routing_policy") return Promise.resolve(null);
      if (command === "list_capability_route_templates") return Promise.resolve([]);
      if (command === "get_feishu_gateway_settings") {
        return Promise.resolve({
          app_id: "cli-app",
          app_secret: "cli-secret",
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
      if (command === "list_channel_connectors") {
        return Promise.resolve([
          {
            channel: "feishu",
            display_name: "飞书连接器",
            capabilities: ["receive_text", "send_text", "group_route", "direct_route"],
          },
          {
            channel: "wecom",
            display_name: "企业微信连接器",
            capabilities: ["receive_text", "send_text", "group_route", "direct_route"],
          },
        ]);
      }
      if (command === "get_channel_connector_diagnostics") {
        const instanceId = payload?.instanceId;
        if (instanceId === "feishu:default") {
          return Promise.resolve({
            connector: {
              channel: "feishu",
              display_name: "飞书连接器",
              capabilities: ["receive_text", "send_text", "group_route", "direct_route"],
            },
            status: "stopped",
            health: {
              adapter_name: "feishu",
              instance_id: "feishu:default",
              state: "stopped",
              last_ok_at: null,
              last_error: null,
              reconnect_attempts: 0,
              queue_depth: 0,
              issue: null,
            },
            replay: {
              retained_events: 0,
              acked_events: 0,
            },
          });
        }
        return Promise.resolve(null);
      }
      return Promise.resolve(null);
    });
  });

  test("hides wecom connector panel and diagnostics on settings page", async () => {
    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "渠道连接器" }));

    await waitFor(() => {
      expect(screen.getByTestId("connector-panel-feishu")).toBeInTheDocument();
    });

    expect(screen.queryByTestId("connector-panel-wecom")).not.toBeInTheDocument();
    expect(screen.queryByText("企业微信连接器")).not.toBeInTheDocument();
    expect(screen.queryByText("企业微信连接异常")).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText("企业微信 Corp ID")).not.toBeInTheDocument();
    expect(screen.getAllByText("连接器诊断").length).toBeGreaterThan(0);
    expect(screen.getAllByText("飞书连接器").length).toBeGreaterThan(0);
  });
});
