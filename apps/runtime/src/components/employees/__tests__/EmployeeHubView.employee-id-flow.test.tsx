import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { EmployeeHubView } from "../EmployeeHubView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("EmployeeHubView employee_id flow", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "C:\\Users\\test\\SkillMint\\workspace" });
      }
      if (command === "set_runtime_preferences") return Promise.resolve(null);
      if (command === "resolve_default_work_dir") return Promise.resolve("C:\\Users\\test\\SkillMint\\workspace");
      return Promise.resolve(null);
    });
  });

  test("uses employee_id as only identity field and auto-generates value", async () => {
    const onSaveEmployee = vi.fn(async (_input: any) => {});

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
        onSaveEmployee={onSaveEmployee}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    expect(screen.getByPlaceholderText("员工名称")).toBeInTheDocument();
    expect(screen.getByText("主技能（用于新会话默认技能路由）")).toBeInTheDocument();
    expect(screen.getByText("默认工作目录（该员工新会话默认目录）")).toBeInTheDocument();
    expect(screen.queryByTestId("employee-routing-priority-input")).not.toBeInTheDocument();
    expect(
      screen.getByText("技能合集（补充授权能力；当前会话默认仍优先使用“主技能”）"),
    ).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("员工名称"), {
      target: { value: "Project Manager" },
    });

    const employeeIdInput = screen.getByPlaceholderText("员工编号（自动生成，可编辑）") as HTMLInputElement;
    expect(employeeIdInput.value).toBe("project_manager");

    fireEvent.click(screen.getByRole("button", { name: "保存员工" }));

    await waitFor(() => {
      expect(onSaveEmployee).toHaveBeenCalledTimes(1);
    });
    expect(onSaveEmployee.mock.calls[0][0]).toMatchObject({
      employee_id: "project_manager",
      role_id: "project_manager",
      openclaw_agent_id: "project_manager",
    });
  });
});
