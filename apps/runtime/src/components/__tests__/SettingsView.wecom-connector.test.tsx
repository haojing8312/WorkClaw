import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("SettingsView wecom connector", () => {
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
          auto_update_enabled: true,
          update_channel: "stable",
          dismissed_update_version: "",
          last_update_check_at: "",
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
      if (command === "get_wecom_gateway_settings") {
        return Promise.resolve({
          corp_id: "wwcorp",
          agent_id: "1000002",
          agent_secret: "secret-x",
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
        return Promise.resolve({
          connector: {
            channel: "wecom",
            display_name: "企业微信连接器",
            capabilities: ["receive_text", "send_text", "group_route", "direct_route"],
          },
          status: "connected",
          health: {
            adapter_name: "wecom",
            instance_id: "wecom:wecom-main",
            state: "running",
            last_ok_at: "2026-03-11T10:00:00Z",
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
      if (command === "set_wecom_gateway_settings") return Promise.resolve(null);
      if (command === "start_wecom_connector") return Promise.resolve("wecom:wecom-main");
      return Promise.resolve(null);
    });
  });

  test("renders wecom connector panel and persists connector actions", async () => {
    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "渠道连接器" }));

    await waitFor(() => {
      expect(screen.getByTestId("connector-panel-wecom")).toBeInTheDocument();
    });

    const corpIdInput = screen.getByPlaceholderText("企业微信 Corp ID");
    fireEvent.change(corpIdInput, { target: { value: "wwcorp-updated" } });

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

  test("shows user-facing recent issue copy when wecom connector is unhealthy", async () => {
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
          auto_update_enabled: true,
          update_channel: "stable",
          dismissed_update_version: "",
          last_update_check_at: "",
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
          app_id: "",
          app_secret: "",
          ingress_token: "",
          encrypt_key: "",
          sidecar_base_url: "",
        });
      }
      if (command === "get_wecom_gateway_settings") {
        return Promise.resolve({
          corp_id: "wwcorp",
          agent_id: "1000002",
          agent_secret: "secret-x",
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
          last_error: "signature mismatch",
          reconnect_attempts: 2,
          queue_depth: 3,
          instance_id: "wecom:wecom-main",
        });
      }
      if (command === "list_channel_connectors") {
        return Promise.resolve([
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
        return Promise.resolve({
          connector: {
            channel: "wecom",
            display_name: "企业微信连接器",
            capabilities: ["receive_text", "send_text", "group_route", "direct_route"],
          },
          status: "authentication_error",
          health: {
            adapter_name: "wecom",
            instance_id: "wecom:wecom-main",
            state: "error",
            last_ok_at: "2026-03-11T10:00:00Z",
            last_error: "signature mismatch",
            reconnect_attempts: 2,
            queue_depth: 3,
            issue: {
              code: "signature_mismatch",
              category: "authentication_error",
              user_message: "签名校验失败",
              technical_message: "signature mismatch",
              retryable: false,
              occurred_at: "2026-03-11T10:00:00Z",
            },
          },
          replay: {
            retained_events: 1,
            acked_events: 0,
          },
        });
      }
      return Promise.resolve(null);
    });

    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "渠道连接器" }));

    await waitFor(() => {
      expect(screen.getByText("企业微信连接异常")).toBeInTheDocument();
    });

    expect(screen.getAllByText("企业微信连接器").length).toBeGreaterThan(0);
    expect(screen.getAllByText("最近问题").length).toBeGreaterThan(0);
    expect(screen.getAllByText(/签名校验失败/).length).toBeGreaterThan(0);
    expect(screen.queryByText("原始错误：signature mismatch")).not.toBeInTheDocument();
    expect(screen.getAllByText("连接器诊断").length).toBeGreaterThan(0);
    expect(screen.getByText("receive_text")).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole("button", { name: "查看技术详情" })[0]);

    expect(screen.getByText("原始错误：signature mismatch")).toBeInTheDocument();
  });
});
