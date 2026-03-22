import type { SessionInfo } from "../../types";
import {
  BUILTIN_EMPLOYEE_CREATOR_SKILL_ID,
  EMPLOYEE_ASSISTANT_QUICK_PROMPTS,
  type EmployeeAssistantSessionContext,
} from "./employeeAssistantService";

type EmployeeLookup = (employeeRef?: string | null) => {
  name?: string;
} | null | undefined;

export function resolveSelectedSessionEmployeeName(
  session: SessionInfo | undefined,
  findEmployeeBySessionReference: EmployeeLookup,
): string | undefined {
  const projectedEmployeeName = (session?.employee_name || "").trim();
  if (projectedEmployeeName) {
    return projectedEmployeeName;
  }
  const sessionEmployeeId = (session?.employee_id || "").trim();
  const matchedEmployee = findEmployeeBySessionReference(sessionEmployeeId);
  if (matchedEmployee?.name) {
    return matchedEmployee.name;
  }
  if ((session?.session_mode || "").trim().toLowerCase() !== "employee_direct") {
    return undefined;
  }
  return undefined;
}

export function resolveSelectedEmployeeAssistantContext(options: {
  selectedSkillId?: string | null;
  selectedSessionId?: string | null;
  selectedSession?: SessionInfo;
  employeeAssistantSessionContexts: Record<string, EmployeeAssistantSessionContext>;
  findEmployeeBySessionReference: EmployeeLookup;
}): EmployeeAssistantSessionContext | undefined {
  if (
    options.selectedSkillId !== BUILTIN_EMPLOYEE_CREATOR_SKILL_ID ||
    !options.selectedSessionId
  ) {
    return undefined;
  }

  const fromSession =
    options.employeeAssistantSessionContexts[options.selectedSessionId];
  if (fromSession) {
    return fromSession;
  }

  const sessionEmployeeId = (options.selectedSession?.employee_id || "").trim();
  if (!sessionEmployeeId) {
    return { mode: "create" };
  }

  const matchedEmployee =
    options.findEmployeeBySessionReference(sessionEmployeeId);
  return {
    mode: "update",
    employeeName: matchedEmployee?.name,
    employeeCode: sessionEmployeeId,
  };
}

export function resolveEmployeeAssistantQuickPrompts(
  selectedSkillId?: string | null,
) {
  return selectedSkillId === BUILTIN_EMPLOYEE_CREATOR_SKILL_ID
    ? EMPLOYEE_ASSISTANT_QUICK_PROMPTS
    : undefined;
}
