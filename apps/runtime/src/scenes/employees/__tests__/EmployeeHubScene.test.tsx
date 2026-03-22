import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { EmployeeHubScene } from "../EmployeeHubScene";
import type { AgentEmployee, SkillManifest } from "../../../types";

const {
  deleteAgentEmployeeMock,
  upsertAgentEmployeeMock,
  buildDefaultEmployeeUpdateInputMock,
} = vi.hoisted(() => ({
  deleteAgentEmployeeMock: vi.fn(),
  upsertAgentEmployeeMock: vi.fn(),
  buildDefaultEmployeeUpdateInputMock: vi.fn(),
}));

let latestViewProps: Record<string, unknown> | null = null;

vi.mock("../employeeHubApi", () => ({
  deleteAgentEmployee: deleteAgentEmployeeMock,
  upsertAgentEmployee: upsertAgentEmployeeMock,
  buildDefaultEmployeeUpdateInput: buildDefaultEmployeeUpdateInputMock,
}));

vi.mock("../../../components/employees/EmployeeHubView", () => ({
  EmployeeHubView: (props: Record<string, unknown>) => {
    latestViewProps = props;
    return (
      <div>
        <div data-testid="initial-tab">{String(props.initialTab ?? "")}</div>
        <div data-testid="selected-employee-id">
          {String(props.selectedEmployeeId ?? "")}
        </div>
        <div data-testid="highlight-employee-id">
          {String(props.highlightEmployeeId ?? "")}
        </div>
        <div data-testid="highlight-message">
          {String(props.highlightMessage ?? "")}
        </div>
        <button
          type="button"
          onClick={() => void (props.onDeleteEmployee as (id: string) => Promise<void>)("emp-1")}
        >
          delete employee
        </button>
        <button
          type="button"
          onClick={() =>
            void (props.onSetAsMainAndEnter as (id: string) => Promise<void>)("emp-1")
          }
        >
          set default
        </button>
        <button
          type="button"
          onClick={() => (props.onDismissHighlight as (() => void) | undefined)?.()}
        >
          dismiss highlight
        </button>
        <button
          type="button"
          onClick={() =>
            void (
              props.onOpenEmployeeCreatorSkill as
                | ((options?: { mode?: "create" | "update"; employeeId?: string }) => Promise<void> | void)
                | undefined
            )?.({ mode: "create" })
          }
        >
          open creator
        </button>
        <button
          type="button"
          onClick={() => (props.onOpenFeishuSettings as (() => void) | undefined)?.()}
        >
          open feishu settings
        </button>
      </div>
    );
  },
}));

const employees: AgentEmployee[] = [
  {
    id: "emp-1",
    employee_id: "employee.alpha",
    name: "Alpha",
    role_id: "employee.alpha",
    persona: "Alpha persona",
    feishu_open_id: "",
    feishu_app_id: "",
    feishu_app_secret: "",
    primary_skill_id: "skill-alpha",
    default_work_dir: "",
    openclaw_agent_id: "employee.alpha",
    routing_priority: 100,
    enabled_scopes: ["app"],
    enabled: true,
    is_default: true,
    skill_ids: ["skill-alpha"],
    created_at: "",
    updated_at: "",
  },
  {
    id: "emp-2",
    employee_id: "employee.beta",
    name: "Beta",
    role_id: "employee.beta",
    persona: "Beta persona",
    feishu_open_id: "",
    feishu_app_id: "",
    feishu_app_secret: "",
    primary_skill_id: "skill-beta",
    default_work_dir: "",
    openclaw_agent_id: "employee.beta",
    routing_priority: 100,
    enabled_scopes: ["app"],
    enabled: true,
    is_default: false,
    skill_ids: ["skill-beta"],
    created_at: "",
    updated_at: "",
  },
];

const skills: SkillManifest[] = [
  {
    id: "skill-alpha",
    name: "Alpha Skill",
    description: "",
    version: "1.0.0",
    author: "test",
    recommended_model: "gpt-5",
    tags: [],
    created_at: "",
  },
];

describe("EmployeeHubScene", () => {
  beforeEach(() => {
    latestViewProps = null;
    deleteAgentEmployeeMock.mockReset();
    upsertAgentEmployeeMock.mockReset();
    buildDefaultEmployeeUpdateInputMock.mockReset();
    deleteAgentEmployeeMock.mockResolvedValue(undefined);
    upsertAgentEmployeeMock.mockResolvedValue("emp-1");
    buildDefaultEmployeeUpdateInputMock.mockImplementation((employee: AgentEmployee) => ({
      id: employee.id,
      is_default: true,
      primary_skill_id: employee.primary_skill_id,
    }));
  });

  it("applies openRequest to tab, selection, and highlight state", async () => {
    render(
      <EmployeeHubScene
        employees={employees}
        skills={skills}
        openRequest={{
          nonce: 1,
          tab: "employees",
          highlightEmployeeId: "emp-2",
          highlightEmployeeName: "Beta",
        }}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("initial-tab").textContent).toBe("employees");
      expect(screen.getByTestId("selected-employee-id").textContent).toBe("emp-2");
      expect(screen.getByTestId("highlight-employee-id").textContent).toBe("emp-2");
      expect(screen.getByTestId("highlight-message").textContent).toContain("Beta");
    });

    fireEvent.click(screen.getByText("dismiss highlight"));

    await waitFor(() => {
      expect(screen.getByTestId("highlight-employee-id").textContent).toBe("");
      expect(screen.getByTestId("highlight-message").textContent).toBe("");
    });
  });

  it("handles employee deletion inside the scene and refreshes employees", async () => {
    const refreshEmployees = vi.fn().mockResolvedValue(undefined);

    render(
      <EmployeeHubScene
        employees={employees}
        skills={skills}
        openRequest={{
          nonce: 2,
          tab: "employees",
          highlightEmployeeId: "emp-1",
          highlightEmployeeName: "Alpha",
        }}
        onRefreshEmployees={refreshEmployees}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    fireEvent.click(screen.getByText("delete employee"));

    await waitFor(() => {
      expect(deleteAgentEmployeeMock).toHaveBeenCalledWith("emp-1");
      expect(refreshEmployees).toHaveBeenCalledTimes(1);
      expect(screen.getByTestId("highlight-employee-id").textContent).toBe("");
    });
  });

  it("sets the default employee inside the scene and delegates start-task navigation", async () => {
    const refreshEmployees = vi.fn().mockResolvedValue(undefined);
    const onEnterStartTask = vi.fn();

    render(
      <EmployeeHubScene
        employees={employees}
        skills={skills}
        onRefreshEmployees={refreshEmployees}
        onEnterStartTask={onEnterStartTask}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    fireEvent.click(screen.getByText("set default"));

    await waitFor(() => {
      expect(buildDefaultEmployeeUpdateInputMock).toHaveBeenCalledWith(employees[0]);
      expect(upsertAgentEmployeeMock).toHaveBeenCalledWith({
        id: "emp-1",
        is_default: true,
        primary_skill_id: "skill-alpha",
      });
      expect(refreshEmployees).toHaveBeenCalledTimes(1);
      expect(onEnterStartTask).toHaveBeenCalledWith("skill-alpha");
    });
  });

  it("clears highlight before delegating employee-hub shell actions", async () => {
    const onLaunchEmployeeCreatorSkill = vi.fn();
    const onOpenFeishuSettingsPanel = vi.fn();

    render(
      <EmployeeHubScene
        employees={employees}
        skills={skills}
        openRequest={{
          nonce: 3,
          tab: "employees",
          highlightEmployeeId: "emp-2",
          highlightEmployeeName: "Beta",
        }}
        onLaunchEmployeeCreatorSkill={onLaunchEmployeeCreatorSkill}
        onOpenFeishuSettingsPanel={onOpenFeishuSettingsPanel}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("highlight-employee-id").textContent).toBe("emp-2");
    });

    fireEvent.click(screen.getByText("open creator"));

    await waitFor(() => {
      expect(onLaunchEmployeeCreatorSkill).toHaveBeenCalledWith({ mode: "create" });
      expect(screen.getByTestId("highlight-employee-id").textContent).toBe("");
    });

    render(
      <EmployeeHubScene
        employees={employees}
        skills={skills}
        openRequest={{
          nonce: 4,
          tab: "employees",
          highlightEmployeeId: "emp-1",
          highlightEmployeeName: "Alpha",
        }}
        onOpenFeishuSettingsPanel={onOpenFeishuSettingsPanel}
        onStartTaskWithEmployee={() => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getAllByTestId("highlight-employee-id")[1].textContent).toBe("emp-1");
    });

    fireEvent.click(screen.getAllByText("open feishu settings")[1]);

    await waitFor(() => {
      expect(onOpenFeishuSettingsPanel).toHaveBeenCalledTimes(1);
      expect(screen.getAllByTestId("highlight-employee-id")[1].textContent).toBe("");
    });
  });
});
