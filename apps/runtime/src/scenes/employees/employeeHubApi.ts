import { invoke } from "@tauri-apps/api/core";
import {
  type AgentEmployee,
  type EmployeeGroup,
  type UpsertAgentEmployeeInput,
} from "../../types";

export async function listAgentEmployees(): Promise<AgentEmployee[]> {
  const raw = await invoke<AgentEmployee[] | null>("list_agent_employees");
  return Array.isArray(raw) ? raw : [];
}

export async function listEmployeeGroups(): Promise<EmployeeGroup[]> {
  const raw = await invoke<EmployeeGroup[] | null>("list_employee_groups");
  return Array.isArray(raw) ? raw : [];
}

export async function upsertAgentEmployee(input: UpsertAgentEmployeeInput): Promise<string> {
  return invoke<string>("upsert_agent_employee", { input });
}

export async function deleteAgentEmployee(employeeId: string): Promise<void> {
  await invoke("delete_agent_employee", { employeeId });
}

export function buildDefaultEmployeeUpdateInput(
  employee: AgentEmployee,
): UpsertAgentEmployeeInput {
  return {
    id: employee.id,
    employee_id: employee.employee_id || employee.role_id,
    name: employee.name,
    role_id: employee.employee_id || employee.role_id,
    persona: employee.persona,
    feishu_open_id: employee.feishu_open_id,
    feishu_app_id: employee.feishu_app_id,
    feishu_app_secret: employee.feishu_app_secret,
    primary_skill_id: employee.primary_skill_id,
    default_work_dir: employee.default_work_dir,
    openclaw_agent_id:
      employee.employee_id || employee.openclaw_agent_id || employee.role_id,
    routing_priority: employee.routing_priority ?? 100,
    enabled_scopes: employee.enabled_scopes?.length
      ? employee.enabled_scopes
      : ["app"],
    enabled: employee.enabled,
    is_default: true,
    skill_ids: employee.skill_ids,
  };
}
