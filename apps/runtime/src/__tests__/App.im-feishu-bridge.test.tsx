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

function listWecomTextCalls() {
  return invokeMock.mock.calls
    .filter(([cmd]) => cmd === "send_wecom_text_message")
    .map(([, payload]) => payload);
}

function defaultInvokeImpl(command: string) {
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
  if (command === "send_wecom_text_message") {
    return Promise.resolve("wecom_reply_1");
  }
  if (command === "answer_user_question") {
    return Promise.resolve(null);
  }
  return Promise.resolve(null);
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

    invokeMock.mockImplementation((command: string) => defaultInvokeImpl(command));
  });

  test("keeps Feishu ask_user delivery on the host side and routes follow-up into answer_user_question", async () => {
    render(<App />);

    const dispatchPayload = {
      session_id: "session-im-1",
      thread_id: "chat-feishu-1",
      role_id: "project_manager",
      role_name: "项目经理",
      source_channel: "feishu",
      prompt: "请先拆解任务",
      agent_type: "general-purpose",
    };

    await act(async () => {
      emit("im-role-dispatch-request", dispatchPayload);
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        request: {
          sessionId: "session-im-1",
          parts: [{ type: "text", text: "请先拆解任务" }],
        },
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
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        request: {
          sessionId: "session-im-1",
          parts: [{ type: "text", text: "请先拆解任务" }],
        },
      });
    });

    expect(
      invokeMock.mock.calls.some(
        ([cmd, payload]) =>
          cmd === "send_feishu_text_message" &&
          String(payload?.text ?? "").includes("请选择方案"),
      ),
    ).toBe(false);

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
        source_channel: "feishu",
        prompt: "@_user_1 你细化一下技术方案 ",
        agent_type: "general-purpose",
      });
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        request: {
          sessionId: "session-im-mention-clean",
          parts: [{ type: "text", text: "你细化一下技术方案" }],
        },
      });
    });
  });

  test("sanitizes plain @mention labels before dispatching to desktop session", async () => {
    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-plain-mention-clean",
        thread_id: "chat-feishu-plain-mention-clean",
        role_id: "dev_team",
        role_name: "开发团队",
        source_channel: "feishu",
        prompt: "@开发团队 你细化一下技术方案 ",
        agent_type: "general-purpose",
      });
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        request: {
          sessionId: "session-im-plain-mention-clean",
          parts: [{ type: "text", text: "你细化一下技术方案" }],
        },
      });
    });
  });

  test("refreshes the session list after feishu IM dispatch creates or reuses a session", async () => {
    render(<App />);

    await waitFor(() => {
      expect(invokeMock.mock.calls.some(([cmd]) => cmd === "list_sessions")).toBe(true);
    });
    const initialListSessionsCalls = invokeMock.mock.calls.filter(([cmd]) => cmd === "list_sessions").length;

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-sidebar-refresh",
        thread_id: "chat-feishu-sidebar-refresh",
        role_id: "project_manager",
        role_name: "项目经理",
        source_channel: "feishu",
        prompt: "请同步这条飞书消息",
        agent_type: "general-purpose",
      });
    });

    await waitFor(() => {
      const nextListSessionsCalls = invokeMock.mock.calls.filter(([cmd]) => cmd === "list_sessions").length;
      expect(nextListSessionsCalls).toBeGreaterThan(initialListSessionsCalls);
    });
  });

  test("does not dedupe different inbound messages that share the same prompt text", async () => {
    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-message-id-dedupe",
        thread_id: "chat-feishu-message-id-dedupe",
        message_id: "om_first",
        role_id: "dev_team",
        role_name: "开发团队",
        source_channel: "feishu",
        prompt: "你好",
        agent_type: "general-purpose",
      });
    });

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-message-id-dedupe",
        thread_id: "chat-feishu-message-id-dedupe",
        message_id: "om_second",
        role_id: "dev_team",
        role_name: "开发团队",
        source_channel: "feishu",
        prompt: "你好",
        agent_type: "general-purpose",
      });
    });

    await waitFor(() => {
      const sendMessageCalls = invokeMock.mock.calls.filter(([cmd]) => cmd === "send_message");
      expect(sendMessageCalls).toHaveLength(2);
    });
  });

  test("forwards stream token chunks to WeCom during IM session", async () => {
    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-stream",
        thread_id: "chat-wecom-stream",
        role_id: "project_manager",
        role_name: "项目经理",
        source_channel: "wecom",
        prompt: "请输出执行进度",
        agent_type: "general-purpose",
      });
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        request: {
          sessionId: "session-im-stream",
          parts: [{ type: "text", text: "请输出执行进度" }],
        },
      });
    });

    await act(async () => {
      emit("stream-token", {
        session_id: "session-im-stream",
        token: "这是一段用于测试企业微信流式转发的长文本".repeat(8),
        done: false,
      });
    });

    await waitFor(() => {
      expect(
        listWecomTextCalls().some(
          (payload) =>
            String(payload?.conversation_id ?? "") === "chat-wecom-stream" &&
            String(payload?.text ?? "").includes("项目经理"),
        ),
      ).toBe(true);
    });
  });

  test("does not forward Feishu stream token chunks during IM session", async () => {
    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-final-only",
        thread_id: "chat-feishu-final-only",
        role_id: "project_manager",
        role_name: "项目经理",
        source_channel: "feishu",
        prompt: "请输出最终答复",
        agent_type: "general-purpose",
      });
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        request: {
          sessionId: "session-im-final-only",
          parts: [{ type: "text", text: "请输出最终答复" }],
        },
      });
    });

    await act(async () => {
      emit("stream-token", {
        session_id: "session-im-final-only",
        token: "子智能体执行中：".repeat(24),
        done: false,
      });
    });

    expect(
      listFeishuTextCalls().some(
        (payload) =>
          String(payload?.chatId ?? "") === "chat-feishu-final-only" &&
          String(payload?.text ?? "").includes("子智能体执行中"),
      ),
    ).toBe(false);
  });

  test("throttles WeCom stream forwarding and flushes buffered chunks after interval", async () => {
    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-throttle",
        thread_id: "chat-wecom-throttle",
        role_id: "project_manager",
        role_name: "项目经理",
        source_channel: "wecom",
        prompt: "请持续汇报进度",
        agent_type: "general-purpose",
      });
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        request: {
          sessionId: "session-im-throttle",
          parts: [{ type: "text", text: "请持续汇报进度" }],
        },
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

    const immediateStreamCalls = listWecomTextCalls().filter(
      (payload) =>
        String(payload?.conversation_id ?? "") === "chat-wecom-throttle" &&
        (String(payload?.text ?? "").includes("AAAA") || String(payload?.text ?? "").includes("BBBB")),
    );
    expect(immediateStreamCalls.length).toBe(1);

    await act(async () => {
      vi.advanceTimersByTime(1200);
      await Promise.resolve();
    });

    const delayedStreamCalls = listWecomTextCalls().filter(
      (payload) =>
        String(payload?.conversation_id ?? "") === "chat-wecom-throttle" &&
        (String(payload?.text ?? "").includes("AAAA") || String(payload?.text ?? "").includes("BBBB")),
    );
    expect(delayedStreamCalls.length).toBeGreaterThanOrEqual(2);
  });

  test("does not forward final Feishu replies from the UI layer", async () => {
    vi.useFakeTimers();
    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-no-duplicate",
        thread_id: "chat-feishu-no-duplicate",
        role_id: "project_manager",
        role_name: "项目经理",
        source_channel: "feishu",
        prompt: "请介绍一下自己",
        agent_type: "general-purpose",
      });
    });

    expect(invokeMock).toHaveBeenCalledWith("send_message", {
      request: {
        sessionId: "session-im-no-duplicate",
        parts: [{ type: "text", text: "请介绍一下自己" }],
      },
    });

    await act(async () => {
      emit("stream-token", {
        session_id: "session-im-no-duplicate",
        token: "已收到。",
        done: false,
      });
      emit("stream-token", {
        session_id: "session-im-no-duplicate",
        token: "",
        done: true,
      });
      await vi.advanceTimersByTimeAsync(2600);
      await Promise.resolve();
      await Promise.resolve();
    });

    const calls = listFeishuTextCalls().filter(
      (payload) =>
        String(payload?.chatId ?? "") === "chat-feishu-no-duplicate" &&
        String(payload?.text ?? "").includes("已收到。"),
    );
    expect(calls).toHaveLength(0);
    vi.useRealTimers();
  });

  test("suppresses identical WeCom outbound messages inside the dedup window", async () => {
    vi.useFakeTimers();
    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-outbound-dedup",
        thread_id: "chat-wecom-outbound-dedup",
        role_id: "project_manager",
        role_name: "项目经理",
        source_channel: "wecom",
        prompt: "请介绍一下自己",
        agent_type: "general-purpose",
      });
      emit("stream-token", {
        session_id: "session-im-outbound-dedup",
        token: "重复测试消息",
        done: false,
      });
      emit("stream-token", {
        session_id: "session-im-outbound-dedup",
        token: "",
        done: true,
      });
      emit("stream-token", {
        session_id: "session-im-outbound-dedup",
        token: "重复测试消息",
        done: false,
      });
      emit("stream-token", {
        session_id: "session-im-outbound-dedup",
        token: "",
        done: true,
      });
      await Promise.resolve();
    });

    const calls = listWecomTextCalls().filter(
      (payload) =>
        String(payload?.conversation_id ?? "") === "chat-wecom-outbound-dedup" &&
        String(payload?.text ?? "").includes("重复测试消息"),
    );
    expect(calls).toHaveLength(1);
    vi.useRealTimers();
  });

  test("does not poll delayed assistant replies for Feishu in the UI layer", async () => {
    vi.useFakeTimers();
    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-delayed-fallback",
        thread_id: "chat-feishu-delayed-fallback",
        role_id: "project_manager",
        role_name: "项目经理",
        source_channel: "feishu",
        prompt: "请稍后回复我",
        agent_type: "general-purpose",
      });
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(
      listFeishuTextCalls().some(
        (payload) =>
          String(payload?.chatId ?? "") === "chat-feishu-delayed-fallback" &&
          String(payload?.text ?? "").includes("延迟到达的最终答复"),
      ),
    ).toBe(false);

    vi.useRealTimers();
  }, 10000);

  test("keeps Feishu closed-loop from delegated stream to clarification answer", async () => {
    render(<App />);

    const dispatchPayload = {
      session_id: "session-im-closed-loop",
      thread_id: "chat-feishu-closed-loop",
      role_id: "project_manager",
      role_name: "项目经理",
      source_channel: "feishu",
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

    expect(
      listFeishuTextCalls().some(
        (payload) =>
          String(payload?.chatId ?? "") === dispatchPayload.thread_id &&
          String(payload?.text ?? "").includes("开发团队正在处理中"),
      ),
    ).toBe(false);

    await act(async () => {
      emit("ask-user-event", {
        session_id: dispatchPayload.session_id,
        question: "需求澄清：确认技术方案 A 还是 B？",
        options: ["A", "B"],
      });
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        request: {
          sessionId: dispatchPayload.session_id,
          parts: [{ type: "text", text: "请先分析并分派开发团队" }],
        },
      });
    });

    expect(
      listFeishuTextCalls().some(
        (payload) =>
          String(payload?.chatId ?? "") === dispatchPayload.thread_id &&
          String(payload?.text ?? "").includes("需求澄清"),
      ),
    ).toBe(false);

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

  test("uses bracket role prefix and switches back to main role after delegated stream for WeCom", async () => {
    render(<App />);

    const dispatchPayload = {
      session_id: "session-im-role-prefix",
      thread_id: "chat-wecom-role-prefix",
      role_id: "project_manager",
      role_name: "项目经理",
      source_channel: "wecom",
      prompt: "请先安排开发团队",
      agent_type: "general-purpose",
    };

    await act(async () => {
      emit("im-role-dispatch-request", dispatchPayload);
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        request: {
          sessionId: dispatchPayload.session_id,
          parts: [{ type: "text", text: "请先安排开发团队" }],
        },
      });
    });

    await act(async () => {
      emit("stream-token", {
        session_id: dispatchPayload.session_id,
        token: "子员工执行中。".repeat(20),
        done: false,
        sub_agent: true,
        role_id: "dev_team",
        role_name: "开发团队",
      });
    });

    await waitFor(() => {
      expect(
        listWecomTextCalls().some(
          (payload) =>
            String(payload?.conversation_id ?? "") === dispatchPayload.thread_id &&
            String(payload?.text ?? "").startsWith("[开发团队] "),
        ),
      ).toBe(true);
    });

    await act(async () => {
      emit("stream-token", {
        session_id: dispatchPayload.session_id,
        token: "",
        done: true,
        sub_agent: true,
      });
    });

    await act(async () => {
      emit("stream-token", {
        session_id: dispatchPayload.session_id,
        token: "主员工汇总输出。".repeat(20),
        done: false,
      });
      emit("stream-token", {
        session_id: dispatchPayload.session_id,
        token: "",
        done: true,
      });
    });

    await waitFor(() => {
      expect(
        listWecomTextCalls().some(
          (payload) =>
            String(payload?.conversation_id ?? "") === dispatchPayload.thread_id &&
            String(payload?.text ?? "").startsWith("[项目经理] ") &&
            String(payload?.text ?? "").includes("主员工汇总输出"),
        ),
      ).toBe(true);
    });
  });

  test("does not retry Feishu final delivery from the UI layer", async () => {
    vi.useFakeTimers();
    invokeMock.mockImplementation((command: string, payload: any) => {
      if (
        command === "send_feishu_text_message" &&
        String(payload?.chatId ?? "") === "chat-feishu-retry" &&
        String(payload?.text ?? "").includes("重试消息")
      ) {
        return Promise.reject(new Error("network timeout"));
      }
      return defaultInvokeImpl(command);
    });

    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-retry",
        thread_id: "chat-feishu-retry",
        role_id: "project_manager",
        role_name: "项目经理",
        source_channel: "feishu",
        prompt: "请开始执行",
        agent_type: "general-purpose",
      });
    });

    expect(invokeMock).toHaveBeenCalledWith("send_message", {
      request: {
        sessionId: "session-im-retry",
        parts: [{ type: "text", text: "请开始执行" }],
      },
    });

    await act(async () => {
      emit("stream-token", {
        session_id: "session-im-retry",
        token: "重试消息".repeat(40),
        done: false,
      });
      emit("stream-token", {
        session_id: "session-im-retry",
        token: "",
        done: true,
      });
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5600);
      await Promise.resolve();
    });

    const calls = listFeishuTextCalls().filter(
      (payload) =>
        String(payload?.chatId ?? "") === "chat-feishu-retry" &&
        String(payload?.text ?? "").includes("重试消息"),
    );
    expect(calls.length).toBe(0);
    vi.useRealTimers();
  });

  test("does not keep retrying Feishu final delivery from the UI layer", async () => {
    vi.useFakeTimers();
    invokeMock.mockImplementation((command: string, payload: any) => {
      if (
        command === "send_feishu_text_message" &&
        String(payload?.chatId ?? "") === "chat-feishu-degrade" &&
        String(payload?.text ?? "").includes("降级消息")
      ) {
        return Promise.reject(new Error("permanent failure"));
      }
      return defaultInvokeImpl(command);
    });

    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-degrade",
        thread_id: "chat-feishu-degrade",
        role_id: "project_manager",
        role_name: "项目经理",
        source_channel: "feishu",
        prompt: "请开始执行",
        agent_type: "general-purpose",
      });
    });

    expect(invokeMock).toHaveBeenCalledWith("send_message", {
      request: {
        sessionId: "session-im-degrade",
        parts: [{ type: "text", text: "请开始执行" }],
      },
    });

    await act(async () => {
      emit("stream-token", {
        session_id: "session-im-degrade",
        token: "降级消息".repeat(40),
        done: false,
      });
      emit("stream-token", {
        session_id: "session-im-degrade",
        token: "",
        done: true,
      });
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(13000);
      await Promise.resolve();
    });

    const degradeCalls = listFeishuTextCalls().filter(
      (payload) =>
        String(payload?.chatId ?? "") === "chat-feishu-degrade" &&
        String(payload?.text ?? "").includes("降级消息"),
    );
    expect(degradeCalls.length).toBe(0);
    vi.useRealTimers();
  });

  test("routes wecom IM replies through the channel-neutral bridge", async () => {
    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-wecom",
        thread_id: "chat-wecom-1",
        role_id: "project_manager",
        role_name: "项目经理",
        source_channel: "wecom",
        prompt: "请同步项目状态",
        agent_type: "general-purpose",
      });
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        request: {
          sessionId: "session-im-wecom",
          parts: [{ type: "text", text: "请同步项目状态" }],
        },
      });
    });

    await act(async () => {
      emit("ask-user-event", {
        session_id: "session-im-wecom",
        question: "请确认企业微信渠道回发",
        options: ["继续", "暂停"],
      });
    });

    await waitFor(() => {
      expect(
        listWecomTextCalls().some(
          (payload) =>
            String(payload?.conversation_id ?? "") === "chat-wecom-1" &&
            String(payload?.text ?? "").includes("请确认企业微信渠道回发"),
        ),
      ).toBe(true);
    });

    expect(
      listFeishuTextCalls().some(
        (payload) =>
          String(payload?.chatId ?? "") === "chat-wecom-1" &&
          String(payload?.text ?? "").includes("请确认企业微信渠道回发"),
      ),
    ).toBe(false);
  });

  test("does not fall back to Feishu when IM dispatch payload omits source_channel", async () => {
    render(<App />);

    await act(async () => {
      emit("im-role-dispatch-request", {
        session_id: "session-im-generic",
        thread_id: "thread-generic",
        role_id: "project_manager",
        role_name: "项目经理",
        prompt: "请先拆解任务",
        agent_type: "general-purpose",
      });
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        request: {
          sessionId: "session-im-generic",
          parts: [{ type: "text", text: "请先拆解任务" }],
        },
      });
    });

    await act(async () => {
      emit("ask-user-event", {
        session_id: "session-im-generic",
        question: "请选择处理方案",
        options: ["A", "B"],
      });
    });

    await waitFor(() => {
      expect(listFeishuTextCalls()).toHaveLength(0);
      expect(listWecomTextCalls()).toHaveLength(0);
    });
  });
});
