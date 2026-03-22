import { expect, type Page, test } from "@playwright/test";

test.setTimeout(180_000);

type TauriInvokeCall = {
  cmd: string;
  args: Record<string, unknown> | undefined;
};

async function installTauriMocks(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const calls: TauriInvokeCall[] = [];
    const providerConfig = {
      id: "model-a",
      provider_key: "openai",
      display_name: "OpenAI",
      protocol_type: "openai",
      base_url: "https://api.openai.com/v1",
      auth_type: "api_key",
      api_key_encrypted: "***",
      org_id: "",
      extra_json: "{}",
      enabled: true,
    };
    const searchConfig = {
      id: "search-a",
      name: "Brave Search",
      api_format: "search_brave",
      base_url: "https://api.search.brave.com",
      model_name: "",
      is_default: true,
    };
    const invoke = async (cmd: string, args?: Record<string, unknown>) => {
      calls.push({ cmd, args });
      switch (cmd) {
        case "list_skills":
          return [
            {
              id: "builtin-general",
              name: "General",
              description: "Default skill",
              version: "1.0.0",
              author: "e2e",
              recommended_model: "model-a",
              tags: [],
              created_at: new Date().toISOString(),
            },
          ];
        case "list_model_configs":
          return [
            {
              id: "model-a",
              name: "OpenAI",
              api_format: "openai",
              base_url: "https://api.openai.com/v1",
              model_name: "gpt-4o-mini",
              is_default: true,
            },
          ];
        case "list_agent_employees":
          return [];
        case "list_search_configs":
          return [searchConfig];
        case "list_mcp_servers":
          return [];
        case "get_runtime_preferences":
          return {
            default_work_dir: "",
            default_language: "zh-CN",
            immersive_translation_enabled: true,
            immersive_translation_display: "translated_only",
            immersive_translation_trigger: "manual",
            translation_engine: "model_then_free",
            translation_model_id: "",
            auto_update_enabled: true,
            update_channel: "stable",
            dismissed_update_version: "",
            last_update_check_at: "",
            launch_at_login: false,
            launch_minimized: false,
            close_to_tray: true,
          };
        case "get_desktop_lifecycle_paths":
          return {
            app_data_dir: "",
            cache_dir: "",
            log_dir: "",
            default_work_dir: "",
          };
        case "get_routing_settings":
          return { max_call_depth: 4, node_timeout_seconds: 60, retry_count: 0 };
        case "list_builtin_provider_plugins":
          return [];
        case "list_provider_configs":
          return [providerConfig];
        case "get_capability_routing_policy":
          return null;
        case "list_capability_route_templates":
          return [];
        case "get_feishu_gateway_settings":
          return {
            app_id: "",
            app_secret: "",
            ingress_token: "",
            encrypt_key: "",
            sidecar_base_url: "",
          };
        case "get_openclaw_plugin_feishu_advanced_settings":
          return {
            groups_json: "",
            dms_json: "",
            footer_json: "",
            account_overrides_json: "",
            render_mode: "auto",
            streaming: "false",
            text_chunk_limit: "4000",
            chunk_mode: "length",
            reply_in_thread: "disabled",
            group_session_scope: "group",
            topic_session_mode: "disabled",
            markdown_mode: "native",
            markdown_table_mode: "native",
            heartbeat_visibility: "visible",
            heartbeat_interval_ms: "30000",
            media_max_mb: "20",
            http_timeout_ms: "60000",
            config_writes: "false",
            webhook_host: "",
            webhook_port: "",
            dynamic_agent_creation_enabled: "false",
            dynamic_agent_creation_workspace_template: "",
            dynamic_agent_creation_agent_dir_template: "",
            dynamic_agent_creation_max_agents: "",
          };
        case "get_feishu_plugin_environment_status":
          return {
            node_available: true,
            npm_available: true,
            node_version: "v22.0.0",
            npm_version: "10.0.0",
            can_install_plugin: true,
            can_start_runtime: true,
            error: null,
          };
        case "get_feishu_setup_progress":
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
          };
        case "get_openclaw_plugin_feishu_runtime_status":
          return {
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
          };
        case "get_openclaw_lark_installer_session_status":
          return {
            running: false,
            mode: null,
            started_at: null,
            last_output_at: null,
            last_error: null,
            prompt_hint: null,
            recent_output: [],
          };
        case "list_openclaw_plugin_channel_hosts":
          return [];
        case "list_feishu_pairing_requests":
          return [];
        case "get_openclaw_plugin_feishu_channel_snapshot":
          return {
            pluginRoot: "D:/plugins/openclaw-lark",
            preparedRoot: "D:/runtime/.workclaw-plugin-host-fixtures/openclaw-lark",
            manifest: {},
            entryPath: "D:/plugins/openclaw-lark/index.js",
            snapshot: {
              channelId: "feishu",
              defaultAccountId: "default",
              accountIds: ["default"],
              accounts: [],
              reloadConfigPrefixes: ["channels.feishu"],
              targetHint: "<chatId|user:openId|chat:chatId>",
            },
            logRecordCount: 1,
          };
        case "get_wecom_gateway_settings":
          return {
            corp_id: "wwcorp",
            agent_id: "1000002",
            agent_secret: "secret-x",
            sidecar_base_url: "",
          };
        case "get_feishu_long_connection_status":
          return {
            running: false,
            started_at: null,
            queued_events: 0,
          };
        case "get_wecom_connector_status":
          return {
            running: false,
            started_at: null,
            last_error: null,
            reconnect_attempts: 0,
            queue_depth: 0,
            instance_id: "wecom:wecom-main",
          };
        case "list_channel_connectors":
          return [
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
          ];
        case "get_channel_connector_diagnostics":
          if (args?.instanceId === "wecom:wecom-main") {
            return {
              connector: {
                channel: "wecom",
                display_name: "企业微信连接器",
                capabilities: ["receive_text", "send_text", "group_route", "direct_route"],
              },
              status: "stopped",
              health: {
                adapter_name: "wecom",
                instance_id: "wecom:wecom-main",
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
            };
          }
          return {
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
          };
        case "list_im_routing_bindings":
          return [];
        case "simulate_im_route":
          return { agentId: "main", matchedBy: "default" };
        default:
          return null;
      }
    };

    const w = window as typeof window & {
      __TAURI_INTERNALS__?: { invoke: typeof invoke };
      __E2E_TAURI_CALLS__?: TauriInvokeCall[];
    };
    try {
      window.localStorage.setItem("workclaw:initial-model-setup-completed", "1");
    } catch {
      // ignore
    }
    w.__TAURI_INTERNALS__ = { invoke };
    w.__E2E_TAURI_CALLS__ = calls;
  });
}

async function readInvokeCalls(page: Page): Promise<TauriInvokeCall[]> {
  return page.evaluate(() => {
    const w = window as typeof window & { __E2E_TAURI_CALLS__?: TauriInvokeCall[] };
    return w.__E2E_TAURI_CALLS__ ?? [];
  });
}

test.beforeEach(async ({ page }) => {
  await installTauriMocks(page);
  await page.goto("/", { waitUntil: "domcontentloaded" });
  await expect(
    page.getByRole("heading", { name: "你的电脑任务，交给打工虾们协作完成" }),
  ).toBeVisible({ timeout: 30_000 });
});

test("settings shows task-first feishu setup anchors while keeping routing data lazy-loaded", async ({ page }) => {
  await page.getByRole("button", { name: "设置" }).first().click();
  await expect(page.getByRole("button", { name: "模型连接" })).toBeVisible();

  await page.getByRole("button", { name: "渠道连接器" }).click();
  await expect(page.getByText("先安装飞书官方插件，再继续机器人接入", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "安装官方插件" })).toBeVisible();
  await expect(page.getByText("Verification Token")).toHaveCount(0);
  await expect(page.getByText("Encrypt Key")).toHaveCount(0);

  const calls = await readInvokeCalls(page);
  expect(calls.some((call) => call.cmd === "list_im_routing_bindings")).toBe(false);
});
