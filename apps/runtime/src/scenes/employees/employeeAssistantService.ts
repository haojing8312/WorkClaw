import type { AgentEmployee } from "../../types";

export type EmployeeAssistantMode = "create" | "update";

export type EmployeeAssistantLaunchOptions = {
  mode?: EmployeeAssistantMode;
  employeeId?: string;
};

export type EmployeeAssistantSessionContext = {
  mode: EmployeeAssistantMode;
  employeeName?: string;
  employeeCode?: string;
};

export type EmployeeAssistantLaunchContext = {
  launchMode: EmployeeAssistantMode;
  targetEmployee: AgentEmployee | null;
  employeeCode: string;
  sessionTitle: string;
  initialMessage: string;
  sessionContext: EmployeeAssistantSessionContext;
  defaultWorkDir: string;
};

export const BUILTIN_EMPLOYEE_CREATOR_SKILL_ID = "builtin-employee-creator";
export const EMPLOYEE_ASSISTANT_DISPLAY_NAME = "智能体员工助手";
export const EMPLOYEE_CREATOR_STARTER_PROMPT =
  "请帮我创建一个新的智能体员工。先问我 1-2 个关键问题，再给出配置草案，确认后再执行创建。";

export const EMPLOYEE_ASSISTANT_QUICK_PROMPTS: Array<{
  label: string;
  prompt: string;
}> = [
  {
    label: "加技能",
    prompt:
      "请帮我修改一个已有智能体员工：给目标员工增加技能。你先调用 list_employees 和 list_skills，然后给出 update_employee 草案（使用 add_skill_ids），我确认后再执行。",
  },
  {
    label: "删技能",
    prompt:
      "请帮我修改一个已有智能体员工：给目标员工移除技能。你先调用 list_employees，再给出 update_employee 草案（使用 remove_skill_ids），我确认后再执行。",
  },
  {
    label: "改主技能",
    prompt:
      "请帮我修改一个已有智能体员工：调整 primary_skill_id。你先确认员工与目标主技能，再给出 update_employee 草案，我确认后再执行。",
  },
  {
    label: "改飞书配置",
    prompt:
      "请帮我修改一个已有智能体员工的飞书配置（open_id / app_id / app_secret）。你先确认目标员工，再给出 update_employee 草案，我确认后再执行。",
  },
  {
    label: "更新画像",
    prompt:
      "请帮我更新已有员工的 AGENTS/SOUL/USER 配置。你先引导我补齐 mission/responsibilities/collaboration/tone/boundaries/user_profile，再给出 update_employee + profile_answers 草案，我确认后再执行。",
  },
];

export function hasEmployeeAssistantSkill(
  skills: Array<{ id: string }>,
): boolean {
  return skills.some((item) => item.id === BUILTIN_EMPLOYEE_CREATOR_SKILL_ID);
}

export function buildEmployeeAssistantUpdatePrompt(
  employee: AgentEmployee,
): string {
  const employeeCode = (
    employee.employee_id ||
    employee.role_id ||
    employee.id ||
    ""
  ).trim();
  return `调整员工任务：请帮我修改智能体员工「${employee.name}」（employee_id: ${employeeCode}）。先确认修改目标，再给出 update_employee 配置草案（包含变更字段与理由），待我确认后再执行。`;
}

export function resolveEmployeeAssistantLaunchContext(
  employees: AgentEmployee[],
  options?: EmployeeAssistantLaunchOptions,
): EmployeeAssistantLaunchContext {
  const requestedMode: EmployeeAssistantMode =
    options?.mode === "update" ? "update" : "create";
  const targetEmployee =
    requestedMode === "update"
      ? employees.find((item) => item.id === (options?.employeeId || "")) ?? null
      : null;
  const launchMode: EmployeeAssistantMode =
    requestedMode === "update" && targetEmployee ? "update" : "create";
  const employeeCode =
    launchMode === "update" && targetEmployee
      ? (targetEmployee.employee_id || targetEmployee.role_id || "").trim()
      : "";

  return {
    launchMode,
    targetEmployee,
    employeeCode,
    sessionTitle:
      launchMode === "update" && targetEmployee
        ? `调整员工：${targetEmployee.name}`
        : "创建员工：新员工",
    initialMessage:
      launchMode === "update" && targetEmployee
        ? buildEmployeeAssistantUpdatePrompt(targetEmployee)
        : EMPLOYEE_CREATOR_STARTER_PROMPT,
    sessionContext:
      launchMode === "update" && targetEmployee
        ? {
            mode: "update",
            employeeName: targetEmployee.name,
            employeeCode,
          }
        : { mode: "create" },
    defaultWorkDir:
      launchMode === "update" && targetEmployee
        ? targetEmployee.default_work_dir
        : "",
  };
}
