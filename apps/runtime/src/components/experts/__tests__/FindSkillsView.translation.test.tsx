import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { FindSkillsView } from "../FindSkillsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("FindSkillsView immersive translation", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  test("renders translated recommendation fields through translate_texts_with_preferences", async () => {
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "recommend_clawhub_skills") {
        return Promise.resolve([
          {
            slug: "video-maker",
            name: "Video Maker",
            description: "Generate short videos",
            stars: 12,
            score: 88,
            reason: "Great match for short video automation",
          },
        ]);
      }
      if (command === "translate_texts_with_preferences") {
        const texts: string[] = payload?.texts ?? [];
        return Promise.resolve(texts.map((text) => `ZH:${text}`));
      }
      return Promise.resolve(null);
    });

    render(
      <FindSkillsView
        installedSkillIds={new Set<string>()}
        onInstall={async () => {}}
      />,
    );

    fireEvent.change(screen.getByPlaceholderText("例如：我想做短视频脚本，最好中文场景可用"), {
      target: { value: "short video" },
    });
    fireEvent.click(screen.getByRole("button", { name: "找技能" }));

    await waitFor(() => {
      expect(screen.getByText("ZH:Video Maker")).toBeInTheDocument();
      expect(screen.getByText("ZH:Generate short videos")).toBeInTheDocument();
      expect(screen.getByText("ZH:Great match for short video automation")).toBeInTheDocument();
    });

    expect(invokeMock).toHaveBeenCalledWith(
      "translate_texts_with_preferences",
      expect.objectContaining({
        texts: expect.any(Array),
      }),
    );
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
      if (command === "recommend_clawhub_skills") {
        return Promise.resolve([
          {
            slug: "video-maker",
            name: "Video Maker",
            description: "Generate short videos",
            stars: 12,
            score: 88,
            reason: "Great match for short video automation",
          },
        ]);
      }
      if (command === "translate_texts_with_preferences") {
        const texts: string[] = payload?.texts ?? [];
        return Promise.resolve(texts.map((text) => `ZH:${text}`));
      }
      return Promise.resolve(null);
    });

    render(
      <FindSkillsView
        installedSkillIds={new Set<string>()}
        onInstall={async () => {}}
      />,
    );

    fireEvent.change(screen.getByPlaceholderText("例如：我想做短视频脚本，最好中文场景可用"), {
      target: { value: "short video" },
    });
    fireEvent.click(screen.getByRole("button", { name: "找技能" }));

    await waitFor(() => {
      expect(screen.getByText("ZH:Video Maker (Video Maker)")).toBeInTheDocument();
      expect(screen.getByText("ZH:Generate short videos (Generate short videos)")).toBeInTheDocument();
      expect(
        screen.getByText("ZH:Great match for short video automation (Great match for short video automation)"),
      ).toBeInTheDocument();
    });
  });
});
