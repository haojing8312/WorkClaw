import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import App from "../App";

const invokeMock = vi.fn();
const chatViewPropsSpy = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

vi.mock("../components/Sidebar", () => ({
  Sidebar: (props: any) => (
    <div>
      <button onClick={props.onOpenStartTask}>start-task</button>
      <button onClick={props.onOpenExperts}>experts</button>
      <button
        onClick={() => {
          props.onSelectSession("session-1");
        }}
      >
        select-first-session
      </button>
      <button
        onClick={() => {
          props.onSelectSession("session-team-entry-1");
        }}
      >
        select-team-session
      </button>
      <div data-testid="sidebar-session-count">{props.sessions?.length ?? 0}</div>
      <div data-testid="sidebar-first-session-id">{props.sessions?.[0]?.id ?? ""}</div>
      <div data-testid="sidebar-first-session-title">
        {props.sessions?.[0]?.display_title || props.sessions?.[0]?.title || ""}
      </div>
    </div>
  ),
}));

vi.mock("../components/ChatView", () => ({
  ChatView: (props: any) => {
    chatViewPropsSpy(props);
    return (
      <div data-testid="chat-view">
        <div data-testid="chat-view-session-id">{props.sessionId}</div>
        <div data-testid="chat-view-session-mode">{props.sessionMode || ""}</div>
        <div data-testid="chat-view-session-title">{props.sessionTitle || ""}</div>
        {props.groupRunStepFocusRequest ? (
          <div data-testid="chat-view-group-run-step-focus">
            {props.groupRunStepFocusRequest.stepId}
          </div>
        ) : null}
        {props.groupRunStepFocusRequest?.eventId ? (
          <div data-testid="chat-view-group-run-event-focus">
            {props.groupRunStepFocusRequest.eventId}
          </div>
        ) : null}
        {props.sessionExecutionContext ? (
          <div data-testid="chat-view-session-execution-context">
            {props.sessionExecutionContext.sourceSessionId}
            {"|"}
            {props.sessionExecutionContext.sourceStepId}
            {"|"}
            {props.sessionExecutionContext.sourceEmployeeId || ""}
            {"|"}
            {props.sessionExecutionContext.assigneeEmployeeId || ""}
            {"|"}
            {(props.sessionExecutionContext.sourceStepTimeline || [])
              .map((item: any) => item.label)
              .join(",")}
          </div>
        ) : null}
        <button
          onClick={() =>
            props.onOpenSession?.("session-step-gongbu-1", {
              focusHint: "正在整理交付清单",
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
            })
          }
        >
          open-execution-session
        </button>
        <button
          onClick={() => props.onReturnToSourceSession?.("session-run-open-step")}
        >
          return-to-source-session
        </button>
        <button
          onClick={() =>
            props.onOpenSession?.("session-run-open-step", {
              groupRunStepFocusId: "step-open-session-1",
            })
          }
        >
          open-source-step-focus
        </button>
        <button
          onClick={() =>
            props.onOpenSession?.("session-run-open-step", {
              groupRunStepFocusId: "step-open-session-1",
              groupRunEventFocusId: "evt-open-session-2",
            })
          }
        >
          open-source-step-event-focus
        </button>
      </div>
    );
  },
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
  NewSessionLanding: (props: any) => (
    <div data-testid="new-session-landing">
      new-session-landing
      <button onClick={() => props.onCreateSessionWithInitialMessage?.("你好")}>
        create-general-session
      </button>
      <button
        onClick={() =>
          props.onCreateTeamEntrySession?.({
            teamId: props.teams?.[0]?.id || "group-1",
            initialMessage: "请安排一次多角色协作",
          })
        }
      >
        create-team-entry-session
      </button>
    </div>
  ),
}));

describe("App chat landing", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    chatViewPropsSpy.mockClear();
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "list_skills") {
        return Promise.resolve([
          {
            id: "builtin-general",
            name: "General",
            description: "desc",
            version: "1.0.0",
            author: "test",
            recommended_model: "model-a",
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
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "session-1",
            title: "Session 1",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            session_mode: "general",
            team_id: "",
          },
          {
            id: "session-team-entry-1",
            title: "默认复杂任务团队",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            session_mode: "team_entry",
            team_id: "group-1",
          },
          {
            id: "session-run-open-step",
            title: "Group Run Session",
            created_at: new Date().toISOString(),
            model_id: "model-a",
          },
          {
            id: "session-step-gongbu-1",
            title: "工部执行会话",
            created_at: new Date().toISOString(),
            model_id: "model-a",
          },
        ]);
      }
      if (command === "list_agent_employees") {
        return Promise.resolve([
          {
            id: "emp-taizi",
            employee_id: "taizi",
            name: "太子",
            role_id: "taizi",
            persona: "",
            feishu_open_id: "",
            feishu_app_id: "",
            feishu_app_secret: "",
            primary_skill_id: "builtin-general",
            default_work_dir: "E:\\\\workspace\\\\taizi",
            openclaw_agent_id: "taizi",
            enabled_scopes: ["app"],
            routing_priority: 100,
            enabled: true,
            is_default: true,
            skill_ids: [],
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "list_employee_groups") {
        return Promise.resolve([
          {
            id: "group-1",
            name: "默认复杂任务团队",
            coordinator_employee_id: "shangshu",
            member_employee_ids: ["taizi", "zhongshu", "shangshu"],
            member_count: 3,
            entry_employee_id: "taizi",
            review_mode: "hard",
            execution_mode: "parallel",
            visibility_mode: "shared",
            template_id: "sansheng-liubu",
            is_bootstrap_seeded: true,
            config_json: "{}",
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "create_session") {
        if (payload?.sessionMode === "team_entry") {
          return Promise.resolve("session-team-entry-1");
        }
        return Promise.resolve("session-created-general");
      }
      return Promise.resolve(null);
    });
  });

  test("renders new-session landing when chat mode has no selected session", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    expect(document.querySelector(".sm-app")).toBeInTheDocument();
  });

  test("renders chat view after selecting a session", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "select-first-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
    });
  });

  test("returns to landing when clicking start-task from selected session", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "select-first-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
    }, { timeout: 3000 });

    fireEvent.click(screen.getByRole("button", { name: "start-task" }));

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });
  });

  test("keeps landing visible before session is selected", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });
  });

  test("homepage creates a general session instead of reusing employee/team context", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "create-general-session" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "create-general-session" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "create_session",
        expect.objectContaining({
          skillId: "builtin-general",
          modelId: "model-a",
          employeeId: "",
          sessionMode: "general",
          teamId: "",
        }),
      );
    });
  });

  test("homepage explicit team shortcut creates a team-entry session", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "create-team-entry-session" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "create-team-entry-session" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "create_session",
        expect.objectContaining({
          skillId: "builtin-general",
          modelId: "model-a",
          workDir: "E:\\\\workspace\\\\taizi",
          employeeId: "taizi",
          title: "默认复杂任务团队",
          sessionMode: "team_entry",
          teamId: "group-1",
        }),
      );
    });
  });

  test("passes explicit team session metadata into chat view after entering via the team shortcut", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "create-team-entry-session" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "create-team-entry-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-team-entry-1");
      expect(screen.getByTestId("chat-view-session-mode")).toHaveTextContent("team_entry");
      expect(screen.getByTestId("chat-view-session-title")).toHaveTextContent("默认复杂任务团队");
    });
  });

  test("keeps the new team session in the sidebar when an older session list request resolves late", async () => {
    let listSessionsCount = 0;
    const resolveInitialListRef: { current: ((value: unknown) => void) | null } = { current: null };

    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "list_skills") {
        return Promise.resolve([
          {
            id: "builtin-general",
            name: "General",
            description: "desc",
            version: "1.0.0",
            author: "test",
            recommended_model: "model-a",
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
      if (command === "list_sessions") {
        listSessionsCount += 1;
        if (listSessionsCount === 1) {
          return new Promise((resolve) => {
            resolveInitialListRef.current = resolve;
          });
        }
        return Promise.resolve([
          {
            id: "session-team-entry-1",
            title: "默认复杂任务团队",
            display_title: "默认复杂任务团队",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            employee_id: "taizi",
            session_mode: "team_entry",
            team_id: "group-1",
          },
        ]);
      }
      if (command === "list_agent_employees") {
        return Promise.resolve([
          {
            id: "emp-taizi",
            employee_id: "taizi",
            name: "太子",
            role_id: "taizi",
            persona: "",
            feishu_open_id: "",
            feishu_app_id: "",
            feishu_app_secret: "",
            primary_skill_id: "builtin-general",
            default_work_dir: "E:\\\\workspace\\\\taizi",
            openclaw_agent_id: "taizi",
            enabled_scopes: ["app"],
            routing_priority: 100,
            enabled: true,
            is_default: true,
            skill_ids: [],
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "list_employee_groups") {
        return Promise.resolve([
          {
            id: "group-1",
            name: "默认复杂任务团队",
            coordinator_employee_id: "shangshu",
            member_employee_ids: ["taizi", "zhongshu", "shangshu"],
            member_count: 3,
            entry_employee_id: "taizi",
            review_mode: "hard",
            execution_mode: "parallel",
            visibility_mode: "shared",
            template_id: "sansheng-liubu",
            is_bootstrap_seeded: true,
            config_json: "{}",
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "create_session") {
        if (payload?.sessionMode === "team_entry") {
          return Promise.resolve("session-team-entry-1");
        }
        return Promise.resolve("session-created-general");
      }
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          operation_permission_mode: "standard",
        });
      }
      return Promise.resolve(null);
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "create-team-entry-session" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "create-team-entry-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("sidebar-first-session-id")).toHaveTextContent("session-team-entry-1");
      expect(screen.getByTestId("sidebar-first-session-title")).toHaveTextContent("默认复杂任务团队");
      expect(screen.getByTestId("sidebar-session-count")).toHaveTextContent("1");
    });

    resolveInitialListRef.current?.([]);

    await waitFor(() => {
      expect(screen.getByTestId("sidebar-first-session-id")).toHaveTextContent("session-team-entry-1");
      expect(screen.getByTestId("sidebar-session-count")).toHaveTextContent("1");
    });
  });

  test("passes execution session context to chat view and returns to the source session", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "select-first-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "open-execution-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-step-gongbu-1");
      expect(screen.getByTestId("chat-view-session-execution-context")).toHaveTextContent(
        "session-run-open-step|step-open-session-1|尚书|工部|step_created · 尚书 -> 工部,step_dispatched · 尚书 -> 工部",
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "return-to-source-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-run-open-step");
    });
  });

  test("passes group run step focus request when reopening the source session", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "select-first-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "open-source-step-focus" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-run-open-step");
      expect(screen.getByTestId("chat-view-group-run-step-focus")).toHaveTextContent(
        "step-open-session-1",
      );
    });
  });

  test("passes group run event focus request when reopening the source session", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "select-first-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "open-source-step-event-focus" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-run-open-step");
      expect(screen.getByTestId("chat-view-group-run-step-focus")).toHaveTextContent(
        "step-open-session-1",
      );
      expect(screen.getByTestId("chat-view-group-run-event-focus")).toHaveTextContent(
        "evt-open-session-2",
      );
    });
  });
});
