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

    expect(screen.queryByPlaceholderText("角色ID（如 project_manager）")).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText("OpenClaw Agent ID（默认同角色ID）")).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText("员工名称")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "手动新建" }));
    expect(screen.getByPlaceholderText("员工名称")).toBeInTheDocument();

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
