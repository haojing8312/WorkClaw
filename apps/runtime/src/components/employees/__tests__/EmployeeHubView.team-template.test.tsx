import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { EmployeeHubView } from "../EmployeeHubView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(async () => null),
}));

function buildEmployee(id: string, name: string) {
  return {
    id: `emp-${id}`,
    employee_id: id,
    name,
    role_id: id,
    persona: "",
    feishu_open_id: "",
    feishu_app_id: "",
    feishu_app_secret: "",
    primary_skill_id: "builtin-general",
    default_work_dir: "",
    openclaw_agent_id: id,
    routing_priority: 100,
    enabled_scopes: ["app"],
    enabled: true,
    is_default: id === "taizi",
    skill_ids: [],
    created_at: "2026-03-07T00:00:00Z",
    updated_at: "2026-03-07T00:00:00Z",
  };
}

describe("EmployeeHubView team template panel", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    let listCalls = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_runtime_preferences") {
        return Promise.resolve({ default_work_dir: "E:\\workspace" });
      }
      if (command === "list_employee_groups") {
        listCalls += 1;
        return Promise.resolve([
          {
            id: "group-sansheng",
            name: "默认复杂任务团队",
            coordinator_employee_id: "shangshu",
            member_employee_ids: ["taizi", "zhongshu", "menxia", "shangshu"],
            member_count: 4,
            template_id: "sansheng-liubu",
            entry_employee_id: "taizi",
            review_mode: "hard",
            execution_mode: "parallel",
            visibility_mode: "team_only",
            is_bootstrap_seeded: true,
            config_json:
              '{"roles":[{"role_type":"entry","employee_key":"taizi"},{"role_type":"planner","employee_key":"zhongshu"},{"role_type":"reviewer","employee_key":"menxia"},{"role_type":"coordinator","employee_key":"shangshu"}]}',
            created_at: "2026-03-07T00:00:00Z",
            updated_at: "2026-03-07T00:00:00Z",
          },
          ...(listCalls > 1
            ? [
                {
                  id: "group-clone",
                  name: "默认复杂任务团队（副本）",
                  coordinator_employee_id: "shangshu",
                  member_employee_ids: ["taizi", "zhongshu", "menxia", "shangshu"],
                  member_count: 4,
                  template_id: "sansheng-liubu",
                  entry_employee_id: "taizi",
                  review_mode: "hard",
                  execution_mode: "parallel",
                  visibility_mode: "team_only",
                  is_bootstrap_seeded: false,
                  config_json: '{"roles":[]}',
                  created_at: "2026-03-07T00:00:00Z",
                  updated_at: "2026-03-07T00:00:00Z",
                },
              ]
            : []),
        ]);
      }
      if (command === "list_employee_group_rules") {
        return Promise.resolve([
          {
            id: "rule-1",
            group_id: "group-sansheng",
            from_employee_id: "zhongshu",
            to_employee_id: "menxia",
            relation_type: "review",
            phase_scope: "plan",
            required: true,
            priority: 110,
            created_at: "2026-03-07T00:00:00Z",
          },
          {
            id: "rule-2",
            group_id: "group-sansheng",
            from_employee_id: "shangshu",
            to_employee_id: "bingbu",
            relation_type: "delegate",
            phase_scope: "execute",
            required: true,
            priority: 130,
            created_at: "2026-03-07T00:00:00Z",
          },
        ]);
      }
      if (command === "clone_employee_group_template") {
        return Promise.resolve("group-clone");
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

  test("shows the seeded default team template instance with coordinator and reviewer roles", async () => {
    render(
      <EmployeeHubView
        employees={[
          buildEmployee("taizi", "太子"),
          buildEmployee("zhongshu", "中书省"),
          buildEmployee("menxia", "门下省"),
          buildEmployee("shangshu", "尚书省"),
          buildEmployee("bingbu", "兵部"),
        ]}
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

    fireEvent.click(screen.getByRole("tab", { name: "团队" }));

    await waitFor(() => {
      expect(screen.getByTestId("employee-group-item-group-sansheng")).toBeInTheDocument();
    });

    const teamCard = screen.getByTestId("employee-group-item-group-sansheng");
    expect(screen.getByTestId("employee-team-seeded-banner-group-sansheng")).toHaveTextContent("已预置默认团队");
    expect(within(teamCard).getByText("默认复杂任务团队")).toBeInTheDocument();
    expect(within(teamCard).getAllByText("入口：太子").length).toBeGreaterThan(0);
    expect(within(teamCard).getAllByText("协调：尚书省").length).toBeGreaterThan(0);
    expect(within(teamCard).getByText("审核：hard")).toBeInTheDocument();
    expect(within(teamCard).getByText("中书省 -> 门下省 · review · plan")).toBeInTheDocument();
    expect(within(teamCard).getByText("尚书省 -> 兵部 · delegate · execute")).toBeInTheDocument();
  });

  test("can clone the default seeded team into a custom team instance", async () => {
    render(
      <EmployeeHubView
        employees={[
          buildEmployee("taizi", "太子"),
          buildEmployee("zhongshu", "中书省"),
          buildEmployee("menxia", "门下省"),
          buildEmployee("shangshu", "尚书省"),
        ]}
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

    fireEvent.click(screen.getByRole("tab", { name: "团队" }));

    await waitFor(() => {
      expect(screen.getByTestId("employee-team-clone-group-sansheng")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("employee-team-clone-group-sansheng"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("clone_employee_group_template", {
        input: {
          source_group_id: "group-sansheng",
          name: "默认复杂任务团队（副本）",
        },
      });
      expect(screen.getByTestId("employee-group-item-group-clone")).toBeInTheDocument();
      expect(screen.getByText("默认复杂任务团队（副本）")).toBeInTheDocument();
    });
  });
});
