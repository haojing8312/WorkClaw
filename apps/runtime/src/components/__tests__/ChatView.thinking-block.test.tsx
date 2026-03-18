import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ChatView } from "../ChatView";
import { resetChatStreamEventSubscriptionsForTest } from "../../lib/chat-stream-events";

const invokeMock = vi.fn<(command: string, payload?: unknown) => Promise<unknown>>();
const listeners = new Map<string, Array<(event: { payload: any }) => void>>();
let messagesResponse: any[] = [];
let scrollIntoViewMock: ReturnType<typeof vi.fn>;

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
    scrollIntoViewMock = vi.fn();
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: scrollIntoViewMock,
    });
    Object.defineProperty(HTMLElement.prototype, "scrollTo", {
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

  function setScrollMetrics(
    element: HTMLElement,
    metrics: { scrollTop: number; clientHeight: number; scrollHeight: number },
  ) {
    Object.defineProperty(element, "scrollTop", {
      configurable: true,
      value: metrics.scrollTop,
      writable: true,
    });
    Object.defineProperty(element, "clientHeight", {
      configurable: true,
      value: metrics.clientHeight,
    });
    Object.defineProperty(element, "scrollHeight", {
      configurable: true,
      value: metrics.scrollHeight,
    });
  }

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

  test("pauses auto-follow after the user scrolls away and shows a jump-to-latest arrow", async () => {
    renderChatView("sess-scroll-lock");

    const scrollRegion = await screen.findByTestId("chat-scroll-region");
    setScrollMetrics(scrollRegion, {
      scrollTop: 120,
      clientHeight: 400,
      scrollHeight: 1000,
    });

    fireEvent.scroll(scrollRegion);
    scrollIntoViewMock.mockClear();

    act(() => {
      emit("stream-token", {
        session_id: "sess-scroll-lock",
        token: "新的流式内容",
        done: false,
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("chat-scroll-jump-button")).toHaveAttribute("aria-label", "跳转到底部");
    });

    expect(scrollIntoViewMock).not.toHaveBeenCalled();
  });

  test("switches the floating arrow between top and bottom jumps", async () => {
    renderChatView("sess-scroll-jump");

    const scrollRegion = await screen.findByTestId("chat-scroll-region");
    const scrollToMock = vi.fn();
    Object.defineProperty(scrollRegion, "scrollTo", {
      configurable: true,
      value: scrollToMock,
    });

    setScrollMetrics(scrollRegion, {
      scrollTop: 600,
      clientHeight: 400,
      scrollHeight: 1000,
    });
    fireEvent.scroll(scrollRegion);

    await waitFor(() => {
      expect(screen.getByTestId("chat-scroll-jump-button")).toHaveAttribute("aria-label", "跳转到顶部");
    });
    expect(screen.getByTestId("chat-scroll-jump-button")).toHaveAttribute("title", "返回顶部");
    expect(screen.getByTestId("chat-scroll-jump-button")).toHaveClass("h-9", "w-9", "bg-[#f4f4f1]/92");

    fireEvent.click(screen.getByTestId("chat-scroll-jump-button"));
    await waitFor(() => {
      expect(scrollToMock).toHaveBeenCalled();
    });
    expect(scrollToMock.mock.calls[0]?.[0]).toMatchObject({ top: expect.any(Number) });
    expect(scrollToMock.mock.calls[0]?.[0]?.top).not.toBe(0);

    setScrollMetrics(scrollRegion, {
      scrollTop: 100,
      clientHeight: 400,
      scrollHeight: 1000,
    });
    fireEvent.scroll(scrollRegion);

    await waitFor(() => {
      expect(screen.getByTestId("chat-scroll-jump-button")).toHaveAttribute("aria-label", "跳转到底部");
    });
    expect(screen.getByTestId("chat-scroll-jump-button")).toHaveAttribute("title", "回到底部并继续跟随");

    fireEvent.click(screen.getByTestId("chat-scroll-jump-button"));
    await waitFor(() => {
      expect(scrollToMock.mock.calls.length).toBeGreaterThan(1);
    });
  });

  test("sending a new message restores bottom-follow even when browsing history", async () => {
    renderChatView("sess-send-scroll");

    const scrollRegion = await screen.findByTestId("chat-scroll-region");
    const scrollToMock = vi.fn();
    Object.defineProperty(scrollRegion, "scrollTo", {
      configurable: true,
      value: scrollToMock,
    });

    setScrollMetrics(scrollRegion, {
      scrollTop: 120,
      clientHeight: 400,
      scrollHeight: 1000,
    });
    fireEvent.scroll(scrollRegion);

    fireEvent.change(screen.getByPlaceholderText("输入消息，Shift+Enter 换行..."), {
      target: { value: "继续处理这个任务" },
    });
    fireEvent.click(screen.getByRole("button", { name: "发送" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", expect.anything());
      expect(scrollToMock).toHaveBeenCalled();
    });
  });

  test("keeps bottom-follow active when streaming resumes after clicking the down arrow", async () => {
    renderChatView("sess-resume-follow");

    const scrollRegion = await screen.findByTestId("chat-scroll-region");
    const scrollToMock = vi.fn();
    Object.defineProperty(scrollRegion, "scrollTo", {
      configurable: true,
      value: scrollToMock,
    });

    setScrollMetrics(scrollRegion, {
      scrollTop: 120,
      clientHeight: 400,
      scrollHeight: 1000,
    });
    fireEvent.scroll(scrollRegion);
    scrollIntoViewMock.mockClear();

    fireEvent.click(screen.getByTestId("chat-scroll-jump-button"));

    act(() => {
      emit("stream-token", {
        session_id: "sess-resume-follow",
        token: "继续往下输出",
        done: false,
      });
    });

    await waitFor(() => {
      expect(scrollToMock).toHaveBeenCalled();
      expect(scrollIntoViewMock).toHaveBeenCalled();
    });
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

    const assistantBubbles = screen.getAllByTestId("chat-streaming-bubble");
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

  test("keeps historical assistant bubbles on a stable width rail while reasoning is collapsed", async () => {
    messagesResponse = [
      {
        id: "assistant-1",
        role: "assistant",
        content: "这里是最终结论。",
        created_at: "2026-03-13T12:00:10Z",
        reasoning: {
          status: "completed",
          duration_ms: 2400,
          content:
            "用户希望我将刚才生成的项目档案报告转换为 PDF 格式。我需要使用内置的 PDF 文档助手技能来处理这个任务。用户想要PDF文件，但我注意到builtin-pdf skill声明了工具但没有可用工具。",
        },
      },
    ];

    renderChatView("sess-history-width");

    await waitFor(() => {
      expect(screen.getByText("已思考 2.4s")).toBeInTheDocument();
    });

    const assistantBubble = screen.getByTestId("chat-message-bubble-assistant-1");

    expect(assistantBubble).toBeTruthy();
    expect(assistantBubble?.className).toContain("w-full");
    expect(assistantBubble?.className).toContain("md:max-w-[48rem]");
    expect(assistantBubble?.className).not.toContain("bg-white");
    expect(assistantBubble?.className).not.toContain("border");
  });

  test("keeps the transcript inside a centered content rail with horizontal breathing room", async () => {
    messagesResponse = [
      {
        id: "assistant-rail",
        role: "assistant",
        content: "这里是内容轨道测试。",
        created_at: "2026-03-13T12:00:12Z",
      },
    ];

    renderChatView("sess-content-rail");

    await waitFor(() => {
      expect(screen.getByText("这里是内容轨道测试。")).toBeInTheDocument();
    });

    const scrollRegion = screen.getByTestId("chat-scroll-region");
    const contentRail = screen.getByTestId("chat-content-rail");

    expect(scrollRegion.className).toContain("px-4");
    expect(scrollRegion.className).toContain("sm:px-6");
    expect(contentRail.className).toContain("max-w-[76rem]");
    expect(contentRail.className).toContain("mx-auto");
  });

  test("renders user messages as lightweight prompt chips instead of strong primary chat bubbles", async () => {
    messagesResponse = [
      {
        id: "user-1",
        role: "user",
        content: "当前文件夹有哪些文件",
        created_at: "2026-03-13T12:00:00Z",
      },
    ];

    renderChatView("sess-user-chip");

    await waitFor(() => {
      expect(screen.getByText("当前文件夹有哪些文件")).toBeInTheDocument();
    });

    const userBubble = screen.getByTestId("chat-message-bubble-user-1");

    expect(userBubble.className).toContain("bg-slate-100");
    expect(userBubble.className).not.toContain("bg-blue-500");
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
