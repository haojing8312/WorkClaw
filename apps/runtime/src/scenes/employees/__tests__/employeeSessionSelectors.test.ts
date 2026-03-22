import { describe, expect, it } from "vitest";
import type { SessionInfo } from "../../../types";
import {
  resolveEmployeeAssistantQuickPrompts,
  resolveSelectedEmployeeAssistantContext,
  resolveSelectedSessionEmployeeName,
} from "../employeeSessionSelectors";

const session: SessionInfo = {
  id: "session-1",
  title: "员工会话",
  created_at: "",
  model_id: "model-a",
  employee_id: "employee.alpha",
  session_mode: "employee_direct",
};

describe("employeeSessionSelectors", () => {
  it("prefers session employee_name and falls back to employee lookup", () => {
    expect(
      resolveSelectedSessionEmployeeName(
        { ...session, employee_name: "Alpha From Session" },
        () => ({ name: "Lookup Alpha" }),
      ),
    ).toBe("Alpha From Session");

    expect(
      resolveSelectedSessionEmployeeName(session, () => ({ name: "Lookup Alpha" })),
    ).toBe("Lookup Alpha");
  });

  it("resolves employee assistant context for create and update sessions", () => {
    expect(
      resolveSelectedEmployeeAssistantContext({
        selectedSkillId: "builtin-employee-creator",
        selectedSessionId: "session-1",
        selectedSession: { ...session, employee_id: "" },
        employeeAssistantSessionContexts: {},
        findEmployeeBySessionReference: () => undefined,
      }),
    ).toEqual({ mode: "create" });

    expect(
      resolveSelectedEmployeeAssistantContext({
        selectedSkillId: "builtin-employee-creator",
        selectedSessionId: "session-1",
        selectedSession: session,
        employeeAssistantSessionContexts: {},
        findEmployeeBySessionReference: () => ({ name: "Alpha" }),
      }),
    ).toEqual({
      mode: "update",
      employeeName: "Alpha",
      employeeCode: "employee.alpha",
    });
  });

  it("returns quick prompts only for the employee assistant skill", () => {
    expect(resolveEmployeeAssistantQuickPrompts("builtin-employee-creator")).toHaveLength(
      5,
    );
    expect(resolveEmployeeAssistantQuickPrompts("builtin-general")).toBeUndefined();
  });
});
