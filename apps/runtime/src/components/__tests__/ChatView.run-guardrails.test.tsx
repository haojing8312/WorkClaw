import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
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
        detail: "达到最大迭代次数 12",
        iteration: 12,
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
});
