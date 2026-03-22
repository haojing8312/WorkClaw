import { useEffect, useState } from "react";
import {
  EmployeeHubView,
  type EmployeeHubTab,
  type EmployeeHubViewProps,
} from "../../components/employees/EmployeeHubView";
import {
  buildDefaultEmployeeUpdateInput,
  deleteAgentEmployee,
  upsertAgentEmployee,
} from "./employeeHubApi";
import type { EmployeeAssistantLaunchOptions } from "./employeeAssistantService";

export interface EmployeeHubOpenRequest {
  nonce: number;
  tab: EmployeeHubTab;
  highlightEmployeeId?: string | null;
  highlightEmployeeName?: string | null;
}

export interface EmployeeHubSceneProps
  extends Omit<
    EmployeeHubViewProps,
    | "initialTab"
    | "highlightEmployeeId"
    | "highlightMessage"
    | "onDismissHighlight"
    | "selectedEmployeeId"
    | "onSelectEmployee"
    | "onDeleteEmployee"
    | "onSetAsMainAndEnter"
    | "onEmployeeGroupsChanged"
    | "onOpenEmployeeCreatorSkill"
    | "onOpenFeishuSettings"
  > {
  openRequest?: EmployeeHubOpenRequest | null;
  onEnterStartTask?: (skillId?: string | null) => void;
  onRefreshEmployeeGroups?: () => Promise<void> | void;
  onLaunchEmployeeCreatorSkill?: (
    options?: EmployeeAssistantLaunchOptions,
  ) => Promise<void> | void;
  onOpenFeishuSettingsPanel?: () => void;
}

export function EmployeeHubScene(props: EmployeeHubSceneProps) {
  const {
    employees,
    onEnterStartTask,
    onLaunchEmployeeCreatorSkill,
    onOpenFeishuSettingsPanel,
    onRefreshEmployeeGroups,
    onRefreshEmployees,
    openRequest,
    ...viewProps
  } = props;
  const [selectedEmployeeId, setSelectedEmployeeId] = useState<string | null>(null);
  const [highlight, setHighlight] = useState<{
    employeeId: string | null;
    employeeName: string | null;
  }>({
    employeeId: null,
    employeeName: null,
  });

  useEffect(() => {
    setSelectedEmployeeId((prev) => {
      if (prev && employees.some((employee) => employee.id === prev)) {
        return prev;
      }
      return employees.find((employee) => employee.is_default)?.id ?? employees[0]?.id ?? null;
    });
  }, [employees]);

  useEffect(() => {
    if (!openRequest) {
      return;
    }
    setHighlight({
      employeeId: openRequest.highlightEmployeeId ?? null,
      employeeName: openRequest.highlightEmployeeName ?? null,
    });
    if (openRequest.highlightEmployeeId) {
      setSelectedEmployeeId(openRequest.highlightEmployeeId);
    }
  }, [openRequest]);

  const highlightMessage = highlight.employeeName
    ? `已由智能体员工助手生成：${highlight.employeeName}`
    : null;

  async function refreshEmployees() {
    await onRefreshEmployees?.();
  }

  async function handleDeleteEmployee(employeeId: string) {
    await deleteAgentEmployee(employeeId);
    await refreshEmployees();
    setHighlight((prev) =>
      prev.employeeId === employeeId
        ? {
            employeeId: null,
            employeeName: null,
          }
        : prev,
    );
  }

  async function handleSetAsMainAndEnter(employeeId: string) {
    const employee = employees.find((item) => item.id === employeeId);
    if (!employee) {
      return;
    }
    await upsertAgentEmployee(buildDefaultEmployeeUpdateInput(employee));
    await refreshEmployees();
    onEnterStartTask?.(employee.primary_skill_id);
  }

  async function handleEmployeeGroupsChanged() {
    await onRefreshEmployeeGroups?.();
  }

  async function handleOpenEmployeeCreatorSkill(
    options?: EmployeeAssistantLaunchOptions,
  ) {
    setHighlight({
      employeeId: null,
      employeeName: null,
    });
    await onLaunchEmployeeCreatorSkill?.(options);
  }

  function handleOpenFeishuSettings() {
    setHighlight({
      employeeId: null,
      employeeName: null,
    });
    onOpenFeishuSettingsPanel?.();
  }

  return (
    <EmployeeHubView
      {...viewProps}
      employees={employees}
      initialTab={openRequest?.tab}
      selectedEmployeeId={selectedEmployeeId}
      onSelectEmployee={setSelectedEmployeeId}
      highlightEmployeeId={highlight.employeeId}
      highlightMessage={highlightMessage}
      onRefreshEmployees={onRefreshEmployees}
      onDeleteEmployee={handleDeleteEmployee}
      onSetAsMainAndEnter={handleSetAsMainAndEnter}
      onEmployeeGroupsChanged={handleEmployeeGroupsChanged}
      onOpenEmployeeCreatorSkill={handleOpenEmployeeCreatorSkill}
      onOpenFeishuSettings={handleOpenFeishuSettings}
      onDismissHighlight={() => {
        setHighlight({
          employeeId: null,
          employeeName: null,
        });
      }}
    />
  );
}
