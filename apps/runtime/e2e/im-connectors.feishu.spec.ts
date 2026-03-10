import { expect, type Page, test } from "@playwright/test";

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

test("settings exposes connector tab and loads routing rules through connector entry", async ({ page }) => {
  await page.getByRole("button", { name: "设置" }).first().click();
  await expect(page.getByRole("button", { name: "模型连接" })).toBeVisible();

  await page.getByRole("button", { name: "渠道连接器" }).click();
  await expect(page.locator("div").filter({ hasText: /^渠道连接器$/ })).toBeVisible();
  await expect(page.getByText("渠道连接器路由向导（当前：飞书）")).toBeVisible();

  const calls = await readInvokeCalls(page);
  expect(calls.some((call) => call.cmd === "list_im_routing_bindings")).toBe(true);
});
