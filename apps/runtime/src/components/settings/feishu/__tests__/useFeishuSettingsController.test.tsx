import { act, renderHook } from "@testing-library/react";
import { useFeishuSettingsController } from "../useFeishuSettingsController";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

function installInvokeMock() {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
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
    if (command === "list_openclaw_plugin_channel_hosts") {
      return Promise.resolve([]);
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
    if (command === "list_feishu_pairing_requests") {
      return Promise.resolve([]);
    }
    return Promise.resolve(null);
  });
}

describe("useFeishuSettingsController", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    installInvokeMock();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("stops the Feishu polling loop after leaving the tab", async () => {
    const { rerender } = renderHook(({ activeTab }) => useFeishuSettingsController({ activeTab }), {
      initialProps: { activeTab: "feishu" as const },
    });

    await act(async () => {
      await Promise.resolve();
    });

    const beforePollCount = invokeMock.mock.calls.filter(([command]) => command === "get_feishu_setup_progress").length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });

    const currentCount = invokeMock.mock.calls.filter(([command]) => command === "get_feishu_setup_progress").length;
    expect(currentCount).toBeGreaterThan(beforePollCount);

    rerender({ activeTab: "models" as const });
    const afterLeaveCount = invokeMock.mock.calls.filter(([command]) => command === "get_feishu_setup_progress").length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(10000);
    });

    expect(
      invokeMock.mock.calls.filter(([command]) => command === "get_feishu_setup_progress").length,
    ).toBe(afterLeaveCount);
  });
});
