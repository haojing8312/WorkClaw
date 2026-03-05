import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { EmployeeHubView } from "../EmployeeHubView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("EmployeeHubView thread 1:1 binding", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "C:\\Users\\test\\WorkClaw\\workspace" });
      }
      if (command === "set_runtime_preferences") return Promise.resolve(null);
      if (command === "resolve_default_work_dir") {
        return Promise.resolve("C:\\Users\\test\\WorkClaw\\workspace");
      }
      if (command === "get_feishu_employee_connection_statuses") {
        return Promise.resolve({
          relay: {
            running: true,
            generation: 1,
            interval_ms: 1500,
            total_accepted: 0,
            last_error: null,
          },
          sidecar: {
            running: true,
            started_at: "2026-03-04T00:00:00Z",
            queued_events: 0,
            running_count: 0,
            items: [],
          },
        });
      }
      if (command === "list_recent_im_threads") {
        return Promise.resolve([
          {
            thread_id: "chat-main",
            source: "feishu",
            last_text_preview: "请项目经理先分析需求",
            last_seen_at: "2026-03-05T08:00:00Z",
          },
          {
            thread_id: "chat-dev",
            source: "feishu",
            last_text_preview: "请开发团队细化方案",
            last_seen_at: "2026-03-05T07:30:00Z",
          },
        ]);
      }
      if (command === "get_thread_employee_bindings") {
        if (payload?.threadId === "chat-main") {
          return Promise.resolve({ thread_id: "chat-main", employee_ids: ["emp-pm"] });
        }
        if (payload?.threadId === "chat-dev") {
          return Promise.resolve({ thread_id: "chat-dev", employee_ids: [] });
        }
      }
      if (command === "bind_thread_employees") return Promise.resolve(null);
      return Promise.resolve(null);
    });
  });

  test("loads recent threads and supports saving single-employee binding", async () => {
    render(
      <EmployeeHubView
        employees={[
          {
            id: "emp-pm",
            employee_id: "project_manager",
            name: "项目经理",
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
            is_default: true,
            skill_ids: [],
            created_at: "2026-03-01T00:00:00Z",
            updated_at: "2026-03-01T00:00:00Z",
          },
          {
            id: "emp-dev",
            employee_id: "developer_team",
            name: "开发团队",
            role_id: "developer_team",
            persona: "",
            feishu_open_id: "",
            feishu_app_id: "",
            feishu_app_secret: "",
            primary_skill_id: "",
            default_work_dir: "",
            openclaw_agent_id: "developer_team",
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
        selectedEmployeeId="emp-pm"
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("list_recent_im_threads", { limit: 20 });
    });

    expect(screen.getByTestId("thread-binding-row-chat-main")).toBeInTheDocument();
    expect(screen.getByTestId("thread-binding-row-chat-dev")).toBeInTheDocument();
    expect(screen.getByTestId("thread-binding-owner-chat-main")).toHaveTextContent("项目经理");
    expect(screen.getByTestId("thread-binding-owner-chat-dev")).toHaveTextContent("未绑定");

    fireEvent.click(screen.getByTestId("thread-binding-row-chat-dev"));
    fireEvent.change(screen.getByTestId("thread-binding-employee-select"), {
      target: { value: "emp-dev" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存 1:1 绑定" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("bind_thread_employees", {
        threadId: "chat-dev",
        employeeIds: ["emp-dev"],
      });
    });
  });
});
