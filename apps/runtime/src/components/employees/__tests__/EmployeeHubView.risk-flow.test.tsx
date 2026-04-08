import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { EmployeeHubView } from "../EmployeeHubView";
import { brandDefaultWorkspacePathExample } from "../../../lib/branding";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("EmployeeHubView risk flow", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: brandDefaultWorkspacePathExample("test") });
      }
      if (command === "set_runtime_preferences") return Promise.resolve(null);
      if (command === "resolve_default_work_dir") return Promise.resolve(brandDefaultWorkspacePathExample("test"));
      return Promise.resolve(null);
    });
  });

  test("delete employee requires high-risk confirmation", async () => {
    const onDeleteEmployee = vi.fn(() => Promise.resolve());

    render(
      <EmployeeHubView
        employees={[
          {
            id: "emp-1",
            employee_id: "project_manager",
            name: "张三",
            role_id: "project_manager",
            persona: "",
            feishu_open_id: "",
            feishu_app_id: "",
            feishu_app_secret: "",
            primary_skill_id: "",
            default_work_dir: "",
            openclaw_agent_id: "project_manager",
            routing_priority: 100,
            enabled_scopes: ["feishu"],
            enabled: true,
            is_default: false,
            skill_ids: [],
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
        selectedEmployeeId="emp-1"
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={onDeleteEmployee}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "删除员工" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    expect(onDeleteEmployee).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "删除员工" }));
    fireEvent.click(screen.getByRole("button", { name: "确认删除" }));

    await waitFor(() => {
      expect(onDeleteEmployee).toHaveBeenCalledWith("emp-1");
      expect(onDeleteEmployee).toHaveBeenCalledTimes(1);
    });
  });

  test("settings tab explains the default workspace under the runtime root", async () => {
    render(
      <EmployeeHubView
        employees={[]}
        skills={[]}
        selectedEmployeeId={null}
        onSelectEmployee={() => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />
    );

    fireEvent.click(screen.getByRole("tab", { name: "设置" }));

    expect(await screen.findByDisplayValue(brandDefaultWorkspacePathExample("test"))).toBeInTheDocument();
    expect(
      screen.getByText(
        `默认：${brandDefaultWorkspacePathExample()}。支持 C/D/E 盘路径，目录不存在会自动创建。`,
      ),
    ).toBeInTheDocument();
  });
});
