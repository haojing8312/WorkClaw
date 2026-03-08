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
    </div>
  ),
}));

vi.mock("../components/ChatView", () => ({
  ChatView: () => <div data-testid="chat-view">chat-view</div>,
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
      if (command === "get_sessions") {
        if (payload?.skillId === "skill-sales") {
          return Promise.resolve([
            {
              id: "session-sales",
              title: "Session Sales",
              created_at: new Date().toISOString(),
              model_id: "model-a",
            },
          ]);
        }
        return Promise.resolve([]);
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
          permissionMode: "accept_edits",
          sessionMode: "employee_direct",
          teamId: "",
        }),
      );
    });

    await waitFor(() => {
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
    });
  });
});
