import { expect, type Page, test } from "@playwright/test";

test.setTimeout(60_000);

type TauriInvokeCall = {
  cmd: string;
  args: Record<string, unknown> | undefined;
};

async function installTauriMocks(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const calls: TauriInvokeCall[] = [];
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
          return [];
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
        case "get_feishu_gateway_settings":
          return {
            app_id: "",
            app_secret: "",
            ingress_token: "",
            encrypt_key: "",
            sidecar_base_url: "",
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
          if (args?.instanceId === "feishu:default") {
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
          }
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
        case "get_routing_settings":
          return { max_call_depth: 4, node_timeout_seconds: 60, retry_count: 0 };
        case "list_builtin_provider_plugins":
          return [];
        case "list_provider_configs":
          return [];
        case "get_capability_routing_policy":
          return null;
        case "list_capability_route_templates":
          return [];
        case "get_desktop_lifecycle_paths":
          return {
            app_data_dir: "",
            cache_dir: "",
            log_dir: "",
            default_work_dir: "",
          };
        case "list_im_routing_bindings":
          return [];
        case "set_wecom_gateway_settings":
          return null;
        case "start_wecom_connector":
          return "wecom:wecom-main";
        case "upsert_im_routing_binding":
          return "rule-wecom-1";
        case "simulate_im_route":
          return { agentId: "wecom-agent", matchedBy: "binding.channel" };
        default:
          return null;
      }
    };

    const w = window as typeof window & {
      __TAURI_INTERNALS__?: { invoke: typeof invoke };
      __E2E_TAURI_CALLS__?: TauriInvokeCall[];
    };
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

test("settings supports minimal wecom connector and routing flow", async ({ page }) => {
  await page.getByRole("button", { name: "设置" }).first().click();
  await page.getByRole("button", { name: "渠道连接器" }).click();

  await expect(page.getByTestId("connector-panel-wecom")).toBeVisible();

  await page.getByPlaceholder("企业微信 Corp ID").fill("wwcorp-updated");
  await page.getByRole("button", { name: "保存企业微信连接器" }).click();

  const retryButtons = page.getByRole("button", { name: "重试连接" });
  await retryButtons.nth(1).click();

  await page.getByLabel("路由渠道").selectOption("wecom");
  await page.getByPlaceholder("agent_id（如 main）").fill("wecom-agent");
  await page.getByRole("button", { name: "保存规则" }).click();
  await page.getByRole("button", { name: "模拟路由" }).click();

  await expect(page.getByText("将由：wecom-agent")).toBeVisible();
  await expect(page.getByText("命中原因：渠道规则")).toBeVisible();

  const calls = await readInvokeCalls(page);
  expect(
    calls.some(
      (call) =>
        call.cmd === "set_wecom_gateway_settings" &&
        String((call.args?.settings as Record<string, unknown> | undefined)?.corp_id ?? "") ===
          "wwcorp-updated",
    ),
  ).toBe(true);
  expect(
    calls.some(
      (call) =>
        call.cmd === "start_wecom_connector" &&
        String(call.args?.corpId ?? "") === "wwcorp-updated" &&
        String(call.args?.agentId ?? "") === "1000002",
    ),
  ).toBe(true);
  expect(
    calls.some(
      (call) =>
        call.cmd === "upsert_im_routing_binding" &&
        String((call.args?.input as Record<string, unknown> | undefined)?.channel ?? "") ===
          "wecom",
    ),
  ).toBe(true);
  expect(
    calls.some(
      (call) =>
        call.cmd === "simulate_im_route" &&
        String((call.args?.payload as Record<string, unknown> | undefined)?.channel ?? "") ===
          "wecom",
    ),
  ).toBe(true);
});
