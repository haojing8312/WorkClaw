import { render, screen, waitFor } from "@testing-library/react";
import { EmployeeHubView } from "../EmployeeHubView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("EmployeeHubView thread binding removal", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "C:\\Users\\test\\WorkClaw\\workspace" });
      }
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
      if (command === "get_employee_memory_stats") {
        return Promise.resolve({
          employee_id: "project_manager",
          total_files: 0,
          total_bytes: 0,
          skills: [],
        });
      }
      return Promise.resolve(null);
    });
  });

  test("does not request thread binding commands", async () => {
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
            enabled_scopes: ["feishu"],
            enabled: true,
            is_default: true,
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
      expect(invokeMock).toHaveBeenCalledWith("get_feishu_employee_connection_statuses", {
        sidecarBaseUrl: null,
      });
    });

    const calledCommands = invokeMock.mock.calls.map(([command]) => String(command));
    expect(calledCommands).not.toContain("list_recent_im_threads");
    expect(calledCommands).not.toContain("get_thread_employee_bindings");
    expect(calledCommands).not.toContain("bind_thread_employees");
  });
});
