import type { AgentEmployee } from "../../types";

export type EmployeeDirectLaunchContext = {
  employee: AgentEmployee;
  skillId: string | null;
  employeeCode: string;
  sessionTitle: string;
  defaultWorkDir: string;
};

export function resolveEmployeeDirectLaunchContext(
  employees: AgentEmployee[],
  employeeId: string,
  fallbackSkillId?: string | null,
): EmployeeDirectLaunchContext | null {
  const employee = employees.find((item) => item.id === employeeId);
  if (!employee) {
    return null;
  }

  return {
    employee,
    skillId: employee.primary_skill_id || fallbackSkillId || null,
    employeeCode: employee.employee_id || employee.role_id || "",
    sessionTitle: employee.name,
    defaultWorkDir: employee.default_work_dir,
  };
}
