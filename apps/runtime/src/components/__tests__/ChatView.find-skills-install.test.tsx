import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ChatView } from "../ChatView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

function buildToolOutput() {
  return JSON.stringify({
    source: "clawhub",
    items: [
      {
        slug: "video-maker",
        name: "Video Maker",
        description: "Generate short videos automatically",
        stars: 12,
        github_url: "https://github.com/example/video-maker",
      },
    ],
  });
}

describe("ChatView find-skills install flow", () => {
  beforeEach(() => {
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
    invokeMock.mockReset();
  });

  test("renders clawhub install candidates and installs after confirm", async () => {
    const onSkillInstalled = vi.fn();
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_messages") {
        return Promise.resolve([
          {
            role: "assistant",
            content: "找到候选技能",
            created_at: new Date().toISOString(),
            streamItems: [
              {
                type: "tool_call",
                toolCall: {
                  id: "tc-1",
                  name: "clawhub_recommend",
                  input: { query: "short video" },
                  output: buildToolOutput(),
                  status: "completed",
                },
              },
            ],
          },
        ]);
      }
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "translate_texts_with_preferences") {
        const texts: string[] = payload?.texts ?? [];
        return Promise.resolve(
          texts.map((text) => {
            if (text === "Video Maker") return "视频制作器";
            if (text === "Generate short videos automatically") return "自动生成短视频";
            return text;
          }),
        );
      }
      if (command === "install_clawhub_skill") {
        return Promise.resolve({ manifest: { id: "clawhub-video-maker" } });
      }
      return Promise.resolve(null);
    });

    render(
      <ChatView
        skill={{
          id: "builtin-find-skills",
          name: "找技能",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "model-a",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "model-a",
            name: "Model A",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model-a",
            is_default: true,
          },
        ]}
        sessionId="session-1"
        installedSkillIds={[]}
        onSkillInstalled={onSkillInstalled}
      />
    );

    await waitFor(() => {
      expect(screen.getByText("可安装技能")).toBeInTheDocument();
      expect(screen.getByText("视频制作器")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "立即安装" }));

    await waitFor(() => {
      expect(screen.getByText("安装技能")).toBeInTheDocument();
      expect(screen.getByText(/是否安装「视频制作器」/)).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "确认安装" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("install_clawhub_skill", {
        slug: "video-maker",
        githubUrl: "https://github.com/example/video-maker",
      });
      expect(onSkillInstalled).toHaveBeenCalledWith("clawhub-video-maker");
    });
  });

  test("shows installed state when skill already exists", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") {
        return Promise.resolve([
          {
            role: "assistant",
            content: "找到候选技能",
            created_at: new Date().toISOString(),
            streamItems: [
              {
                type: "tool_call",
                toolCall: {
                  id: "tc-1",
                  name: "clawhub_search",
                  input: { query: "video" },
                  output: buildToolOutput(),
                  status: "completed",
                },
              },
            ],
          },
        ]);
      }
      if (command === "get_sessions") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    render(
      <ChatView
        skill={{
          id: "builtin-find-skills",
          name: "找技能",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "model-a",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "model-a",
            name: "Model A",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model-a",
            is_default: true,
          },
        ]}
        sessionId="session-1"
        installedSkillIds={["clawhub-video-maker"]}
      />
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "已安装" })).toBeDisabled();
    });
  });

  test("shows duplicate-name error when install conflicts by display name", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") {
        return Promise.resolve([
          {
            role: "assistant",
            content: "找到候选技能",
            created_at: new Date().toISOString(),
            streamItems: [
              {
                type: "tool_call",
                toolCall: {
                  id: "tc-1",
                  name: "clawhub_recommend",
                  input: { query: "short video" },
                  output: buildToolOutput(),
                  status: "completed",
                },
              },
            ],
          },
        ]);
      }
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "install_clawhub_skill") {
        return Promise.reject("DUPLICATE_SKILL_NAME:Video Maker");
      }
      return Promise.resolve(null);
    });

    render(
      <ChatView
        skill={{
          id: "builtin-find-skills",
          name: "找技能",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "model-a",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "model-a",
            name: "Model A",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model-a",
            is_default: true,
          },
        ]}
        sessionId="session-1"
        installedSkillIds={[]}
      />,
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "立即安装" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "立即安装" }));
    fireEvent.click(screen.getByRole("button", { name: "确认安装" }));

    await waitFor(() => {
      expect(screen.getByText("技能名称冲突：已存在「Video Maker」，请先重命名后再安装。")).toBeInTheDocument();
    });
  });

  test("renders bilingual inline candidate text when preference is bilingual_inline", async () => {
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          default_work_dir: "E:\\workspace",
          default_language: "zh-CN",
          immersive_translation_enabled: true,
          immersive_translation_display: "bilingual_inline",
        });
      }
      if (command === "get_messages") {
        return Promise.resolve([
          {
            role: "assistant",
            content: "找到候选技能",
            created_at: new Date().toISOString(),
            streamItems: [
              {
                type: "tool_call",
                toolCall: {
                  id: "tc-1",
                  name: "clawhub_recommend",
                  input: { query: "short video" },
                  output: buildToolOutput(),
                  status: "completed",
                },
              },
            ],
          },
        ]);
      }
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "translate_texts_with_preferences") {
        const texts: string[] = payload?.texts ?? [];
        return Promise.resolve(
          texts.map((text) => {
            if (text === "Video Maker") return "视频制作器";
            if (text === "Generate short videos automatically") return "自动生成短视频";
            return text;
          }),
        );
      }
      return Promise.resolve(null);
    });

    render(
      <ChatView
        skill={{
          id: "builtin-find-skills",
          name: "找技能",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "model-a",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "model-a",
            name: "Model A",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model-a",
            is_default: true,
          },
        ]}
        sessionId="session-1"
        installedSkillIds={[]}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("视频制作器 (Video Maker)")).toBeInTheDocument();
      expect(
        screen.getByText("自动生成短视频 (Generate short videos automatically)"),
      ).toBeInTheDocument();
    });
  });

  test("does not render install card for github fallback repo links", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") {
        return Promise.resolve([
          {
            role: "assistant",
            content:
              "未找到可直接安装技能。GitHub 备选仓库： https://github.com/obra/superpowers",
            created_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "get_sessions") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    render(
      <ChatView
        skill={{
          id: "builtin-find-skills",
          name: "找技能",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "model-a",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "model-a",
            name: "Model A",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model-a",
            is_default: true,
          },
        ]}
        sessionId="session-1"
        installedSkillIds={[]}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("https://github.com/obra/superpowers")).toBeInTheDocument();
    });

    expect(screen.queryByText("GitHub 仓库")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "立即安装" })).not.toBeInTheDocument();
  });
});
