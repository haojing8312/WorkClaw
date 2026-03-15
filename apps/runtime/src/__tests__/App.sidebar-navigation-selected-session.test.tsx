import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import App from "../App";

const invokeMock = vi.fn();

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
      <button onClick={props.onOpenEmployees}>employees</button>
      <button onClick={() => props.onSelectSession("session-1")}>select-first-session</button>
    </div>
  ),
}));

vi.mock("../components/ChatView", () => ({
  ChatView: () => <div data-testid="chat-view">chat-view</div>,
}));

vi.mock("../components/experts/ExpertsView", () => ({
  ExpertsView: () => <div data-testid="experts-view">experts-view</div>,
}));

vi.mock("../components/employees/EmployeeHubView", () => ({
  EmployeeHubView: () => <div data-testid="employees-view">employees-view</div>,
}));

vi.mock("../components/packaging/PackagingView", () => ({
  PackagingView: () => <div data-testid="packaging-view">packaging-view</div>,
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

describe("App sidebar navigation with selected session", () => {
  afterEach(() => {
    cleanup();
    window.localStorage.clear();
    window.location.hash = "";
  });

  beforeEach(() => {
    invokeMock.mockReset();
    window.localStorage.setItem("workclaw:initial-model-setup-completed", "1");
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
        return Promise.resolve([
          {
            id: "session-1",
            title: "Session 1",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            skill_id: "builtin-general",
            session_mode: "general",
            team_id: "",
          },
        ]);
      }
      if (command === "list_agent_employees") {
        return Promise.resolve([
          {
            id: "emp-1",
            employee_id: "emp_1",
            name: "默认员工",
            role_id: "emp_1",
            persona: "",
            feishu_open_id: "",
            feishu_app_id: "",
            feishu_app_secret: "",
            primary_skill_id: "builtin-general",
            default_work_dir: "",
            openclaw_agent_id: "emp_1",
            enabled_scopes: ["app"],
            routing_priority: 100,
            enabled: true,
            is_default: true,
            skill_ids: ["builtin-general"],
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
      return Promise.resolve(null);
    });
  });

  test("allows opening experts and employees even when a session is selected", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "select-first-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "experts" }));

    await waitFor(() => {
      expect(screen.getByTestId("experts-view")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "employees" }));

    await waitFor(() => {
      expect(screen.getByTestId("employees-view")).toBeInTheDocument();
    });
  });
});
