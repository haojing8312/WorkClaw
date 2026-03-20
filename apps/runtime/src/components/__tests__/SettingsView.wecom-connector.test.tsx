import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();
const { openExternalUrlMock } = vi.hoisted(() => ({
  openExternalUrlMock: vi.fn(() => Promise.resolve()),
}));

type InvokeOverride = (payload?: Record<string, unknown>) => Promise<unknown>;

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("../../utils/openExternalUrl", () => ({
  openExternalUrl: openExternalUrlMock,
}));

function installInvokeMock(overrides: Record<string, InvokeOverride> = {}) {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string, payload?: Record<string, unknown>) => {
    const override = overrides[command];
    if (override) {
      return override(payload);
    }
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
    if (command === "get_openclaw_plugin_feishu_advanced_settings") {
      return Promise.resolve({
        groups_json: '{\n  "oc_demo": {\n    "enabled": true,\n    "requireMention": true\n  }\n}',
        dms_json: '{\n  "ou_demo": {\n    "enabled": true,\n    "systemPrompt": "优先回答测试问题"\n  }\n}',
        footer_json: '{\n  "status": true,\n  "elapsed": true\n}',
        account_overrides_json: '{\n  "default": {\n    "renderMode": "card"\n  }\n}',
        render_mode: "card",
        streaming: "true",
        text_chunk_limit: "2400",
        chunk_mode: "newline",
        reply_in_thread: "enabled",
        group_session_scope: "group_sender",
        topic_session_mode: "enabled",
        markdown_mode: "native",
        markdown_table_mode: "native",
        heartbeat_visibility: "visible",
        heartbeat_interval_ms: "30000",
        media_max_mb: "20",
        http_timeout_ms: "60000",
        config_writes: "true",
        webhook_host: "127.0.0.1",
        webhook_port: "8787",
        dynamic_agent_creation_enabled: "true",
        dynamic_agent_creation_workspace_template: "workspace/{sender_id}",
        dynamic_agent_creation_agent_dir_template: "agents/{sender_id}",
        dynamic_agent_creation_max_agents: "48",
      });
    }
    if (command === "get_feishu_plugin_environment_status") {
      return Promise.resolve({
        node_available: true,
        npm_available: true,
        node_version: "v22.0.0",
        npm_version: "10.0.0",
        can_install_plugin: true,
        can_start_runtime: true,
        error: null,
      });
    }
    if (command === "get_feishu_setup_progress") {
      return Promise.resolve({
        environment: {
          node_available: true,
          npm_available: true,
          node_version: "v22.0.0",
          npm_version: "10.0.0",
          can_install_plugin: true,
          can_start_runtime: true,
          error: null,
        },
        credentials_configured: true,
        plugin_installed: true,
        plugin_version: "2026.3.17",
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "pending",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "awaiting_auth",
      });
    }
    if (command === "set_openclaw_plugin_feishu_advanced_settings") {
      const settings = (payload?.settings as Record<string, string> | undefined) ?? {};
      return Promise.resolve({
        groups_json: settings.groups_json ?? "",
        dms_json: settings.dms_json ?? "",
        footer_json: settings.footer_json ?? "",
        account_overrides_json: settings.account_overrides_json ?? "",
        render_mode: settings.render_mode ?? "",
        streaming: settings.streaming ?? "",
        text_chunk_limit: settings.text_chunk_limit ?? "",
        chunk_mode: settings.chunk_mode ?? "",
        reply_in_thread: settings.reply_in_thread ?? "",
        group_session_scope: settings.group_session_scope ?? "",
        topic_session_mode: settings.topic_session_mode ?? "",
        markdown_mode: settings.markdown_mode ?? "",
        markdown_table_mode: settings.markdown_table_mode ?? "",
        heartbeat_visibility: settings.heartbeat_visibility ?? "",
        heartbeat_interval_ms: settings.heartbeat_interval_ms ?? "",
        media_max_mb: settings.media_max_mb ?? "",
        http_timeout_ms: settings.http_timeout_ms ?? "",
        config_writes: settings.config_writes ?? "",
        webhook_host: settings.webhook_host ?? "",
        webhook_port: settings.webhook_port ?? "",
        dynamic_agent_creation_enabled: settings.dynamic_agent_creation_enabled ?? "",
        dynamic_agent_creation_workspace_template:
          settings.dynamic_agent_creation_workspace_template ?? "",
        dynamic_agent_creation_agent_dir_template:
          settings.dynamic_agent_creation_agent_dir_template ?? "",
        dynamic_agent_creation_max_agents:
          settings.dynamic_agent_creation_max_agents ?? "",
      });
    }
    if (command === "get_feishu_long_connection_status") {
      return Promise.resolve({
        running: false,
        started_at: null,
        queued_events: 0,
      });
    }
    if (command === "get_openclaw_plugin_feishu_runtime_status") {
      return Promise.resolve({
        plugin_id: "openclaw-lark",
        account_id: "default",
        running: false,
        started_at: null,
        last_stop_at: null,
        last_event_at: null,
        last_error: null,
        pid: null,
        port: null,
        recent_logs: [],
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
    if (command === "start_openclaw_lark_installer_session") {
      return Promise.resolve({
        running: true,
        mode: payload?.mode || "link",
        started_at: "2026-03-19T10:00:00Z",
        last_output_at: "2026-03-19T10:00:01Z",
        last_error: null,
        prompt_hint: "请输入机器人 App ID",
        recent_output: ["[system] official installer started"],
      });
    }
    if (command === "send_openclaw_lark_installer_input") {
      return Promise.resolve({
        running: true,
        mode: "link",
        started_at: "2026-03-19T10:00:00Z",
        last_output_at: "2026-03-19T10:00:02Z",
        last_error: null,
        prompt_hint: null,
        recent_output: ["[manual-input] cli-app"],
      });
    }
    if (command === "stop_openclaw_lark_installer_session") {
      return Promise.resolve({
        running: false,
        mode: "link",
        started_at: "2026-03-19T10:00:00Z",
        last_output_at: "2026-03-19T10:00:03Z",
        last_error: null,
        prompt_hint: null,
        recent_output: ["[system] official installer finished"],
      });
    }
    if (command === "probe_openclaw_plugin_feishu_credentials") {
      return Promise.resolve({
        ok: true,
        app_id: payload?.appId || "cli-app",
        bot_name: "WorkClaw Bot",
        bot_open_id: "ou_bot_open_id",
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
    if (command === "list_openclaw_plugin_channel_hosts") {
      return Promise.resolve([
        {
          plugin_id: "openclaw-lark",
          npm_spec: "@larksuite/openclaw-lark",
          version: "2026.3.17",
          channel: "feishu",
          display_name: "Feishu",
          capabilities: ["media", "reactions", "threads", "outbound", "pairing"],
          reload_config_prefixes: ["channels.feishu"],
          target_hint: "<chatId|user:openId|chat:chatId>",
          docs_path: "/channels/feishu",
          status: "ready",
          error: null,
        },
      ]);
    }
    if (command === "get_openclaw_plugin_feishu_channel_snapshot") {
      return Promise.resolve({
        pluginRoot: "D:/plugins/openclaw-lark",
        preparedRoot: "D:/runtime/.workclaw-plugin-host-fixtures/openclaw-lark",
        manifest: {},
        entryPath: "D:/plugins/openclaw-lark/index.js",
        snapshot: {
          channelId: "feishu",
          defaultAccountId: "default",
          accountIds: ["default"],
          accounts: [
            {
              accountId: "default",
              account: {
                accountId: "default",
                enabled: true,
                configured: true,
              },
              describedAccount: {
                accountId: "default",
                enabled: true,
                configured: true,
              },
              allowFrom: [],
              warnings: [],
            },
          ],
          reloadConfigPrefixes: ["channels.feishu"],
          targetHint: "<chatId|user:openId|chat:chatId>",
        },
        logRecordCount: 1,
      });
    }
    if (command === "list_feishu_pairing_requests") {
      return Promise.resolve([
        {
          id: "pairing-1",
          channel: "feishu",
          account_id: "default",
          sender_id: "ou_applicant",
          chat_id: "ou_applicant",
          code: "PAIR1234",
          status: "pending",
          created_at: "2026-03-19T10:00:00Z",
          updated_at: "2026-03-19T10:00:00Z",
          resolved_at: null,
          resolved_by_user: "",
        },
      ]);
    }
    if (command === "approve_feishu_pairing_request" || command === "deny_feishu_pairing_request") {
      return Promise.resolve({
        id: "pairing-1",
        channel: "feishu",
        account_id: "default",
        sender_id: "ou_applicant",
        chat_id: "ou_applicant",
        code: "PAIR1234",
        status: command === "approve_feishu_pairing_request" ? "approved" : "denied",
        created_at: "2026-03-19T10:00:00Z",
        updated_at: "2026-03-19T10:01:00Z",
        resolved_at: "2026-03-19T10:01:00Z",
        resolved_by_user: "settings-ui",
      });
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
}

describe("SettingsView connector visibility", () => {
  beforeEach(() => {
    installInvokeMock();
    openExternalUrlMock.mockClear();
  });

  test("shows the redesigned feishu connector anchors without legacy console controls", async () => {
    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "渠道连接器" }));

    await waitFor(() => {
      expect(screen.getByText("飞书连接")).toBeInTheDocument();
    });

    expect(screen.getByText("飞书连接")).toBeInTheDocument();
    expect(screen.getByText("检查运行环境")).toBeInTheDocument();
    expect(screen.getByText("绑定已有机器人")).toBeInTheDocument();
    expect(screen.getByText("完成飞书授权")).toBeInTheDocument();
    expect(screen.getByText("接待设置")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "连接配置" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "官方插件" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "配对与授权" })).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText("飞书事件订阅 Verification Token")).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText("飞书事件订阅 Encrypt Key")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "新建机器人" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "运行新建机器人向导" })).not.toBeInTheDocument();
  });

  test("opens the employee hub callback from the routing card", async () => {
    const onOpenEmployees = vi.fn();

    installInvokeMock({
      get_feishu_setup_progress: async () => ({
        environment: {
          node_available: true,
          npm_available: true,
          node_version: "v22.0.0",
          npm_version: "10.0.0",
          can_install_plugin: true,
          can_start_runtime: true,
          error: null,
        },
        credentials_configured: true,
        plugin_installed: true,
        plugin_version: "2026.3.17",
        runtime_running: true,
        runtime_last_error: null,
        auth_status: "approved",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "ready_for_routing",
      }),
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" onOpenEmployees={onOpenEmployees} />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "去设置接待员工" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "去设置接待员工" }));

    expect(onOpenEmployees).toHaveBeenCalledTimes(1);
  });

  test("opens the official docs through the desktop external-url helper", async () => {
    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "查看官方文档" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "查看官方文档" }));

    await waitFor(() => {
      expect(openExternalUrlMock).toHaveBeenCalledWith(
        "https://bytedance.larkoffice.com/docx/MFK7dDFLFoVlOGxWCv5cTXKmnMh#M0usd9GLwoiBxtx1UyjcpeMhnRe",
      );
    });
  });

  test("groups advanced feishu settings into readable sections", async () => {
    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(screen.getByText("高级设置")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("高级设置"));

    await waitFor(() => {
      expect(screen.getByText("消息与展示")).toBeInTheDocument();
      expect(screen.getByText("群聊与私聊规则")).toBeInTheDocument();
      expect(screen.getByText("运行与行为")).toBeInTheDocument();
      expect(screen.getByText("动态 Agent 相关")).toBeInTheDocument();
    });
  });

  test("shows condensed connection diagnostics before raw logs", async () => {
    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(screen.getByText("连接详情")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("连接详情"));

    await waitFor(() => {
      expect(screen.getByText("这里展示当前连接是否正常、最近一次事件，以及排查问题时最有用的诊断摘要。")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "复制诊断摘要" })).toBeInTheDocument();
      expect(screen.getByText("原始日志（最近 3 条）")).toBeInTheDocument();
    });
  });

  test("shows routing completion guidance when no default employee is configured", async () => {
    installInvokeMock({
      get_feishu_setup_progress: async () => ({
        environment: {
          node_available: true,
          npm_available: true,
          node_version: "v22.0.0",
          npm_version: "10.0.0",
          can_install_plugin: true,
          can_start_runtime: true,
          error: null,
        },
        credentials_configured: true,
        plugin_installed: true,
        plugin_version: "2026.3.17",
        runtime_running: true,
        runtime_last_error: null,
        auth_status: "approved",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "ready_for_routing",
      }),
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(screen.getAllByText("还差默认员工").length).toBeGreaterThan(0);
    });

    expect(screen.getAllByText("还差默认员工").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "去设置接待员工" })).toBeInTheDocument();
  });

  test("shows ready-to-receive state when default routing employee exists", async () => {
    installInvokeMock({
      get_feishu_setup_progress: async () => ({
        environment: {
          node_available: true,
          npm_available: true,
          node_version: "v22.0.0",
          npm_version: "10.0.0",
          can_install_plugin: true,
          can_start_runtime: true,
          error: null,
        },
        credentials_configured: true,
        plugin_installed: true,
        plugin_version: "2026.3.17",
        runtime_running: true,
        runtime_last_error: null,
        auth_status: "approved",
        pending_pairings: 0,
        default_routing_employee_name: "太子",
        scoped_routing_count: 2,
        summary_state: "ready_for_routing",
      }),
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(screen.getAllByText("已可接待").length).toBeGreaterThan(0);
    });

    expect(screen.getByText("默认接待员工和 2 条群聊范围规则都已生效。")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "调整接待设置" })).toBeInTheDocument();
  });
});
