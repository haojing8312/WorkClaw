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

function renderChat(onSkillInstalled?: (skillId: string) => Promise<void> | void) {
  return render(
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
}

describe("ChatView risk flow", () => {
  beforeEach(() => {
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
    invokeMock.mockReset();
  });

  test("canceling install confirmation does not call install_clawhub_skill", async () => {
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
      return Promise.resolve(null);
    });

    renderChat();

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "立即安装" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "立即安装" }));
    await waitFor(() => expect(screen.getByRole("dialog")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "取消" }));

    const installCalls = invokeMock.mock.calls.filter(([command]) => command === "install_clawhub_skill");
    expect(installCalls).toHaveLength(0);
  });

  test("double confirm click only triggers one install invoke", async () => {
    const onSkillInstalled = vi.fn();
    let resolveInstall: (value: { manifest: { id: string } }) => void = () => {};

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
      if (command === "install_clawhub_skill") {
        return new Promise((resolve) => {
          resolveInstall = resolve as (value: { manifest: { id: string } }) => void;
        });
      }
      return Promise.resolve(null);
    });

    renderChat(onSkillInstalled);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "立即安装" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "立即安装" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "确认安装" })).toBeInTheDocument());

    const confirmButton = screen.getByRole("button", { name: "确认安装" });
    fireEvent.click(confirmButton);
    fireEvent.click(confirmButton);

    const installCalls = invokeMock.mock.calls.filter(([command]) => command === "install_clawhub_skill");
    expect(installCalls).toHaveLength(1);

    resolveInstall({ manifest: { id: "clawhub-video-maker" } });

    await waitFor(() => {
      expect(onSkillInstalled).toHaveBeenCalledWith("clawhub-video-maker");
    });
  });
});
