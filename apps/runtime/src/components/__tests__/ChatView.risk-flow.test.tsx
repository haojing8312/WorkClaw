import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ChatView } from "../ChatView";

const invokeMock = vi.fn();
const listenHandlers = new Map<string, (payload: { payload: any }) => void>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, handler: (payload: { payload: any }) => void) => {
    listenHandlers.set(event, handler);
    return Promise.resolve(() => {
      listenHandlers.delete(event);
    });
  },
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

function renderChat(
  onSkillInstalled?: (skillId: string) => Promise<void> | void,
  operationPermissionMode: "standard" | "full_access" = "standard"
) {
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
      operationPermissionMode={operationPermissionMode}
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
    listenHandlers.clear();
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

  test("renders human-readable critical action confirmation dialog", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_pending_approvals") return Promise.resolve([]);
      if (command === "resolve_approval") return Promise.resolve(null);
      return Promise.resolve(null);
    });

    renderChat();

    await waitFor(() => {
      expect(listenHandlers.has("approval-created")).toBe(true);
    });

    await act(async () => {
      listenHandlers.get("approval-created")?.({
        payload: {
          approval_id: "approval-1",
          session_id: "session-1",
          tool_name: "file_delete",
          tool_input: { path: "E:\\workspace\\danger.txt" },
          title: "删除文件",
          summary: "将删除 E:\\workspace\\danger.txt",
          impact: "该操作不可逆，删除后无法自动恢复。",
          irreversible: true,
        },
      });
    });

    await waitFor(() => {
      expect(screen.getByRole("dialog")).toBeInTheDocument();
    });
    expect(screen.getByText("删除文件")).toBeInTheDocument();
    expect(screen.getByText("将删除 E:\\workspace\\danger.txt")).toBeInTheDocument();
    expect(screen.getByText("该操作不可逆，删除后无法自动恢复。")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "允许一次" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("resolve_approval", {
        approvalId: "approval-1",
        decision: "allow_once",
        source: "desktop",
      });
    });
  });

  test("shows queued approvals sequentially and removes remote-resolved entries", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_pending_approvals") return Promise.resolve([]);
      if (command === "resolve_approval") return Promise.resolve(null);
      return Promise.resolve(null);
    });

    renderChat();

    await waitFor(() => {
      expect(listenHandlers.has("approval-created")).toBe(true);
      expect(listenHandlers.has("approval-resolved")).toBe(true);
    });

    await act(async () => {
      listenHandlers.get("approval-created")?.({
        payload: {
          approval_id: "approval-a",
          session_id: "session-1",
          tool_name: "file_delete",
          tool_input: { path: "E:\\workspace\\a.txt" },
          title: "删除文件 A",
          summary: "将删除 E:\\workspace\\a.txt",
          impact: "A 不可恢复",
          irreversible: true,
        },
      });
      listenHandlers.get("approval-created")?.({
        payload: {
          approval_id: "approval-b",
          session_id: "session-1",
          tool_name: "file_delete",
          tool_input: { path: "E:\\workspace\\b.txt" },
          title: "删除文件 B",
          summary: "将删除 E:\\workspace\\b.txt",
          impact: "B 不可恢复",
          irreversible: true,
        },
      });
    });

    await waitFor(() => {
      expect(screen.getByText("删除文件 A")).toBeInTheDocument();
    });
    expect(screen.getByText("还有 1 条待审批")).toBeInTheDocument();

    await act(async () => {
      listenHandlers.get("approval-resolved")?.({
        payload: {
          approval_id: "approval-a",
          session_id: "session-1",
          status: "approved",
        },
      });
    });

    await waitFor(() => {
      expect(screen.getByText("删除文件 B")).toBeInTheDocument();
    });
  });

  test("shows full access badge when chat runs in full access mode", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    renderChat(undefined, "full_access");

    await waitFor(() => {
      expect(screen.getByTestId("full-access-badge")).toBeInTheDocument();
    });
    expect(screen.getByText("全自动模式")).toBeInTheDocument();
  });
});
