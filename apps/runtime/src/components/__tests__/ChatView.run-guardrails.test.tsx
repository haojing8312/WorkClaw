import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ChatView } from "../ChatView";

const invokeMock = vi.fn<(command: string, payload?: unknown) => Promise<unknown>>();
const listeners = new Map<string, Array<(event: { payload: any }) => void>>();

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
        current.filter((item) => item !== cb),
      );
    });
  },
}));

function emit(name: string, payload: any) {
  const arr = listeners.get(name) ?? [];
  arr.forEach((fn) => fn({ payload }));
}

describe("ChatView run guardrails", () => {
  beforeEach(() => {
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
    listeners.clear();
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_session_runs") return Promise.resolve([]);
      return Promise.resolve(null);
    });
  });

  afterEach(() => {
    cleanup();
  });

  it("renders max-turn stop as stopped task copy instead of execution exception", async () => {
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
        sessionId="sess-stop"
      />,
    );

    act(() => {
      emit("agent-state-event", {
        session_id: "sess-stop",
        state: "stopped",
        detail: "达到最大迭代次数 100",
        iteration: 100,
        stop_reason_kind: "max_turns",
        stop_reason_title: "任务达到执行步数上限",
        stop_reason_message: "已达到执行步数上限，系统已自动停止。",
      });
    });

    await waitFor(() => {
      expect(screen.getByText("任务达到执行步数上限")).toBeInTheDocument();
      expect(screen.queryByText(/执行异常/)).not.toBeInTheDocument();
    });
  });

  it("offers a continue action for max-turn stops and sends another 100-turn budget", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_session_runs")
        return Promise.resolve([
          {
            id: "run-max",
            session_id: "sess-stop-continue",
            user_message_id: "user-1",
            assistant_message_id: null,
            status: "failed",
            buffered_text: "",
            error_kind: "max_turns",
            error_message: "已达到执行步数上限，系统已自动停止。",
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          },
        ]);
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
        sessionId="sess-stop-continue"
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("run-failure-card-run-max")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "继续执行" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("send_message", {
        request: {
          sessionId: "sess-stop-continue",
          parts: [{ type: "text", text: "继续" }],
          maxIterations: 100,
        },
      });
    });
  });

  it("renders browser stop with last completed step hint", async () => {
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
        sessionId="sess-browser-stop"
      />,
    );

    act(() => {
      emit("agent-state-event", {
        session_id: "sess-browser-stop",
        state: "stopped",
        detail: "系统检测到连续多轮没有有效进展，已自动停止本轮任务。",
        iteration: 9,
        stop_reason_kind: "no_progress",
        stop_reason_title: "任务长时间没有进展",
        stop_reason_message: "系统检测到连续多轮没有有效进展，已自动停止本轮任务。",
        stop_reason_last_completed_step: "已填写封面标题",
      });
    });

    await waitFor(() => {
      expect(screen.getByText("任务长时间没有进展")).toBeInTheDocument();
      expect(screen.getByText("最后完成步骤：已填写封面标题")).toBeInTheDocument();
    });
  });

  it("renders policy-blocked stop with friendly recovery messaging", async () => {
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
        sessionId="sess-policy-blocked"
      />,
    );

    act(() => {
      emit("agent-state-event", {
        session_id: "sess-policy-blocked",
        state: "stopped",
        detail: "目标路径不在当前工作目录范围内。你可以先切换当前会话的工作目录后重试。",
        iteration: 3,
        stop_reason_kind: "policy_blocked",
        stop_reason_title: "当前任务无法继续执行",
        stop_reason_message: "本次请求触发了安全或工作区限制，系统已停止继续尝试。",
      });
    });

    await waitFor(() => {
      expect(screen.getByText("当前任务无法继续执行")).toBeInTheDocument();
      expect(screen.getByText("本次请求触发了安全或工作区限制，系统已停止继续尝试。")).toBeInTheDocument();
      expect(screen.getByText("目标路径不在当前工作目录范围内。你可以先切换当前会话的工作目录后重试。")).toBeInTheDocument();
    });
  });

  it("renders a friendly retrying banner for automatic network retries", async () => {
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
        sessionId="sess-retrying"
      />,
    );

    act(() => {
      emit("agent-state-event", {
        session_id: "sess-retrying",
        state: "retrying",
        detail: null,
        iteration: 1,
      });
    });

    await waitFor(() => {
      expect(screen.getByText("网络异常，正在自动重试")).toBeInTheDocument();
    });
  });

  it("restores buffered output for a waiting approval run when reopening a session", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_session_runs")
        return Promise.resolve([
          {
            id: "run-waiting-approval",
            session_id: "sess-waiting-approval",
            user_message_id: "user-1",
            assistant_message_id: null,
            status: "waiting_approval",
            buffered_text: "等待审批中的中间输出",
            error_kind: null,
            error_message: null,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          },
        ]);
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
        sessionId="sess-waiting-approval"
      />
    );

    await waitFor(() => {
      expect(screen.getByText("等待审批中的中间输出")).toBeInTheDocument();
    });
  });
});
