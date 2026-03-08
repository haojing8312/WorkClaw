import { AgentEmployee, EmployeeGroup } from "../../types";

export interface EmployeeHubRunLike {
  status?: string | null;
}

export type EmployeeHubEmployeeFilter = "all" | "available" | "missing-skills" | "pending-connection";
export type EmployeeHubTeamFilter = "all" | "incomplete-team";
export type EmployeeHubRunFilter = "all" | "running";

export interface EmployeeHubOverviewMetrics {
  employees: number;
  teams: number;
  availableEmployees: number;
  runningTeams: number;
  pendingItems: number;
}

export interface EmployeeHubPendingItem {
  id: "missing-skills" | "pending-connection" | "incomplete-team";
  label: string;
  count: number;
  targetTab: "employees" | "teams";
}

function hasIdentifier(employee: AgentEmployee): boolean {
  return Boolean((employee.employee_id || employee.role_id || "").trim());
}

function hasExplicitSkill(employee: AgentEmployee): boolean {
  return Boolean(employee.primary_skill_id.trim()) || employee.skill_ids.some((skillId) => skillId.trim().length > 0);
}

function hasDefaultAssistantFallback(employee: AgentEmployee): boolean {
  return hasIdentifier(employee);
}

function hasFeishuIntent(employee: AgentEmployee): boolean {
  return [employee.feishu_open_id, employee.feishu_app_id, employee.feishu_app_secret].some((value) => value.trim().length > 0);
}

function hasValidFeishuPair(employee: AgentEmployee): boolean {
  return employee.feishu_app_id.trim().length > 0 && employee.feishu_app_secret.trim().length > 0;
}

function isAvailableEmployee(employee: AgentEmployee): boolean {
  return employee.enabled && hasIdentifier(employee) && (hasExplicitSkill(employee) || hasDefaultAssistantFallback(employee));
}

function isMissingSkillsEmployee(employee: AgentEmployee): boolean {
  return employee.enabled && hasIdentifier(employee) && !hasExplicitSkill(employee);
}

function isPendingConnectionEmployee(employee: AgentEmployee): boolean {
  return employee.enabled && hasFeishuIntent(employee) && !hasValidFeishuPair(employee);
}

function isIncompleteTeam(group: EmployeeGroup): boolean {
  return !(group.entry_employee_id || "").trim() || !(group.coordinator_employee_id || "").trim();
}

export function matchesEmployeeHubEmployeeFilter(employee: AgentEmployee, filter: EmployeeHubEmployeeFilter): boolean {
  switch (filter) {
    case "available":
      return isAvailableEmployee(employee);
    case "missing-skills":
      return isMissingSkillsEmployee(employee);
    case "pending-connection":
      return isPendingConnectionEmployee(employee);
    case "all":
    default:
      return true;
  }
}

export function matchesEmployeeHubTeamFilter(group: EmployeeGroup, filter: EmployeeHubTeamFilter): boolean {
  switch (filter) {
    case "incomplete-team":
      return isIncompleteTeam(group);
    case "all":
    default:
      return true;
  }
}

export function matchesEmployeeHubRunFilter(run: EmployeeHubRunLike, filter: EmployeeHubRunFilter): boolean {
  switch (filter) {
    case "running":
      return ["queued", "running", "waiting_review"].includes(String(run.status || "").trim().toLowerCase());
    case "all":
    default:
      return true;
  }
}

export function buildEmployeeHubPendingItems(input: {
  employees: AgentEmployee[];
  groups: EmployeeGroup[];
}): EmployeeHubPendingItem[] {
  const missingSkills = input.employees.filter((employee) => isMissingSkillsEmployee(employee)).length;
  const pendingConnection = input.employees.filter((employee) => isPendingConnectionEmployee(employee)).length;
  const incompleteTeams = input.groups.filter((group) => isIncompleteTeam(group)).length;

  const items: EmployeeHubPendingItem[] = [];
  if (missingSkills > 0) {
    items.push({
      id: "missing-skills",
      label: `${missingSkills} 名员工待补充技能`,
      count: missingSkills,
      targetTab: "employees",
    });
  }
  if (pendingConnection > 0) {
    items.push({
      id: "pending-connection",
      label: `${pendingConnection} 名员工未完成连接配置`,
      count: pendingConnection,
      targetTab: "employees",
    });
  }
  if (incompleteTeams > 0) {
    items.push({
      id: "incomplete-team",
      label: `${incompleteTeams} 个团队角色不完整`,
      count: incompleteTeams,
      targetTab: "teams",
    });
  }
  return items;
}

export function buildEmployeeHubMetrics(input: {
  employees: AgentEmployee[];
  groups: EmployeeGroup[];
  runs?: EmployeeHubRunLike[];
}): EmployeeHubOverviewMetrics {
  const pendingItems = buildEmployeeHubPendingItems(input);
  const runningTeams = (input.runs || []).filter((run) => matchesEmployeeHubRunFilter(run, "running")).length;

  return {
    employees: input.employees.length,
    teams: input.groups.length,
    availableEmployees: input.employees.filter((employee) => isAvailableEmployee(employee)).length,
    runningTeams,
    pendingItems: pendingItems.length,
  };
}
