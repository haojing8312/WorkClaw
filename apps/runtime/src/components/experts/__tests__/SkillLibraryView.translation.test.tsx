import { render, screen, waitFor } from "@testing-library/react";
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

  test("translates library texts through translate_texts_with_preferences", async () => {
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

    expect(invokeMock).toHaveBeenCalledWith(
      "translate_texts_with_preferences",
      expect.objectContaining({
        texts: expect.any(Array),
      }),
    );
  });

  test("falls back to source text when translation command fails", async () => {
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
      expect(screen.getByText("ZH:Video Maker (Video Maker)")).toBeInTheDocument();
      expect(screen.getByText("ZH:Generate short videos (Generate short videos)")).toBeInTheDocument();
    });
  });
});
