import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import App from "../App";

const invokeMock = vi.fn();
let employeeListCalls = 0;

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

vi.mock("../components/Sidebar", () => ({
  Sidebar: (props: any) => (
    <div>
      <button onClick={props.onOpenEmployees}>open-employees</button>
      <button onClick={props.onOpenStartTask}>open-start-task</button>
    </div>
  ),
}));

vi.mock("../components/ChatView", () => ({
  ChatView: (props: any) => (
    <div data-testid="chat-view">
      {props.initialMessage ? <div data-testid="chat-initial-message">{props.initialMessage}</div> : null}
      <button onClick={() => props.onSessionUpdate?.()}>trigger-session-refresh</button>
    </div>
  ),
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

vi.mock("../components/employees/EmployeeHubView", () => ({
  EmployeeHubView: (props: any) => (
    <div>
      <div data-testid="employee-count">{props.employees.length}</div>
      <div data-testid="employee-highlight-id">{props.highlightEmployeeId || ""}</div>
      <div data-testid="employee-highlight-message">{props.highlightMessage || ""}</div>
      <button onClick={() => props.onOpenEmployeeCreatorSkill?.()}>open-employee-creator</button>
      <button onClick={() => props.onDismissHighlight?.()}>dismiss-employee-highlight</button>
    </div>
  ),
}));

describe("App employee creator skill flow", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    employeeListCalls = 0;
    window.localStorage.clear();
    window.location.hash = "#/employees";

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
            id: "builtin-employee-creator",
            name: "创建员工",
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
      if (command === "list_agent_employees") {
        employeeListCalls += 1;
        if (employeeListCalls <= 1) {
          return Promise.resolve([]);
        }
        return Promise.resolve([
          {
            id: "emp-created",
            employee_id: "project_manager",
            name: "项目经理",
            role_id: "project_manager",
            persona: "",
            feishu_open_id: "",
            feishu_app_id: "",
            feishu_app_secret: "",
            primary_skill_id: "builtin-general",
            default_work_dir: "D:\\\\workspace\\\\project_manager",
            openclaw_agent_id: "project_manager",
            routing_priority: 100,
            enabled_scopes: ["feishu"],
            enabled: true,
            is_default: false,
            skill_ids: ["builtin-general"],
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "get_sessions") {
        if (payload?.skillId === "builtin-employee-creator") {
          return Promise.resolve([
            {
              id: "session-creator",
              title: "创建员工会话",
              created_at: new Date().toISOString(),
              model_id: "model-a",
              permission_mode: "accept_edits",
            },
          ]);
        }
        return Promise.resolve([]);
      }
      if (command === "create_session") {
        return Promise.resolve("session-creator");
      }
      return Promise.resolve(null);
    });
  });

  afterEach(() => {
    window.location.hash = "";
  });

  test("opens builtin employee creator and refreshes employees after chat session update", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "open-employee-creator" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "open-employee-creator" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "create_session",
        expect.objectContaining({
          skillId: "builtin-employee-creator",
          modelId: "model-a",
          employeeId: "",
        }),
      );
    });

    await waitFor(() => {
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
      expect(screen.getByTestId("chat-initial-message")).toHaveTextContent("请帮我创建一个新的智能体员工");
    });

    fireEvent.click(screen.getByRole("button", { name: "trigger-session-refresh" }));

    await waitFor(() => {
      const listCalls = invokeMock.mock.calls.filter((call) => call[0] === "list_agent_employees");
      expect(listCalls.length).toBeGreaterThanOrEqual(2);
    });

    fireEvent.click(screen.getByRole("button", { name: "open-employees" }));

    await waitFor(() => {
      expect(screen.getByTestId("employee-count")).toHaveTextContent("1");
      expect(screen.getByTestId("employee-highlight-id")).toHaveTextContent("emp-created");
      expect(screen.getByTestId("employee-highlight-message")).toHaveTextContent("已由创建员工助手生成");
    });

    fireEvent.click(screen.getByRole("button", { name: "dismiss-employee-highlight" }));

    await waitFor(() => {
      expect(screen.getByTestId("employee-highlight-id")).toHaveTextContent("");
      expect(screen.getByTestId("employee-highlight-message")).toHaveTextContent("");
    });
  });
});
