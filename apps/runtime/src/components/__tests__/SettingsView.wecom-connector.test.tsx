import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  SettingsView,
  buildFeishuOnboardingState,
  type FeishuOnboardingInput,
} from "../SettingsView";
import type { OpenClawLarkInstallerSessionStatus } from "../../types";

const invokeMock = vi.fn();
const { openExternalUrlMock } = vi.hoisted(() => ({
  openExternalUrlMock: vi.fn(() => Promise.resolve()),
}));

type InvokeOverride = (payload?: Record<string, unknown>) => Promise<unknown>;

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(() => Promise.resolve(null)),
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
        runtime_root_dir: "",
        pending_runtime_root_dir: null,
        last_runtime_migration_status: null,
        last_runtime_migration_message: null,
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

  afterEach(() => {
    vi.useRealTimers();
  });

  test("shows the redesigned feishu connector anchors and keeps advanced new-bot access", async () => {
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
    expect(within(screen.getByTestId("feishu-onboarding-step")).getByRole("button", { name: "启动连接" })).toBeInTheDocument();
    expect(screen.getByText("高级设置与控制台")).toBeInTheDocument();
  });

  test("refreshes plugin host inspection while feishu tab stays open without auto-starting runtime", async () => {
    vi.useFakeTimers();

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    expect(screen.getByText("飞书连接")).toBeInTheDocument();

    const initialHostLoads = invokeMock.mock.calls.filter(
      ([command]) => command === "list_openclaw_plugin_channel_hosts",
    ).length;
    const initialSnapshotLoads = invokeMock.mock.calls.filter(
      ([command]) => command === "get_openclaw_plugin_feishu_channel_snapshot",
    ).length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });

    expect(
      invokeMock.mock.calls.filter(([command]) => command === "list_openclaw_plugin_channel_hosts")
        .length,
    ).toBeGreaterThan(initialHostLoads);
    expect(
      invokeMock.mock.calls.filter(
        ([command]) => command === "get_openclaw_plugin_feishu_channel_snapshot",
      ).length,
    ).toBeGreaterThan(initialSnapshotLoads);
    expect(
      invokeMock.mock.calls.some(
        ([command]) => command === "start_openclaw_plugin_feishu_runtime",
      ),
    ).toBe(false);
  }, 15000);

  test("does not auto-start feishu runtime when opening the feishu tab", async () => {
    const { rerender } = render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(screen.getByText("飞书连接")).toBeInTheDocument();
    });

    expect(
      invokeMock.mock.calls.some(
        ([command]) => command === "start_openclaw_plugin_feishu_runtime",
      ),
    ).toBe(false);
  });

  test("polls feishu setup progress in background without re-running environment detection", async () => {
    vi.useFakeTimers();

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    const initialProgressLoads = invokeMock.mock.calls.filter(
      ([command]) => command === "get_feishu_setup_progress",
    ).length;
    const initialEnvironmentLoads = invokeMock.mock.calls.filter(
      ([command]) => command === "get_feishu_plugin_environment_status",
    ).length;
    const initialRuntimeStatusLoads = invokeMock.mock.calls.filter(
      ([command]) => command === "get_openclaw_plugin_feishu_runtime_status",
    ).length;

    expect(initialProgressLoads).toBe(1);
    expect(initialEnvironmentLoads).toBe(0);
    expect(initialRuntimeStatusLoads).toBe(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });

    expect(
      invokeMock.mock.calls.filter(([command]) => command === "get_feishu_setup_progress").length,
    ).toBeGreaterThan(initialProgressLoads);
    expect(
      invokeMock.mock.calls.filter(
        ([command]) => command === "get_feishu_plugin_environment_status",
      ).length,
    ).toBe(initialEnvironmentLoads);
    expect(
      invokeMock.mock.calls.filter(
        ([command]) => command === "get_openclaw_plugin_feishu_runtime_status",
      ).length,
    ).toBeGreaterThan(initialRuntimeStatusLoads);
  }, 15000);

  test("stops feishu background polling after leaving the feishu tab", async () => {
    vi.useFakeTimers();

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    fireEvent.click(screen.getByRole("button", { name: "模型连接" }));

    const hostLoadsBefore = invokeMock.mock.calls.filter(
      ([command]) => command === "list_openclaw_plugin_channel_hosts",
    ).length;
    const progressLoadsBefore = invokeMock.mock.calls.filter(
      ([command]) => command === "get_feishu_setup_progress",
    ).length;
    const runtimeLoadsBefore = invokeMock.mock.calls.filter(
      ([command]) => command === "get_openclaw_plugin_feishu_runtime_status",
    ).length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });

    expect(
      invokeMock.mock.calls.filter(([command]) => command === "list_openclaw_plugin_channel_hosts").length,
    ).toBe(hostLoadsBefore);
    expect(
      invokeMock.mock.calls.filter(([command]) => command === "get_feishu_setup_progress").length,
    ).toBe(progressLoadsBefore);
    expect(
      invokeMock.mock.calls.filter(([command]) => command === "get_openclaw_plugin_feishu_runtime_status").length,
    ).toBe(runtimeLoadsBefore);
  }, 15000);

  test("renders one primary onboarding step at a time", async () => {
    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(screen.getByTestId("feishu-onboarding-step")).toBeInTheDocument();
    });

    expect(screen.getAllByTestId("feishu-onboarding-step")).toHaveLength(1);
    expect(screen.getByTestId("feishu-onboarding-step")).toHaveTextContent("完成授权");
    expect(screen.queryByText("检查运行环境")).not.toBeVisible();
    expect(screen.queryByText("绑定已有机器人")).not.toBeVisible();
  });

  test("switches from skip to reopen within the local onboarding flow", async () => {
    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(within(screen.getByTestId("feishu-onboarding-step")).getByRole("button", { name: "启动连接" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "暂时跳过" }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "重新打开引导" })).toBeInTheDocument();
    });

    expect(
      within(screen.getByTestId("feishu-onboarding-step")).queryByRole("button", { name: "启动连接" }),
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "重新打开引导" }));

    await waitFor(() => {
      expect(within(screen.getByTestId("feishu-onboarding-step")).getByRole("button", { name: "启动连接" })).toBeInTheDocument();
      expect(screen.getByTestId("feishu-onboarding-step")).toBeInTheDocument();
    });
  });

  test("reconciles local skip state when backend onboarding progress advances", async () => {
    let setupProgressCalls = 0;

    installInvokeMock({
      get_feishu_setup_progress: async () => {
        setupProgressCalls += 1;
        if (setupProgressCalls === 1) {
          return {
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
            auth_status: "pending",
            pending_pairings: 0,
            default_routing_employee_name: null,
            scoped_routing_count: 0,
            summary_state: "awaiting_auth",
          };
        }

        return {
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
        };
      },
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(within(screen.getByTestId("feishu-onboarding-step")).getByRole("button", { name: "启动连接" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "暂时跳过" }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "重新打开引导" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "重新打开引导" }));

    await waitFor(() => {
      expect(within(screen.getByTestId("feishu-onboarding-step")).getByRole("button", { name: "启动连接" })).toBeInTheDocument();
    });

    fireEvent.click(within(screen.getByTestId("feishu-onboarding-step")).getByRole("button", { name: "刷新授权状态" }));

    await waitFor(() => {
      expect(screen.getByText("设置接待")).toBeInTheDocument();
      expect(within(screen.getByTestId("feishu-onboarding-step")).getByRole("button", { name: "请从员工中心继续" })).toBeDisabled();
      expect(screen.queryByText("已跳过引导。需要时随时点击“重新打开引导”。")).not.toBeInTheDocument();
    });
  });

  test("keeps the existing console available as an advanced section", async () => {
    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(screen.getByText("高级设置与控制台")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("高级设置与控制台"));

    await waitFor(() => {
      expect(screen.getByText("连接详情")).toBeInTheDocument();
      expect(screen.getByText("高级设置")).toBeInTheDocument();
    });
  });

  test("starts the advanced create-bot installer session from the redesigned feishu page", async () => {
    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(screen.getByText("高级设置与控制台")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("高级设置与控制台"));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "新建机器人向导（高级）" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "新建机器人向导（高级）" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("start_openclaw_lark_installer_session", {
        mode: "create",
        appId: null,
        appSecret: null,
      });
    });
  });

  test("shows an immediate launching state for the advanced create-bot flow", async () => {
    let resolveStart: ((value: OpenClawLarkInstallerSessionStatus) => void) | null = null;
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
        credentials_configured: false,
        plugin_installed: true,
        plugin_version: "2026.3.17",
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "pending",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "ready_to_bind",
      }),
      start_openclaw_lark_installer_session: () =>
        new Promise<OpenClawLarkInstallerSessionStatus>((resolve) => {
          resolveStart = resolve;
        }),
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    const onboardingStep = await screen.findByTestId("feishu-onboarding-step");
    fireEvent.click(within(onboardingStep).getByRole("button", { name: "新建机器人向导（高级）" }));

    expect(within(onboardingStep).getByRole("button", { name: "启动中..." })).toBeDisabled();
    expect(within(onboardingStep).getByText("正在启动飞书官方创建机器人向导，请稍候...")).toBeInTheDocument();

    const startResolver = resolveStart as ((value: OpenClawLarkInstallerSessionStatus) => void) | null;
    expect(startResolver).not.toBeNull();
    (startResolver as (value: OpenClawLarkInstallerSessionStatus) => void)({
      running: true,
      mode: "create",
      started_at: "2026-03-22T00:00:00Z",
      last_output_at: "2026-03-22T00:00:01Z",
      last_error: null,
      prompt_hint: "请使用飞书扫码完成机器人创建",
      recent_output: ["[system] official installer started"],
    });

    await waitFor(() => {
      expect(within(onboardingStep).getByText("已启动飞书官方创建机器人向导")).toBeInTheDocument();
    });
  });

  test("switches the guided Feishu branch between existing and create paths", async () => {
    installInvokeMock({
      get_feishu_gateway_settings: async () => ({
        app_id: "",
        app_secret: "",
        ingress_token: "",
        encrypt_key: "",
        sidecar_base_url: "",
      }),
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
        credentials_configured: false,
        plugin_installed: false,
        plugin_version: null,
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "pending",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "ready_to_bind",
      }),
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    const onboardingStep = await screen.findByTestId("feishu-onboarding-step");

    fireEvent.click(within(onboardingStep).getByRole("button", { name: "绑定已有机器人" }));

    await waitFor(() => {
      expect(within(onboardingStep).getByRole("button", { name: "验证机器人信息" })).toBeInTheDocument();
      expect(screen.getByTestId("feishu-onboarding-state")).toHaveTextContent("绑定已有机器人");
    });

    fireEvent.click(screen.getByText("高级设置与控制台"));

    await waitFor(() => {
      expect(screen.getByText("App ID")).toBeInTheDocument();
      expect(screen.getByText("App Secret")).toBeInTheDocument();
    });

    fireEvent.click(within(onboardingStep).getByRole("button", { name: "新建机器人" }));

    await waitFor(() => {
      expect(within(onboardingStep).getByRole("button", { name: "新建机器人向导（高级）" })).toBeInTheDocument();
      expect(screen.getByTestId("feishu-onboarding-state")).toHaveTextContent("新建机器人");
      expect(screen.queryByText("App ID")).not.toBeInTheDocument();
      expect(screen.queryByText("App Secret")).not.toBeInTheDocument();
    });
  });

  test("requires existing-robot credentials before continuing but not on the create path", async () => {
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
        credentials_configured: false,
        plugin_installed: false,
        plugin_version: null,
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "pending",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "ready_to_bind",
      }),
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    const onboardingStep = await screen.findByTestId("feishu-onboarding-step");

    fireEvent.click(within(onboardingStep).getByRole("button", { name: "绑定已有机器人" }));
    fireEvent.click(screen.getByText("高级设置与控制台"));

    await waitFor(() => {
      expect(screen.getByPlaceholderText("cli_xxx")).toBeInTheDocument();
      expect(screen.getByPlaceholderText("填写机器人的 App Secret")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByPlaceholderText("cli_xxx"), { target: { value: "" } });
    fireEvent.change(screen.getByPlaceholderText("填写机器人的 App Secret"), { target: { value: "" } });

    fireEvent.click(within(onboardingStep).getByRole("button", { name: "验证机器人信息" }));

    await waitFor(() => {
      expect(within(onboardingStep).getByText("请先填写已有机器人的 App ID 和 App Secret")).toBeInTheDocument();
      expect(screen.getAllByText("请先填写已有机器人的 App ID 和 App Secret")).toHaveLength(1);
    });

    fireEvent.click(within(onboardingStep).getByRole("button", { name: "新建机器人" }));
    fireEvent.click(within(onboardingStep).getByRole("button", { name: "新建机器人向导（高级）" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("start_openclaw_lark_installer_session", {
        mode: "create",
        appId: null,
        appSecret: null,
      });
    });
  });

  test("shows create-path installer failures inline inside the guided step", async () => {
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
        credentials_configured: false,
        plugin_installed: false,
        plugin_version: null,
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "pending",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "ready_to_bind",
      }),
      get_openclaw_lark_installer_session_status: async () => ({
        running: false,
        mode: "create",
        started_at: null,
        last_output_at: null,
        last_error: null,
        prompt_hint: null,
        recent_output: [],
      }),
      start_openclaw_lark_installer_session: async () => {
        throw "simulated install failure";
      },
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    const onboardingStep = await screen.findByTestId("feishu-onboarding-step");

    fireEvent.click(within(onboardingStep).getByRole("button", { name: "新建机器人向导（高级）" }));

    await waitFor(() => {
      expect(within(onboardingStep).getByText("启动飞书官方创建机器人向导失败: simulated install failure")).toBeInTheDocument();
      expect(screen.getAllByText("启动飞书官方创建机器人向导失败: simulated install failure")).toHaveLength(1);
    });
  });

  test("surfaces the create installer output directly inside the guided panel", async () => {
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
        credentials_configured: false,
        plugin_installed: true,
        plugin_version: "2026.3.17",
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "pending",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "ready_to_bind",
      }),
      get_openclaw_lark_installer_session_status: async () => ({
        running: true,
        mode: "create",
        started_at: "2026-03-22T00:00:00Z",
        last_output_at: "2026-03-22T00:00:02Z",
        last_error: null,
        prompt_hint: "请使用飞书扫码完成机器人创建",
        recent_output: [
          "[system] official installer started (pid=123, mode=create)",
          "Scan with Feishu to configure your bot",
        ],
      }),
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    const guidedPanel = await screen.findByTestId("feishu-guided-installer-panel");
    expect(within(guidedPanel).getByText("飞书官方创建机器人向导正在这里继续")).toBeInTheDocument();
    expect(within(guidedPanel).getByText("向导运行中")).toBeInTheDocument();
    expect(within(guidedPanel).getByText("请使用飞书扫码完成机器人创建")).toBeInTheDocument();
    expect(within(guidedPanel).getByText(/Scan with Feishu to configure your bot/)).toBeInTheDocument();
  });

  test("pins the qr block above filtered installer logs in the guided panel", async () => {
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
        credentials_configured: false,
        plugin_installed: true,
        plugin_version: "2026.3.17",
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "pending",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "ready_to_bind",
      }),
      get_openclaw_lark_installer_session_status: async () => ({
        running: true,
        mode: "create",
        started_at: "2026-03-22T00:00:00Z",
        last_output_at: "2026-03-22T00:00:02Z",
        last_error: null,
        prompt_hint: "请使用飞书扫码完成机器人创建",
        recent_output: [
          "[DEBUG] Request: {",
          "  host: 'https://accounts.feishu.cn',",
          "}",
          "████",
          "█  █",
          "████",
          "[DEBUG] Poll result: {",
          "  \"error\": \"authorization_pending\"",
          "}",
          "[stderr] - Fetching configuration results (正在获取你的机器人配置结果)...",
        ],
      }),
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    const guidedPanel = await screen.findByTestId("feishu-guided-installer-panel");
    expect(within(guidedPanel).getByTestId("feishu-guided-installer-qr")).toHaveTextContent("████");
    expect(within(guidedPanel).getByText("请使用飞书扫码继续")).toBeInTheDocument();
    expect(within(guidedPanel).getByText("[stderr] - Fetching configuration results (正在获取你的机器人配置结果)...")).toBeInTheDocument();
    expect(within(guidedPanel).queryByText("[DEBUG] Request: {")).not.toBeInTheDocument();
    expect(within(guidedPanel).queryByText("[DEBUG] Poll result: {")).not.toBeInTheDocument();
  });

  test("shows auth-start failures inline inside the active onboarding step", async () => {
    installInvokeMock({
      get_feishu_gateway_settings: async () => ({
        app_id: "cli-app",
        app_secret: "cli-secret",
        ingress_token: "",
        encrypt_key: "",
        sidecar_base_url: "",
      }),
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
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "pending",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "awaiting_auth",
      }),
      start_openclaw_plugin_feishu_runtime: async () => {
        throw "runtime failed";
      },
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    const onboardingStep = await screen.findByTestId("feishu-onboarding-step");

    fireEvent.click(within(onboardingStep).getByRole("button", { name: "启动连接" }));

    await waitFor(() => {
      expect(within(onboardingStep).getByText("安装并启动飞书连接失败: runtime failed")).toBeInTheDocument();
      expect(screen.getAllByText("安装并启动飞书连接失败: runtime failed")).toHaveLength(1);
    });
  });

  test("uses the backend-derived create_robot branch when the primary CTA is clicked before any path chip", async () => {
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
        credentials_configured: false,
        plugin_installed: false,
        plugin_version: null,
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "pending",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "ready_to_bind",
      }),
      get_openclaw_lark_installer_session_status: async () => ({
        running: false,
        mode: "create",
        started_at: null,
        last_output_at: null,
        last_error: null,
        prompt_hint: null,
        recent_output: [],
      }),
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    const onboardingStep = await screen.findByTestId("feishu-onboarding-step");

    expect(screen.getByTestId("feishu-onboarding-state")).toHaveTextContent("新建机器人");

    fireEvent.click(within(onboardingStep).getByRole("button", { name: "新建机器人向导（高级）" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("start_openclaw_lark_installer_session", {
        mode: "create",
        appId: null,
        appSecret: null,
      });
    });
  });

  test("shows install/start failures inline inside the authorization onboarding step", async () => {
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
        plugin_installed: false,
        plugin_version: null,
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "pending",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "awaiting_auth",
      }),
      list_openclaw_plugin_channel_hosts: async () => [],
      install_openclaw_plugin_from_npm: async () => {
        throw new Error("npm offline");
      },
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    const onboardingStep = await screen.findByTestId("feishu-onboarding-step");

    fireEvent.click(within(onboardingStep).getByRole("button", { name: "安装并启动" }));

    await waitFor(() => {
      expect(within(onboardingStep).getByText("安装并启动飞书连接失败: Error: npm offline")).toBeInTheDocument();
      expect(screen.getAllByText("安装并启动飞书连接失败: Error: npm offline")).toHaveLength(1);
    });
  });

  test("uses the authorization primary action instead of becoming a no-op", async () => {
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
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "pending",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "awaiting_auth",
      }),
      start_openclaw_plugin_feishu_runtime: async () => {
        throw new Error("runtime boom");
      },
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    const onboardingStep = await screen.findByTestId("feishu-onboarding-step");

    fireEvent.click(within(onboardingStep).getByRole("button", { name: "启动连接" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("start_openclaw_plugin_feishu_runtime", {
        pluginId: "openclaw-lark",
        accountId: null,
      });
      expect(within(onboardingStep).getByText("安装并启动飞书连接失败: Error: runtime boom")).toBeInTheDocument();
      expect(screen.getAllByText("安装并启动飞书连接失败: Error: runtime boom")).toHaveLength(1);
    });
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
      expect(within(screen.getByTestId("feishu-onboarding-step")).getByRole("button", { name: "去设置接待员工" })).toBeInTheDocument();
    });

    fireEvent.click(within(screen.getByTestId("feishu-onboarding-step")).getByRole("button", { name: "去设置接待员工" }));

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
      expect(screen.getByText("高级设置与控制台")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("高级设置与控制台"));

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
      expect(screen.getByText("高级设置与控制台")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("高级设置与控制台"));
    fireEvent.click(screen.getByText("连接详情"));

    await waitFor(() => {
      expect(screen.getByText("这里展示当前连接是否正常、最近一次事件，以及排查问题时最有用的诊断摘要。")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "复制诊断摘要" })).toBeInTheDocument();
      expect(screen.getByText("原始日志（最近 3 条）")).toBeInTheDocument();
    });
  });

  test("shows pending pairing approvals as a normal connection detail state", async () => {
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
        plugin_version: "2026.3.25",
        runtime_running: true,
        runtime_last_error: null,
        auth_status: "approved",
        pending_pairings: 1,
        default_routing_employee_name: "太子",
        scoped_routing_count: 0,
        summary_state: "awaiting_pairing_approval",
      }),
      get_openclaw_plugin_feishu_runtime_status: async () => ({
        plugin_id: "openclaw-lark",
        account_id: "default",
        running: true,
        started_at: "2026-03-24T23:00:00Z",
        last_stop_at: null,
        last_event_at: "2026-03-24T23:06:00Z",
        last_error: null,
        pid: 4321,
        port: 3100,
        recent_logs: [
          "[info] runtime: feishu[default]: sender ou_4866 not paired, creating pairing request",
          "[pairing] feishu: created request 5a776683-bb67-48ac-86bf-7029a5057823 for ou_4866 code=6X4ZN54W",
        ],
      }),
      list_feishu_pairing_requests: async () => [
        {
          id: "pairing-pending-1",
          channel: "feishu",
          account_id: "default",
          sender_id: "ou_4866",
          chat_id: "oc_chat_1",
          code: "6X4ZN54W",
          status: "pending",
          created_at: "2026-03-24T23:06:00Z",
          updated_at: "2026-03-24T23:06:00Z",
          resolved_at: null,
          resolved_by_user: "",
        },
      ],
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(screen.getByText("高级设置与控制台")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("高级设置与控制台"));
    fireEvent.click(screen.getByText("连接详情"));

    await waitFor(() => {
      expect(screen.getAllByText("连接正常，但有新的接入请求等待批准。").length).toBeGreaterThan(0);
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
    expect(
      within(screen.getByTestId("feishu-onboarding-step")).getByRole("button", {
        name: "请从员工中心继续",
      }),
    ).toBeDisabled();
  });

  test("shows a continue onboarding entry when setup is incomplete", async () => {
    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    await waitFor(() => {
      expect(within(screen.getByTestId("feishu-onboarding-step")).getByRole("button", { name: "启动连接" })).toBeInTheDocument();
    });
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

    expect(screen.getAllByText("默认接待员工和 2 条群聊范围规则都已生效。").length).toBeGreaterThan(0);
    expect(screen.getAllByRole("button", { name: "调整接待设置" }).length).toBeGreaterThan(0);
  });

  test("exposes a guided step order for unfinished Feishu setup", () => {
    const onboarding = buildFeishuOnboardingState({
      summaryState: "plugin_not_installed",
      installerMode: "link",
    });

    expect(onboarding.skipped).toBe(false);
    expect(onboarding.currentStep).toBe("plugin");
    expect(onboarding.stepOrder).toEqual(["environment", "plugin", "existing_robot", "authorize", "approve_pairing", "routing"]);
    expect(onboarding.canContinue).toBe(false);
  });

  test("prefers the explicit summary state over setup progress state", () => {
    const onboarding = buildFeishuOnboardingState({
      summaryState: "awaiting_auth",
      setupProgress: {
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
        plugin_version: null,
        runtime_running: true,
        runtime_last_error: null,
        auth_status: "approved",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "plugin_not_installed",
      },
    });

    expect(onboarding.currentStep).toBe("authorize");
    expect(onboarding.stepOrder).toEqual(["environment", "plugin", "existing_robot", "authorize", "approve_pairing", "routing"]);
  });

  test("defaults branch selection to create robot once plugin install is ready", () => {
    const existingRobot = buildFeishuOnboardingState({
      summaryState: "ready_to_bind",
      installerMode: "link",
    });
    const createRobot = buildFeishuOnboardingState({
      summaryState: "ready_to_bind",
      installerMode: "create",
    });

    expect(existingRobot.currentStep).toBe("create_robot");
    expect(existingRobot.mode).toBe("create_robot");
    expect(createRobot.currentStep).toBe("create_robot");
    expect(createRobot.mode).toBe("create_robot");
  });

  test("treats ready_to_bind as branch selection instead of falling back to environment", () => {
    const onboarding = buildFeishuOnboardingState({
      summaryState: "ready_to_bind",
      installerMode: null,
    });

    expect(onboarding.currentStep).toBe("create_robot");
    expect(onboarding.stepOrder).toEqual(["environment", "plugin", "create_robot", "authorize", "approve_pairing", "routing"]);
    expect(onboarding.mode).toBe("create_robot");
  });

  test("routes plugin_starting into the authorization step instead of dropping back to environment", () => {
    const onboarding = buildFeishuOnboardingState({
      summaryState: "plugin_starting",
      installerMode: "create",
    });

    expect(onboarding.currentStep).toBe("authorize");
    expect(onboarding.stepOrder).toEqual(["environment", "plugin", "create_robot", "authorize", "approve_pairing", "routing"]);
  });

  test("routes pending pairing approval into its own onboarding step", () => {
    const onboarding = buildFeishuOnboardingState({
      summaryState: "awaiting_pairing_approval",
      setupProgress: {
        runtime_running: true,
        auth_status: "pending",
        pending_pairings: 1,
      },
      installerMode: "create",
    });

    expect(onboarding.currentStep).toBe("approve_pairing");
    expect(onboarding.stepOrder).toEqual(["environment", "plugin", "create_robot", "authorize", "approve_pairing", "routing"]);
  });

  test("refreshes pending pairing approval while the feishu tab stays open", async () => {
    let progressCalls = 0;
    let pairingCalls = 0;
    installInvokeMock({
      get_feishu_setup_progress: async () => {
        progressCalls += 1;
        if (progressCalls < 2) {
          return {
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
            scoped_routing_count: 0,
            summary_state: "ready",
          };
        }
        return {
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
          pending_pairings: 1,
          default_routing_employee_name: "太子",
          scoped_routing_count: 0,
          summary_state: "awaiting_pairing_approval",
        };
      },
      list_feishu_pairing_requests: async () => {
        pairingCalls += 1;
        if (pairingCalls < 2) {
          return [];
        }
        return [
          {
            id: "pairing-refresh",
            channel: "feishu",
            account_id: "default",
            sender_id: "ou_refresh",
            chat_id: "ou_refresh",
            code: "REFRESH1",
            status: "pending",
            created_at: "2026-03-19T10:00:00Z",
            updated_at: "2026-03-19T10:00:00Z",
            resolved_at: null,
            resolved_by_user: "",
          },
        ];
      },
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    expect(await screen.findByText("飞书已接通，正在接收消息")).toBeInTheDocument();
    const initialProgressCalls = progressCalls;
    const initialPairingCalls = pairingCalls;

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 5200));
    });

    expect(progressCalls).toBeGreaterThan(initialProgressCalls);
    expect(pairingCalls).toBeGreaterThan(initialPairingCalls);
  }, 12000);

  test("shows plugin-install guidance before branch selection on a fresh machine", async () => {
    installInvokeMock({
      get_feishu_gateway_settings: async () => ({
        app_id: "",
        app_secret: "",
        ingress_token: "",
        encrypt_key: "",
        sidecar_base_url: "",
      }),
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
        credentials_configured: false,
        plugin_installed: false,
        plugin_version: null,
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "unknown",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "plugin_not_installed",
      }),
      get_openclaw_lark_installer_session_status: async () => ({
        running: false,
        mode: null,
        started_at: null,
        last_output_at: null,
        last_error: null,
        prompt_hint: null,
        recent_output: [],
      }),
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    const onboardingStep = await screen.findByTestId("feishu-onboarding-step");

    expect(screen.getByText("运行环境")).toBeInTheDocument();
    expect(screen.getAllByText("已准备好").length).toBeGreaterThan(0);
    expect(screen.getByText("先安装飞书官方插件，再继续机器人接入")).toBeInTheDocument();
    expect(screen.getByText("先安装飞书官方插件。安装完成后，再继续新建机器人或绑定已有机器人。")).toBeInTheDocument();
    expect(within(onboardingStep).getByRole("button", { name: "安装官方插件" })).toBeInTheDocument();
    expect(within(onboardingStep).queryByRole("button", { name: "新建机器人" })).not.toBeInTheDocument();
  });

  test("installs the official plugin from the guided plugin step before branching", async () => {
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
        credentials_configured: false,
        plugin_installed: false,
        plugin_version: null,
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "unknown",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "plugin_not_installed",
      }),
      install_openclaw_plugin_from_npm: async () => ({
        plugin_id: "openclaw-lark",
        npm_spec: "@larksuite/openclaw-lark",
        version: "2026.3.17",
        install_path: "D:/plugins/openclaw-lark",
        source_type: "npm",
        manifest_json: "{}",
        installed_at: "2026-03-21T00:00:00Z",
        updated_at: "2026-03-21T00:00:00Z",
      }),
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    const onboardingStep = await screen.findByTestId("feishu-onboarding-step");
    fireEvent.click(within(onboardingStep).getByRole("button", { name: "安装官方插件" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("install_openclaw_plugin_from_npm", {
        pluginId: "openclaw-lark",
        npmSpec: "@larksuite/openclaw-lark",
      });
    });
  });

  test("shows the next-step notice after the create installer finishes successfully", async () => {
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
        credentials_configured: false,
        plugin_installed: true,
        plugin_version: "2026.3.17",
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "unknown",
        pending_pairings: 0,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "ready_to_bind",
      }),
      get_openclaw_lark_installer_session_status: async () => ({
        running: false,
        mode: "create",
        started_at: "2026-03-21T00:00:00Z",
        last_output_at: "2026-03-21T00:00:10Z",
        last_error: null,
        prompt_hint: null,
        recent_output: [
          "[stderr] Success! Bot configured.",
          "[system] official installer finished",
        ],
      }),
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    expect(await screen.findByText("机器人创建已完成，请点击“启动连接”继续完成授权。")).toBeInTheDocument();
  });

  test("shows pairing approval guidance and approves the pending request from the guided step", async () => {
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
        auth_status: "pending",
        pending_pairings: 1,
        default_routing_employee_name: null,
        scoped_routing_count: 0,
        summary_state: "awaiting_pairing_approval",
      }),
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    expect(await screen.findByText("批准飞书接入请求")).toBeInTheDocument();
    expect(screen.getAllByText("批准这次接入").length).toBeGreaterThan(0);

    fireEvent.click(screen.getAllByRole("button", { name: "批准这次接入" })[0]!);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("approve_feishu_pairing_request", {
        requestId: "pairing-1",
        resolvedByUser: "settings-ui",
      });
    });
  });

  test("explains that approved connections will auto-restore when runtime is not running", async () => {
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
        runtime_running: false,
        runtime_last_error: null,
        auth_status: "approved",
        pending_pairings: 0,
        default_routing_employee_name: "太子",
        scoped_routing_count: 0,
        summary_state: "plugin_starting",
      }),
    });

    render(<SettingsView onClose={() => {}} initialTab="feishu" />);

    expect(await screen.findByText("正在恢复飞书连接")).toBeInTheDocument();
    expect(
      screen.getByText("WorkClaw 会自动尝试恢复上次已接通的飞书连接；如果恢复失败，再手动点击“启动连接”。"),
    ).toBeInTheDocument();
  });

  test("keeps the rest of the app accessible when Feishu setup is skipped", () => {
    const skipped = buildFeishuOnboardingState({
      summaryState: "skipped",
      skipped: true,
    });

    expect(skipped.skipped).toBe(true);
    expect(skipped.canContinue).toBe(true);
    expect(skipped.currentStep).toBe("skipped");
    expect(skipped.stepOrder).toEqual(["skipped"]);
  });
});
