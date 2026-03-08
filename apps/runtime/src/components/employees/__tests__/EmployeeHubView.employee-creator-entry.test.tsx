import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { EmployeeHubView } from "../EmployeeHubView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("EmployeeHubView employee creator skill entry", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "C:\\Users\\test\\WorkClaw\\workspace" });
      }
      if (command === "set_runtime_preferences") return Promise.resolve(null);
      if (command === "resolve_default_work_dir") {
        return Promise.resolve("C:\\Users\\test\\WorkClaw\\workspace");
      }
      return Promise.resolve(null);
    });
  });

  test("can open builtin employee creator skill from employee hub", async () => {
    const onOpenEmployeeCreatorSkill = vi.fn();

    render(
      <EmployeeHubView
        employees={[]}
        skills={[
          {
            id: "builtin-general",
            name: "通用助手",
            description: "",
            version: "1.0.0",
            author: "",
            recommended_model: "",
            tags: [],
            created_at: "2026-03-01T00:00:00Z",
          },
        ]}
        selectedEmployeeId={null}
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
        onOpenEmployeeCreatorSkill={onOpenEmployeeCreatorSkill}
      />
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_runtime_preferences");
    });
    expect(
      screen.getByText("用员工编号统一管理 OpenClaw 与飞书路由。主员工默认进入且拥有全技能权限。"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("通过对话描述岗位需求，系统会自动给出技能匹配与配置建议，并在你确认后创建员工。"),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByTestId("open-employee-creator-skill"));
    expect(onOpenEmployeeCreatorSkill).toHaveBeenCalledTimes(1);
    expect(onOpenEmployeeCreatorSkill).toHaveBeenCalledWith({ mode: "create" });
  });

  test("uses creator skill as default path when clicking 新建员工", async () => {
    const onOpenEmployeeCreatorSkill = vi.fn();

    render(
      <EmployeeHubView
        employees={[]}
        skills={[
          {
            id: "builtin-general",
            name: "通用助手",
            description: "",
            version: "1.0.0",
            author: "",
            recommended_model: "",
            tags: [],
            created_at: "2026-03-01T00:00:00Z",
          },
        ]}
        selectedEmployeeId={null}
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
        onOpenEmployeeCreatorSkill={onOpenEmployeeCreatorSkill}
      />
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_runtime_preferences");
    });

    fireEvent.click(screen.getByRole("button", { name: "新建员工" }));
    expect(onOpenEmployeeCreatorSkill).toHaveBeenCalledTimes(1);
    expect(onOpenEmployeeCreatorSkill).toHaveBeenCalledWith({ mode: "create" });
  });

  test("removes manual creation entry and form in favor of skill-first flow", async () => {
    render(
      <EmployeeHubView
        employees={[]}
        skills={[
          {
            id: "builtin-general",
            name: "通用助手",
            description: "",
            version: "1.0.0",
            author: "",
            recommended_model: "",
            tags: [],
            created_at: "2026-03-01T00:00:00Z",
          },
        ]}
        selectedEmployeeId={null}
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_runtime_preferences");
    });

    expect(screen.queryByRole("button", { name: "手动新建" })).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText("员工名称")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "保存员工" })).not.toBeInTheDocument();
  });

  test("shows creator highlight banner and supports dismiss", async () => {
    const onDismissHighlight = vi.fn();

    render(
      <EmployeeHubView
        employees={[
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
            default_work_dir: "",
            openclaw_agent_id: "project_manager",
            routing_priority: 100,
            enabled_scopes: ["feishu"],
            enabled: true,
            is_default: false,
            skill_ids: ["builtin-general"],
            created_at: "2026-03-01T00:00:00Z",
            updated_at: "2026-03-01T00:00:00Z",
          },
        ]}
        skills={[
          {
            id: "builtin-general",
            name: "通用助手",
            description: "",
            version: "1.0.0",
            author: "",
            recommended_model: "",
            tags: [],
            created_at: "2026-03-01T00:00:00Z",
          },
        ]}
        selectedEmployeeId="emp-created"
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
        highlightEmployeeId="emp-created"
        highlightMessage="已由智能体员工助手生成：项目经理"
        onDismissHighlight={onDismissHighlight}
      />
    );
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_runtime_preferences");
    });

    expect(screen.getByTestId("employee-creator-highlight")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: "员工" }));
    expect(screen.getByTestId("employee-item-emp-created")).toBeInTheDocument();

    fireEvent.click(screen.getByTestId("employee-creator-highlight-dismiss"));
    expect(onDismissHighlight).toHaveBeenCalledTimes(1);
  });

  test("uses update mode when clicking 调整员工 on selected employee", async () => {
    const onOpenEmployeeCreatorSkill = vi.fn();

    render(
      <EmployeeHubView
        employees={[
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
            default_work_dir: "",
            openclaw_agent_id: "project_manager",
            routing_priority: 100,
            enabled_scopes: ["feishu"],
            enabled: true,
            is_default: false,
            skill_ids: ["builtin-general"],
            created_at: "2026-03-01T00:00:00Z",
            updated_at: "2026-03-01T00:00:00Z",
          },
        ]}
        skills={[
          {
            id: "builtin-general",
            name: "通用助手",
            description: "",
            version: "1.0.0",
            author: "",
            recommended_model: "",
            tags: [],
            created_at: "2026-03-01T00:00:00Z",
          },
        ]}
        selectedEmployeeId="emp-created"
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
        onOpenEmployeeCreatorSkill={onOpenEmployeeCreatorSkill}
      />
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_runtime_preferences");
    });

    fireEvent.click(screen.getByRole("tab", { name: "员工" }));
    fireEvent.click(screen.getByRole("button", { name: "调整员工" }));
    expect(onOpenEmployeeCreatorSkill).toHaveBeenCalledWith({
      mode: "update",
      employeeId: "emp-created",
    });
  });
});
