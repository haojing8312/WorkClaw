import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { EmployeeHubView } from "../EmployeeHubView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(async () => null),
}));

function buildEmployee(id: string) {
  return {
    id: `emp-${id}`,
    employee_id: id,
    name: `员工-${id}`,
    role_id: id,
    persona: "",
    feishu_open_id: "",
    feishu_app_id: "",
    feishu_app_secret: "",
    primary_skill_id: "builtin-general",
    default_work_dir: "",
    openclaw_agent_id: id,
    enabled_scopes: ["feishu"],
    routing_priority: 100,
    enabled: true,
    is_default: id === "pm",
    skill_ids: [],
    created_at: "2026-03-05T00:00:00Z",
    updated_at: "2026-03-05T00:00:00Z",
  };
}

describe("EmployeeHubView group orchestrator panel", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "E:\\workspace" });
      }
      if (command === "list_employee_groups") {
        return Promise.resolve([]);
      }
      if (command === "create_employee_group") {
        return Promise.resolve("group-created");
      }
      if (command === "get_feishu_employee_connection_statuses") {
        return Promise.resolve({
          relay: { running: false, generation: 0, interval_ms: 1500, total_accepted: 0 },
          sidecar: { running: false, queued_events: 0, running_count: 0, items: [] },
        });
      }
      if (command === "set_runtime_preferences") return Promise.resolve(null);
      if (command === "resolve_default_work_dir") return Promise.resolve("E:\\workspace");
      return Promise.resolve(null);
    });
  });

  test("creates employee group with selected members and coordinator", async () => {
    render(
      <EmployeeHubView
        employees={[buildEmployee("pm"), buildEmployee("dev"), buildEmployee("qa")]}
        skills={[
          {
            id: "builtin-general",
            name: "通用助手",
            description: "",
            version: "1.0.0",
            author: "",
            recommended_model: "",
            tags: [],
            created_at: "2026-03-05T00:00:00Z",
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
      expect(invokeMock).toHaveBeenCalledWith("list_employee_groups");
    });

    fireEvent.change(screen.getByTestId("employee-group-name"), { target: { value: "交付协作群" } });
    fireEvent.change(screen.getByTestId("employee-group-coordinator"), { target: { value: "pm" } });
    fireEvent.click(screen.getByTestId("employee-group-member-emp-pm"));
    fireEvent.click(screen.getByTestId("employee-group-member-emp-dev"));
    fireEvent.click(screen.getByTestId("employee-group-member-emp-qa"));
    fireEvent.click(screen.getByTestId("employee-group-create"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("create_employee_group", {
        input: {
          name: "交付协作群",
          coordinator_employee_id: "pm",
          member_employee_ids: ["pm", "dev", "qa"],
        },
      });
    });
  });

  test("shows warning when selecting more than 10 members", async () => {
    const employees = Array.from({ length: 11 }).map((_, i) => buildEmployee(`e${i + 1}`));
    render(
      <EmployeeHubView
        employees={employees}
        skills={[]}
        selectedEmployeeId={null}
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
      />
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("list_employee_groups");
    });

    for (let i = 1; i <= 11; i += 1) {
      fireEvent.click(screen.getByTestId(`employee-group-member-emp-e${i}`));
    }

    expect(screen.getByText("群组成员最多 10 人")).toBeInTheDocument();
  });

  test("starts group run with instruction and shows report", async () => {
    const openSessionMock = vi.fn();
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "E:\\workspace" });
      }
      if (command === "list_employee_groups") {
        return Promise.resolve([
          {
            id: "group-1",
            name: "交付协作群",
            coordinator_employee_id: "pm",
            member_employee_ids: ["pm", "dev", "qa"],
            member_count: 3,
            created_at: "2026-03-05T00:00:00Z",
            updated_at: "2026-03-05T00:00:00Z",
          },
        ]);
      }
      if (command === "start_employee_group_run") {
        return Promise.resolve({
          run_id: "run-1",
          group_id: "group-1",
          session_id: "session-group-1",
          session_skill_id: "builtin-general",
          state: "done",
          current_round: 1,
          final_report: "计划：共 3 步\n执行：已完成 3 步。\n汇报：已完成。",
          steps: [],
        });
      }
      if (command === "get_feishu_employee_connection_statuses") {
        return Promise.resolve({
          relay: { running: false, generation: 0, interval_ms: 1500, total_accepted: 0 },
          sidecar: { running: false, queued_events: 0, running_count: 0, items: [] },
        });
      }
      if (command === "set_runtime_preferences") return Promise.resolve(null);
      if (command === "resolve_default_work_dir") return Promise.resolve("E:\\workspace");
      return Promise.resolve(null);
    });

    render(
      <EmployeeHubView
        employees={[buildEmployee("pm"), buildEmployee("dev"), buildEmployee("qa")]}
        skills={[]}
        selectedEmployeeId={null}
        onSelectEmployee={() => {}}
        onSaveEmployee={async () => {}}
        onDeleteEmployee={async () => {}}
        onSetAsMainAndEnter={() => {}}
        onStartTaskWithEmployee={() => {}}
        onOpenGroupRunSession={openSessionMock}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("employee-group-item-group-1")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByTestId("employee-group-run-goal-group-1"), {
      target: { value: "请输出发布方案并分工执行" },
    });
    fireEvent.click(screen.getByTestId("employee-group-run-start-group-1"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("start_employee_group_run", {
        input: {
          group_id: "group-1",
          user_goal: "请输出发布方案并分工执行",
          execution_window: 3,
          max_retry_per_step: 1,
          timeout_employee_ids: [],
        },
      });
      expect(screen.getByTestId("employee-group-run-report-group-1")).toHaveTextContent("计划：共 3 步");
      expect(openSessionMock).toHaveBeenCalledWith("session-group-1", "builtin-general");
    });
  });
});
