import { expect, type Page, test } from "@playwright/test";

type TauriMockSkill = {
  id: string;
  name: string;
  description: string;
  version: string;
  author: string;
  recommended_model: string;
  tags: string[];
  created_at: string;
};

type TauriMockModel = {
  id: string;
  name: string;
  api_format: string;
  base_url: string;
  model_name: string;
  is_default: boolean;
};

type TauriInvokeCall = {
  cmd: string;
  args: Record<string, unknown> | undefined;
};

async function installTauriMocks(page: Page): Promise<void> {
  const skills: TauriMockSkill[] = [
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
    {
      id: "local-e2e-skill",
      name: "E2E Local Skill",
      description: "Skill used by Playwright smoke tests",
      version: "local",
      author: "e2e",
      recommended_model: "model-a",
      tags: ["e2e"],
      created_at: new Date().toISOString(),
    },
  ];
  const models: TauriMockModel[] = [
    {
      id: "model-a",
      name: "OpenAI",
      api_format: "openai",
      base_url: "https://api.openai.com/v1",
      model_name: "gpt-4o-mini",
      is_default: true,
    },
  ];

  await page.addInitScript(
    ({ mockedSkills, mockedModels }) => {
      const calls: TauriInvokeCall[] = [];
      const sessions: Array<{
        id: string;
        title: string;
        created_at: string;
        skill_id: string;
        work_dir: string;
      }> = [];
      let sessionCounter = 1;
      let runtimePreferences = {
        default_work_dir: "",
        default_language: "zh-CN",
        immersive_translation_enabled: true,
        immersive_translation_display: "translated_only",
      };

      const providerConfig = {
        id: mockedModels[0]?.id || "model-a",
        provider_key: "openai",
        display_name: mockedModels[0]?.name || "OpenAI",
        protocol_type: "openai",
        base_url: mockedModels[0]?.base_url || "https://api.openai.com/v1",
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
      try {
        window.localStorage.setItem("workclaw:initial-model-setup-completed", "1");
      } catch {
        // ignore
      }

      const invoke = async (cmd: string, args?: Record<string, unknown>) => {
        calls.push({ cmd, args });
        switch (cmd) {
          case "list_skills":
            return mockedSkills;
          case "list_model_configs":
            return mockedModels;
          case "list_agent_employees":
            return [];
          case "get_sessions":
            if (!args?.skillId || typeof args.skillId !== "string") {
              return sessions;
            }
            return sessions.filter((item) => item.skill_id === args.skillId);
          case "create_session": {
            const sessionId = `session-e2e-${sessionCounter++}`;
            const skillId =
              typeof args?.skillId === "string"
                ? args.skillId
                : mockedSkills[0]?.id || "builtin-general";
            sessions.push({
              id: sessionId,
              title: "E2E Session",
              created_at: new Date().toISOString(),
              skill_id: skillId,
              work_dir: "",
            });
            return sessionId;
          }
          case "list_search_configs":
            return [searchConfig];
          case "get_runtime_preferences":
            return runtimePreferences;
          case "list_mcp_servers":
            return [];
          case "get_model_api_key":
            return "sk-e2e-mock";
          case "save_provider_config":
            return null;
          case "list_provider_configs":
            return [providerConfig];
          case "set_runtime_preferences":
            runtimePreferences = {
              ...runtimePreferences,
              ...(args?.input as Record<string, unknown> | undefined),
            };
            return { ...runtimePreferences };
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
    },
    { mockedSkills: skills, mockedModels: models },
  );
}

async function readInvokeCalls(page: Page): Promise<TauriInvokeCall[]> {
  return page.evaluate(() => {
    const w = window as typeof window & { __E2E_TAURI_CALLS__?: TauriInvokeCall[] };
    return w.__E2E_TAURI_CALLS__ ?? [];
  });
}

test.describe.configure({ timeout: 60_000 });

test.beforeEach(async ({ page }) => {
  await installTauriMocks(page);
  for (let attempt = 0; attempt < 2; attempt += 1) {
    try {
      await page.goto("/", { waitUntil: "domcontentloaded" });
      break;
    } catch (error) {
      if (attempt === 1) {
        throw error;
      }
    }
  }
  await expect(
    page.getByRole("heading", { name: "你的电脑任务，交给打工虾们协作完成" }),
  ).toBeVisible({ timeout: 30_000 });
});

test("navigates across start task, settings and experts", async ({ page }) => {
  await expect(
    page.getByRole("heading", { name: "你的电脑任务，交给打工虾们协作完成" }),
  ).toBeVisible();

  await page.getByRole("button", { name: "设置" }).first().click();
  await expect(page.getByRole("button", { name: "模型连接" })).toBeVisible();

  await page.getByRole("button", { name: "专家技能" }).first().click();
  await expect(page.getByRole("heading", { name: "专家技能" })).toBeVisible();

  await page.getByRole("button", { name: "开始任务" }).first().click();
  await expect(
    page.getByRole("heading", { name: "你的电脑任务，交给打工虾们协作完成" }),
  ).toBeVisible();
});

test("saves default language and translation preference from settings", async ({ page }) => {
  await page.getByRole("button", { name: "设置" }).first().click();
  await expect(page.getByRole("button", { name: "模型连接" })).toBeVisible();
  await page.getByRole("button", { name: "桌面 / 系统" }).click();

  await page.getByLabel("默认语言").selectOption("en-US");
  await page.getByRole("button", { name: "保存语言与翻译设置" }).click();
  await expect(page.getByText("已保存")).toBeVisible();

  const calls = await readInvokeCalls(page);
  const saveCall = calls.find((call) => call.cmd === "set_runtime_preferences");
  expect(saveCall).toBeTruthy();
  expect(saveCall?.args?.input).toMatchObject({
    default_language: "en-US",
  });
});

test("can start a task from experts skill card and open chat directly", async ({ page }) => {
  await page.getByRole("button", { name: "专家技能" }).first().click();
  await expect(page.getByRole("heading", { name: "专家技能" })).toBeVisible();

  const skillCard = page
    .locator("div.bg-white.border.border-gray-200.rounded-xl.p-4")
    .filter({ hasText: "E2E Local Skill" });
  await expect(skillCard).toBeVisible();
  await skillCard.getByRole("button", { name: "开始任务" }).click();

  await expect(
    page.getByTestId("e2e-chat-view"),
  ).toBeVisible();
  await expect(page.getByTestId("e2e-chat-session-id")).toContainText("session-e2e-");
});

test("creates a new session from start task composer", async ({ page }) => {
  const input = page.getByPlaceholder("先描述你要完成什么任务...");
  await input.fill("请帮我检查本地项目并给出下一步建议");
  await input.press("Enter");

  await expect(page.getByTestId("e2e-chat-view")).toBeVisible();
  await expect(page.getByTestId("e2e-chat-session-id")).toContainText("session-e2e-");
});
