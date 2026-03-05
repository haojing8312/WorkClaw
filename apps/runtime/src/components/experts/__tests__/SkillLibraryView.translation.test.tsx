import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SkillLibraryView } from "../SkillLibraryView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

class MockIntersectionObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}

describe("SkillLibraryView immersive translation", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    Object.defineProperty(window, "IntersectionObserver", {
      writable: true,
      value: MockIntersectionObserver,
    });
  });

  test("translates library texts after clicking translate action", async () => {
    let resolvePrefs!: (value: any) => void;
    const prefsPromise = new Promise((resolve) => {
      resolvePrefs = resolve;
    });
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_runtime_preferences") {
        return prefsPromise;
      }
      if (command === "list_clawhub_library") {
        return Promise.resolve({
          items: [
            {
              slug: "video-maker",
              name: "Video Maker",
              summary: "Generate short videos",
              tags: ["video", "latest"],
              stars: 12,
              downloads: 99,
            },
          ],
          next_cursor: null,
        });
      }
      if (command === "translate_texts_with_preferences") {
        const texts: string[] = payload?.texts ?? [];
        return Promise.resolve(texts.map((text) => `ZH:${text}`));
      }
      return Promise.resolve(null);
    });

    render(
      <SkillLibraryView
        installedSkillIds={new Set<string>()}
        onInstall={async () => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Video Maker")).toBeInTheDocument();
    });

    expect(
      invokeMock.mock.calls.filter(([command]) => command === "translate_texts_with_preferences"),
    ).toHaveLength(0);

    resolvePrefs({
      default_work_dir: "E:\\workspace",
      default_language: "zh-CN",
      immersive_translation_enabled: true,
      immersive_translation_display: "translated_only",
      immersive_translation_trigger: "manual",
    });

    await waitFor(() => {
      expect(
        invokeMock.mock.calls.filter(([command]) => command === "translate_texts_with_preferences"),
      ).toHaveLength(0);
    });

    fireEvent.click(screen.getByRole("button", { name: "翻译本页" }));

    await waitFor(() => {
      expect(screen.getByText("ZH:Video Maker")).toBeInTheDocument();
      expect(screen.getByText("ZH:Generate short videos")).toBeInTheDocument();
    });

    expect(invokeMock).toHaveBeenCalledWith(
      "translate_texts_with_preferences",
      expect.objectContaining({
        texts: expect.any(Array),
      }),
    );
  });

  test("requests clawhub library sorted by downloads", async () => {
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "list_clawhub_library") {
        return Promise.resolve({
          items: [
            {
              slug: "video-maker",
              name: "Video Maker",
              summary: "Generate short videos",
              tags: ["video", "latest"],
              stars: 12,
              downloads: 99,
            },
          ],
          next_cursor: null,
        });
      }
      if (command === "translate_texts_with_preferences") {
        const texts: string[] = payload?.texts ?? [];
        return Promise.resolve(texts);
      }
      return Promise.resolve(null);
    });

    render(
      <SkillLibraryView
        installedSkillIds={new Set<string>()}
        onInstall={async () => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Video Maker")).toBeInTheDocument();
    });

    expect(invokeMock).toHaveBeenCalledWith(
      "list_clawhub_library",
      expect.objectContaining({
        sort: "downloads",
      }),
    );
  });

  test("shows translation error when manual translate request fails", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_clawhub_library") {
        return Promise.resolve({
          items: [
            {
              slug: "video-maker",
              name: "Video Maker",
              summary: "Generate short videos",
              tags: ["video"],
              stars: 12,
              downloads: 99,
            },
          ],
          next_cursor: null,
        });
      }
      if (command === "translate_texts_with_preferences") {
        return Promise.reject(new Error("translate failed"));
      }
      return Promise.resolve(null);
    });

    render(
      <SkillLibraryView
        installedSkillIds={new Set<string>()}
        onInstall={async () => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Video Maker")).toBeInTheDocument();
      expect(screen.getByText("Generate short videos")).toBeInTheDocument();
    });

    await waitFor(() => {
      expect(screen.getByText("翻译失败：translate failed")).toBeInTheDocument();
    });
  });

  test("renders bilingual inline when runtime preference is bilingual_inline", async () => {
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          default_work_dir: "E:\\workspace",
          default_language: "zh-CN",
          immersive_translation_enabled: true,
          immersive_translation_display: "bilingual_inline",
        });
      }
      if (command === "list_clawhub_library") {
        return Promise.resolve({
          items: [
            {
              slug: "video-maker",
              name: "Video Maker",
              summary: "Generate short videos",
              tags: ["video"],
              stars: 12,
              downloads: 99,
            },
          ],
          next_cursor: null,
        });
      }
      if (command === "translate_texts_with_preferences") {
        const texts: string[] = payload?.texts ?? [];
        return Promise.resolve(texts.map((text) => `ZH:${text}`));
      }
      return Promise.resolve(null);
    });

    render(
      <SkillLibraryView
        installedSkillIds={new Set<string>()}
        onInstall={async () => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Video Maker")).toBeInTheDocument();
    });

    await waitFor(() => {
      expect(screen.getByText("ZH:Video Maker (Video Maker)")).toBeInTheDocument();
      expect(screen.getByText("ZH:Generate short videos (Generate short videos)")).toBeInTheDocument();
    });
  });

  test("shows translation fallback hint when immersive translation returns source text", async () => {
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          default_work_dir: "E:\\workspace",
          default_language: "zh-CN",
          immersive_translation_enabled: true,
          immersive_translation_display: "bilingual_inline",
        });
      }
      if (command === "list_clawhub_library") {
        return Promise.resolve({
          items: [
            {
              slug: "video-maker",
              name: "Video Maker",
              summary: "Generate short videos",
              tags: ["video"],
              stars: 12,
              downloads: 99,
            },
          ],
          next_cursor: null,
        });
      }
      if (command === "translate_texts_with_preferences") {
        const texts: string[] = payload?.texts ?? [];
        return Promise.resolve(texts);
      }
      return Promise.resolve(null);
    });

    render(
      <SkillLibraryView
        installedSkillIds={new Set<string>()}
        onInstall={async () => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Video Maker")).toBeInTheDocument();
    });

    await waitFor(() => {
      expect(
        screen.getByText("未命中可用翻译服务，当前展示原文。请检查默认模型与网络。"),
      ).toBeInTheDocument();
    });
  });

  test("opens and closes skill detail drawer when clicking a library card", async () => {
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "list_clawhub_library") {
        return Promise.resolve({
          items: [
            {
              slug: "video-maker",
              name: "Video Maker",
              summary: "Generate short videos",
              tags: ["video"],
              stars: 12,
              downloads: 99,
            },
          ],
          next_cursor: null,
        });
      }
      if (command === "get_clawhub_skill_detail" && payload?.slug === "video-maker") {
        return Promise.resolve({
          slug: "video-maker",
          name: "Video Maker",
          summary: "Generate short videos",
          description: "Create short videos for social media",
          author: "Acme Team",
          github_url: "https://github.com/acme/video-maker",
          source_url: "https://clawhub.ai/skills/video-maker",
          updated_at: "2026-03-01T00:00:00.000Z",
          stars: 123,
          downloads: 4567,
          tags: ["video", "creator"],
        });
      }
      return Promise.resolve(null);
    });

    render(
      <SkillLibraryView
        installedSkillIds={new Set<string>()}
        onInstall={async () => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Video Maker")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("Video Maker"));

    await waitFor(() => {
      expect(screen.getByText("技能详情")).toBeInTheDocument();
      expect(screen.getByText("Slug")).toBeInTheDocument();
      expect(screen.getByText("video-maker")).toBeInTheDocument();
      expect(screen.getByText("Acme Team")).toBeInTheDocument();
      expect(screen.getByText("https://github.com/acme/video-maker")).toBeInTheDocument();
    });

    expect(invokeMock).toHaveBeenCalledWith("get_clawhub_skill_detail", {
      slug: "video-maker",
    });

    fireEvent.click(screen.getByRole("button", { name: "关闭" }));

    expect(
      screen.queryByText("技能详情"),
    ).not.toBeInTheDocument();
  });

  test("shows graceful fallback when skill detail endpoint returns 404", async () => {
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "list_clawhub_library") {
        return Promise.resolve({
          items: [
            {
              slug: "video-maker",
              name: "Video Maker",
              summary: "Generate short videos",
              tags: ["video"],
              stars: 12,
              downloads: 99,
            },
          ],
          next_cursor: null,
        });
      }
      if (command === "get_clawhub_skill_detail" && payload?.slug === "video-maker") {
        return Promise.reject(new Error("ClawHub 详情加载失败: HTTP 404 Not Found"));
      }
      return Promise.resolve(null);
    });

    render(
      <SkillLibraryView
        installedSkillIds={new Set<string>()}
        onInstall={async () => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Video Maker")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("Video Maker"));

    await waitFor(() => {
      expect(screen.getByText("技能详情")).toBeInTheDocument();
      expect(screen.getByText("暂无更详细信息，已展示基础信息。")).toBeInTheDocument();
    });

    expect(screen.queryByText(/详情加载失败/)).not.toBeInTheDocument();
  });

  test("auto-translates newly lazy-loaded cards in auto mode", async () => {
    let intersectionCallback!: IntersectionObserverCallback;
    class TriggerableIntersectionObserver {
      constructor(cb: IntersectionObserverCallback) {
        intersectionCallback = cb;
      }
      observe() {}
      unobserve() {}
      disconnect() {}
    }

    Object.defineProperty(window, "IntersectionObserver", {
      writable: true,
      value: TriggerableIntersectionObserver,
    });

    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          default_work_dir: "E:\\workspace",
          default_language: "zh-CN",
          immersive_translation_enabled: true,
          immersive_translation_display: "translated_only",
          immersive_translation_trigger: "auto",
        });
      }
      if (command === "list_clawhub_library") {
        if (!payload?.cursor) {
          return Promise.resolve({
            items: [
              {
                slug: "video-maker",
                name: "Video Maker",
                summary: "Generate short videos",
                tags: ["video"],
                stars: 12,
                downloads: 99,
              },
            ],
            next_cursor: "cursor-2",
          });
        }
        return Promise.resolve({
          items: [
            {
              slug: "prompt-engineer",
              name: "Prompt Engineer",
              summary: "Craft robust prompts",
              tags: ["prompt"],
              stars: 8,
              downloads: 40,
            },
          ],
          next_cursor: null,
        });
      }
      if (command === "translate_texts_with_preferences") {
        const texts: string[] = payload?.texts ?? [];
        return Promise.resolve(texts.map((text) => `ZH:${text}`));
      }
      return Promise.resolve(null);
    });

    render(
      <SkillLibraryView
        installedSkillIds={new Set<string>()}
        onInstall={async () => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("ZH:Video Maker")).toBeInTheDocument();
      expect(screen.getByText("ZH:Generate short videos")).toBeInTheDocument();
    });

    intersectionCallback(
      [{ isIntersecting: true } as unknown as IntersectionObserverEntry],
      {} as IntersectionObserver,
    );

    await waitFor(() => {
      expect(screen.getByText("ZH:Prompt Engineer")).toBeInTheDocument();
      expect(screen.getByText("ZH:Craft robust prompts")).toBeInTheDocument();
    });
  });

  test("manual translate processes long list in progressive batches", async () => {
    const longItems = Array.from({ length: 55 }, (_, index) => ({
      slug: `skill-${index + 1}`,
      name: `Skill ${index + 1}`,
      summary: `Summary ${index + 1}`,
      tags: [`tag-${index + 1}`],
      stars: 1,
      downloads: 1,
    }));

    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          default_language: "zh-CN",
          immersive_translation_enabled: true,
          immersive_translation_display: "translated_only",
          immersive_translation_trigger: "manual",
        });
      }
      if (command === "list_clawhub_library") {
        return Promise.resolve({
          items: longItems,
          next_cursor: null,
        });
      }
      if (command === "translate_texts_with_preferences") {
        const texts: string[] = payload?.texts ?? [];
        return Promise.resolve(texts.map((text) => `ZH:${text}`));
      }
      return Promise.resolve(null);
    });

    render(
      <SkillLibraryView
        installedSkillIds={new Set<string>()}
        onInstall={async () => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Skill 1")).toBeInTheDocument();
      expect(screen.getByText("Skill 55")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "翻译本页" }));

    await waitFor(() => {
      expect(screen.getByText("ZH:Skill 1")).toBeInTheDocument();
      expect(screen.getByText("ZH:Skill 55")).toBeInTheDocument();
    });

    expect(
      invokeMock.mock.calls.filter(([command]) => command === "translate_texts_with_preferences").length,
    ).toBeGreaterThan(1);
  });
});
