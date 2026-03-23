import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();
let mcpServers: Array<{ id: string; name: string; command: string; args?: string[]; env?: Record<string, string> }> = [];
let mcpId = 0;

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

function installDefaultInvokeMock() {
  invokeMock.mockImplementation((command: string, payload?: Record<string, unknown>) => {
    if (command === "list_model_configs") return Promise.resolve([]);
    if (command === "list_provider_configs") return Promise.resolve([]);
    if (command === "list_search_configs") return Promise.resolve([]);
    if (command === "get_routing_settings") {
      return Promise.resolve({ max_call_depth: 4, node_timeout_seconds: 60, retry_count: 0 });
    }
    if (command === "list_builtin_provider_plugins") return Promise.resolve([]);
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
    if (command === "list_mcp_servers") return Promise.resolve([...mcpServers]);
    if (command === "add_mcp_server") {
      const { name, command: cmd, args, env } = payload as {
        name: string;
        command: string;
        args: string[];
        env: Record<string, string>;
      };
      mcpId += 1;
      mcpServers = [...mcpServers, { id: `mcp-${mcpId}`, name, command: cmd, args, env }];
      return Promise.resolve(null);
    }
    if (command === "remove_mcp_server") {
      const { id } = payload as { id: string };
      mcpServers = mcpServers.filter((server) => server.id !== id);
      return Promise.resolve(null);
    }
    return Promise.resolve(null);
  });
}

describe("SettingsView MCP settings", () => {
  beforeEach(() => {
    mcpServers = [];
    mcpId = 0;
    invokeMock.mockReset();
    installDefaultInvokeMock();
  });

  test("adds and removes an MCP server through the extracted section", async () => {
    render(<SettingsView onClose={() => {}} initialTab="mcp" />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "MCP 服务器" })).toHaveClass("text-[var(--sm-primary-strong)]");
    });

    fireEvent.change(screen.getByRole("combobox"), { target: { value: "brave-search" } });
    expect(screen.getByPlaceholderText("请输入 BRAVE_API_KEY")).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("请输入 BRAVE_API_KEY"), {
      target: { value: "brave-test-key" },
    });
    fireEvent.click(screen.getByRole("button", { name: "添加 MCP 服务器" }));

    await waitFor(() => {
      expect(screen.getByText("brave-search")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "删除" }));

    await waitFor(() => {
      expect(screen.queryByText("brave-search")).not.toBeInTheDocument();
    });
  });

  test("surfaces MCP env JSON validation errors before invoke", async () => {
    render(<SettingsView onClose={() => {}} initialTab="mcp" />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "MCP 服务器" })).toHaveClass("text-[var(--sm-primary-strong)]");
    });

    fireEvent.change(screen.getByPlaceholderText("例: filesystem"), { target: { value: "broken-json-server" } });
    fireEvent.change(screen.getByPlaceholderText("例: npx"), { target: { value: "npx" } });
    fireEvent.click(screen.getByRole("button", { name: "高级：环境变量 JSON 配置" }));
    fireEvent.change(screen.getByPlaceholderText('例: {"API_KEY": "xxx"}'), {
      target: { value: "{broken" },
    });
    fireEvent.click(screen.getByRole("button", { name: "添加 MCP 服务器" }));

    await waitFor(() => {
      expect(screen.getByText("环境变量 JSON 格式错误")).toBeInTheDocument();
    });
    expect(invokeMock).not.toHaveBeenCalledWith(
      "add_mcp_server",
      expect.objectContaining({ name: "broken-json-server" }),
    );
  });
});
