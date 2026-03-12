import { act, render, screen, waitFor } from "@testing-library/react";
import { ChatView } from "../ChatView";

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
    listeners.clear();
    invokeMock.mockReset();
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

  test("clears thinking banner and shows failure card when latest run ends with billing failure", async () => {
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
      expect(screen.getByText("正在分析任务")).toBeInTheDocument();
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
      expect(screen.queryByText("正在分析任务")).not.toBeInTheDocument();
      expect(screen.getByTestId("run-failure-card-run-1")).toHaveTextContent("模型余额不足");
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
});
