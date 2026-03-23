import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("SettingsView semantic theme", () => {
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
      if (command === "list_openclaw_plugin_channel_hosts") return Promise.resolve([]);
      if (command === "list_feishu_pairing_requests") return Promise.resolve([]);
      if (command === "get_openclaw_plugin_feishu_runtime_status") return Promise.resolve(null);
      return Promise.resolve(null);
    });
  });

  test("keeps first-use dev tools hidden for regular users", async () => {
    render(<SettingsView onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "模型连接" })).toBeInTheDocument();
    });
    expect(screen.queryByTestId("model-setup-dev-tools")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "重置首次引导状态" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "打开首次配置弹层" })).not.toBeInTheDocument();
  });

  test("uses semantic classes and keeps regular-user tabs focused", async () => {
    render(<SettingsView onClose={() => {}} />);

    const modelsTab = screen.getByRole("button", { name: "模型连接" });
    expect(modelsTab).toHaveClass("sm-btn");
    expect(screen.queryByRole("button", { name: "Providers" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "能力路由" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "健康检查" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "MCP 服务器" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "自动路由" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "渠道连接器" })).toBeInTheDocument();

    const searchTab = screen.getByRole("button", { name: "搜索引擎" });
    expect(searchTab).toHaveClass("sm-btn");
    fireEvent.click(searchTab);
    await waitFor(() => {
      expect(screen.getByText("快速选择搜索引擎")).toBeInTheDocument();
    });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "MCP 服务器" }));
    });
    await act(async () => {
      fireEvent.change(screen.getByRole("combobox"), { target: { value: "brave-search" } });
    });
    expect(screen.getByPlaceholderText("请输入 BRAVE_API_KEY")).toBeInTheDocument();
  });

  test("honors initialTab and keeps the close affordance visible", async () => {
    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "渠道连接器" })).toHaveClass("text-[var(--sm-primary-strong)]");
    });
    expect(screen.getByRole("button", { name: "返回" })).toBeInTheDocument();
  });

  test("shows first-use dev tools only when explicitly enabled", async () => {
    const onDevResetFirstUseOnboarding = vi.fn();
    const onDevOpenQuickModelSetup = vi.fn();

    render(
      <SettingsView
        onClose={() => {}}
        showDevModelSetupTools
        onDevResetFirstUseOnboarding={onDevResetFirstUseOnboarding}
        onDevOpenQuickModelSetup={onDevOpenQuickModelSetup}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("model-setup-dev-tools")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "重置首次引导状态" }));
    fireEvent.click(screen.getByRole("button", { name: "打开首次配置弹层" }));

    expect(onDevResetFirstUseOnboarding).toHaveBeenCalledTimes(1);
    expect(onDevOpenQuickModelSetup).toHaveBeenCalledTimes(1);
  });
});
