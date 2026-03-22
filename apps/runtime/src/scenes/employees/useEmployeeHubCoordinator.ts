import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";
import type { AgentEmployee, EmployeeGroup } from "../../types";
import type { EmployeeHubOpenRequest } from "./EmployeeHubScene";
import { BUILTIN_EMPLOYEE_CREATOR_SKILL_ID } from "./employeeAssistantService";
import {
  createEmployeeHubOpenRequest,
  retargetEmployeeHubOpenRequest,
} from "./employeeHubNavigation";
import { listAgentEmployees, listEmployeeGroups } from "./employeeHubApi";

type EmployeeHubTab = EmployeeHubOpenRequest["tab"];

export function useEmployeeHubCoordinator(options: {
  employees: AgentEmployee[];
  selectedSkillId?: string | null;
  loadModels: () => Promise<unknown>;
  loadSessions: (skillId: string) => void | Promise<void>;
  setEmployees: Dispatch<SetStateAction<AgentEmployee[]>>;
  setEmployeeGroups: Dispatch<SetStateAction<EmployeeGroup[]>>;
  setShowSettings: Dispatch<SetStateAction<boolean>>;
  navigate: (view: "employees") => void;
  openSettingsAtTab: (tab: "feishu") => void;
}) {
  const {
    employees,
    loadModels,
    loadSessions,
    navigate,
    openSettingsAtTab,
    selectedSkillId,
    setEmployeeGroups,
    setEmployees,
    setShowSettings,
  } = options;
  const [employeeHubOpenRequest, setEmployeeHubOpenRequest] =
    useState<EmployeeHubOpenRequest | null>(null);
  const employeesRef = useRef<AgentEmployee[]>(employees);

  useEffect(() => {
    employeesRef.current = employees;
  }, [employees]);

  const loadEmployees = useCallback(async (): Promise<AgentEmployee[]> => {
    try {
      const list = await listAgentEmployees();
      setEmployees(list);
      return list;
    } catch {
      setEmployees([]);
      return [];
    }
  }, [setEmployees]);

  const loadEmployeeGroups = useCallback(async (): Promise<EmployeeGroup[]> => {
    try {
      const list = await listEmployeeGroups();
      setEmployeeGroups(list);
      return list;
    } catch {
      setEmployeeGroups([]);
      return [];
    }
  }, [setEmployeeGroups]);

  const openEmployeeHub = useCallback(
    async (tab: EmployeeHubTab = "overview") => {
      await loadModels();
      setEmployeeHubOpenRequest(createEmployeeHubOpenRequest(tab));
      setShowSettings(false);
      navigate("employees");
    },
    [loadModels, navigate, setShowSettings],
  );

  const retargetEmployeeHub = useCallback((tab: EmployeeHubTab = "overview") => {
    setEmployeeHubOpenRequest((prev) => retargetEmployeeHubOpenRequest(prev, tab));
  }, []);

  const handleSessionRefresh = useCallback(() => {
    if (selectedSkillId) {
      void loadSessions(selectedSkillId);
    }
    const previousEmployeeIds = new Set(
      employeesRef.current.map((item) => item.id),
    );
    void (async () => {
      try {
        const latest = await loadEmployees();
        if (selectedSkillId !== BUILTIN_EMPLOYEE_CREATOR_SKILL_ID) {
          return;
        }
        const created = latest.find((item) => !previousEmployeeIds.has(item.id));
        if (created) {
          setEmployeeHubOpenRequest(
            createEmployeeHubOpenRequest("employees", {
              highlightEmployeeId: created.id,
              highlightEmployeeName: created.name,
            }),
          );
        }
      } catch (error) {
        console.error("刷新员工列表失败:", error);
      }
    })();
  }, [loadEmployees, loadSessions, selectedSkillId]);

  const handleEmployeeGroupsChanged = useCallback(async () => {
    await loadEmployeeGroups();
  }, [loadEmployeeGroups]);

  const handleOpenEmployeeHubFeishuSettings = useCallback(() => {
    setEmployeeHubOpenRequest(createEmployeeHubOpenRequest("overview"));
    openSettingsAtTab("feishu");
  }, [openSettingsAtTab]);

  return {
    employeeHubOpenRequest,
    handleEmployeeGroupsChanged,
    handleOpenEmployeeHubFeishuSettings,
    handleSessionRefresh,
    loadEmployeeGroups,
    loadEmployees,
    openEmployeeHub,
    retargetEmployeeHub,
  };
}
