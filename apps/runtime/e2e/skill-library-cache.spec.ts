import { expect, type Page, test } from "@playwright/test";

type TauriInvokeCall = {
  cmd: string;
  args: Record<string, unknown> | undefined;
};

type ClawhubStats = {
  libraryNetworkFetches: number;
  detailNetworkFetches: number;
  libraryCacheKeys: string[];
  detailCacheKeys: string[];
};

async function installTauriMocks(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const calls: TauriInvokeCall[] = [];
    const sessions: Array<{
      id: string;
      title: string;
      created_at: string;
      skill_id: string;
      work_dir: string;
    }> = [];
    let sessionCounter = 1;
    let clawhubNetworkAvailable = true;

    const runtimePreferences = {
      default_work_dir: "",
      default_language: "zh-CN",
      immersive_translation_enabled: true,
      immersive_translation_display: "translated_only",
      immersive_translation_trigger: "manual",
      translation_engine: "model_then_free",
      translation_model_id: "",
    };

    const localSkills = [
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

    const models = [
      {
        id: "model-a",
        name: "OpenAI",
        api_format: "openai",
        base_url: "https://api.openai.com/v1",
        model_name: "gpt-4o-mini",
        is_default: true,
      },
    ];
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

    const libraryItems = [
      {
        slug: "video-maker",
        name: "Video Maker",
        summary: "Generate short videos",
        tags: ["video", "creator"],
        stars: 120,
        downloads: 7800,
      },
    ];

    const detailsBySlug: Record<string, unknown> = {
      "video-maker": {
        slug: "video-maker",
        name: "Video Maker",
        summary: "Generate short videos",
        description: "Create short videos for social media",
        author: "Acme Team",
        github_url: "https://github.com/acme/video-maker",
        source_url: "https://www.clawhub.ai/skills/video-maker",
        updated_at: "2026-03-05T10:00:00.000Z",
        stars: 120,
        downloads: 7800,
        tags: ["video", "creator"],
      },
    };

    const libraryCache = new Map<string, unknown>();
    const detailCache = new Map<string, unknown>();
    let libraryNetworkFetches = 0;
    let detailNetworkFetches = 0;

    const delay = (ms: number) =>
      new Promise<void>((resolve) => {
        window.setTimeout(resolve, ms);
      });

    const buildLibraryKey = (args?: Record<string, unknown>) => {
      const cursor =
        typeof args?.cursor === "string" && args.cursor.trim().length > 0
          ? args.cursor.trim()
          : "__first__";
      const sort = typeof args?.sort === "string" && args.sort.trim().length > 0 ? args.sort : "updated";
      const limit = typeof args?.limit === "number" ? args.limit : 20;
      return `sort=${sort}:limit=${String(limit)}:cursor=${cursor}`;
    };

    const invoke = async (cmd: string, args?: Record<string, unknown>) => {
      calls.push({ cmd, args });
      switch (cmd) {
        case "list_skills":
          return localSkills;
        case "list_model_configs":
          return models;
        case "list_agent_employees":
          return [];
        case "list_search_configs":
          return [searchConfig];
        case "list_mcp_servers":
          return [];
        case "list_provider_configs":
          return [providerConfig];
        case "list_im_routing_bindings":
          return [];
        case "simulate_im_route":
          return { agentId: "main", matchedBy: "default" };
        case "get_runtime_preferences":
          return runtimePreferences;
        case "get_sessions":
          if (!args?.skillId || typeof args.skillId !== "string") {
            return sessions;
          }
          return sessions.filter((item) => item.skill_id === args.skillId);
        case "create_session": {
          const sessionId = `session-e2e-${sessionCounter++}`;
          const skillId =
            typeof args?.skillId === "string" ? args.skillId : "builtin-general";
          sessions.push({
            id: sessionId,
            title: "E2E Session",
            created_at: new Date().toISOString(),
            skill_id: skillId,
            work_dir: "",
          });
          return sessionId;
        }
        case "list_clawhub_library": {
          const key = buildLibraryKey(args);
          if (clawhubNetworkAvailable) {
            if (!libraryCache.has(key)) {
              await delay(240);
            }
            libraryNetworkFetches += 1;
            const payload = { items: libraryItems, next_cursor: null };
            libraryCache.set(key, payload);
            return payload;
          }
          if (libraryCache.has(key)) {
            return libraryCache.get(key);
          }
          throw new Error("ClawHub 列表加载失败: network unavailable");
        }
        case "get_clawhub_skill_detail": {
          const slug = typeof args?.slug === "string" ? args.slug : "";
          if (!slug) {
            throw new Error("slug 不能为空");
          }
          if (clawhubNetworkAvailable) {
            if (!detailCache.has(slug)) {
              await delay(180);
            }
            detailNetworkFetches += 1;
            const payload = detailsBySlug[slug];
            if (!payload) {
              throw new Error("skill not found");
            }
            detailCache.set(slug, payload);
            return payload;
          }
          if (detailCache.has(slug)) {
            return detailCache.get(slug);
          }
          throw new Error("ClawHub 详情加载失败: network unavailable");
        }
        default:
          return null;
      }
    };

    const w = window as typeof window & {
      __TAURI_INTERNALS__?: { invoke: typeof invoke };
      __E2E_TAURI_CALLS__?: TauriInvokeCall[];
      __E2E_SET_CLAWHUB_NETWORK__?: (available: boolean) => void;
      __E2E_READ_CLAWHUB_STATS__?: () => ClawhubStats;
    };
    try {
      window.localStorage.setItem("workclaw:initial-model-setup-completed", "1");
    } catch {
      // ignore
    }
    w.__TAURI_INTERNALS__ = { invoke };
    w.__E2E_TAURI_CALLS__ = calls;
    w.__E2E_SET_CLAWHUB_NETWORK__ = (available: boolean) => {
      clawhubNetworkAvailable = available;
    };
    w.__E2E_READ_CLAWHUB_STATS__ = () => ({
      libraryNetworkFetches,
      detailNetworkFetches,
      libraryCacheKeys: Array.from(libraryCache.keys()),
      detailCacheKeys: Array.from(detailCache.keys()),
    });
  });
}

async function readInvokeCalls(page: Page): Promise<TauriInvokeCall[]> {
  return page.evaluate(() => {
    const w = window as typeof window & { __E2E_TAURI_CALLS__?: TauriInvokeCall[] };
    return w.__E2E_TAURI_CALLS__ ?? [];
  });
}

async function readClawhubStats(page: Page): Promise<ClawhubStats> {
  return page.evaluate(() => {
    const w = window as typeof window & {
      __E2E_READ_CLAWHUB_STATS__?: () => ClawhubStats;
    };
    if (!w.__E2E_READ_CLAWHUB_STATS__) {
      throw new Error("missing clawhub stats reader");
    }
    return w.__E2E_READ_CLAWHUB_STATS__();
  });
}

async function setClawhubNetwork(page: Page, available: boolean): Promise<void> {
  await page.evaluate((next) => {
    const w = window as typeof window & {
      __E2E_SET_CLAWHUB_NETWORK__?: (available: boolean) => void;
    };
    if (!w.__E2E_SET_CLAWHUB_NETWORK__) {
      throw new Error("missing network switch");
    }
    w.__E2E_SET_CLAWHUB_NETWORK__(next);
  }, available);
}

test.describe.configure({ timeout: 90_000 });

test.beforeEach(async ({ page }) => {
  await installTauriMocks(page);
  await page.goto("/", { waitUntil: "domcontentloaded" });
  await expect(
    page.getByRole("heading", { name: "你的电脑任务，交给打工虾们协作完成" }),
  ).toBeVisible({ timeout: 30_000 });
  await page.getByRole("button", { name: "专家技能" }).first().click();
  await expect(page.getByRole("heading", { name: "专家技能" })).toBeVisible();
});

test("skill library reopens with cached list when network is offline", async ({ page }) => {
  await page.getByRole("button", { name: "技能库" }).click();
  await expect(page.getByText("Video Maker")).toBeVisible();

  const onlineStats = await readClawhubStats(page);
  expect(onlineStats.libraryNetworkFetches).toBeGreaterThan(0);
  expect(onlineStats.libraryCacheKeys.length).toBeGreaterThan(0);

  await setClawhubNetwork(page, false);
  await page.getByRole("button", { name: "我的技能" }).click();
  await page.getByRole("button", { name: "技能库" }).click();

  await expect(page.getByText("Video Maker")).toBeVisible();
  await expect(page.getByText("ClawHub 列表加载失败")).not.toBeVisible();

  const offlineStats = await readClawhubStats(page);
  expect(offlineStats.libraryNetworkFetches).toBe(onlineStats.libraryNetworkFetches);

  const calls = await readInvokeCalls(page);
  expect(calls.filter((item) => item.cmd === "list_clawhub_library").length).toBeGreaterThanOrEqual(2);
});

test("skill detail reopens from cache when network is offline", async ({ page }) => {
  await page.getByRole("button", { name: "技能库" }).click();
  await expect(page.getByText("Video Maker")).toBeVisible();

  await page.getByText("Video Maker").first().click();
  await expect(page.getByText("技能详情")).toBeVisible();
  await expect(page.getByText("Acme Team")).toBeVisible();
  await page.getByRole("button", { name: "关闭", exact: true }).click();
  await expect(page.getByText("技能详情")).not.toBeVisible();

  const onlineStats = await readClawhubStats(page);
  expect(onlineStats.detailNetworkFetches).toBe(1);
  expect(onlineStats.detailCacheKeys).toContain("video-maker");

  await setClawhubNetwork(page, false);
  await page.getByText("Video Maker").first().click();

  await expect(page.getByText("技能详情")).toBeVisible();
  await expect(page.getByText("Acme Team")).toBeVisible();
  await expect(page.getByText("详情加载失败")).not.toBeVisible();

  const offlineStats = await readClawhubStats(page);
  expect(offlineStats.detailNetworkFetches).toBe(1);
});
