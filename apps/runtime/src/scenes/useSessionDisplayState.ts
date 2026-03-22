import { useCallback, useMemo } from "react";
import type { TaskTabStripItem } from "../components/TaskTabStrip";
import {
  resolveSelectedEmployeeAssistantContext,
  resolveSelectedSessionEmployeeName,
} from "./employees/employeeSessionSelectors";
import type { EmployeeAssistantSessionContext } from "./employees/employeeAssistantService";
import type {
  AgentEmployee,
  EmployeeGroup,
  SessionInfo,
  SkillManifest,
} from "../types";

type WorkTab =
  | {
      id: string;
      kind: "start-task";
    }
  | {
      id: string;
      kind: "session";
      sessionId: string;
    };

const DEFAULT_SESSION_TITLE = "New Chat";

export function useSessionDisplayState(options: {
  employeeAssistantSessionContexts: Record<string, EmployeeAssistantSessionContext>;
  employeeGroups: EmployeeGroup[];
  employees: AgentEmployee[];
  getEffectiveSessionRuntimeStatus: (sessionId?: string | null, runtimeStatus?: string | null) => string | null;
  imManagedSessionIds: string[];
  selectedSessionId: string | null;
  selectedSkillId: string | null;
  skills: SkillManifest[];
  tabs: WorkTab[];
  visibleSessions: SessionInfo[];
}) {
  const {
    employeeAssistantSessionContexts,
    employeeGroups,
    employees,
    getEffectiveSessionRuntimeStatus,
    imManagedSessionIds,
    selectedSessionId,
    selectedSkillId,
    skills,
    tabs,
    visibleSessions,
  } = options;

  const landingTeams = useMemo(() => {
    return employeeGroups.map((group) => {
      const entryCode = (group.entry_employee_id || group.coordinator_employee_id || "").trim();
      const entryEmployee = employees.find((item) => (item.employee_id || item.role_id || "").trim() === entryCode);
      const coordinatorEmployee = employees.find(
        (item) => (item.employee_id || item.role_id || "").trim() === (group.coordinator_employee_id || "").trim(),
      );
      return {
        id: group.id,
        name: group.name,
        description: `入口：${entryEmployee?.name || entryCode || "未设置"} · 协调：${
          coordinatorEmployee?.name || group.coordinator_employee_id || "未设置"
        }`,
        memberCount: group.member_count || group.member_employee_ids?.length || 0,
      };
    });
  }, [employeeGroups, employees]);

  const selectedSkill = useMemo(
    () => skills.find((skill) => skill.id === selectedSkillId) ?? null,
    [selectedSkillId, skills],
  );

  const selectedSession = useMemo(
    () => visibleSessions.find((session) => session.id === selectedSessionId),
    [selectedSessionId, visibleSessions],
  );

  const taskTabs = useMemo<TaskTabStripItem[]>(() => {
    return tabs.map((tab) => {
      if (tab.kind === "session") {
        const session = visibleSessions.find((item) => item.id === tab.sessionId);
        return {
          id: tab.id,
          kind: tab.kind,
          title: (session?.display_title || session?.title || "").trim() || DEFAULT_SESSION_TITLE,
          runtimeStatus: getEffectiveSessionRuntimeStatus(session?.id || tab.sessionId, session?.runtime_status),
        };
      }
      return {
        id: tab.id,
        kind: tab.kind,
        title: "开始任务",
      };
    });
  }, [getEffectiveSessionRuntimeStatus, tabs, visibleSessions]);

  const findEmployeeBySessionReference = useCallback(
    (employeeRef?: string | null) => {
      const normalizedRef = (employeeRef || "").trim().toLowerCase();
      if (!normalizedRef) return undefined;
      return employees.find((item) => {
        const employeeCode = (item.employee_id || item.role_id || item.id || "").trim().toLowerCase();
        return employeeCode === normalizedRef || item.id.trim().toLowerCase() === normalizedRef;
      });
    },
    [employees],
  );

  const selectedSessionEmployeeName = resolveSelectedSessionEmployeeName(
    selectedSession,
    findEmployeeBySessionReference,
  );

  const selectedEmployeeAssistantContext = resolveSelectedEmployeeAssistantContext({
    selectedSkillId: selectedSkill?.id,
    selectedSessionId,
    selectedSession,
    employeeAssistantSessionContexts,
    findEmployeeBySessionReference,
  });

  const selectedSessionImManaged = selectedSessionId ? imManagedSessionIds.includes(selectedSessionId) : false;

  return {
    landingTeams,
    selectedEmployeeAssistantContext,
    selectedSession,
    selectedSessionEmployeeName,
    selectedSessionImManaged,
    selectedSkill,
    taskTabs,
  };
}
