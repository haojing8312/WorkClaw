import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import App from "../App";
import { storageKey } from "../lib/branding";

const invokeMock = vi.fn();
const chatViewPropsSpy = vi.fn();
const LAST_SELECTED_SESSION_ID_KEY = storageKey("last-selected-session-id");
const LAST_SELECTED_SESSION_SNAPSHOT_KEY = storageKey("last-selected-session-snapshot");

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
      <button
        onClick={() => {
          props.onSelectSession("session-step-gongbu-1");
        }}
      >
        select-last-session
      </button>
      <button
        onClick={() => {
          props.onDeleteSession("session-1");
        }}
      >
        delete-first-session
      </button>
      <button
        onClick={() => {
          props.onDeleteSession("session-step-gongbu-1");
        }}
      >
        delete-last-session
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
        <div data-testid="chat-view-runtime-stream-text">
          {(props.persistedRuntimeState?.streamItems || [])
            .filter((item: any) => item.type === "text")
            .map((item: any) => item.content || "")
            .join("")}
        </div>
        <div data-testid="chat-view-runtime-agent-state">
          {props.persistedRuntimeState?.agentState?.state || ""}
        </div>
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
        <button
          onClick={() =>
            props.onSessionBlockingStateChange?.({
              blocking: true,
              status: "thinking",
            })
          }
        >
          set-session-thinking
        </button>
        <button
          onClick={() =>
            props.onSessionBlockingStateChange?.({
              blocking: false,
              status: null,
            })
          }
        >
          clear-session-thinking
        </button>
        <button
          onClick={() =>
            props.onPersistRuntimeState?.({
              streaming: true,
              streamItems: [{ type: "text", content: "已缓存的运行中输出" }],
              toolManifest: [],
              streamReasoning: {
                status: "thinking",
                content: "恢复中",
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
            })
          }
        >
          persist-runtime-state
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

vi.mock("../components/TaskTabStrip", () => ({
  TaskTabStrip: (props: any) => (
    <div data-testid="task-tab-strip">
      <div data-testid="task-tab-count">{props.tabs?.length ?? 0}</div>
      <div data-testid="task-tab-active-id">{props.activeTabId ?? ""}</div>
      {props.tabs?.map((tab: any) => (
        <div key={tab.id}>
          <button data-testid={`task-tab-${tab.id}`} onClick={() => props.onSelectTab(tab.id)}>
            {tab.title}
          </button>
          <div data-testid={`task-tab-kind-${tab.id}`}>{tab.kind}</div>
          <div data-testid={`task-tab-active-${tab.id}`}>{String(props.activeTabId === tab.id)}</div>
          <div data-testid={`task-tab-runtime-${tab.id}`}>{tab.runtimeStatus || ""}</div>
          <button
            aria-label={`close-${tab.id}`}
            onClick={() => props.onCloseTab(tab.id)}
          >
            close
          </button>
        </div>
      ))}
      <button onClick={props.onCreateTab}>create-task-tab</button>
    </div>
  ),
}));

describe("App chat landing", () => {
  afterEach(() => {
    cleanup();
    window.localStorage.clear();
    window.location.hash = "";
  });

  beforeEach(() => {
    invokeMock.mockReset();
    chatViewPropsSpy.mockClear();
    window.localStorage.clear();
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
          {
            id: "skill-gongbu",
            name: "工部协作",
            description: "gongbu",
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
            skill_id: "builtin-general",
            session_mode: "general",
            team_id: "",
            runtime_status: "running",
          },
          {
            id: "session-team-entry-1",
            title: "默认复杂任务团队",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            skill_id: "builtin-general",
            session_mode: "team_entry",
            team_id: "group-1",
            runtime_status: "completed",
          },
          {
            id: "session-run-open-step",
            title: "Group Run Session",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            skill_id: "builtin-general",
            runtime_status: "running",
          },
          {
            id: "session-step-gongbu-1",
            title: "工部执行会话",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            skill_id: "skill-gongbu",
            runtime_status: "completed",
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
      if (command === "delete_session") {
        return Promise.resolve(null);
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

  test("restores the last selected session on startup when the session still exists", async () => {
    window.localStorage.setItem(LAST_SELECTED_SESSION_ID_KEY, "session-step-gongbu-1");

    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-step-gongbu-1");
    });

    expect(chatViewPropsSpy).toHaveBeenLastCalledWith(
      expect.objectContaining({
        skill: expect.objectContaining({ id: "skill-gongbu" }),
      }),
    );
    expect(screen.queryByTestId("new-session-landing")).not.toBeInTheDocument();
  });

  test("hydrates the sidebar and selected skill from the persisted session snapshot before sessions finish loading", async () => {
    window.localStorage.setItem(LAST_SELECTED_SESSION_ID_KEY, "session-step-gongbu-1");
    window.localStorage.setItem(
      LAST_SELECTED_SESSION_SNAPSHOT_KEY,
      JSON.stringify({
        id: "session-step-gongbu-1",
        title: "工部执行会话",
        display_title: "工部执行会话",
        created_at: "2026-03-17T00:00:00Z",
        model_id: "model-a",
        skill_id: "skill-gongbu",
        session_mode: "general",
        team_id: "",
      }),
    );

    invokeMock.mockImplementation((command: string) => {
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
          {
            id: "skill-gongbu",
            name: "工部协作",
            description: "gongbu",
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
      if (command === "list_search_configs") {
        return Promise.resolve([
          {
            id: "search-a",
            name: "Search A",
            api_format: "openai",
            base_url: "https://search.example.com",
            model_name: "search-model",
            is_default: true,
          },
        ]);
      }
      if (command === "list_sessions") {
        return new Promise(() => {});
      }
      if (command === "list_agent_employees") {
        return Promise.resolve([]);
      }
      if (command === "list_employee_groups") {
        return Promise.resolve([]);
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
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-step-gongbu-1");
      expect(screen.getByTestId("sidebar-first-session-id")).toHaveTextContent("session-step-gongbu-1");
      expect(screen.getByTestId("sidebar-first-session-title")).toHaveTextContent("工部执行会话");
    });

    expect(chatViewPropsSpy).toHaveBeenLastCalledWith(
      expect.objectContaining({
        skill: expect.objectContaining({ id: "skill-gongbu" }),
      }),
    );
  });

  test("keeps the hydrated session visible after opening start task and creating a new session before session list hydration finishes", async () => {
    window.localStorage.setItem(LAST_SELECTED_SESSION_ID_KEY, "session-step-gongbu-1");
    window.localStorage.setItem(
      LAST_SELECTED_SESSION_SNAPSHOT_KEY,
      JSON.stringify({
        id: "session-step-gongbu-1",
        title: "工部执行会话",
        display_title: "工部执行会话",
        created_at: "2026-03-17T00:00:00Z",
        model_id: "model-a",
        skill_id: "skill-gongbu",
        session_mode: "general",
        team_id: "",
      }),
    );

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
          {
            id: "skill-gongbu",
            name: "工部协作",
            description: "gongbu",
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
      if (command === "list_search_configs") {
        return Promise.resolve([
          {
            id: "search-a",
            name: "Search A",
            api_format: "openai",
            base_url: "https://search.example.com",
            model_name: "search-model",
            is_default: true,
          },
        ]);
      }
      if (command === "list_sessions") {
        return new Promise(() => {});
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
            default_work_dir: "",
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
        return Promise.resolve([]);
      }
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          operation_permission_mode: "standard",
        });
      }
      if (command === "create_session") {
        return Promise.resolve("session-created-general");
      }
      return Promise.resolve(payload ?? null);
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-step-gongbu-1");
      expect(screen.getByTestId("sidebar-session-count")).toHaveTextContent("1");
    });

    fireEvent.click(screen.getByRole("button", { name: "start-task" }));

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    expect(screen.getByTestId("sidebar-session-count")).toHaveTextContent("1");
    expect(screen.getByTestId("sidebar-first-session-id")).toHaveTextContent("session-step-gongbu-1");

    fireEvent.click(screen.getByRole("button", { name: "create-general-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-created-general");
    });

    expect(screen.getByTestId("sidebar-session-count")).toHaveTextContent("2");
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

  test("deleting the selected first session focuses the next session", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "select-first-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "delete-first-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-team-entry-1");
    });
  });

  test("deleting the selected last session focuses the previous session", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "select-last-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-step-gongbu-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "delete-last-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-run-open-step");
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
          {
            id: "skill-gongbu",
            name: "工部协作",
            description: "gongbu",
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

  test("opens a new start-task tab when the current session is still running", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "select-first-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "start-task" }));

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
      expect(screen.getByTestId("task-tab-count")).toHaveTextContent("2");
    });

    expect(screen.getByTestId("task-tab-count")).toHaveTextContent("2");
  });

  test("reuses the current tab when the current session has already ended", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "select-last-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-step-gongbu-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "start-task" }));

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
      expect(screen.getByTestId("task-tab-count")).toHaveTextContent("1");
    });

    expect(screen.getByTestId("task-tab-count")).toHaveTextContent("1");
  });

  test("creates a fresh start-task tab from the tab strip plus button", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
      expect(screen.getByTestId("task-tab-count")).toHaveTextContent("1");
    });

    fireEvent.click(screen.getByRole("button", { name: "create-task-tab" }));

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
      expect(screen.getByTestId("task-tab-count")).toHaveTextContent("2");
    });
  });

  test("switches back to the running session when its tab is selected again", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "select-first-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "start-task" }));

    await waitFor(() => {
      expect(screen.getByTestId("task-tab-count")).toHaveTextContent("2");
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    fireEvent.click(within(screen.getByTestId("task-tab-strip")).getByRole("button", { name: "Session 1" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-1");
    });
  });

  test("restores persisted runtime state after switching away and back to the same session tab", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "select-first-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "persist-runtime-state" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-runtime-stream-text")).toHaveTextContent("已缓存的运行中输出");
      expect(screen.getByTestId("chat-view-runtime-agent-state")).toHaveTextContent("thinking");
    });

    fireEvent.click(screen.getByRole("button", { name: "start-task" }));

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
      expect(screen.getByTestId("task-tab-count")).toHaveTextContent("2");
    });

    fireEvent.click(within(screen.getByTestId("task-tab-strip")).getByRole("button", { name: "Session 1" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-1");
      expect(screen.getByTestId("chat-view-runtime-stream-text")).toHaveTextContent("已缓存的运行中输出");
      expect(screen.getByTestId("chat-view-runtime-agent-state")).toHaveTextContent("thinking");
    });
  });

  test("treats local thinking state as blocking and opens a new tab even before runtime status refreshes", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "select-last-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-id")).toHaveTextContent("session-step-gongbu-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "set-session-thinking" }));
    fireEvent.click(screen.getByRole("button", { name: "start-task" }));

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
      expect(screen.getByTestId("task-tab-count")).toHaveTextContent("2");
      expect(within(screen.getByTestId("task-tab-strip")).getByText("thinking")).toBeInTheDocument();
    });
  });
});
