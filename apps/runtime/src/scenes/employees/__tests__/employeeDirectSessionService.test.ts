import { describe, expect, it } from "vitest";
import { resolveEmployeeDirectLaunchContext } from "../employeeDirectSessionService";
import type { AgentEmployee } from "../../../types";

const employees: AgentEmployee[] = [
  {
    id: "emp-sales",
    employee_id: "sales_lead",
    name: "销售主管",
    role_id: "sales_lead",
    persona: "",
    feishu_open_id: "",
    feishu_app_id: "",
    feishu_app_secret: "",
    primary_skill_id: "skill-sales",
    default_work_dir: "D:\\workspace\\sales",
    openclaw_agent_id: "sales_lead",
    routing_priority: 100,
    enabled_scopes: ["app"],
    enabled: true,
    is_default: false,
    skill_ids: ["skill-sales"],
    created_at: "",
    updated_at: "",
  },
];

describe("employeeDirectSessionService", () => {
  it("resolves employee direct launch context from the selected employee", () => {
    expect(
      resolveEmployeeDirectLaunchContext(employees, "emp-sales", "builtin-general"),
    ).toEqual({
      employee: employees[0],
      skillId: "skill-sales",
      employeeCode: "sales_lead",
      sessionTitle: "销售主管",
      defaultWorkDir: "D:\\workspace\\sales",
    });
  });

  it("falls back to the default skill when employee primary skill is missing", () => {
    expect(
      resolveEmployeeDirectLaunchContext(
        [{ ...employees[0], primary_skill_id: "" }],
        "emp-sales",
        "builtin-general",
      ),
    ).toMatchObject({
      skillId: "builtin-general",
    });
  });

  it("returns null for unknown employees", () => {
    expect(
      resolveEmployeeDirectLaunchContext(employees, "missing", "builtin-general"),
    ).toBeNull();
  });
});
