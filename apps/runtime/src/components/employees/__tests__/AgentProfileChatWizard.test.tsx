import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { AgentProfileChatWizard } from "../AgentProfileChatWizard";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("AgentProfileChatWizard", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "generate_agent_profile_draft") {
        return Promise.resolve({
          employee_id: "project_manager",
          employee_name: "项目经理",
          agents_md: "# AGENTS\n\nAgent profile draft",
          soul_md: "# SOUL\n\nSoul profile draft",
          user_md: "# USER\n\nUser profile draft",
        });
      }
      if (command === "apply_agent_profile") {
        return Promise.resolve({
          files: [
            { path: "E:/workspace/openclaw/project_manager/AGENTS.md", ok: true, error: null },
            { path: "E:/workspace/openclaw/project_manager/SOUL.md", ok: true, error: null },
            { path: "E:/workspace/openclaw/project_manager/USER.md", ok: true, error: null },
          ],
        });
      }
      return Promise.resolve(null);
    });
  });

  test("asks one question at a time and applies markdown files", async () => {
    render(
      <AgentProfileChatWizard
        employee={{
          id: "emp-1",
          employee_id: "project_manager",
          name: "项目经理",
          role_id: "project_manager",
          persona: "",
          feishu_open_id: "",
          feishu_app_id: "",
          feishu_app_secret: "",
          primary_skill_id: "builtin-general",
          default_work_dir: "E:/workspace",
          openclaw_agent_id: "project_manager",
          routing_priority: 100,
          enabled_scopes: ["feishu"],
          enabled: true,
          is_default: false,
          skill_ids: [],
          created_at: "2026-03-03T00:00:00Z",
          updated_at: "2026-03-03T00:00:00Z",
        }}
      />,
    );

    expect(screen.getByText("这个员工最核心的业务使命是什么？")).toBeInTheDocument();

    fireEvent.change(
      screen.getByPlaceholderText("例如：把需求推进到可上线交付，并对里程碑负责"),
      { target: { value: "负责项目交付与风险管理" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "下一题" }));
    expect(screen.getByText("它日常需要承担哪些关键职责？")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "生成预览" }));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "generate_agent_profile_draft",
        expect.objectContaining({
          payload: expect.objectContaining({
            employee_db_id: "emp-1",
          }),
        }),
      );
      expect(screen.getByText("AGENTS.md")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "应用到员工目录" }));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "apply_agent_profile",
        expect.objectContaining({
          payload: expect.objectContaining({
            employee_db_id: "emp-1",
          }),
        }),
      );
      expect(screen.getByText("3/3 文件写入成功")).toBeInTheDocument();
    });
  });
});
