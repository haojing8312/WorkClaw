import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { EmployeeHubView } from "../EmployeeHubView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("EmployeeHubView employee creation flow", () => {
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

  test("uses skill-first creation and hides manual employee form", async () => {
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

    expect(screen.queryByRole("button", { name: "手动新建" })).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText("员工名称")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "保存员工" })).not.toBeInTheDocument();
    expect(screen.getByText("已移除手动创建流程，请通过「智能体员工助手」对话式完成创建与配置。")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "新建员工" }));
    expect(onOpenEmployeeCreatorSkill).toHaveBeenCalledTimes(1);
    expect(onOpenEmployeeCreatorSkill).toHaveBeenCalledWith({ mode: "create" });
  });

  test("renders profile files states and keeps the update entrypoint on selected employees", async () => {
    const onOpenEmployeeCreatorSkill = vi.fn();
    invokeMock.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "C:\\Users\\test\\SkillMint\\workspace" });
      }
      if (command === "set_runtime_preferences") return Promise.resolve(null);
      if (command === "resolve_default_work_dir") return Promise.resolve("C:\\Users\\test\\SkillMint\\workspace");
      if (command === "get_openclaw_plugin_feishu_runtime_status") {
        return Promise.resolve({
          plugin_id: "@larksuite/openclaw-lark",
          account_id: "default",
          running: false,
          started_at: null,
          last_error: null,
          last_event_at: null,
          recent_logs: [],
        });
      }
      if (command === "list_im_routing_bindings") return Promise.resolve([]);
      if (command === "get_agent_profile_files" && payload?.employeeDbId === "emp-profile") {
        return Promise.resolve({
          profile_dir: "D:\\profiles\\emp-profile",
          files: [
            { name: "AGENTS.md", exists: true, content: "你是项目经理", error: null },
            { name: "SOUL.md", exists: false, content: "", error: null },
            { name: "USER.md", exists: false, content: "", error: "permission denied" },
          ],
        });
      }
      return Promise.resolve(null);
    });

    render(
      <EmployeeHubView
        employees={[
          {
            id: "emp-profile",
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
        selectedEmployeeId="emp-profile"
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
        onOpenEmployeeCreatorSkill={onOpenEmployeeCreatorSkill}
      />
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_agent_profile_files", { employeeDbId: "emp-profile" });
    });

    expect(screen.getByText("AGENTS / SOUL / USER（只读）")).toBeInTheDocument();
    expect(screen.getByText("目录：D:\\profiles\\emp-profile")).toBeInTheDocument();
    expect(screen.getByText("你是项目经理")).toBeInTheDocument();
    expect(screen.getByText("SOUL.md （未生成）")).toBeInTheDocument();
    expect(screen.getByText("读取失败：permission denied")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "更新画像" }));
    expect(onOpenEmployeeCreatorSkill).toHaveBeenCalledWith({ mode: "update", employeeId: "emp-profile" });
  });
});
