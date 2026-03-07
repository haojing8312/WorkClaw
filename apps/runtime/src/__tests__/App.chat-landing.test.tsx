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
    </div>
  ),
}));

vi.mock("../components/ChatView", () => ({
  ChatView: (props: any) => {
    chatViewPropsSpy(props);
    return (
      <div data-testid="chat-view">
        <div data-testid="chat-view-session-id">{props.sessionId}</div>
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
  NewSessionLanding: () => <div data-testid="new-session-landing">new-session-landing</div>,
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
