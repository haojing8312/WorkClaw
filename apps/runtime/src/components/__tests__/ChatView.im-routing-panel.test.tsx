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
      if (command === "list_sessions") return Promise.resolve([]);
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
      expect(screen.getByText("【架构师】场景=opportunity_review。用户输入：请开始评审")).toBeInTheDocument();
    });

    await waitFor(() => {
      expect(screen.getByText(/已自动路由 1 个子 Skill/)).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText(/已自动路由 1 个子 Skill/));

    await waitFor(() => {
      expect(screen.getByText("IM 协作时间线")).toBeInTheDocument();
      expect(screen.getAllByText("架构师").length).toBeGreaterThan(0);
      expect(screen.getByText("正在评估技术可行性")).toBeInTheDocument();
      expect(screen.getByText("任务已分发(plan) -> 架构师")).toBeInTheDocument();
      expect(screen.getByText("路由决策")).toBeInTheDocument();
      expect(screen.getByText("agent: peer-agent")).toBeInTheDocument();
      expect(screen.getByText("matched_by: binding.peer")).toBeInTheDocument();
      expect(screen.getByText("session_key: agent:peer-agent:main")).toBeInTheDocument();
    });
  });

  test("suppresses local ask-user dialog for IM-managed session", async () => {
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
        sessionId="session-im"
        suppressAskUserPrompt
      />
    );

    act(() => {
      emit("ask-user-event", {
        session_id: "session-im",
        question: "请确认是否继续",
        options: ["继续", "暂停"],
      });
    });

    await waitFor(() => {
      expect(screen.queryByText("请确认是否继续")).not.toBeInTheDocument();
    });
  });

  test("shows main/sub employee badges in IM timeline", async () => {
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
        sessionId="session-role-badge"
      />
    );

    act(() => {
      emit("im-role-event", {
        session_id: "session-role-badge",
        thread_id: "thread-role-badge",
        role_id: "project_manager",
        role_name: "项目经理",
        sender_role: "main_agent",
        status: "running",
        summary: "主员工分析中",
      });
      emit("im-role-event", {
        session_id: "session-role-badge",
        thread_id: "thread-role-badge",
        role_id: "dev_team",
        role_name: "开发团队",
        sender_role: "sub_agent",
        status: "running",
        summary: "子员工执行中",
      });
      emit("skill-route-node-updated", {
        session_id: "session-role-badge",
        route_run_id: "r-role-badge",
        node_id: "n-role-badge",
        skill_name: "dispatch",
        depth: 1,
        status: "running",
      });
    });

    fireEvent.click(screen.getByText(/已自动路由 1 个子 Skill/));

    await waitFor(() => {
      expect(screen.getByText("主员工")).toBeInTheDocument();
      expect(screen.getByText("子员工")).toBeInTheDocument();
    });
  });

  test("shows sub-agent streaming panel with role name in chat area", async () => {
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
        sessionId="session-sub-stream"
      />
    );

    act(() => {
      emit("stream-token", {
        session_id: "session-sub-stream",
        token: "这是开发团队的流式方案输出",
        done: false,
        sub_agent: true,
        role_name: "开发团队",
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("sub-agent-stream-buffer")).toHaveTextContent("开发团队");
      expect(screen.getByTestId("sub-agent-stream-buffer")).toHaveTextContent("这是开发团队的流式方案输出");
    });
  });

  test("shows delegation card in chat area for main to sub employee handoff", async () => {
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
        sessionId="session-delegation-card"
      />
    );

    act(() => {
      emit("im-role-event", {
        session_id: "session-delegation-card",
        thread_id: "thread-delegation-card",
        role_id: "project_manager",
        role_name: "项目经理",
        sender_role: "main_agent",
        status: "running",
        summary: "主员工分析中",
      });
      emit("im-role-dispatch-request", {
        session_id: "session-delegation-card",
        thread_id: "thread-delegation-card",
        role_id: "dev_team",
        role_name: "开发团队",
        sender_role: "main_agent",
        target_employee_id: "dev_team",
        task_id: "task-001",
        parent_task_id: "task-root",
        prompt: "请细化技术方案",
        agent_type: "plan",
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("delegation-card-task-001")).toHaveTextContent("项目经理 已将任务分配给 开发团队");
      expect(screen.getByTestId("delegation-card-task-001")).toHaveTextContent("执行中");
    });
  });

  test("shows collaboration status bar with main owner and delegated target", async () => {
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
        sessionId="session-collab-status"
      />
    );

    act(() => {
      emit("im-role-event", {
        session_id: "session-collab-status",
        thread_id: "thread-collab-status",
        role_id: "project_manager",
        role_name: "项目经理",
        sender_role: "main_agent",
        status: "running",
        summary: "主员工分析中",
      });
      emit("im-role-dispatch-request", {
        session_id: "session-collab-status",
        thread_id: "thread-collab-status",
        role_id: "dev_team",
        role_name: "开发团队",
        sender_role: "main_agent",
        task_id: "task-collab-001",
        prompt: "请细化技术方案",
        agent_type: "plan",
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("team-collab-status-bar")).toHaveTextContent("项目经理");
      expect(screen.getByTestId("team-collab-status-bar")).toHaveTextContent("已委派 开发团队");
    });
  });

  test("keeps delegation history collapsed until expanded by user", async () => {
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
        sessionId="session-delegation-history"
      />
    );

    act(() => {
      emit("im-role-event", {
        session_id: "session-delegation-history",
        thread_id: "thread-delegation-history",
        role_id: "project_manager",
        role_name: "项目经理",
        sender_role: "main_agent",
        status: "running",
      });
      emit("im-role-dispatch-request", {
        session_id: "session-delegation-history",
        thread_id: "thread-delegation-history",
        role_id: "dev_team",
        role_name: "开发团队",
        sender_role: "main_agent",
        task_id: "task-history-1",
        prompt: "请先产出技术方案",
        agent_type: "plan",
      });
      emit("im-role-event", {
        session_id: "session-delegation-history",
        thread_id: "thread-delegation-history",
        role_id: "dev_team",
        role_name: "开发团队",
        sender_role: "sub_agent",
        status: "completed",
        task_id: "task-history-1",
      });
      emit("im-role-dispatch-request", {
        session_id: "session-delegation-history",
        thread_id: "thread-delegation-history",
        role_id: "qa_team",
        role_name: "测试团队",
        sender_role: "main_agent",
        task_id: "task-history-2",
        prompt: "请评审测试范围",
        agent_type: "plan",
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("delegation-card-task-history-2")).toBeInTheDocument();
      expect(screen.queryByTestId("delegation-history-panel")).not.toBeInTheDocument();
      expect(screen.getByTestId("delegation-history-toggle")).toHaveTextContent("1");
    });

    fireEvent.click(screen.getByTestId("delegation-history-toggle"));

    await waitFor(() => {
      expect(screen.getByTestId("delegation-history-panel")).toBeInTheDocument();
      expect(screen.getByTestId("delegation-card-task-history-1")).toBeInTheDocument();
    });
  });

  test("shows high-priority ask-user action card when clarification is required", async () => {
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
        sessionId="session-ask-user-card"
      />
    );

    act(() => {
      emit("ask-user-event", {
        session_id: "session-ask-user-card",
        question: "预算是按月还是按年报价？",
        options: ["按月", "按年"],
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("ask-user-action-card")).toHaveTextContent("需要你的确认");
      expect(screen.getByTestId("ask-user-action-card")).toHaveTextContent("预算是按月还是按年报价？");
      expect(screen.getByRole("button", { name: "按月" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "按年" })).toBeInTheDocument();
    });
  });

  test("shows main employee summarizing hint after delegated sub employee completes", async () => {
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
        sessionId="session-summary-hint"
      />
    );

    act(() => {
      emit("im-role-event", {
        session_id: "session-summary-hint",
        thread_id: "thread-summary-hint",
        role_id: "project_manager",
        role_name: "项目经理",
        sender_role: "main_agent",
        status: "running",
      });
      emit("im-role-dispatch-request", {
        session_id: "session-summary-hint",
        thread_id: "thread-summary-hint",
        role_id: "dev_team",
        role_name: "开发团队",
        sender_role: "main_agent",
        task_id: "task-summary-hint-1",
        prompt: "请输出技术方案",
        agent_type: "plan",
      });
      emit("im-role-event", {
        session_id: "session-summary-hint",
        thread_id: "thread-summary-hint",
        role_id: "dev_team",
        role_name: "开发团队",
        sender_role: "sub_agent",
        status: "completed",
        task_id: "task-summary-hint-1",
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("team-collab-status-bar")).toHaveTextContent("项目经理");
      expect(screen.getByTestId("team-collab-status-bar")).toHaveTextContent("正在汇总最终答复");
      expect(screen.getByTestId("delegation-card-task-summary-hint-1")).toHaveTextContent("已完成");
    });
  });

  test("shows final summary sent hint after main employee completes", async () => {
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
        sessionId="session-final-summary"
      />
    );

    act(() => {
      emit("im-role-event", {
        session_id: "session-final-summary",
        thread_id: "thread-final-summary",
        role_id: "project_manager",
        role_name: "项目经理",
        sender_role: "main_agent",
        status: "running",
      });
      emit("im-role-dispatch-request", {
        session_id: "session-final-summary",
        thread_id: "thread-final-summary",
        role_id: "dev_team",
        role_name: "开发团队",
        sender_role: "main_agent",
        task_id: "task-final-summary-1",
        prompt: "请输出技术方案",
        agent_type: "plan",
      });
      emit("im-role-event", {
        session_id: "session-final-summary",
        thread_id: "thread-final-summary",
        role_id: "dev_team",
        role_name: "开发团队",
        sender_role: "sub_agent",
        status: "completed",
        task_id: "task-final-summary-1",
      });
      emit("im-role-event", {
        session_id: "session-final-summary",
        thread_id: "thread-final-summary",
        role_id: "project_manager",
        role_name: "项目经理",
        sender_role: "main_agent",
        status: "completed",
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("team-collab-status-bar")).toHaveTextContent("项目经理");
      expect(screen.getByTestId("team-collab-status-bar")).toHaveTextContent("已输出最终汇总");
    });
  });

  test("shows session source badge in chat area for feishu synced session", async () => {
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
        sessionId="session-source-badge"
        sessionSourceChannel="feishu"
        sessionSourceLabel="飞书同步"
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("chat-session-source-badge")).toHaveTextContent("飞书同步");
    });
  });

  test("shows group orchestration board with phase, round and member statuses", async () => {
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
        sessionId="session-group-board"
      />
    );

    act(() => {
      emit("im-role-event", {
        session_id: "session-group-board",
        thread_id: "thread-group-board",
        role_id: "project_manager",
        role_name: "项目经理",
        sender_role: "main_agent",
        status: "running",
      });
      emit("im-role-dispatch-request", {
        session_id: "session-group-board",
        thread_id: "thread-group-board",
        role_id: "dev_team",
        role_name: "开发团队",
        sender_role: "main_agent",
        task_id: "task-group-1",
        prompt: "请产出技术方案",
        agent_type: "plan",
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("阶段：执行");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("轮次：第 1 轮");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("开发团队");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("running");
    });
  });

  test("shows group orchestration board from backend run snapshot", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "get_employee_group_run_snapshot") {
        return Promise.resolve({
          run_id: "run-snapshot-1",
          group_id: "group-snapshot-1",
          session_id: "session-group-snapshot",
          state: "executing",
          current_round: 2,
          final_report: "计划：共 3 步",
          steps: [
            { id: "s1", round_no: 1, assignee_employee_id: "开发团队", status: "completed", output: "" },
            { id: "s2", round_no: 2, assignee_employee_id: "测试团队", status: "running", output: "" },
          ],
        });
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
        sessionId="session-group-snapshot"
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("阶段：执行");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("轮次：第 2 轮");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("开发团队");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("测试团队");
    });
  });
});
