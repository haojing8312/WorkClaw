import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import App from "../App";

const invokeMock = vi.fn();
let employeeListCalls = 0;
let createdSessionCount = 0;

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
      {props.employeeAssistantContext ? (
        <div data-testid="chat-assistant-context">
          {props.employeeAssistantContext.mode}:{props.employeeAssistantContext.employeeName || ""}:{props.employeeAssistantContext.employeeCode || ""}
        </div>
      ) : null}
      <div data-testid="chat-quick-prompts">
        {(props.quickPrompts || []).map((item: { label: string }, idx: number) => (
          <span key={`${item.label}-${idx}`}>{item.label}</span>
        ))}
      </div>
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
      <div data-testid="employee-snapshot">
        {JSON.stringify(
          (props.employees || []).map((item: any) => ({
            id: item.id,
            employee_id: item.employee_id,
            name: item.name,
            primary_skill_id: item.primary_skill_id,
            skill_ids: item.skill_ids,
          })),
        )}
      </div>
      <button onClick={() => props.onOpenEmployeeCreatorSkill?.({ mode: "create" })}>open-employee-assistant-create</button>
      <button onClick={() => props.onOpenEmployeeCreatorSkill?.({ mode: "update", employeeId: "emp-created" })}>open-employee-assistant-adjust</button>
    </div>
  ),
}));

describe("App employee assistant create+update regression", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    employeeListCalls = 0;
    createdSessionCount = 0;
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
            name: "智能体员工助手",
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
          return Promise.resolve([]);
        }
        if (employeeListCalls === 2) {
          return Promise.resolve([
            {
              id: "emp-created",
              employee_id: "project_manager",
              name: "项目经理",
              role_id: "project_manager",
              persona: "推进需求上线",
              feishu_open_id: "",
              feishu_app_id: "",
              feishu_app_secret: "",
              primary_skill_id: "builtin-general",
              default_work_dir: "D:\\\\workspace\\\\project_manager",
              openclaw_agent_id: "project_manager",
              enabled_scopes: ["feishu"],
              enabled: true,
              is_default: false,
              skill_ids: ["builtin-general"],
              created_at: new Date().toISOString(),
              updated_at: new Date().toISOString(),
            },
          ]);
        }
        return Promise.resolve([
          {
            id: "emp-created",
            employee_id: "project_manager",
            name: "项目经理-升级",
            role_id: "project_manager",
            persona: "推进需求上线并负责技能编排",
            feishu_open_id: "",
            feishu_app_id: "",
            feishu_app_secret: "",
            primary_skill_id: "docx-helper",
            default_work_dir: "D:\\\\workspace\\\\project_manager",
            openclaw_agent_id: "project_manager",
            enabled_scopes: ["feishu"],
            enabled: true,
            is_default: false,
            skill_ids: ["docx-helper", "find-skills"],
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          },
        ]);
      }
      if (command === "list_sessions") {
        return Promise.resolve([
          {
            id: `session-creator-${Math.max(createdSessionCount, 1)}`,
            title: "智能体员工助手会话",
            created_at: new Date().toISOString(),
            model_id: "model-a",
            permission_mode: "standard",
          },
        ]);
      }
      if (command === "create_session") {
        createdSessionCount += 1;
        return Promise.resolve(`session-creator-${createdSessionCount}`);
      }
      return Promise.resolve(null);
    });
  });

  afterEach(() => {
    window.location.hash = "";
  });

  test("keeps employee detail synced after assistant creates and then updates employee", async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "open-employee-assistant-create" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "open-employee-assistant-create" }));

    await waitFor(() => {
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
      expect(invokeMock).toHaveBeenCalledWith(
        "create_session",
        expect.objectContaining({
          skillId: "builtin-employee-creator",
          modelId: "model-a",
          employeeId: "",
          title: "创建员工：新员工",
        }),
      );
      expect(screen.getByTestId("chat-initial-message")).toHaveTextContent("请帮我创建一个新的智能体员工");
      expect(screen.getByTestId("chat-assistant-context")).toHaveTextContent("create");
      expect(screen.getByTestId("chat-quick-prompts")).toHaveTextContent("加技能");
      expect(screen.getByTestId("chat-quick-prompts")).toHaveTextContent("删技能");
    });

    fireEvent.click(screen.getByRole("button", { name: "trigger-session-refresh" }));

    await waitFor(() => {
      const listCalls = invokeMock.mock.calls.filter((call) => call[0] === "list_agent_employees");
      expect(listCalls.length).toBeGreaterThanOrEqual(2);
    });

    fireEvent.click(screen.getByRole("button", { name: "open-employees" }));
    await waitFor(() => {
      expect(screen.getByTestId("employee-snapshot")).toHaveTextContent("项目经理");
      expect(screen.getByTestId("employee-snapshot")).toHaveTextContent("builtin-general");
    });

    fireEvent.click(screen.getByRole("button", { name: "open-employee-assistant-adjust" }));
    await waitFor(() => {
      expect(screen.getByTestId("chat-view")).toBeInTheDocument();
      expect(invokeMock).toHaveBeenCalledWith(
        "create_session",
        expect.objectContaining({
          skillId: "builtin-employee-creator",
          modelId: "model-a",
          employeeId: "project_manager",
          title: "调整员工：项目经理",
        }),
      );
      expect(screen.getByTestId("chat-initial-message")).toHaveTextContent("请帮我修改智能体员工「项目经理」");
      expect(screen.getByTestId("chat-assistant-context")).toHaveTextContent("update:项目经理:project_manager");
    });
    fireEvent.click(screen.getByRole("button", { name: "trigger-session-refresh" }));

    await waitFor(() => {
      const listCalls = invokeMock.mock.calls.filter((call) => call[0] === "list_agent_employees");
      expect(listCalls.length).toBeGreaterThanOrEqual(3);
    });

    fireEvent.click(screen.getByRole("button", { name: "open-employees" }));
    await waitFor(() => {
      expect(screen.getByTestId("employee-snapshot")).toHaveTextContent("项目经理-升级");
      expect(screen.getByTestId("employee-snapshot")).toHaveTextContent("docx-helper");
      expect(screen.getByTestId("employee-snapshot")).toHaveTextContent("find-skills");
    });
  });
});
