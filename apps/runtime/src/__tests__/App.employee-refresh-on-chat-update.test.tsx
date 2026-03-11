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
      <button onClick={() => props.onSelectSession("session-general")}>select-general-session</button>
      <button onClick={props.onOpenEmployees}>open-employees</button>
    </div>
  ),
}));

vi.mock("../components/ChatView", () => ({
  ChatView: (props: any) => (
    <div data-testid="chat-view">
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
    <div data-testid="employee-snapshot">
      {JSON.stringify((props.employees || []).map((item: any) => item.name))}
    </div>
  ),
}));

describe("App employee list refresh on chat update", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    employeeListCalls = 0;
    window.localStorage.clear();
    window.location.hash = "#/start-task";

    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "list_skills") {
        return Promise.resolve([
          {
            id: "builtin-general",
            name: "通用助手",
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
        if (employeeListCalls === 1) {
          return Promise.resolve([
            {
              id: "emp-old",
              employee_id: "old_employee",
              name: "旧员工",
              role_id: "old_employee",
              persona: "",
              feishu_open_id: "",
              feishu_app_id: "",
              feishu_app_secret: "",
              primary_skill_id: "builtin-general",
              default_work_dir: "",
              openclaw_agent_id: "old_employee",
              enabled_scopes: ["feishu"],
              enabled: true,
              is_default: true,
              skill_ids: ["builtin-general"],
              created_at: new Date().toISOString(),
              updated_at: new Date().toISOString(),
            },
          ]);
        }
        return Promise.resolve([
          {
            id: "emp-old",
            employee_id: "old_employee",
            name: "旧员工",
            role_id: "old_employee",
            persona: "",
            feishu_open_id: "",
            feishu_app_id: "",
            feishu_app_secret: "",
            primary_skill_id: "builtin-general",
            default_work_dir: "",
            openclaw_agent_id: "old_employee",
            enabled_scopes: ["feishu"],
            enabled: true,
            is_default: true,
            skill_ids: ["builtin-general"],
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          },
          {
            id: "emp-new",
            employee_id: "new_employee",
            name: "新员工",
            role_id: "new_employee",
            persona: "",
            feishu_open_id: "",
            feishu_app_id: "",
            feishu_app_secret: "",
            primary_skill_id: "builtin-general",
            default_work_dir: "",
            openclaw_agent_id: "new_employee",
            enabled_scopes: ["feishu"],
            enabled: true,
            is_default: false,
            skill_ids: ["builtin-general"],
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: "session-general",
            title: "General Session",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            permission_mode: "standard",
          },
        ]);
      }
      return Promise.resolve(null);
    });
  });

  afterEach(() => {
    window.location.hash = "";
  });

  test("refreshes employee list after chat update even in non-employee-assistant skill", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("new-session-landing")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "select-general-session" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "trigger-session-refresh" }));

    await waitFor(() => {
      const calls = invokeMock.mock.calls.filter((call) => call[0] === "list_agent_employees");
      expect(calls.length).toBeGreaterThanOrEqual(2);
    });

    fireEvent.click(screen.getByRole("button", { name: "open-employees" }));

    await waitFor(() => {
      expect(screen.getByTestId("employee-snapshot")).toHaveTextContent("新员工");
    });
  });
});
