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

  test("shows a team-entry header and waiting state before the first task is sent", async () => {
    render(
      <ChatView
        skill={{
          id: "builtin-general",
          name: "通用助手",
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
        sessionId="session-team-entry-empty"
        sessionMode="team_entry"
        sessionTitle="默认复杂任务团队"
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("chat-session-display-title")).toHaveTextContent("团队协作");
      expect(screen.getByTestId("chat-session-display-subtitle")).toHaveTextContent("默认复杂任务团队");
      expect(screen.getByTestId("team-entry-empty-state")).toHaveTextContent("团队已就绪");
      expect(screen.getByTestId("team-entry-empty-state")).toHaveTextContent("默认复杂任务团队");
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

  test("renders current phase, review round, waiting owner and recent events from backend snapshot", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "get_employee_group_run_snapshot") {
        return Promise.resolve({
          run_id: "run-review-1",
          group_id: "group-review-1",
          session_id: "session-review-1",
          state: "waiting_review",
          current_round: 2,
          current_phase: "review",
          review_round: 2,
          status_reason: "缺少回滚方案",
          waiting_for_employee_id: "门下省",
          waiting_for_user: false,
          final_report: "计划：共 3 步",
          steps: [
            {
              id: "step-plan",
              round_no: 2,
              step_type: "plan",
              assignee_employee_id: "中书省",
              status: "completed",
              output: "",
            },
            {
              id: "step-review",
              round_no: 2,
              step_type: "review",
              assignee_employee_id: "门下省",
              status: "blocked",
              output: "",
            },
          ],
          events: [
            {
              id: "evt-1",
              step_id: "step-review",
              event_type: "review_requested",
              payload_json: "{\"comment\":\"请审议\"}",
              created_at: "2026-03-07T00:00:00Z",
            },
            {
              id: "evt-2",
              step_id: "step-plan",
              event_type: "step_completed",
              payload_json: "{\"phase\":\"plan\"}",
              created_at: "2026-03-07T00:01:00Z",
            },
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
        sessionId="session-review-1"
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("阶段：审核");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("轮次：第 2 轮");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("审议轮次：2");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("等待：门下省");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("缺少回滚方案");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("中书省");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("门下省");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("review_requested");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("step_completed");
    });
  });

  test("approves pending review and continues group run from orchestration board", async () => {
    let snapshotState = "waiting_review";
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "get_employee_group_run_snapshot") {
        if (snapshotState === "done") {
          return Promise.resolve({
            run_id: "run-review-approve",
            group_id: "group-review-approve",
            session_id: "session-review-approve",
            state: "done",
            current_round: 1,
            current_phase: "finalize",
            review_round: 1,
            status_reason: "",
            waiting_for_employee_id: "",
            waiting_for_user: false,
            final_report: "计划：共 3 步\n执行：已完成 3 步。\n汇报：团队协作已完成。",
            steps: [
              {
                id: "step-plan",
                round_no: 1,
                step_type: "plan",
                assignee_employee_id: "中书省",
                status: "completed",
                output: "",
              },
              {
                id: "step-execute",
                round_no: 1,
                step_type: "execute",
                assignee_employee_id: "兵部",
                status: "completed",
                output: "MOCK_RESPONSE",
              },
            ],
            events: [
              {
                id: "evt-approve",
                step_id: "step-review",
                event_type: "review_passed",
                payload_json: "{\"comment\":\"方案通过\"}",
                created_at: "2026-03-07T00:02:00Z",
              },
            ],
          });
        }
        return Promise.resolve({
          run_id: "run-review-approve",
          group_id: "group-review-approve",
          session_id: "session-review-approve",
          state: "waiting_review",
          current_round: 1,
          current_phase: "review",
          review_round: 1,
          status_reason: "等待门下省审议",
          waiting_for_employee_id: "门下省",
          waiting_for_user: false,
          final_report: "计划：共 3 步",
          steps: [
            {
              id: "step-plan",
              round_no: 1,
              step_type: "plan",
              assignee_employee_id: "中书省",
              status: "completed",
              output: "",
            },
            {
              id: "step-review",
              round_no: 1,
              step_type: "review",
              assignee_employee_id: "门下省",
              status: "pending",
              output: "",
            },
          ],
          events: [],
        });
      }
      if (command === "review_group_run_step") {
        expect(payload).toEqual({
          runId: "run-review-approve",
          action: "approve",
          comment: "前端确认通过",
        });
        return Promise.resolve(null);
      }
      if (command === "continue_employee_group_run") {
        expect(payload).toEqual({ runId: "run-review-approve" });
        snapshotState = "done";
        return Promise.resolve({
          run_id: "run-review-approve",
          group_id: "group-review-approve",
          session_id: "session-review-approve",
          state: "done",
          current_round: 1,
          current_phase: "finalize",
          review_round: 1,
          status_reason: "",
          waiting_for_employee_id: "",
          waiting_for_user: false,
          final_report: "计划：共 3 步\n执行：已完成 3 步。\n汇报：团队协作已完成。",
          steps: [
            {
              id: "step-plan",
              round_no: 1,
              step_type: "plan",
              assignee_employee_id: "中书省",
              status: "completed",
              output: "",
            },
            {
              id: "step-execute",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "兵部",
              status: "completed",
              output: "MOCK_RESPONSE",
            },
          ],
          events: [],
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
        sessionId="session-review-approve"
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("阶段：审核");
      expect(screen.getByTestId("group-run-review-approve")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("group-run-review-approve"));

    await waitFor(() => {
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("阶段：汇报");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("兵部");
      expect(invokeMock).toHaveBeenCalledWith("review_group_run_step", {
        runId: "run-review-approve",
        action: "approve",
        comment: "前端确认通过",
      });
      expect(invokeMock).toHaveBeenCalledWith("continue_employee_group_run", {
        runId: "run-review-approve",
      });
    });
  });

  test("rejects pending review and restarts review cycle from orchestration board", async () => {
    let snapshotState = "waiting_review";
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "get_employee_group_run_snapshot") {
        if (snapshotState === "re_review") {
          return Promise.resolve({
            run_id: "run-review-reject",
            group_id: "group-review-reject",
            session_id: "session-review-reject",
            state: "waiting_review",
            current_round: 1,
            current_phase: "review",
            review_round: 2,
            status_reason: "等待门下省复审",
            waiting_for_employee_id: "门下省",
            waiting_for_user: false,
            final_report: "计划：已按意见补充回滚方案",
            steps: [
              {
                id: "step-plan-1",
                round_no: 1,
                step_type: "plan",
                assignee_employee_id: "中书省",
                status: "completed",
                output: "已按缺少回滚方案补充计划",
              },
              {
                id: "step-review-2",
                round_no: 1,
                step_type: "review",
                assignee_employee_id: "门下省",
                status: "pending",
                output: "",
              },
            ],
            events: [
              {
                id: "evt-review-rejected",
                step_id: "step-review-1",
                event_type: "review_rejected",
                payload_json: "{\"reason\":\"缺少回滚方案\",\"review_round\":2}",
                created_at: "2026-03-07T00:03:00Z",
              },
            ],
          });
        }
        return Promise.resolve({
          run_id: "run-review-reject",
          group_id: "group-review-reject",
          session_id: "session-review-reject",
          state: "waiting_review",
          current_round: 1,
          current_phase: "review",
          review_round: 1,
          status_reason: "等待门下省审议",
          waiting_for_employee_id: "门下省",
          waiting_for_user: false,
          final_report: "计划：共 3 步",
          steps: [
            {
              id: "step-plan-1",
              round_no: 1,
              step_type: "plan",
              assignee_employee_id: "中书省",
              status: "completed",
              output: "",
            },
            {
              id: "step-review-1",
              round_no: 1,
              step_type: "review",
              assignee_employee_id: "门下省",
              status: "pending",
              output: "",
            },
          ],
          events: [],
        });
      }
      if (command === "review_group_run_step") {
        expect(payload).toEqual({
          runId: "run-review-reject",
          action: "reject",
          comment: "前端要求补充方案",
        });
        return Promise.resolve(null);
      }
      if (command === "continue_employee_group_run") {
        expect(payload).toEqual({ runId: "run-review-reject" });
        snapshotState = "re_review";
        return Promise.resolve({
          run_id: "run-review-reject",
          group_id: "group-review-reject",
          session_id: "session-review-reject",
          state: "waiting_review",
          current_round: 1,
          current_phase: "review",
          review_round: 2,
          status_reason: "等待门下省复审",
          waiting_for_employee_id: "门下省",
          waiting_for_user: false,
          final_report: "计划：已按意见补充回滚方案",
          steps: [
            {
              id: "step-plan-1",
              round_no: 1,
              step_type: "plan",
              assignee_employee_id: "中书省",
              status: "completed",
              output: "已按缺少回滚方案补充计划",
            },
            {
              id: "step-review-2",
              round_no: 1,
              step_type: "review",
              assignee_employee_id: "门下省",
              status: "pending",
              output: "",
            },
          ],
          events: [
            {
              id: "evt-review-rejected",
              step_id: "step-review-1",
              event_type: "review_rejected",
              payload_json: "{\"reason\":\"缺少回滚方案\",\"review_round\":2}",
              created_at: "2026-03-07T00:03:00Z",
            },
          ],
        });
      }
      if (command === "get_model_configs") return Promise.resolve([]);
      if (command === "get_session_runtime_bindings") return Promise.resolve(null);
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
        sessionId="session-review-reject"
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("阶段：审核");
      expect(screen.getByTestId("group-run-review-reject")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("group-run-review-reject"));

    await waitFor(() => {
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("阶段：审核");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("审议轮次：2");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("等待门下省复审");
      expect(invokeMock).toHaveBeenCalledWith("review_group_run_step", {
        runId: "run-review-reject",
        action: "reject",
        comment: "前端要求补充方案",
      });
      expect(invokeMock).toHaveBeenCalledWith("continue_employee_group_run", {
        runId: "run-review-reject",
      });
    });
  });

  test("pauses and resumes group run from orchestration board", async () => {
    let snapshotState = "running";
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "get_employee_group_run_snapshot") {
        if (snapshotState === "paused") {
          expect(payload).toEqual({ sessionId: "session-run-control" });
          return Promise.resolve({
            run_id: "run-control-1",
            group_id: "group-control-1",
            session_id: "session-run-control",
            state: "paused",
            current_round: 1,
            current_phase: "execute",
            review_round: 0,
            status_reason: "前端人工暂停",
            waiting_for_employee_id: "兵部",
            waiting_for_user: false,
            final_report: "计划：共 3 步",
            steps: [
              {
                id: "step-execute-running",
                round_no: 1,
                step_type: "execute",
                assignee_employee_id: "兵部",
                status: "running",
                output: "",
              },
            ],
            events: [],
          });
        }
        return Promise.resolve({
          run_id: "run-control-1",
          group_id: "group-control-1",
          session_id: "session-run-control",
          state: "executing",
          current_round: 1,
          current_phase: "execute",
          review_round: 0,
          status_reason: "",
          waiting_for_employee_id: "兵部",
          waiting_for_user: false,
          final_report: "计划：共 3 步",
          steps: [
            {
              id: "step-execute-running",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "兵部",
              status: "running",
              output: "",
            },
          ],
          events: [],
        });
      }
      if (command === "pause_employee_group_run") {
        expect(payload).toEqual({
          runId: "run-control-1",
          reason: "前端人工暂停",
        });
        snapshotState = "paused";
        return Promise.resolve(null);
      }
      if (command === "resume_employee_group_run") {
        expect(payload).toEqual({ runId: "run-control-1" });
        return Promise.resolve(null);
      }
      if (command === "continue_employee_group_run") {
        expect(payload).toEqual({ runId: "run-control-1" });
        snapshotState = "resumed";
        return Promise.resolve({
          run_id: "run-control-1",
          group_id: "group-control-1",
          session_id: "session-run-control",
          state: "done",
          current_round: 1,
          current_phase: "finalize",
          review_round: 0,
          status_reason: "",
          waiting_for_employee_id: "",
          waiting_for_user: false,
          final_report: "计划：共 3 步\n执行：已完成 3 步。\n汇报：团队协作已完成。",
          steps: [
            {
              id: "step-execute-running",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "兵部",
              status: "completed",
              output: "MOCK_RESPONSE",
            },
          ],
          events: [
            {
              id: "evt-run-resumed",
              step_id: "",
              event_type: "run_resumed",
              payload_json: "{\"state\":\"executing\",\"phase\":\"execute\"}",
              created_at: "2026-03-07T00:05:00Z",
            },
          ],
        });
      }
      if (command === "get_model_configs") return Promise.resolve([]);
      if (command === "get_session_runtime_bindings") return Promise.resolve(null);
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
        sessionId="session-run-control"
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("group-run-pause")).toBeInTheDocument();
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("阶段：执行");
    });

    fireEvent.click(screen.getByTestId("group-run-pause"));

    await waitFor(() => {
      expect(screen.getByTestId("group-run-resume")).toBeInTheDocument();
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("阶段：已暂停");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("前端人工暂停");
    });

    fireEvent.click(screen.getByTestId("group-run-resume"));

    await waitFor(() => {
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("阶段：汇报");
      expect(invokeMock).toHaveBeenCalledWith("pause_employee_group_run", {
        runId: "run-control-1",
        reason: "前端人工暂停",
      });
      expect(invokeMock).toHaveBeenCalledWith("resume_employee_group_run", {
        runId: "run-control-1",
      });
      expect(invokeMock).toHaveBeenCalledWith("continue_employee_group_run", {
        runId: "run-control-1",
      });
    });
  });

  test("retries failed group run steps from orchestration board", async () => {
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "get_employee_group_run_snapshot") {
        if (payload?.sessionId === "session-run-retry" && invokeMock.mock.calls.some(([name]) => name === "retry_employee_group_run_failed_steps")) {
          return Promise.resolve({
            run_id: "run-retry-1",
            group_id: "group-retry-1",
            session_id: "session-run-retry",
            state: "done",
            current_round: 2,
            current_phase: "finalize",
            review_round: 0,
            status_reason: "",
            waiting_for_employee_id: "",
            waiting_for_user: false,
            final_report: "计划：共 3 步\n执行：失败步骤已重试完成。\n汇报：团队协作已完成。",
            steps: [
              {
                id: "step-failed-1",
                round_no: 1,
                step_type: "execute",
                assignee_employee_id: "兵部",
                status: "completed",
                output: "重试后完成",
              },
            ],
            events: [
              {
                id: "evt-retry",
                step_id: "step-failed-1",
                event_type: "step_completed",
                payload_json: "{}",
                created_at: "2026-03-07T00:06:00Z",
              },
            ],
          });
        }
        return Promise.resolve({
          run_id: "run-retry-1",
          group_id: "group-retry-1",
          session_id: "session-run-retry",
          state: "failed",
          current_round: 1,
          current_phase: "execute",
          review_round: 0,
          status_reason: "兵部执行失败",
          waiting_for_employee_id: "兵部",
          waiting_for_user: false,
          final_report: "计划：共 3 步",
          steps: [
            {
              id: "step-failed-1",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "兵部",
              status: "failed",
              output: "超时",
            },
          ],
          events: [],
        });
      }
      if (command === "retry_employee_group_run_failed_steps") {
        expect(payload).toEqual({ runId: "run-retry-1" });
        return Promise.resolve(null);
      }
      if (command === "get_model_configs") return Promise.resolve([]);
      if (command === "get_session_runtime_bindings") return Promise.resolve(null);
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
        sessionId="session-run-retry"
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("group-run-retry-failed")).toBeInTheDocument();
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("兵部执行失败");
    });

    fireEvent.click(screen.getByTestId("group-run-retry-failed"));

    await waitFor(() => {
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("阶段：汇报");
      expect(invokeMock).toHaveBeenCalledWith("retry_employee_group_run_failed_steps", {
        runId: "run-retry-1",
      });
    });
  });

  test("reassigns failed step to selected candidate from orchestration board", async () => {
    let snapshotState = "failed";
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "get_employee_group_run_snapshot") {
        if (snapshotState === "done") {
          return Promise.resolve({
            run_id: "run-reassign-1",
            group_id: "group-reassign-1",
            session_id: "session-run-reassign",
            state: "done",
            current_round: 1,
            current_phase: "finalize",
            review_round: 0,
            status_reason: "",
            waiting_for_employee_id: "",
            waiting_for_user: false,
            final_report: "计划：共 3 步\n执行：改派后完成。\n汇报：团队协作已完成。",
            steps: [
              {
                id: "step-failed-reassign",
                round_no: 1,
                step_type: "execute",
                assignee_employee_id: "工部",
                session_id: "session-step-gongbu",
                attempt_no: 2,
                status: "completed",
                output_summary: "已输出改派后的执行结果",
                output: "MOCK_RESPONSE",
              },
              {
                id: "step-other-member",
                round_no: 1,
                step_type: "execute",
                assignee_employee_id: "兵部",
                status: "completed",
                output: "",
              },
              {
                id: "step-other-member-2",
                round_no: 1,
                step_type: "execute",
                assignee_employee_id: "礼部",
                status: "completed",
                output: "",
              },
            ],
            events: [
              {
                id: "evt-reassign",
                step_id: "step-failed-reassign",
                event_type: "step_reassigned",
                payload_json:
                  "{\"assignee_employee_id\":\"工部\",\"dispatch_source_employee_id\":\"门下\",\"previous_assignee_employee_id\":\"兵部\",\"previous_output_summary\":\"超时\"}",
                created_at: "2026-03-07T00:07:00Z",
              },
              {
                id: "evt-step-completed",
                step_id: "step-failed-reassign",
                event_type: "step_completed",
                payload_json:
                  "{\"assignee_employee_id\":\"工部\",\"dispatch_source_employee_id\":\"门下\",\"session_id\":\"session-step-gongbu\",\"output_summary\":\"已输出改派后的执行结果\"}",
                created_at: "2026-03-07T00:08:00Z",
              },
            ],
          });
        }
        return Promise.resolve({
          run_id: "run-reassign-1",
          group_id: "group-reassign-1",
          session_id: "session-run-reassign",
          state: "failed",
          current_round: 1,
          current_phase: "execute",
          review_round: 0,
          status_reason: "兵部执行失败",
          waiting_for_employee_id: "兵部",
          waiting_for_user: false,
          final_report: "计划：共 3 步",
          steps: [
            {
              id: "step-failed-reassign",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "兵部",
              attempt_no: 1,
              status: "failed",
              output_summary: "超时",
              output: "超时",
            },
            {
              id: "step-other-member",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "礼部",
              status: "pending",
              output: "",
            },
            {
              id: "step-other-member-2",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "工部",
              status: "pending",
              output: "",
            },
          ],
          events: [],
        });
      }
      if (command === "reassign_group_run_step") {
        expect(payload).toEqual({
          stepId: "step-failed-reassign",
          assigneeEmployeeId: "工部",
        });
        return Promise.resolve(null);
      }
      if (command === "continue_employee_group_run") {
        expect(payload).toEqual({ runId: "run-reassign-1" });
        snapshotState = "done";
        return Promise.resolve({
          run_id: "run-reassign-1",
          group_id: "group-reassign-1",
          session_id: "session-run-reassign",
          state: "done",
          current_round: 1,
          current_phase: "finalize",
          review_round: 0,
          status_reason: "",
          waiting_for_employee_id: "",
          waiting_for_user: false,
          final_report: "计划：共 3 步\n执行：改派后完成。\n汇报：团队协作已完成。",
          steps: [
              {
                id: "step-failed-reassign",
                round_no: 1,
                step_type: "execute",
                assignee_employee_id: "工部",
                session_id: "session-step-gongbu",
                attempt_no: 2,
                status: "completed",
                output_summary: "已输出改派后的执行结果",
                output: "MOCK_RESPONSE",
              },
            {
              id: "step-other-member",
              round_no: 1,
              step_type: "execute",
                assignee_employee_id: "兵部",
                status: "completed",
                output: "",
              },
              {
                id: "step-other-member-2",
                round_no: 1,
                step_type: "execute",
                assignee_employee_id: "礼部",
                status: "completed",
                output: "",
              },
            ],
            events: [
              {
                id: "evt-reassign",
                step_id: "step-failed-reassign",
                event_type: "step_reassigned",
                payload_json:
                  "{\"assignee_employee_id\":\"工部\",\"dispatch_source_employee_id\":\"门下\",\"previous_assignee_employee_id\":\"兵部\",\"previous_output_summary\":\"超时\"}",
                created_at: "2026-03-07T00:07:00Z",
              },
              {
                id: "evt-step-completed",
                step_id: "step-failed-reassign",
                event_type: "step_completed",
                payload_json:
                  "{\"assignee_employee_id\":\"工部\",\"dispatch_source_employee_id\":\"门下\",\"session_id\":\"session-step-gongbu\",\"output_summary\":\"已输出改派后的执行结果\"}",
                created_at: "2026-03-07T00:08:00Z",
              },
            ],
        });
      }
      if (command === "get_model_configs") return Promise.resolve([]);
      if (command === "get_session_runtime_bindings") return Promise.resolve(null);
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
        sessionId="session-run-reassign"
      />
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "改派给礼部" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "改派给工部" })).toBeInTheDocument();
      expect(screen.getByTestId("group-run-step-card-step-failed-reassign")).toHaveTextContent(
        "当前状态：失败",
      );
      expect(screen.getByTestId("group-run-step-card-step-failed-reassign")).toHaveTextContent(
        "尝试次数：1",
      );
      expect(screen.getByTestId("group-run-step-card-step-failed-reassign")).toHaveTextContent(
        "最近失败：超时",
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "改派给工部" }));

    await waitFor(() => {
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("阶段：汇报");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("工部");
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent(
        "step_reassigned · 门下 -> 工部",
      );
      expect(screen.getByTestId("group-run-step-card-step-failed-reassign")).toHaveTextContent(
        "当前负责人：工部",
      );
      expect(screen.getByTestId("group-run-step-card-step-failed-reassign")).toHaveTextContent(
        "来源人：门下",
      );
      expect(screen.getByTestId("group-run-step-card-step-failed-reassign")).toHaveTextContent(
        "原负责人：兵部",
      );
      expect(screen.getByTestId("group-run-step-card-step-failed-reassign")).toHaveTextContent(
        "当前状态：已完成",
      );
      expect(screen.getByTestId("group-run-step-card-step-failed-reassign")).toHaveTextContent(
        "尝试次数：2",
      );
      expect(screen.getByTestId("group-run-step-card-step-failed-reassign")).toHaveTextContent(
        "最近失败：超时",
      );
      expect(invokeMock).toHaveBeenCalledWith("reassign_group_run_step", {
        stepId: "step-failed-reassign",
        assigneeEmployeeId: "工部",
      });
      expect(invokeMock).toHaveBeenCalledWith("continue_employee_group_run", {
        runId: "run-reassign-1",
      });
    });

    expect(
      screen.queryByTestId("group-run-step-card-step-failed-reassign-details"),
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("group-run-step-card-step-failed-reassign-toggle"));

    await waitFor(() => {
      expect(
        screen.getByTestId("group-run-step-card-step-failed-reassign-details"),
      ).toHaveTextContent("session_id：session-step-gongbu");
      expect(
        screen.getByTestId("group-run-step-card-step-failed-reassign-details"),
      ).toHaveTextContent("输出摘要：已输出改派后的执行结果");
      expect(
        screen.getByTestId("group-run-step-card-step-failed-reassign-details"),
      ).toHaveTextContent("最近事件时间：2026-03-07T00:08:00Z");
    });
  });

  test("filters reassign candidates to execute-eligible group members", async () => {
    let snapshotState = "failed";
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_employee_groups") {
        return Promise.resolve([
          {
            id: "group-reassign-members-1",
            name: "协作团队",
            coordinator_employee_id: "尚书",
            member_employee_ids: ["兵部", "礼部", "工部", "户部"],
            member_count: 4,
            template_id: "sansheng-liubu",
            entry_employee_id: "尚书",
            review_mode: "hard",
            execution_mode: "parallel",
            visibility_mode: "internal",
            is_bootstrap_seeded: true,
            config_json: "{}",
            created_at: "2026-03-07T00:00:00Z",
            updated_at: "2026-03-07T00:00:00Z",
          },
        ]);
      }
      if (command === "list_employee_group_rules") {
        expect(payload).toEqual({ groupId: "group-reassign-members-1" });
        return Promise.resolve([
          {
            id: "rule-execute-gongbu",
            group_id: "group-reassign-members-1",
            from_employee_id: "尚书",
            to_employee_id: "工部",
            relation_type: "delegate",
            phase_scope: "execute",
            required: false,
            priority: 10,
            created_at: "2026-03-07T00:00:00Z",
          },
          {
            id: "rule-execute-hubu",
            group_id: "group-reassign-members-1",
            from_employee_id: "尚书",
            to_employee_id: "户部",
            relation_type: "delegate",
            phase_scope: "execute",
            required: false,
            priority: 20,
            created_at: "2026-03-07T00:00:00Z",
          },
          {
            id: "rule-execute-libu-other-source",
            group_id: "group-reassign-members-1",
            from_employee_id: "门下",
            to_employee_id: "礼部",
            relation_type: "delegate",
            phase_scope: "execute",
            required: false,
            priority: 30,
            created_at: "2026-03-07T00:00:00Z",
          },
        ]);
      }
      if (command === "get_employee_group_run_snapshot") {
        if (snapshotState === "done") {
          return Promise.resolve({
            run_id: "run-reassign-members-1",
            group_id: "group-reassign-members-1",
            session_id: "session-run-reassign-members",
            state: "done",
            current_round: 1,
            current_phase: "finalize",
            review_round: 0,
            status_reason: "",
            waiting_for_employee_id: "",
            waiting_for_user: false,
            final_report: "计划：共 3 步\n执行：改派后完成。\n汇报：团队协作已完成。",
            steps: [
              {
                id: "step-failed-reassign-members",
                round_no: 1,
                step_type: "execute",
                assignee_employee_id: "户部",
                status: "completed",
                output: "MOCK_RESPONSE",
              },
              {
                id: "step-other-member-members",
                round_no: 1,
                step_type: "execute",
                assignee_employee_id: "工部",
                status: "completed",
                output: "",
              },
            ],
            events: [
              {
                id: "evt-reassign-members",
                step_id: "step-failed-reassign-members",
                event_type: "step_reassigned",
                payload_json: "{\"assignee_employee_id\":\"户部\"}",
                created_at: "2026-03-07T00:07:00Z",
              },
            ],
          });
        }
        return Promise.resolve({
          run_id: "run-reassign-members-1",
          group_id: "group-reassign-members-1",
          session_id: "session-run-reassign-members",
          state: "failed",
          current_round: 1,
          current_phase: "execute",
          review_round: 0,
          status_reason: "兵部执行失败",
          waiting_for_employee_id: "兵部",
          waiting_for_user: false,
          final_report: "计划：共 3 步",
          steps: [
            {
              id: "step-failed-reassign-members",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "兵部",
              status: "failed",
              output: "超时",
            },
            {
              id: "step-other-member-members",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "工部",
              status: "pending",
              output: "",
            },
          ],
          events: [],
        });
      }
      if (command === "reassign_group_run_step") {
        expect(payload).toEqual({
          stepId: "step-failed-reassign-members",
          assigneeEmployeeId: "户部",
        });
        return Promise.resolve(null);
      }
      if (command === "continue_employee_group_run") {
        expect(payload).toEqual({ runId: "run-reassign-members-1" });
        snapshotState = "done";
        return Promise.resolve({
          run_id: "run-reassign-members-1",
          group_id: "group-reassign-members-1",
          session_id: "session-run-reassign-members",
          state: "done",
          current_round: 1,
          current_phase: "finalize",
          review_round: 0,
          status_reason: "",
          waiting_for_employee_id: "",
          waiting_for_user: false,
          final_report: "计划：共 3 步\n执行：改派后完成。\n汇报：团队协作已完成。",
          steps: [
            {
              id: "step-failed-reassign-members",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "户部",
              status: "completed",
              output: "MOCK_RESPONSE",
            },
            {
              id: "step-other-member-members",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "工部",
              status: "completed",
              output: "",
            },
          ],
          events: [
            {
              id: "evt-reassign-members",
              step_id: "step-failed-reassign-members",
              event_type: "step_reassigned",
              payload_json: "{\"assignee_employee_id\":\"户部\"}",
              created_at: "2026-03-07T00:07:00Z",
            },
          ],
        });
      }
      if (command === "get_model_configs") return Promise.resolve([]);
      if (command === "get_session_runtime_bindings") return Promise.resolve(null);
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
        sessionId="session-run-reassign-members"
      />
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "改派给户部" })).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "改派给礼部" })).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "改派给尚书" })).not.toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "改派给户部" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("reassign_group_run_step", {
        stepId: "step-failed-reassign-members",
        assigneeEmployeeId: "户部",
      });
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("户部");
    });
  });

  test("prefers failed step dispatch source over coordinator for reassign candidates", async () => {
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "list_employee_groups") {
        return Promise.resolve([
          {
            id: "group-reassign-source-1",
            name: "协作团队",
            coordinator_employee_id: "尚书",
            member_employee_ids: ["兵部", "礼部", "工部", "户部"],
            member_count: 4,
            template_id: "sansheng-liubu",
            entry_employee_id: "尚书",
            review_mode: "hard",
            execution_mode: "parallel",
            visibility_mode: "internal",
            is_bootstrap_seeded: true,
            config_json: "{}",
            created_at: "2026-03-07T00:00:00Z",
            updated_at: "2026-03-07T00:00:00Z",
          },
        ]);
      }
      if (command === "list_employee_group_rules") {
        expect(payload).toEqual({ groupId: "group-reassign-source-1" });
        return Promise.resolve([
          {
            id: "rule-source-shangshu-gongbu",
            group_id: "group-reassign-source-1",
            from_employee_id: "尚书",
            to_employee_id: "工部",
            relation_type: "delegate",
            phase_scope: "execute",
            required: false,
            priority: 10,
            created_at: "2026-03-07T00:00:00Z",
          },
          {
            id: "rule-source-shangshu-hubu",
            group_id: "group-reassign-source-1",
            from_employee_id: "尚书",
            to_employee_id: "户部",
            relation_type: "delegate",
            phase_scope: "execute",
            required: false,
            priority: 20,
            created_at: "2026-03-07T00:00:00Z",
          },
          {
            id: "rule-source-menxia-libu",
            group_id: "group-reassign-source-1",
            from_employee_id: "门下",
            to_employee_id: "礼部",
            relation_type: "delegate",
            phase_scope: "execute",
            required: false,
            priority: 30,
            created_at: "2026-03-07T00:00:00Z",
          },
        ]);
      }
      if (command === "get_employee_group_run_snapshot") {
        return Promise.resolve({
          run_id: "run-reassign-source-1",
          group_id: "group-reassign-source-1",
          session_id: "session-run-reassign-source",
          state: "failed",
          current_round: 1,
          current_phase: "execute",
          review_round: 0,
          status_reason: "兵部执行失败",
          waiting_for_employee_id: "兵部",
          waiting_for_user: false,
          final_report: "计划：共 3 步",
          steps: [
            {
              id: "step-failed-reassign-source",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "兵部",
              dispatch_source_employee_id: "门下",
              status: "failed",
              output: "超时",
            },
          ],
          events: [],
        });
      }
      if (command === "get_model_configs") return Promise.resolve([]);
      if (command === "get_session_runtime_bindings") return Promise.resolve(null);
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
        sessionId="session-run-reassign-source"
      />
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "改派给礼部" })).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "改派给工部" })).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "改派给户部" })).not.toBeInTheDocument();
      expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("来源：门下");
    });
  });

  test("reassigns a specific failed step from multiple failed steps", async () => {
    let snapshotState = "failed";
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "get_employee_group_run_snapshot") {
        if (snapshotState === "reassigned") {
          return Promise.resolve({
            run_id: "run-reassign-multi-1",
            group_id: "group-reassign-multi-1",
            session_id: "session-run-reassign-multi",
            state: "failed",
            current_round: 1,
            current_phase: "execute",
            review_round: 0,
            status_reason: "兵部执行失败",
            waiting_for_employee_id: "兵部",
            waiting_for_user: false,
            final_report: "计划：共 4 步",
            steps: [
              {
                id: "step-failed-reassign-1",
                round_no: 1,
                step_type: "execute",
                assignee_employee_id: "兵部",
                status: "failed",
                output: "超时",
              },
              {
                id: "step-failed-reassign-2",
                round_no: 1,
                step_type: "execute",
                assignee_employee_id: "户部",
                status: "pending",
                output: "",
              },
              {
                id: "step-other-member",
                round_no: 1,
                step_type: "execute",
                assignee_employee_id: "礼部",
                status: "pending",
                output: "",
              },
              {
                id: "step-other-member-2",
                round_no: 1,
                step_type: "execute",
                assignee_employee_id: "工部",
                status: "pending",
                output: "",
              },
            ],
            events: [
              {
                id: "evt-reassign-multi",
                step_id: "step-failed-reassign-2",
                event_type: "step_reassigned",
                payload_json: "{\"assignee_employee_id\":\"户部\"}",
                created_at: "2026-03-07T00:08:00Z",
              },
            ],
          });
        }
        return Promise.resolve({
          run_id: "run-reassign-multi-1",
          group_id: "group-reassign-multi-1",
          session_id: "session-run-reassign-multi",
          state: "failed",
          current_round: 1,
          current_phase: "execute",
          review_round: 0,
          status_reason: "兵部、礼部执行失败",
          waiting_for_employee_id: "兵部",
          waiting_for_user: false,
          final_report: "计划：共 4 步",
          steps: [
            {
              id: "step-failed-reassign-1",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "兵部",
              status: "failed",
              output: "超时",
            },
            {
              id: "step-failed-reassign-2",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "礼部",
              status: "failed",
              output: "资料缺失",
            },
            {
              id: "step-other-member",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "工部",
              status: "pending",
              output: "",
            },
            {
              id: "step-other-member-2",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "户部",
              status: "pending",
              output: "",
            },
          ],
          events: [],
        });
      }
      if (command === "reassign_group_run_step") {
        expect(payload).toEqual({
          stepId: "step-failed-reassign-2",
          assigneeEmployeeId: "户部",
        });
        return Promise.resolve(null);
      }
      if (command === "continue_employee_group_run") {
        expect(payload).toEqual({ runId: "run-reassign-multi-1" });
        snapshotState = "reassigned";
        return Promise.resolve({
          run_id: "run-reassign-multi-1",
          group_id: "group-reassign-multi-1",
          session_id: "session-run-reassign-multi",
          state: "failed",
          current_round: 1,
          current_phase: "execute",
          review_round: 0,
          status_reason: "兵部执行失败",
          waiting_for_employee_id: "兵部",
          waiting_for_user: false,
          final_report: "计划：共 4 步",
          steps: [
            {
              id: "step-failed-reassign-1",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "兵部",
              status: "failed",
              output: "超时",
            },
            {
              id: "step-failed-reassign-2",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "户部",
              status: "pending",
              output: "",
            },
            {
              id: "step-other-member",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "工部",
              status: "pending",
              output: "",
            },
            {
              id: "step-other-member-2",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "礼部",
              status: "pending",
              output: "",
            },
          ],
          events: [
            {
              id: "evt-reassign-multi",
              step_id: "step-failed-reassign-2",
              event_type: "step_reassigned",
              payload_json: "{\"assignee_employee_id\":\"户部\"}",
              created_at: "2026-03-07T00:08:00Z",
            },
          ],
        });
      }
      if (command === "get_model_configs") return Promise.resolve([]);
      if (command === "get_session_runtime_bindings") return Promise.resolve(null);
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
        sessionId="session-run-reassign-multi"
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("group-run-reassign-step-failed-reassign-1-礼部")).toBeInTheDocument();
      expect(screen.getByTestId("group-run-reassign-step-failed-reassign-2-户部")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("group-run-reassign-step-failed-reassign-2-户部"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("reassign_group_run_step", {
        stepId: "step-failed-reassign-2",
        assigneeEmployeeId: "户部",
      });
      expect(invokeMock).toHaveBeenCalledWith("continue_employee_group_run", {
        runId: "run-reassign-multi-1",
      });
    });
  });

  test("opens the execution session from step details when session id is available", async () => {
    const handleOpenSession = vi.fn();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "get_employee_group_run_snapshot") {
        return Promise.resolve({
          run_id: "run-open-step-session-1",
          group_id: "group-open-step-session-1",
          session_id: "session-run-open-step",
          state: "executing",
          current_round: 1,
          current_phase: "execute",
          review_round: 0,
          status_reason: "",
          waiting_for_employee_id: "工部",
          waiting_for_user: false,
          final_report: "计划：共 2 步",
          steps: [
            {
              id: "step-open-session-1",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "工部",
              dispatch_source_employee_id: "尚书",
              status: "running",
              output: "正在整理交付清单",
              session_id: "session-step-gongbu-1",
            },
          ],
          events: [
            {
              id: "evt-open-session-1",
              step_id: "step-open-session-1",
              event_type: "step_created",
              payload_json: "{\"assignee_employee_id\":\"工部\",\"dispatch_source_employee_id\":\"尚书\"}",
              created_at: "2026-03-07T00:59:00Z",
            },
            {
              id: "evt-open-session-2",
              step_id: "step-open-session-1",
              event_type: "step_dispatched",
              payload_json: "{\"assignee_employee_id\":\"工部\",\"dispatch_source_employee_id\":\"尚书\"}",
              created_at: "2026-03-07T01:00:00Z",
            },
          ],
        });
      }
      if (command === "get_model_configs") return Promise.resolve([]);
      if (command === "get_session_runtime_bindings") return Promise.resolve(null);
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
        sessionId="session-run-open-step"
        onOpenSession={handleOpenSession}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("group-run-step-card-step-open-session-1")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("group-run-step-card-step-open-session-1-toggle"));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "查看执行会话" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "查看执行会话" }));

    expect(handleOpenSession).toHaveBeenCalledWith("session-step-gongbu-1", {
      focusHint: "正在整理交付清单",
      sourceSessionId: "session-run-open-step",
      sourceStepId: "step-open-session-1",
      sourceEmployeeId: "尚书",
      assigneeEmployeeId: "工部",
      sourceStepTimeline: [
        {
          eventId: "evt-open-session-1",
          linkedSessionId: "session-step-gongbu-1",
          label: "step_created · 尚书 -> 工部",
          createdAt: "2026-03-07T00:59:00Z",
        },
        {
          eventId: "evt-open-session-2",
          linkedSessionId: "session-step-gongbu-1",
          label: "step_dispatched · 尚书 -> 工部",
          createdAt: "2026-03-07T01:00:00Z",
        },
      ],
    });
  });

  test("opens the linked execution session from a step event item when available", async () => {
    const handleOpenSession = vi.fn();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "get_employee_group_run_snapshot") {
        return Promise.resolve({
          run_id: "run-open-step-event-session-1",
          group_id: "group-open-step-event-session-1",
          session_id: "session-run-open-step",
          state: "executing",
          current_round: 1,
          current_phase: "execute",
          review_round: 0,
          status_reason: "",
          waiting_for_employee_id: "工部",
          waiting_for_user: false,
          final_report: "计划：共 2 步",
          steps: [
            {
              id: "step-open-session-1",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "工部",
              dispatch_source_employee_id: "尚书",
              status: "running",
              output: "正在整理交付清单",
              session_id: "session-step-gongbu-1",
            },
          ],
          events: [
            {
              id: "evt-open-session-1",
              step_id: "step-open-session-1",
              event_type: "step_created",
              payload_json:
                "{\"assignee_employee_id\":\"工部\",\"dispatch_source_employee_id\":\"尚书\",\"session_id\":\"session-step-gongbu-1\"}",
              created_at: "2026-03-07T00:59:00Z",
            },
            {
              id: "evt-open-session-2",
              step_id: "step-open-session-1",
              event_type: "step_dispatched",
              payload_json:
                "{\"assignee_employee_id\":\"工部\",\"dispatch_source_employee_id\":\"尚书\",\"session_id\":\"session-step-gongbu-1\"}",
              created_at: "2026-03-07T01:00:00Z",
            },
          ],
        });
      }
      if (command === "get_model_configs") return Promise.resolve([]);
      if (command === "get_session_runtime_bindings") return Promise.resolve(null);
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
        sessionId="session-run-open-step"
        onOpenSession={handleOpenSession}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("group-run-step-card-step-open-session-1")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("group-run-step-card-step-open-session-1-toggle"));

    await waitFor(() => {
      expect(screen.getByTestId("group-run-step-card-step-open-session-1-event-evt-open-session-2")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("group-run-step-card-step-open-session-1-event-evt-open-session-2"));

    expect(handleOpenSession).toHaveBeenCalledWith("session-step-gongbu-1", {
      focusHint: "正在整理交付清单",
      sourceSessionId: "session-run-open-step",
      sourceStepId: "step-open-session-1",
      sourceEmployeeId: "尚书",
      assigneeEmployeeId: "工部",
      sourceStepTimeline: [
        {
          eventId: "evt-open-session-1",
          linkedSessionId: "session-step-gongbu-1",
          label: "step_created · 尚书 -> 工部",
          createdAt: "2026-03-07T00:59:00Z",
        },
        {
          eventId: "evt-open-session-2",
          linkedSessionId: "session-step-gongbu-1",
          label: "step_dispatched · 尚书 -> 工部",
          createdAt: "2026-03-07T01:00:00Z",
        },
      ],
    });
  });

  test("distinguishes session-linked events from log-only events in step details", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "get_employee_group_run_snapshot") {
        return Promise.resolve({
          run_id: "run-event-visual-1",
          group_id: "group-event-visual-1",
          session_id: "session-run-event-visual",
          state: "executing",
          current_round: 1,
          current_phase: "execute",
          review_round: 0,
          status_reason: "",
          waiting_for_employee_id: "工部",
          waiting_for_user: false,
          final_report: "计划：共 2 步",
          steps: [
            {
              id: "step-event-visual-1",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "工部",
              dispatch_source_employee_id: "尚书",
              status: "running",
              output: "正在整理交付清单",
            },
          ],
          events: [
            {
              id: "evt-linkable",
              step_id: "step-event-visual-1",
              event_type: "step_dispatched",
              payload_json:
                "{\"assignee_employee_id\":\"工部\",\"dispatch_source_employee_id\":\"尚书\",\"session_id\":\"session-step-gongbu-1\"}",
              created_at: "2026-03-07T01:00:00Z",
            },
            {
              id: "evt-log-only",
              step_id: "step-event-visual-1",
              event_type: "step_created",
              payload_json:
                "{\"assignee_employee_id\":\"工部\",\"dispatch_source_employee_id\":\"尚书\"}",
              created_at: "2026-03-07T00:59:00Z",
            },
          ],
        });
      }
      if (command === "get_model_configs") return Promise.resolve([]);
      if (command === "get_session_runtime_bindings") return Promise.resolve(null);
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
        sessionId="session-run-event-visual"
        onOpenSession={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("group-run-step-card-step-event-visual-1")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("group-run-step-card-step-event-visual-1-toggle"));

    await waitFor(() => {
      expect(screen.getByTestId("group-run-step-card-step-event-visual-1-event-evt-linkable")).toHaveAttribute(
        "data-group-run-step-event-linkable",
        "true",
      );
      expect(screen.getByTestId("group-run-step-card-step-event-visual-1-event-evt-linkable")).toHaveTextContent(
        "执行会话",
      );
      expect(screen.getByTestId("group-run-step-card-step-event-visual-1-event-evt-log-only")).toHaveAttribute(
        "data-group-run-step-event-linkable",
        "false",
      );
      expect(screen.getByTestId("group-run-step-card-step-event-visual-1-event-evt-log-only")).toHaveTextContent(
        "日志",
      );
    });
  });

  test("highlights the matched assistant message when a session focus request is provided", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") {
        return Promise.resolve([
          {
            role: "user",
            content: "请整理交付清单",
            created_at: "2026-03-07T01:10:00Z",
          },
          {
            role: "assistant",
            content: "已收到。正在整理交付清单，并补充执行明细。",
            created_at: "2026-03-07T01:10:10Z",
          },
          {
            role: "assistant",
            content: "这是另一条无关输出。",
            created_at: "2026-03-07T01:10:20Z",
          },
        ]);
      }
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "get_employee_group_run_snapshot") return Promise.resolve(null);
      if (command === "get_model_configs") return Promise.resolve([]);
      if (command === "get_session_runtime_bindings") return Promise.resolve(null);
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
        sessionId="session-focus-1"
        sessionFocusRequest={{ nonce: 1, snippet: "正在整理交付清单" }}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("chat-message-1")).toHaveAttribute("data-session-focus-highlighted", "true");
    });
  });

  test("shows execution session context bar and returns to the source session", async () => {
    const handleOpenSession = vi.fn();
    const handleReturnToSourceSession = vi.fn();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "get_employee_group_run_snapshot") return Promise.resolve(null);
      if (command === "get_model_configs") return Promise.resolve([]);
      if (command === "get_session_runtime_bindings") return Promise.resolve(null);
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
        sessionId="session-step-gongbu-1"
        sessionExecutionContext={{
          sourceSessionId: "session-run-open-step",
          sourceStepId: "step-open-session-1",
          sourceEmployeeId: "尚书",
          assigneeEmployeeId: "工部",
          sourceStepTimeline: [
            {
              eventId: "evt-open-session-1",
              label: "step_created · 尚书 -> 工部",
              createdAt: "2026-03-07T00:59:00Z",
            },
            {
              eventId: "evt-open-session-2",
              label: "step_dispatched · 尚书 -> 工部",
              createdAt: "2026-03-07T01:00:00Z",
            },
          ],
        }}
        onOpenSession={handleOpenSession}
        onReturnToSourceSession={handleReturnToSourceSession}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("chat-session-execution-context-bar")).toHaveTextContent("来源 step：step-open-session-1");
      expect(screen.getByTestId("chat-session-execution-context-bar")).toHaveTextContent("来源员工：尚书");
      expect(screen.getByTestId("chat-session-execution-context-bar")).toHaveTextContent("当前负责人：工部");
      expect(screen.getByRole("button", { name: "返回协作看板" })).toBeInTheDocument();
      expect(screen.getByTestId("chat-session-execution-context-timeline")).toHaveTextContent(
        "step_created · 尚书 -> 工部",
      );
      expect(screen.getByTestId("chat-session-execution-context-timeline")).toHaveTextContent(
        "step_dispatched · 尚书 -> 工部",
      );
    });

    fireEvent.click(screen.getByTestId("chat-session-execution-context-timeline-item-0"));

    expect(handleOpenSession).toHaveBeenCalledWith("session-run-open-step", {
      groupRunStepFocusId: "step-open-session-1",
      groupRunEventFocusId: "evt-open-session-1",
    });

    fireEvent.click(screen.getByRole("button", { name: "返回协作看板" }));

    expect(handleReturnToSourceSession).toHaveBeenCalledWith("session-run-open-step");
  });

  test("highlights the matching group run step card when a step focus request is provided", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "get_employee_group_run_snapshot") {
        return Promise.resolve({
          run_id: "run-focus-step-1",
          group_id: "group-focus-step-1",
          session_id: "session-run-focus-step",
          state: "executing",
          current_round: 1,
          current_phase: "execute",
          review_round: 0,
          status_reason: "",
          waiting_for_employee_id: "工部",
          waiting_for_user: false,
          final_report: "计划：共 2 步",
          steps: [
            {
              id: "step-open-session-1",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "工部",
              dispatch_source_employee_id: "尚书",
              status: "running",
              output: "正在整理交付清单",
              session_id: "session-step-gongbu-1",
            },
          ],
          events: [],
        });
      }
      if (command === "get_model_configs") return Promise.resolve([]);
      if (command === "get_session_runtime_bindings") return Promise.resolve(null);
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
        sessionId="session-run-focus-step"
        groupRunStepFocusRequest={{ nonce: 1, stepId: "step-open-session-1" } as any}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("group-run-step-card-step-open-session-1")).toHaveAttribute(
        "data-group-run-step-highlighted",
        "true",
      );
    });
  });

  test("expands the matching group run step details and highlights the requested event", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_messages") return Promise.resolve([]);
      if (command === "list_sessions") return Promise.resolve([]);
      if (command === "get_sessions") return Promise.resolve([]);
      if (command === "get_employee_group_run_snapshot") {
        return Promise.resolve({
          run_id: "run-focus-event-1",
          group_id: "group-focus-event-1",
          session_id: "session-run-focus-event",
          state: "executing",
          current_round: 1,
          current_phase: "execute",
          review_round: 0,
          status_reason: "",
          waiting_for_employee_id: "工部",
          waiting_for_user: false,
          final_report: "计划：共 2 步",
          steps: [
            {
              id: "step-open-session-1",
              round_no: 1,
              step_type: "execute",
              assignee_employee_id: "工部",
              dispatch_source_employee_id: "尚书",
              status: "running",
              output: "正在整理交付清单",
              session_id: "session-step-gongbu-1",
            },
          ],
          events: [
            {
              id: "evt-open-session-1",
              step_id: "step-open-session-1",
              event_type: "step_created",
              payload_json: "{\"assignee_employee_id\":\"工部\",\"dispatch_source_employee_id\":\"尚书\"}",
              created_at: "2026-03-07T00:59:00Z",
            },
            {
              id: "evt-open-session-2",
              step_id: "step-open-session-1",
              event_type: "step_dispatched",
              payload_json: "{\"assignee_employee_id\":\"工部\",\"dispatch_source_employee_id\":\"尚书\"}",
              created_at: "2026-03-07T01:00:00Z",
            },
          ],
        });
      }
      if (command === "get_model_configs") return Promise.resolve([]);
      if (command === "get_session_runtime_bindings") return Promise.resolve(null);
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
        sessionId="session-run-focus-event"
        groupRunStepFocusRequest={
          { nonce: 1, stepId: "step-open-session-1", eventId: "evt-open-session-2" } as any
        }
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("group-run-step-card-step-open-session-1-details")).toBeInTheDocument();
      expect(screen.getByTestId("group-run-step-card-step-open-session-1-event-evt-open-session-2")).toHaveAttribute(
        "data-group-run-step-event-highlighted",
        "true",
      );
    });
  });
});
