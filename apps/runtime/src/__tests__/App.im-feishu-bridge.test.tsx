import { act, render, waitFor } from "@testing-library/react";
import App from "../App";

const invokeMock = vi.fn();
const listeners = new Map<string, Array<(event: { payload: any }) => void>>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
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
        current.filter((item) => item !== cb),
      );
    });
  },
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

vi.mock("../components/Sidebar", () => ({
  Sidebar: () => <div data-testid="sidebar">sidebar</div>,
}));

vi.mock("../components/ChatView", () => ({
  ChatView: () => <div data-testid="chat-view">chat-view</div>,
}));

vi.mock("../components/packaging/PackagingView", () => ({
  PackagingView: () => <div data-testid="packaging-view">packaging-view</div>,
}));

vi.mock("../components/experts/ExpertsView", () => ({
  ExpertsView: () => <div data-testid="experts-view">experts-view</div>,
}));

vi.mock("../components/experts/ExpertCreateView", () => ({
  ExpertCreateView: () => <div data-testid="experts-new-view">experts-new-view</div>,
}));

vi.mock("../components/SettingsView", () => ({
  SettingsView: () => <div data-testid="settings-view">settings-view</div>,
}));

vi.mock("../components/InstallDialog", () => ({
  InstallDialog: () => <div data-testid="install-dialog">install-dialog</div>,
}));

vi.mock("../components/NewSessionLanding", () => ({
  NewSessionLanding: () => <div data-testid="new-session-landing">new-session-landing</div>,
}));

function emit(name: string, payload: any) {
  const arr = listeners.get(name) ?? [];
  arr.forEach((fn) => fn({ payload }));
}

function listFeishuTextCalls() {
  return invokeMock.mock.calls
    .filter(([cmd]) => cmd === "send_feishu_text_message")
    .map(([, payload]) => payload);
}

describe("App feishu IM bridge", () => {
  beforeEach(() => {
    vi.useRealTimers();
    listeners.clear();
    invokeMock.mockReset();
    Object.defineProperty(window as typeof window & { __TAURI_INTERNALS__?: unknown }, "__TAURI_INTERNALS__", {
      configurable: true,
      value: { transformCallback: vi.fn() },
    });

    invokeMock.mockImplementation((command: string) => {
      if (command === "list_skills") {
        return Promise.resolve([
          {
            id: "builtin-general",
            name: "General",
            description: "desc",
            version: "1.0.0",
            author: "test",
            recommended_model: "",
            tags: [],
            created_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "list_model_configs") {
        return Promise.resolve([
          {
            id: "model-a",
            name: "Model A",
            api_format: "openai",
            base_url: "https://example.com",
            model_name: "model-a",
            is_default: true,
          },
        ]);
      }
      if (command === "list_agent_employees") {
        return Promise.resolve([]);
      }
      if (command === "get_sessions") {
        return Promise.resolve([]);
      }
      if (command === "send_message") {
        return Promise.resolve(null);
      }
      if (command === "get_messages") {
        return Promise.resolve([
          {
            role: "assistant",
            content: "这是最终答复",
            created_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "send_feishu_text_message") {
        return Promise.resolve("om_reply_1");
      }
      if (command === "answer_user_question") {
        return Promise.resolve(null);
      }
      return Promise.resolve(null);
    });
  });

  test("forwards ask_user to Feishu and routes follow-up message into answer_user_question", async () => {
    render(<App />);

    const dispatchPayload = {
      session_id: "session-im-1",
      thread_id: "chat-feishu-1",
      role_id: "project_manager",
      role_name: "项目经理",
      prompt: "请先拆解任务",
      agent_type: "general-purpose",
    };

    await act(async () => {
      emit("im-role-dispatch-request", dispatchPayload);
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        sessionId: "session-im-1",
        userMessage: "请先拆解任务",
      });
    });

    await act(async () => {
      emit("ask-user-event", {
        session_id: "session-im-1",
        question: "请选择方案",
        options: ["方案A", "方案B"],
      });
    });

    await waitFor(() => {
      expect(
        invokeMock.mock.calls.some(
          ([cmd, payload]) =>
            cmd === "send_feishu_text_message" &&
            String(payload?.text ?? "").includes("请选择方案"),
        ),
      ).toBe(true);
    });

    await act(async () => {
      emit("im-role-dispatch-request", {
        ...dispatchPayload,
        prompt: "我选方案A",
      });
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("answer_user_question", {
        answer: "我选方案A",
      });
    });

    const sendMessageCalls = invokeMock.mock.calls.filter(([cmd]) => cmd === "send_message");
    expect(sendMessageCalls).toHaveLength(1);
  });

  test("sanitizes feishu mention placeholders before dispatching to desktop session", async () => {
    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-mention-clean",
        thread_id: "chat-feishu-mention-clean",
        role_id: "dev_team",
        role_name: "开发团队",
        prompt: "@_user_1 你细化一下技术方案 ",
        agent_type: "general-purpose",
      });
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        sessionId: "session-im-mention-clean",
        userMessage: "你细化一下技术方案",
      });
    });
  });

  test("forwards stream token chunks to Feishu during IM session", async () => {
    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-stream",
        thread_id: "chat-feishu-stream",
        role_id: "project_manager",
        role_name: "项目经理",
        prompt: "请输出执行进度",
        agent_type: "general-purpose",
      });
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        sessionId: "session-im-stream",
        userMessage: "请输出执行进度",
      });
    });

    await act(async () => {
      emit("stream-token", {
        session_id: "session-im-stream",
        token: "这是一段用于测试飞书流式转发的长文本".repeat(8),
        done: false,
      });
    });

    await waitFor(() => {
      expect(
        invokeMock.mock.calls.some(
          ([cmd, payload]) =>
            cmd === "send_feishu_text_message" &&
            String(payload?.chatId ?? "") === "chat-feishu-stream" &&
            String(payload?.text ?? "").includes("项目经理"),
        ),
      ).toBe(true);
    });
  });

  test("forwards sub-agent stream token chunks to Feishu during IM session", async () => {
    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-sub-agent",
        thread_id: "chat-feishu-sub-agent",
        role_id: "project_manager",
        role_name: "项目经理",
        prompt: "请安排开发团队接管",
        agent_type: "general-purpose",
      });
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        sessionId: "session-im-sub-agent",
        userMessage: "请安排开发团队接管",
      });
    });

    await act(async () => {
      emit("stream-token", {
        session_id: "session-im-sub-agent",
        token: "子智能体执行中：".repeat(24),
        done: false,
        sub_agent: true,
      });
    });

    await waitFor(() => {
      expect(
        listFeishuTextCalls().some(
          (payload) =>
            String(payload?.chatId ?? "") === "chat-feishu-sub-agent" &&
            String(payload?.text ?? "").includes("子智能体执行中"),
        ),
      ).toBe(true);
    });
  });

  test("throttles Feishu stream forwarding and flushes buffered chunks after interval", async () => {
    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-throttle",
        thread_id: "chat-feishu-throttle",
        role_id: "project_manager",
        role_name: "项目经理",
        prompt: "请持续汇报进度",
        agent_type: "general-purpose",
      });
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        sessionId: "session-im-throttle",
        userMessage: "请持续汇报进度",
      });
    });

    vi.useFakeTimers();

    await act(async () => {
      emit("stream-token", {
        session_id: "session-im-throttle",
        token: "A".repeat(130),
        done: false,
      });
      emit("stream-token", {
        session_id: "session-im-throttle",
        token: "B".repeat(130),
        done: false,
      });
    });

    const immediateStreamCalls = listFeishuTextCalls().filter(
      (payload) =>
        String(payload?.chatId ?? "") === "chat-feishu-throttle" &&
        (String(payload?.text ?? "").includes("AAAA") || String(payload?.text ?? "").includes("BBBB")),
    );
    expect(immediateStreamCalls.length).toBe(1);

    await act(async () => {
      vi.advanceTimersByTime(1200);
      await Promise.resolve();
    });

    const delayedStreamCalls = listFeishuTextCalls().filter(
      (payload) =>
        String(payload?.chatId ?? "") === "chat-feishu-throttle" &&
        (String(payload?.text ?? "").includes("AAAA") || String(payload?.text ?? "").includes("BBBB")),
    );
    expect(delayedStreamCalls.length).toBeGreaterThanOrEqual(2);
  });

  test("keeps Feishu closed-loop from delegated stream to clarification answer", async () => {
    render(<App />);

    const dispatchPayload = {
      session_id: "session-im-closed-loop",
      thread_id: "chat-feishu-closed-loop",
      role_id: "project_manager",
      role_name: "项目经理",
      prompt: "请先分析并分派开发团队",
      agent_type: "general-purpose",
    };

    await act(async () => {
      emit("im-role-dispatch-request", dispatchPayload);
      emit("stream-token", {
        session_id: dispatchPayload.session_id,
        token: "开发团队正在处理中。".repeat(12),
        done: false,
        sub_agent: true,
      });
    });

    await waitFor(() => {
      expect(
        listFeishuTextCalls().some(
          (payload) =>
            String(payload?.chatId ?? "") === dispatchPayload.thread_id &&
            String(payload?.text ?? "").includes("开发团队正在处理中"),
        ),
      ).toBe(true);
    });

    await act(async () => {
      emit("ask-user-event", {
        session_id: dispatchPayload.session_id,
        question: "需求澄清：确认技术方案 A 还是 B？",
        options: ["A", "B"],
      });
    });

    await waitFor(() => {
      expect(
        listFeishuTextCalls().some(
          (payload) =>
            String(payload?.chatId ?? "") === dispatchPayload.thread_id &&
            String(payload?.text ?? "").includes("需求澄清"),
        ),
      ).toBe(true);
    });

    await act(async () => {
      emit("im-role-dispatch-request", {
        ...dispatchPayload,
        prompt: "选 A",
      });
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("answer_user_question", { answer: "选 A" });
    });
  });
});
