import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ChatView } from "../ChatView";

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
        current.filter((item) => item !== cb)
      );
    });
  },
}));

function emit(name: string, payload: any) {
  const arr = listeners.get(name) ?? [];
  arr.forEach((fn) => fn({ payload }));
}

describe("ChatView IM routing panel", () => {
  beforeEach(() => {
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
    listeners.clear();
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      return Promise.resolve(null);
    });
  });

  test("shows routing timeline events for role collaboration", async () => {
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
        sessionId="session-a"
      />
    );

    act(() => {
      emit("skill-route-node-updated", {
        session_id: "session-a",
        route_run_id: "r1",
        node_id: "n1",
        skill_name: "presales-skill",
        depth: 1,
        status: "running",
      });

      emit("im-role-event", {
        session_id: "session-a",
        thread_id: "thread-1",
        role_id: "architect",
        role_name: "架构师",
        status: "running",
        summary: "正在评估技术可行性",
      });
      emit("im-role-dispatch-request", {
        session_id: "session-a",
        thread_id: "thread-1",
        role_id: "architect",
        role_name: "架构师",
        prompt: "场景=opportunity_review。用户输入：请开始评审",
        agent_type: "plan",
      });
      emit("im-route-decision", {
        session_id: "session-a",
        thread_id: "thread-1",
        agent_id: "peer-agent",
        session_key: "agent:peer-agent:main",
        matched_by: "binding.peer",
      });
    });

    await waitFor(() => {
      expect(screen.getByText(/已自动路由 1 个子 Skill/)).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText(/已自动路由 1 个子 Skill/));

    await waitFor(() => {
      expect(screen.getByText("IM 协作时间线")).toBeInTheDocument();
      expect(screen.getAllByText("架构师").length).toBeGreaterThan(0);
      expect(screen.getByText("正在评估技术可行性")).toBeInTheDocument();
      expect(screen.getByText("任务已分发(plan)")).toBeInTheDocument();
      expect(screen.getByText("路由决策")).toBeInTheDocument();
      expect(screen.getByText("agent: peer-agent")).toBeInTheDocument();
      expect(screen.getByText("matched_by: binding.peer")).toBeInTheDocument();
      expect(screen.getByText("session_key: agent:peer-agent:main")).toBeInTheDocument();
    });
  });
});
