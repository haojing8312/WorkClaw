import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { EmployeeHubView } from "../EmployeeHubView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

function buildEmployee(id: string, overrides: Record<string, unknown> = {}) {
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
    created_at: "2026-03-08T00:00:00Z",
    updated_at: "2026-03-08T00:00:00Z",
    ...overrides,
  };
}

describe("EmployeeHubView overview home", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "E:\\workspace" });
      }
      if (command === "set_runtime_preferences") return Promise.resolve(null);
      if (command === "resolve_default_work_dir") return Promise.resolve("E:\\workspace");
      if (command === "list_employee_groups") {
        return Promise.resolve([
          {
            id: "group-complete",
            name: "完整协作团队",
            coordinator_employee_id: "pm",
            entry_employee_id: "pm",
            member_employee_ids: ["pm", "dev"],
            member_count: 2,
            created_at: "2026-03-08T00:00:00Z",
            updated_at: "2026-03-08T00:00:00Z",
          },
          {
            id: "group-incomplete",
            name: "待完善团队",
            coordinator_employee_id: "dev",
            entry_employee_id: "",
            member_employee_ids: ["dev", "qa"],
            member_count: 2,
            created_at: "2026-03-08T00:00:00Z",
            updated_at: "2026-03-08T00:00:00Z",
          },
        ]);
      }
      if (command === "list_employee_group_runs") {
        return Promise.resolve([
          {
            id: "run-1",
            group_id: "group-complete",
            group_name: "完整协作团队",
            goal: "复杂任务拆解",
            status: "running",
            started_at: "2026-03-08T10:00:00Z",
            finished_at: "",
            session_id: "session-group-1",
            session_skill_id: "builtin-general",
          },
          {
            id: "run-2",
            group_id: "group-incomplete",
            group_name: "待完善团队",
            goal: "周报复盘",
            status: "completed",
            started_at: "2026-03-08T09:00:00Z",
            finished_at: "2026-03-08T09:20:00Z",
            session_id: "session-group-2",
            session_skill_id: "builtin-general",
          },
        ]);
      }
      if (command === "get_feishu_employee_connection_statuses") {
        return Promise.resolve({
          relay: { running: false, generation: 0, interval_ms: 1500, total_accepted: 0 },
          sidecar: { running: false, queued_events: 0, running_count: 0, items: [] },
        });
      }
      return Promise.resolve(null);
    });
  });

  test("defaults to an overview-first home instead of rendering team and employee detail panels", async () => {
    render(
      <EmployeeHubView
        employees={[
          buildEmployee("pm"),
          buildEmployee("dev", {
            feishu_open_id: "ou_dev_pending",
          }),
          buildEmployee("qa", {
            primary_skill_id: "",
            skill_ids: [],
          }),
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
            created_at: "2026-03-08T00:00:00Z",
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

    expect(screen.getByRole("heading", { name: "智能体员工" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "总览" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("员工总数")).toBeInTheDocument();
    expect(screen.getByText("团队总数")).toBeInTheDocument();
    expect(screen.getByTestId("employee-overview-metric-employees")).toHaveTextContent("3");
    expect(screen.getByTestId("employee-overview-metric-teams")).toHaveTextContent("2");
    expect(screen.getByText("1 名员工未完成连接配置")).toBeInTheDocument();
    expect(screen.getByText("1 个团队角色不完整")).toBeInTheDocument();
    expect(screen.getByText("最近运行")).toBeInTheDocument();
    expect(screen.getByText("复杂任务拆解")).toBeInTheDocument();
    expect(screen.queryByText("拉群协作（最多 10 人）")).not.toBeInTheDocument();
    expect(screen.queryByText("员工详情")).not.toBeInTheDocument();
  });

  test("drills into filtered employee and run lists from overview cards", async () => {
    render(
      <EmployeeHubView
        employees={[
          buildEmployee("pm"),
          buildEmployee("dev", {
            feishu_open_id: "ou_dev_pending",
          }),
          buildEmployee("qa", {
            primary_skill_id: "",
            skill_ids: [],
          }),
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
            created_at: "2026-03-08T00:00:00Z",
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

    const pendingConnectionRow = screen.getByText("1 名员工未完成连接配置").closest("div");
    expect(pendingConnectionRow).not.toBeNull();
    fireEvent.click(within(pendingConnectionRow as HTMLElement).getByRole("button", { name: "去处理" }));

    expect(screen.getByRole("tab", { name: "员工" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("当前筛选：待完善连接")).toBeInTheDocument();
    expect(screen.getByTestId("employee-item-emp-dev")).toBeInTheDocument();
    expect(screen.queryByTestId("employee-item-emp-pm")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "总览" }));
    fireEvent.click(screen.getByRole("button", { name: "查看运行中团队" }));

    expect(screen.getByRole("tab", { name: "运行" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("当前筛选：运行中")).toBeInTheDocument();
    expect(screen.getByText("复杂任务拆解")).toBeInTheDocument();
    expect(screen.queryByText("周报复盘")).not.toBeInTheDocument();
  });

  test("shows a single empty-state message for overview and runs when no recent runs exist", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "E:\\workspace" });
      }
      if (command === "set_runtime_preferences") return Promise.resolve(null);
      if (command === "resolve_default_work_dir") return Promise.resolve("E:\\workspace");
      if (command === "list_employee_groups") return Promise.resolve([]);
      if (command === "list_employee_group_runs") return Promise.resolve([]);
      if (command === "get_feishu_employee_connection_statuses") {
        return Promise.resolve({
          relay: { running: false, generation: 0, interval_ms: 1500, total_accepted: 0 },
          sidecar: { running: false, queued_events: 0, running_count: 0, items: [] },
        });
      }
      return Promise.resolve(null);
    });

    render(
      <EmployeeHubView
        employees={[buildEmployee("pm")]}
        skills={[
          {
            id: "builtin-general",
            name: "通用助手",
            description: "",
            version: "1.0.0",
            author: "",
            recommended_model: "",
            tags: [],
            created_at: "2026-03-08T00:00:00Z",
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
      expect(invokeMock).toHaveBeenCalledWith("list_employee_group_runs", { limit: 10 });
    });

    expect(screen.getByText("还没有运行记录，发起一次团队任务后会显示在这里。")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "运行" }));

    expect(screen.getByText("还没有运行记录，可先到团队页发起一次任务。")).toBeInTheDocument();
  });
});
