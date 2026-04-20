import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(() => Promise.resolve(null)),
}));

describe("SettingsView model connection feedback", () => {
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
      if (command === "test_connection_cmd") {
        return Promise.resolve({
          ok: false,
          kind: "auth",
          title: "鉴权失败",
          message: "请检查 API Key、组织权限或接口访问范围是否正确。",
          raw_message: "Unauthorized: invalid_api_key",
        });
      }
      return Promise.resolve(null);
    });
  });

  test("shows shared auth guidance for structured model connection failures", async () => {
    render(<SettingsView onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "模型连接" })).toBeInTheDocument();
    });

    fireEvent.change(screen.getByTestId("settings-model-provider-api-key"), {
      target: { value: "sk-auth-test" },
    });
    fireEvent.click(screen.getByRole("button", { name: "测试连接" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "test_connection_cmd",
        expect.objectContaining({
          apiKey: "sk-auth-test",
        }),
      );
    });

    expect(screen.getByText("鉴权失败")).toBeInTheDocument();
    expect(screen.getByText("请检查 API Key、组织权限或接口访问范围是否正确。")).toBeInTheDocument();
    expect(screen.getByText("Unauthorized: invalid_api_key")).toBeInTheDocument();
  });

  test("saves the supports-vision flag in model connection config", async () => {
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
      if (command === "save_model_config") return Promise.resolve("model-vision");
      if (command === "save_provider_config") return Promise.resolve("model-vision");
      if (command === "set_default_model") return Promise.resolve(null);
      if (command === "get_feishu_gateway_settings") {
        return Promise.resolve({
          app_id: "",
          app_secret: "",
          ingress_token: "",
          encrypt_key: "",
          sidecar_base_url: "",
        });
      }
      if (command === "get_openclaw_plugin_feishu_advanced_settings") {
        return Promise.resolve({
          groups_json: "",
          dms_json: "",
          footer_json: "",
          account_overrides_json: "",
          render_mode: "",
          streaming: "",
          text_chunk_limit: "",
          chunk_mode: "",
          reply_in_thread: "",
          group_session_scope: "",
          topic_session_mode: "",
          markdown_mode: "",
          markdown_table_mode: "",
          heartbeat_visibility: "",
          heartbeat_interval_ms: "",
          media_max_mb: "",
          http_timeout_ms: "",
          config_writes: "",
          webhook_host: "",
          webhook_port: "",
          dynamic_agent_creation_enabled: "",
          dynamic_agent_creation_workspace_template: "",
          dynamic_agent_creation_agent_dir_template: "",
          dynamic_agent_creation_max_agents: "",
        });
      }
      if (command === "get_openclaw_lark_installer_session_status") {
        return Promise.resolve({
          running: false,
          mode: null,
          started_at: null,
          last_output_at: null,
          last_error: null,
          prompt_hint: null,
          recent_output: [],
        });
      }
      if (command === "get_feishu_setup_progress") {
        return Promise.resolve({
          environment: null,
          summary_state: "skipped",
          runtime_running: false,
          auth_status: "approved",
          pending_pairings: 0,
          plugin_installed: false,
          plugin_version: "",
          default_routing_employee_name: "",
          scoped_routing_count: 0,
          runtime_last_error: null,
        });
      }
      if (command === "list_feishu_pairing_requests") return Promise.resolve([]);
      if (command === "list_im_channel_registry") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    render(<SettingsView onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByTestId("settings-model-provider-save")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByTestId("settings-model-provider-name"), {
      target: { value: "Qwen Vision" },
    });
    fireEvent.change(screen.getByTestId("settings-model-provider-base-url"), {
      target: { value: "https://example.com/v1" },
    });
    fireEvent.change(screen.getByTestId("settings-model-provider-model-name"), {
      target: { value: "qwen-vl-max" },
    });
    fireEvent.change(screen.getByTestId("settings-model-provider-api-key"), {
      target: { value: "sk-vision" },
    });
    fireEvent.click(screen.getByTestId("settings-model-provider-supports-vision"));
    fireEvent.click(screen.getByTestId("settings-model-provider-save"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_model_config",
        expect.objectContaining({
          config: expect.objectContaining({
            supports_vision: true,
          }),
        }),
      );
    });
  });

  test("syncs the saved vision connection into the default vision route", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_model_configs") return Promise.resolve([]);
      if (command === "list_mcp_servers") return Promise.resolve([]);
      if (command === "list_search_configs") return Promise.resolve([]);
      if (command === "get_routing_settings") {
        return Promise.resolve({ max_call_depth: 4, node_timeout_seconds: 60, retry_count: 0 });
      }
      if (command === "list_builtin_provider_plugins") return Promise.resolve([]);
      if (command === "list_provider_configs") return Promise.resolve([]);
      if (command === "get_capability_routing_policy") {
        return Promise.resolve({
          capability: "vision",
          primary_provider_id: "",
          primary_model: "",
          fallback_chain_json: "[]",
          timeout_ms: 90000,
          retry_count: 1,
          enabled: true,
        });
      }
      if (command === "list_capability_route_templates") return Promise.resolve([]);
      if (command === "save_model_config") return Promise.resolve("model-vision");
      if (command === "save_provider_config") return Promise.resolve("model-vision");
      if (command === "set_default_model") return Promise.resolve(null);
      if (command === "set_capability_routing_policy") return Promise.resolve(null);
      if (command === "get_feishu_gateway_settings") {
        return Promise.resolve({
          app_id: "",
          app_secret: "",
          ingress_token: "",
          encrypt_key: "",
          sidecar_base_url: "",
        });
      }
      if (command === "get_openclaw_plugin_feishu_advanced_settings") {
        return Promise.resolve({
          groups_json: "",
          dms_json: "",
          footer_json: "",
          account_overrides_json: "",
          render_mode: "",
          streaming: "",
          text_chunk_limit: "",
          chunk_mode: "",
          reply_in_thread: "",
          group_session_scope: "",
          topic_session_mode: "",
          markdown_mode: "",
          markdown_table_mode: "",
          heartbeat_visibility: "",
          heartbeat_interval_ms: "",
          media_max_mb: "",
          http_timeout_ms: "",
          config_writes: "",
          webhook_host: "",
          webhook_port: "",
          dynamic_agent_creation_enabled: "",
          dynamic_agent_creation_workspace_template: "",
          dynamic_agent_creation_agent_dir_template: "",
          dynamic_agent_creation_max_agents: "",
        });
      }
      if (command === "get_openclaw_lark_installer_session_status") {
        return Promise.resolve({
          running: false,
          mode: null,
          started_at: null,
          last_output_at: null,
          last_error: null,
          prompt_hint: null,
          recent_output: [],
        });
      }
      if (command === "get_feishu_setup_progress") {
        return Promise.resolve({
          environment: null,
          summary_state: "skipped",
          runtime_running: false,
          auth_status: "approved",
          pending_pairings: 0,
          plugin_installed: false,
          plugin_version: "",
          default_routing_employee_name: "",
          scoped_routing_count: 0,
          runtime_last_error: null,
        });
      }
      if (command === "list_feishu_pairing_requests") return Promise.resolve([]);
      if (command === "list_im_channel_registry") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    render(<SettingsView onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByTestId("settings-model-provider-save")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByTestId("settings-model-provider-name"), {
      target: { value: "Qwen Vision" },
    });
    fireEvent.change(screen.getByTestId("settings-model-provider-base-url"), {
      target: { value: "https://example.com/v1" },
    });
    fireEvent.change(screen.getByTestId("settings-model-provider-model-name"), {
      target: { value: "qwen-vl-max" },
    });
    fireEvent.change(screen.getByTestId("settings-model-provider-api-key"), {
      target: { value: "sk-vision" },
    });
    fireEvent.click(screen.getByTestId("settings-model-provider-supports-vision"));
    fireEvent.click(screen.getByTestId("settings-model-provider-save"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_capability_routing_policy", {
        policy: expect.objectContaining({
          capability: "vision",
          primary_provider_id: "model-vision",
          primary_model: "qwen-vl-max",
          timeout_ms: 90000,
          retry_count: 1,
          enabled: true,
        }),
      });
    });
  });

  test("keeps the existing default chat model when adding a vision-only connection", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_model_configs") {
        return Promise.resolve([
          {
            id: "model-minimax",
            name: "MiniMax",
            api_format: "openai",
            base_url: "https://api.minimax.io/v1",
            model_name: "MiniMax-M2.7",
            is_default: true,
            supports_vision: false,
          },
        ]);
      }
      if (command === "list_mcp_servers") return Promise.resolve([]);
      if (command === "list_search_configs") return Promise.resolve([]);
      if (command === "get_routing_settings") {
        return Promise.resolve({ max_call_depth: 4, node_timeout_seconds: 60, retry_count: 0 });
      }
      if (command === "list_builtin_provider_plugins") return Promise.resolve([]);
      if (command === "list_provider_configs") {
        return Promise.resolve([
          {
            id: "model-minimax",
            provider_key: "minimax",
            display_name: "MiniMax",
            protocol_type: "openai",
            base_url: "https://api.minimax.io/v1",
            auth_type: "api_key",
            api_key_encrypted: "",
            org_id: "",
            extra_json: "{}",
            enabled: true,
          },
        ]);
      }
      if (command === "get_capability_routing_policy") {
        return Promise.resolve({
          capability: "vision",
          primary_provider_id: "",
          primary_model: "",
          fallback_chain_json: "[]",
          timeout_ms: 90000,
          retry_count: 1,
          enabled: true,
        });
      }
      if (command === "list_capability_route_templates") return Promise.resolve([]);
      if (command === "save_model_config") return Promise.resolve("model-vision");
      if (command === "save_provider_config") return Promise.resolve("model-vision");
      if (command === "set_capability_routing_policy") return Promise.resolve(null);
      if (command === "get_feishu_gateway_settings") {
        return Promise.resolve({
          app_id: "",
          app_secret: "",
          ingress_token: "",
          encrypt_key: "",
          sidecar_base_url: "",
        });
      }
      if (command === "get_openclaw_plugin_feishu_advanced_settings") {
        return Promise.resolve({
          groups_json: "",
          dms_json: "",
          footer_json: "",
          account_overrides_json: "",
          render_mode: "",
          streaming: "",
          text_chunk_limit: "",
          chunk_mode: "",
          reply_in_thread: "",
          group_session_scope: "",
          topic_session_mode: "",
          markdown_mode: "",
          markdown_table_mode: "",
          heartbeat_visibility: "",
          heartbeat_interval_ms: "",
          media_max_mb: "",
          http_timeout_ms: "",
          config_writes: "",
          webhook_host: "",
          webhook_port: "",
          dynamic_agent_creation_enabled: "",
          dynamic_agent_creation_workspace_template: "",
          dynamic_agent_creation_agent_dir_template: "",
          dynamic_agent_creation_max_agents: "",
        });
      }
      if (command === "get_openclaw_lark_installer_session_status") {
        return Promise.resolve({
          running: false,
          mode: null,
          started_at: null,
          last_output_at: null,
          last_error: null,
          prompt_hint: null,
          recent_output: [],
        });
      }
      if (command === "get_feishu_setup_progress") {
        return Promise.resolve({
          environment: null,
          summary_state: "skipped",
          runtime_running: false,
          auth_status: "approved",
          pending_pairings: 0,
          plugin_installed: false,
          plugin_version: "",
          default_routing_employee_name: "",
          scoped_routing_count: 0,
          runtime_last_error: null,
        });
      }
      if (command === "list_feishu_pairing_requests") return Promise.resolve([]);
      if (command === "list_im_channel_registry") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    render(<SettingsView onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText("MiniMax")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByTestId("settings-model-provider-name"), {
      target: { value: "Custom Vision" },
    });
    fireEvent.change(screen.getByTestId("settings-model-provider-base-url"), {
      target: { value: "http://111.51.78.135:8060/v1" },
    });
    fireEvent.change(screen.getByTestId("settings-model-provider-model-name"), {
      target: { value: "Qwen3-VL-32B-Instruct" },
    });
    fireEvent.change(screen.getByTestId("settings-model-provider-api-key"), {
      target: { value: "sk-custom-vision" },
    });
    fireEvent.click(screen.getByTestId("settings-model-provider-supports-vision"));
    fireEvent.click(screen.getByTestId("settings-model-provider-save"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_model_config",
        expect.objectContaining({
          config: expect.objectContaining({
            is_default: false,
            supports_vision: true,
          }),
        }),
      );
    });

    expect(invokeMock).not.toHaveBeenCalledWith("set_default_model", {
      modelId: "model-vision",
    });
  });
});
