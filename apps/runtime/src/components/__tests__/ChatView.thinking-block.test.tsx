import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ChatView } from "../ChatView";
import { resetChatStreamEventSubscriptionsForTest } from "../../lib/chat-stream-events";

const invokeMock = vi.fn<(command: string, payload?: unknown) => Promise<unknown>>();
const listeners = new Map<string, Array<(event: { payload: any }) => void>>();
let messagesResponse: any[] = [];

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, payload?: unknown) => invokeMock(command, payload),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (name: string, cb: (event: { payload: any }) => void) => {
    const arr = listeners.get(name) ?? [];
    arr.push(cb);
    listeners.set(name, arr);
    return Promise.resolve(() => {
      const current = listeners.get(name) ?? [];
      listeners.set(
        name,
        current.filter((item) => item !== cb)
      );
    });
  },
}));

function emit(name: string, payload: any) {
  const arr = listeners.get(name) ?? [];
  arr.forEach((fn) => fn({ payload }));
}

function renderChatView(sessionId = "sess-thinking") {
  return render(
    <ChatView
      skill={{
        id: "builtin-general",
        name: "General",
        description: "desc",
        version: "1.0.0",
        author: "test",
        recommended_model: "",
        tags: [],
        created_at: new Date().toISOString(),
      }}
      models={[
        {
          id: "m1",
          name: "model",
          api_format: "openai",
          base_url: "https://example.com",
          model_name: "model",
          is_default: true,
        },
      ]}
      sessionId={sessionId}
    />
  );
}

function renderChatViewInStrictMode(sessionId = "sess-thinking") {
  return render(
    <StrictMode>
      <ChatView
        skill={{
          id: "builtin-general",
          name: "General",
          description: "desc",
          version: "1.0.0",
          author: "test",
          recommended_model: "",
          tags: [],
          created_at: new Date().toISOString(),
        }}
        models={[
          {
            id: "m1",
            name: "model",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model",
            is_default: true,
          },
        ]}
        sessionId={sessionId}
      />
    </StrictMode>
  );
}

function renderChatViewWithModels(models: Array<{
  id: string;
  name: string;
  api_format: string;
  base_url: string;
  model_name: string;
  is_default: boolean;
}>, sessionId = "sess-thinking") {
  return render(
    <ChatView
      skill={{
        id: "builtin-general",
        name: "General",
        description: "desc",
        version: "1.0.0",
        author: "test",
        recommended_model: "",
        tags: [],
        created_at: new Date().toISOString(),
      }}
      models={models}
      sessionId={sessionId}
    />
  );
}

describe("ChatView thinking block", () => {
  beforeEach(() => {
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
    resetChatStreamEventSubscriptionsForTest();
    listeners.clear();
    messagesResponse = [];
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve(messagesResponse);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_session_runs") return Promise.resolve([]);
      return Promise.resolve(null);
    });
  });

  afterEach(() => {
    cleanup();
  });

  test("shows thinking state immediately but hides expand affordance before reasoning arrives", async () => {
    renderChatView();

    act(() => {
      emit("agent-state-event", {
        session_id: "sess-thinking",
        state: "thinking",
        detail: null,
        iteration: 1,
      });
    });

    await waitFor(() => {
      expect(screen.getByText("思考中")).toBeInTheDocument();
    });

    expect(screen.queryByTestId("thinking-block-toggle")).not.toBeInTheDocument();
  });

  test("renders collapsible reasoning content separately from the answer stream", async () => {
    renderChatView("sess-stream");

    act(() => {
      emit("agent-state-event", {
        session_id: "sess-stream",
        state: "thinking",
        detail: null,
        iteration: 1,
      });
      emit("assistant-reasoning-started", {
        session_id: "sess-stream",
        started_at: "2026-03-13T12:00:00Z",
      });
      emit("assistant-reasoning-delta", {
        session_id: "sess-stream",
        text: "先分析需求，再组织输出。",
        created_at: "2026-03-13T12:00:01Z",
      });
      emit("stream-token", {
        session_id: "sess-stream",
        token: "这是正式答案。",
        done: false,
      });
    });

    await waitFor(() => {
      expect(screen.getByText("思考中")).toBeInTheDocument();
      expect(screen.getByText("这是正式答案。")).toBeInTheDocument();
      expect(screen.getByTestId("thinking-block-toggle")).toBeInTheDocument();
    });

    expect(screen.queryByText("先分析需求，再组织输出。")).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("thinking-block-toggle"));

    expect(screen.getByText("先分析需求，再组织输出。")).toBeInTheDocument();
  });

  test("does not duplicate early stream content in StrictMode", async () => {
    renderChatViewInStrictMode("sess-strict");

    act(() => {
      emit("agent-state-event", {
        session_id: "sess-strict",
        state: "thinking",
        detail: null,
        iteration: 1,
      });
      emit("assistant-reasoning-started", {
        session_id: "sess-strict",
      });
      emit("assistant-reasoning-delta", {
        session_id: "sess-strict",
        text: "让我先查一下资料。",
      });
      emit("stream-token", {
        session_id: "sess-strict",
        token: "我来帮你整理结果。",
        done: false,
      });
    });

    await waitFor(() => {
      expect(screen.getByText("我来帮你整理结果。")).toBeInTheDocument();
      expect(screen.getByTestId("thinking-block-toggle")).toBeInTheDocument();
    });

    expect(screen.getAllByText("我来帮你整理结果。")).toHaveLength(1);

    fireEvent.click(screen.getByTestId("thinking-block-toggle"));
    expect(screen.getAllByText("让我先查一下资料。")).toHaveLength(1);
  });

  test("keeps concurrent session streams isolated", async () => {
    render(
      <>
        <ChatView
          skill={{
            id: "builtin-general",
            name: "General",
            description: "desc",
            version: "1.0.0",
            author: "test",
            recommended_model: "",
            tags: [],
            created_at: new Date().toISOString(),
          }}
          models={[
            {
              id: "m1",
              name: "model",
              api_format: "openai",
              base_url: "https://example.com",
              model_name: "model",
              is_default: true,
            },
          ]}
          sessionId="sess-a"
        />
        <ChatView
          skill={{
            id: "builtin-general",
            name: "General",
            description: "desc",
            version: "1.0.0",
            author: "test",
            recommended_model: "",
            tags: [],
            created_at: new Date().toISOString(),
          }}
          models={[
            {
              id: "m1",
              name: "model",
              api_format: "openai",
              base_url: "https://example.com",
              model_name: "model",
              is_default: true,
            },
          ]}
          sessionId="sess-b"
        />
      </>
    );

    act(() => {
      emit("stream-token", {
        session_id: "sess-a",
        token: "A 会话输出",
        done: false,
      });
      emit("stream-token", {
        session_id: "sess-b",
        token: "B 会话输出",
        done: false,
      });
    });

    await waitFor(() => {
      expect(screen.getByText("A 会话输出")).toBeInTheDocument();
      expect(screen.getByText("B 会话输出")).toBeInTheDocument();
    });

    const assistantBubbles = document.querySelectorAll(".max-w-\\[80\\%\\]");
    expect(assistantBubbles).toHaveLength(2);
    expect(within(assistantBubbles[0] as HTMLElement).getByText("A 会话输出")).toBeInTheDocument();
    expect(within(assistantBubbles[1] as HTMLElement).getByText("B 会话输出")).toBeInTheDocument();
    expect(within(assistantBubbles[0] as HTMLElement).queryByText("B 会话输出")).not.toBeInTheDocument();
    expect(within(assistantBubbles[1] as HTMLElement).queryByText("A 会话输出")).not.toBeInTheDocument();
  });

  test("does not render thinking block for non-chat protocols", async () => {
    renderChatViewWithModels(
      [
        {
          id: "m1",
          name: "search-provider",
          api_format: "search_tavily",
          base_url: "https://search.example.com",
          model_name: "search-model",
          is_default: true,
        },
      ],
      "sess-no-indicator"
    );

    act(() => {
      emit("agent-state-event", {
        session_id: "sess-no-indicator",
        state: "thinking",
        detail: null,
        iteration: 1,
      });
    });

    await waitFor(() => {
      expect(screen.queryByText("思考中")).not.toBeInTheDocument();
    });
  });

  test("shows completed duration for persisted historical reasoning", async () => {
    messagesResponse = [
      {
        id: "assistant-1",
        role: "assistant",
        content: "这里是最终结论。",
        created_at: "2026-03-13T12:00:10Z",
        reasoning: {
          status: "completed",
          duration_ms: 2400,
          content: "先拆解问题，再汇总答案。",
        },
      },
    ];

    renderChatView("sess-history");

    await waitFor(() => {
      expect(screen.getByText("已思考 2.4s")).toBeInTheDocument();
      expect(screen.getByText("这里是最终结论。")).toBeInTheDocument();
    });

    expect(screen.queryByText("先拆解问题，再汇总答案。")).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("thinking-block-toggle-assistant-1"));

    expect(screen.getByText("先拆解问题，再汇总答案。")).toBeInTheDocument();
  });

  test("renders a new thinking state after existing history so it stays near the bottom viewport", async () => {
    messagesResponse = [
      {
        id: "user-1",
        role: "user",
        content: "第一轮问题",
        created_at: "2026-03-15T02:00:00Z",
      },
      {
        id: "assistant-1",
        role: "assistant",
        content: "第一轮回答",
        created_at: "2026-03-15T02:00:01Z",
      },
      {
        id: "user-2",
        role: "user",
        content: "第二轮问题",
        created_at: "2026-03-15T02:00:02Z",
      },
      {
        id: "assistant-2",
        role: "assistant",
        content: "第二轮回答",
        created_at: "2026-03-15T02:00:03Z",
      },
    ];

    renderChatView("sess-thinking-history");

    await waitFor(() => {
      expect(screen.getByText("第二轮回答")).toBeInTheDocument();
    });

    act(() => {
      emit("agent-state-event", {
        session_id: "sess-thinking-history",
        state: "thinking",
        detail: null,
        iteration: 2,
      });
    });

    await waitFor(() => {
      expect(screen.getByText("思考中")).toBeInTheDocument();
    });

    const latestHistoryMessage = screen.getByText("第二轮回答");
    const thinkingLabel = screen.getByText("思考中");

    expect(latestHistoryMessage.compareDocumentPosition(thinkingLabel) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
});
