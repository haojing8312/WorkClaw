import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import App from "../App";

const invokeMock = vi.fn();

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
      <div data-testid="sidebar-session-count">{props.sessions?.length ?? 0}</div>
      <div data-testid="sidebar-first-session-id">{props.sessions?.[0]?.id ?? ""}</div>
      <div data-testid="sidebar-first-session-title">
        {props.sessions?.[0]?.display_title || props.sessions?.[0]?.title || ""}
      </div>
    </div>
  ),
}));

vi.mock("../components/ChatView", () => ({
  ChatView: (props: any) => (
    <div data-testid="chat-view">
      <div data-testid="chat-view-session-title">{props.sessionTitle || ""}</div>
      <div data-testid="chat-view-session-mode">{props.sessionMode || ""}</div>
      <div data-testid="chat-view-session-employee-name">{props.sessionEmployeeName || ""}</div>
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
    <button onClick={() => props.onStartTaskWithEmployee("emp-sales")}>chat-with-employee</button>
  ),
}));

describe("App employee chat entry", () => {
  beforeEach(() => {
    invokeMock.mockReset();
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
            id: "skill-sales",
            name: "Sales Assistant",
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
        return Promise.resolve([
          {
            id: "emp-sales",
            employee_id: "sales_lead",
            name: "销售主管",
            role_id: "sales_lead",
            persona: "",
            feishu_open_id: "",
            feishu_app_id: "",
            feishu_app_secret: "",
            primary_skill_id: "skill-sales",
            default_work_dir: "D:\\\\workspace\\\\sales",
            openclaw_agent_id: "sales_lead",
            enabled_scopes: ["feishu"],
            enabled: true,
            is_default: false,
            skill_ids: [],
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "session-sales",
            title: "销售主管",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            session_mode: "employee_direct",
          },
        ]);
      }
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          operation_permission_mode: "standard",
        });
      }
      if (command === "create_session") {
        return Promise.resolve("session-sales");
      }
      return Promise.resolve(null);
    });
  });

  test("creates a session from employee view and enters chat directly", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "chat-with-employee" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "chat-with-employee" }));

    await waitFor(() => {
        expect(invokeMock).toHaveBeenCalledWith(
        "create_session",
        expect.objectContaining({
          skillId: "skill-sales",
          modelId: "model-a",
          workDir: "D:\\\\workspace\\\\sales",
          employeeId: "sales_lead",
          title: "销售主管",
          permissionMode: "standard",
          sessionMode: "employee_direct",
          teamId: "",
        }),
      );
    });

    await waitFor(() => {
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
    });

    await waitFor(() => {
      expect(screen.getByTestId("chat-view-session-title")).toHaveTextContent("销售主管");
      expect(screen.getByTestId("chat-view-session-mode")).toHaveTextContent("employee_direct");
      expect(screen.getByTestId("chat-view-session-employee-name")).toHaveTextContent("销售主管");
    });
  });

  test("keeps the new employee session in the sidebar when an older session list request resolves late", async () => {
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
            id: "skill-sales",
            name: "Sales Assistant",
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
        return Promise.resolve([
          {
            id: "emp-sales",
            employee_id: "sales_lead",
            name: "销售主管",
            role_id: "sales_lead",
            persona: "",
            feishu_open_id: "",
            feishu_app_id: "",
            feishu_app_secret: "",
            primary_skill_id: "skill-sales",
            default_work_dir: "D:\\\\workspace\\\\sales",
            openclaw_agent_id: "sales_lead",
            enabled_scopes: ["feishu"],
            enabled: true,
            is_default: false,
            skill_ids: [],
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
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
            id: "session-sales",
            title: "销售主管",
            display_title: "销售主管",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            employee_id: "sales_lead",
            session_mode: "employee_direct",
            team_id: "",
          },
        ]);
      }
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          operation_permission_mode: "standard",
        });
      }
      if (command === "create_session") {
        return Promise.resolve("session-sales");
      }
      return Promise.resolve(payload ?? null);
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "chat-with-employee" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "chat-with-employee" }));

    await waitFor(() => {
      expect(screen.getByTestId("sidebar-first-session-id")).toHaveTextContent("session-sales");
      expect(screen.getByTestId("sidebar-first-session-title")).toHaveTextContent("销售主管");
    });

    resolveInitialListRef.current?.([]);

    await waitFor(() => {
      expect(screen.getByTestId("sidebar-first-session-id")).toHaveTextContent("session-sales");
      expect(screen.getByTestId("sidebar-session-count")).toHaveTextContent("1");
    });
  });

  test("keeps an optimistic employee session in the sidebar when reloading sessions fails", async () => {
    let listSessionsCount = 0;

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
            id: "skill-sales",
            name: "Sales Assistant",
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
        return Promise.resolve([
          {
            id: "emp-sales",
            employee_id: "sales_lead",
            name: "销售主管",
            role_id: "sales_lead",
            persona: "",
            feishu_open_id: "",
            feishu_app_id: "",
            feishu_app_secret: "",
            primary_skill_id: "skill-sales",
            default_work_dir: "D:\\\\workspace\\\\sales",
            openclaw_agent_id: "sales_lead",
            enabled_scopes: ["feishu"],
            enabled: true,
            is_default: false,
            skill_ids: [],
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "list_sessions") {
        listSessionsCount += 1;
        if (listSessionsCount === 1) {
          return Promise.resolve([]);
        }
        return Promise.reject(new Error("database is locked"));
      }
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          operation_permission_mode: "standard",
        });
      }
      if (command === "record_frontend_diagnostic_event") {
        return Promise.resolve(null);
      }
      if (command === "create_session") {
        return Promise.resolve("session-sales");
      }
      return Promise.resolve(payload ?? null);
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "chat-with-employee" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "chat-with-employee" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
    });

    await waitFor(() => {
      expect(screen.getByTestId("sidebar-session-count")).toHaveTextContent("1");
      expect(screen.getByTestId("sidebar-first-session-id")).toHaveTextContent("session-sales");
      expect(screen.getByTestId("sidebar-first-session-title")).toHaveTextContent("销售主管");
    });
  });
});
