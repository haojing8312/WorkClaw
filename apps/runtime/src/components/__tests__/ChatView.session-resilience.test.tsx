import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { ChatView } from "../ChatView";
import { resetChatStreamEventSubscriptionsForTest } from "../../lib/chat-stream-events";

const invokeMock = vi.fn<(command: string, payload?: unknown) => Promise<unknown>>();
const listeners = new Map<string, Array<(event: { payload: any }) => void>>();
let sessionRunsResponse: any[] = [];
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

describe("ChatView session resilience", () => {
  beforeEach(() => {
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
    Object.defineProperty(HTMLElement.prototype, "scrollTo", {
      configurable: true,
      value: vi.fn(),
    });
    resetChatStreamEventSubscriptionsForTest();
    listeners.clear();
    invokeMock.mockReset();
    window.localStorage.clear();
    sessionRunsResponse = [];
    messagesResponse = [];
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve(messagesResponse);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_session_runs") return Promise.resolve(sessionRunsResponse);
      return Promise.resolve(null);
    });
  });

  afterEach(() => {
    cleanup();
    window.localStorage.clear();
    vi.useRealTimers();
  });

  test("clears thinking block and shows failure card when latest run ends with billing failure", async () => {
    messagesResponse = [
      {
        id: "user-1",
        role: "user",
        content: "继续执行",
        created_at: new Date().toISOString(),
      },
    ];

    render(
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
        sessionId="sess-1"
      />
    );

    act(() => {
      emit("agent-state-event", {
        session_id: "sess-1",
        state: "thinking",
        detail: null,
        iteration: 1,
      });
      emit("stream-token", {
        session_id: "sess-1",
        token: "已经生成 2 个文件",
        done: false,
      });
    });

    await waitFor(() => {
      expect(screen.getByText("思考中")).toBeInTheDocument();
    });

    sessionRunsResponse = [
      {
        id: "run-1",
        session_id: "sess-1",
        user_message_id: "user-1",
        assistant_message_id: null,
        status: "failed",
        buffered_text: "已经生成 2 个文件",
        error_kind: "billing",
        error_message: "模型余额不足",
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
    ];

    act(() => {
      emit("agent-state-event", {
        session_id: "sess-1",
        state: "error",
        detail: "模型余额不足",
        iteration: 1,
      });
      emit("stream-token", {
        session_id: "sess-1",
        token: "",
        done: true,
      });
    });

    await waitFor(() => {
      expect(screen.queryByText("思考中")).not.toBeInTheDocument();
      expect(screen.getByTestId("run-failure-card-run-1")).toHaveTextContent("模型余额不足");
      expect(screen.getByTestId("run-failure-card-run-1")).toHaveTextContent(
        "当前模型平台返回余额或额度不足，请到对应服务商控制台充值或检查套餐额度。",
      );
      expect(screen.getByTestId("run-failure-card-run-1")).toHaveTextContent("已经生成 2 个文件");
    });
  });

  test("renders failed run card after the assistant message that belongs to the same run", async () => {
    messagesResponse = [
      {
        id: "user-1",
        role: "user",
        content: "先完成第一轮",
        created_at: "2026-03-11T00:00:01Z",
      },
      {
        id: "assistant-1",
        role: "assistant",
        content: "第一轮已完成",
        created_at: "2026-03-11T00:00:02Z",
      },
      {
        id: "user-2",
        role: "user",
        content: "继续第二轮",
        created_at: "2026-03-11T00:00:03Z",
      },
      {
        id: "assistant-2",
        role: "assistant",
        content: "第二轮已生成 2 个文件",
        created_at: "2026-03-11T00:00:04Z",
      },
    ];
    sessionRunsResponse = [
      {
        id: "run-2",
        session_id: "sess-2",
        user_message_id: "user-2",
        assistant_message_id: "assistant-2",
        status: "failed",
        buffered_text: "第二轮已生成 2 个文件",
        error_kind: "billing",
        error_message: "模型余额不足",
        created_at: "2026-03-11T00:00:03Z",
        updated_at: "2026-03-11T00:00:05Z",
      },
    ];

    render(
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
        sessionId="sess-2"
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("chat-message-3")).toBeInTheDocument();
      expect(screen.getByTestId("run-failure-card-run-2")).toBeInTheDocument();
    });

    const assistantMessage = screen.getByTestId("chat-message-3");
    const failureCard = screen.getByTestId("run-failure-card-run-2");

    expect(assistantMessage.compareDocumentPosition(failureCard) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  test("restores buffered assistant output for a waiting approval run when reopening a session", async () => {
    messagesResponse = [
      {
        id: "user-1",
        role: "user",
        content: "继续执行旧任务",
        created_at: "2026-03-16T00:00:01Z",
      },
    ];
    sessionRunsResponse = [
      {
        id: "run-active-1",
        session_id: "sess-recover",
        user_message_id: "user-1",
        assistant_message_id: null,
        status: "waiting_approval",
        buffered_text: "这是切回会话后应恢复的中间输出",
        error_kind: null,
        error_message: null,
        created_at: "2026-03-16T00:00:02Z",
        updated_at: "2026-03-16T00:00:03Z",
      },
    ];

    render(
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
        sessionId="sess-recover"
      />
    );

    await waitFor(() => {
      expect(screen.getByText("这是切回会话后应恢复的中间输出")).toBeInTheDocument();
    });
  });

  test("restores buffered assistant output for a waiting approval run when reopening a session", async () => {
    messagesResponse = [
      {
        id: "user-approval-1",
        role: "user",
        content: "继续执行需要确认的任务",
        created_at: "2026-03-16T00:00:01Z",
      },
    ];
    sessionRunsResponse = [
      {
        id: "run-approval-1",
        session_id: "sess-waiting-approval",
        user_message_id: "user-approval-1",
        assistant_message_id: null,
        status: "waiting_approval",
        buffered_text: "请先确认这个工具调用，我会在确认后继续执行。",
        error_kind: null,
        error_message: null,
        created_at: "2026-03-16T00:00:02Z",
        updated_at: "2026-03-16T00:00:03Z",
      },
    ];

    render(
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
        sessionId="sess-waiting-approval"
      />
    );

    await waitFor(() => {
      expect(
        screen.getByText("请先确认这个工具调用，我会在确认后继续执行。"),
      ).toBeInTheDocument();
    });
  });

  test("hydrates persisted runtime state immediately when reopening an active session", async () => {
    render(
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
        sessionId="sess-persisted-runtime"
        persistedRuntimeState={{
          streaming: true,
          streamItems: [{ type: "text", content: "这是重新打开后应立即显示的流式输出" }],
          streamReasoning: {
            status: "thinking",
            content: "先恢复本地运行态",
          },
          agentState: {
            state: "thinking",
            iteration: 1,
          },
          subAgentBuffer: "",
          subAgentRoleName: "",
          mainRoleName: "",
          mainSummaryDelivered: false,
          delegationCards: [],
        }}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("chat-streaming-bubble")).toBeInTheDocument();
      expect(screen.getByText("这是重新打开后应立即显示的流式输出")).toBeInTheDocument();
      expect(screen.getByText("思考中")).toBeInTheDocument();
    });
  });

  test("still sends the initial message after the parent clears the pending value", async () => {
    vi.useFakeTimers();

    function InitialMessageHarness() {
      const [initialMessage, setInitialMessage] = useState("请先开始执行");

      return (
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
          sessionId="sess-initial"
          initialMessage={initialMessage}
          onInitialMessageConsumed={() => setInitialMessage("")}
        />
      );
    }

    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_session_runs") return Promise.resolve([]);
      if (command === "send_message") return Promise.resolve(null);
      return Promise.resolve(null);
    });

    render(<InitialMessageHarness />);

    await act(async () => {
      vi.advanceTimersByTime(1);
      await Promise.resolve();
    });

    expect(invokeMock).toHaveBeenCalledWith("send_message", {
      request: {
        sessionId: "sess-initial",
        parts: [{ type: "text", text: "请先开始执行" }],
      },
    });
  });

  test("typing 继续 after a max-turn stop grants another 100-turn budget", async () => {
    sessionRunsResponse = [
      {
        id: "run-max",
        session_id: "sess-continue",
        user_message_id: "user-1",
        assistant_message_id: null,
        status: "failed",
        buffered_text: "",
        error_kind: "max_turns",
        error_message: "已达到执行步数上限，系统已自动停止。",
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
    ];

    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_session_runs") return Promise.resolve(sessionRunsResponse);
      if (command === "send_message") return Promise.resolve(null);
      return Promise.resolve(null);
    });

    render(
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
        sessionId="sess-continue"
      />
    );

    const composer = await screen.findByPlaceholderText("输入消息，Shift+Enter 换行...");
    fireEvent.change(composer, { target: { value: "继续" } });
    fireEvent.click(screen.getByRole("button", { name: "发送" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        request: {
          sessionId: "sess-continue",
          parts: [{ type: "text", text: "继续" }],
          maxIterations: 100,
        },
      });
    });
  });

  test("shows a single friendly auth failure surface instead of repeating raw transport errors", async () => {
    const rawAuthError =
      '{"type":"error","error":{"type":"authentication_error","message":"login fail: Please carry the API secret key in the \'Authorization\' field of the request header"},"request_id":"060d83de3828d796eb11939cf30ed6b8"}';

    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve(messagesResponse);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_session_runs") return Promise.resolve(sessionRunsResponse);
      if (command === "send_message") {
        sessionRunsResponse = [
          {
            id: "run-auth-1",
            session_id: "sess-auth",
            user_message_id: "user-persisted-auth",
            assistant_message_id: null,
            status: "failed",
            buffered_text: "",
            error_kind: "auth",
            error_message: rawAuthError,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          },
        ];
        messagesResponse = [
          {
            id: "user-persisted-auth",
            role: "user",
            content: "你好",
            created_at: new Date().toISOString(),
          },
        ];
        return Promise.reject(new Error(rawAuthError));
      }
      return Promise.resolve(null);
    });

    render(
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
        sessionId="sess-auth"
      />
    );

    const composer = await screen.findByPlaceholderText("输入消息，Shift+Enter 换行...");
    fireEvent.change(composer, { target: { value: "你好" } });
    fireEvent.click(screen.getByRole("button", { name: "发送" }));

    const failureCard = await screen.findByTestId("run-failure-card-run-auth-1");
    expect(failureCard).toHaveTextContent("鉴权失败");
    expect(failureCard).toHaveTextContent("请检查 API Key、组织权限或接口访问范围是否正确。");
    expect(failureCard).not.toHaveTextContent("request_id");
    expect(screen.queryByText(new RegExp(rawAuthError.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")))).not.toBeInTheDocument();
    expect(screen.queryByText(/错误:/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Authorization/)).not.toBeInTheDocument();
  });

  test("falls back to friendly billing copy when a failed run only has raw provider quota JSON", async () => {
    sessionRunsResponse = [
      {
        id: "run-billing-raw",
        session_id: "sess-billing-raw",
        user_message_id: "user-billing-raw",
        assistant_message_id: null,
        status: "failed",
        buffered_text: "",
        error_kind: null,
        error_message:
          '{"error":{"message":"insufficient_quota","code":"insufficient_quota"},"request_id":"quota-123"}',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
    ];
    messagesResponse = [
      {
        id: "user-billing-raw",
        role: "user",
        content: "继续执行",
        created_at: new Date().toISOString(),
      },
    ];

    render(
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
        sessionId="sess-billing-raw"
      />
    );

    const failureCard = await screen.findByTestId("run-failure-card-run-billing-raw");
    expect(failureCard).toHaveTextContent("模型余额不足");
    expect(failureCard).toHaveTextContent(
      "当前模型平台返回余额或额度不足，请到对应服务商控制台充值或检查套餐额度。",
    );
    expect(failureCard).not.toHaveTextContent("insufficient_quota");
    expect(failureCard).not.toHaveTextContent("request_id");
  });

  test("keeps technical error details collapsed until the user asks to view them", async () => {
    sessionRunsResponse = [
      {
        id: "run-unknown-raw",
        session_id: "sess-unknown-raw",
        user_message_id: "user-unknown-raw",
        assistant_message_id: null,
        status: "failed",
        buffered_text: "",
        error_kind: "unknown",
        error_message: "upstream gateway exploded with trace id abc-123",
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
    ];
    messagesResponse = [
      {
        id: "user-unknown-raw",
        role: "user",
        content: "继续执行",
        created_at: new Date().toISOString(),
      },
    ];

    render(
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
        sessionId="sess-unknown-raw"
      />
    );

    const failureCard = await screen.findByTestId("run-failure-card-run-unknown-raw");
    expect(failureCard).toHaveTextContent("连接失败");
    expect(failureCard).not.toHaveTextContent("trace id abc-123");

    const expandButton = screen.getByRole("button", { name: "查看技术详情" });
    fireEvent.click(expandButton);

    expect(screen.getByRole("button", { name: "隐藏技术详情" })).toBeInTheDocument();
    expect(failureCard).toHaveTextContent("upstream gateway exploded with trace id abc-123");
  });

  test("shows network failure recovery guidance", async () => {
    sessionRunsResponse = [
      {
        id: "run-network",
        session_id: "sess-network",
        user_message_id: "user-network",
        assistant_message_id: null,
        status: "failed",
        buffered_text: "已经抓取到前两页数据",
        error_kind: "network",
        error_message: "error sending request for url",
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
    ];
    messagesResponse = [
      {
        id: "user-network",
        role: "user",
        content: "继续处理报表",
        created_at: new Date().toISOString(),
      },
    ];

    render(
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
        sessionId="sess-network"
      />
    );

    const failureCard = await screen.findByTestId("run-failure-card-run-network");
    expect(failureCard).toHaveTextContent("网络连接失败");
    expect(failureCard).toHaveTextContent("无法连接到模型接口，请检查 Base URL、网络环境或代理配置。");
    expect(failureCard).toHaveTextContent("已经保留当前任务的历史消息和部分输出");
    expect(failureCard).toHaveTextContent("网络恢复后可直接输入“继续”");
    expect(failureCard).toHaveTextContent("已经抓取到前两页数据");
  });

  test("keeps prior session content visible when reload after send fails", async () => {
    messagesResponse = [
      {
        id: "user-existing",
        role: "user",
        content: "先整理现有上下文",
        created_at: "2026-03-16T00:00:01Z",
      },
      {
        id: "assistant-existing",
        role: "assistant",
        content: "现有上下文已经保留",
        created_at: "2026-03-16T00:00:02Z",
      },
    ];
    sessionRunsResponse = [
      {
        id: "run-existing",
        session_id: "sess-reload-failure",
        user_message_id: "user-existing",
        assistant_message_id: "assistant-existing",
        status: "failed",
        buffered_text: "现有上下文已经保留",
        error_kind: "network",
        error_message: "error sending request for url",
        created_at: "2026-03-16T00:00:01Z",
        updated_at: "2026-03-16T00:00:03Z",
      },
    ];

    let failReload = false;
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") {
        if (failReload) return Promise.reject(new Error("reload messages failed"));
        return Promise.resolve(messagesResponse);
      }
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_session_runs") {
        if (failReload) return Promise.reject(new Error("reload runs failed"));
        return Promise.resolve(sessionRunsResponse);
      }
      if (command === "send_message") {
        failReload = true;
        return Promise.reject(new Error("network timeout"));
      }
      return Promise.resolve(null);
    });

    render(
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
        sessionId="sess-reload-failure"
      />
    );

    await waitFor(() => {
      expect(screen.getAllByText("现有上下文已经保留").length).toBeGreaterThan(0);
    });

    const composer = screen.getByPlaceholderText("输入消息，Shift+Enter 换行...");
    fireEvent.change(composer, { target: { value: "继续" } });
    fireEvent.click(screen.getByRole("button", { name: "发送" }));

    await waitFor(() => {
      expect(screen.getAllByText("现有上下文已经保留").length).toBeGreaterThan(0);
      expect(screen.getByTestId("run-failure-card-run-existing")).toBeInTheDocument();
    });
  });

  test("keeps unsent drafts isolated per session when switching between conversations", async () => {
    const { rerender } = render(
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
    );

    const composer = await screen.findByPlaceholderText("输入消息，Shift+Enter 换行...");
    fireEvent.change(composer, { target: { value: "整理 A 会话草稿" } });

    rerender(
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
    );

    await waitFor(() => {
      expect(screen.getByPlaceholderText("输入消息，Shift+Enter 换行...")).toHaveValue("");
    });

    fireEvent.change(screen.getByPlaceholderText("输入消息，Shift+Enter 换行..."), {
      target: { value: "继续 B 会话草稿" },
    });

    rerender(
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
    );

    await waitFor(() => {
      expect(screen.getByPlaceholderText("输入消息，Shift+Enter 换行...")).toHaveValue("整理 A 会话草稿");
    });
  });

  test("auto denies pending approval during cleanup", async () => {
    const { unmount } = render(
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
        sessionId="sess-confirm"
      />
    );

    act(() => {
      emit("approval-created", {
        approval_id: "approval-cleanup-1",
        session_id: "sess-confirm",
        tool_name: "bash",
        tool_input: { command: "rm -rf ." },
        title: "高危操作确认",
        summary: "将执行命令",
      });
    });

    unmount();

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("resolve_approval", {
        approvalId: "approval-cleanup-1",
        decision: "deny",
        source: "desktop_cleanup",
      });
    });
  });
});
