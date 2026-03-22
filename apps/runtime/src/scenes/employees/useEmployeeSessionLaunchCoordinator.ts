import { useCallback } from "react";
import type { Dispatch, SetStateAction } from "react";
import { getDefaultModelId } from "../../lib/default-model";
import type {
  AgentEmployee,
  EmployeeGroup,
  ModelConfig,
  SkillManifest,
} from "../../types";
import {
  BUILTIN_EMPLOYEE_CREATOR_SKILL_ID,
  EMPLOYEE_ASSISTANT_DISPLAY_NAME,
  hasEmployeeAssistantSkill,
  resolveEmployeeAssistantLaunchContext,
  type EmployeeAssistantLaunchOptions,
  type EmployeeAssistantSessionContext,
} from "./employeeAssistantService";
import { resolveEmployeeDirectLaunchContext } from "./employeeDirectSessionService";

type CreateRuntimeSessionInput = {
  skillId: string;
  modelId: string;
  workDir?: string;
  employeeId?: string;
  title?: string;
  sessionMode: "general" | "employee_direct" | "team_entry";
  teamId?: string;
};

export function useEmployeeSessionLaunchCoordinator(options: {
  employees: AgentEmployee[];
  employeeGroups: EmployeeGroup[];
  skills: SkillManifest[];
  models: ModelConfig[];
  defaultSkillId: string | null;
  creatingSession: boolean;
  loadEmployees: () => Promise<AgentEmployee[]>;
  loadEmployeeGroups: () => Promise<EmployeeGroup[]>;
  loadSkills: () => Promise<SkillManifest[]>;
  loadSessions: (skillId: string) => void | Promise<void>;
  navigate: (view: "start-task" | "experts") => void;
  prepareTabForNewTask: () => string;
  setSelectedSkillId: Dispatch<SetStateAction<string | null>>;
  setCreateSessionError: Dispatch<SetStateAction<string | null>>;
  setCreatingSession: Dispatch<SetStateAction<boolean>>;
  resolveSessionLaunchWorkDir: (preferredWorkDir?: string) => Promise<string>;
  createRuntimeSession: (input: CreateRuntimeSessionInput) => Promise<string>;
  appendOptimisticEmployeeSession: (input: {
    sessionId: string;
    skillId: string;
    modelId: string;
    title: string;
    employeeId: string;
    sessionMode: "employee_direct" | "team_entry";
    teamId?: string;
    workDir: string;
  }) => void;
  activateSessionTab: (sessionId: string, tabId: string) => void;
  activateExistingSession: (sessionId: string) => void;
  setPendingInitialMessage: Dispatch<
    SetStateAction<{ sessionId: string; message: string } | null>
  >;
  setEmployeeAssistantSessionContexts: Dispatch<
    SetStateAction<Record<string, EmployeeAssistantSessionContext>>
  >;
}) {
  const {
    activateSessionTab,
    activateExistingSession,
    appendOptimisticEmployeeSession,
    createRuntimeSession,
    creatingSession,
    defaultSkillId,
    loadEmployeeGroups,
    loadEmployees,
    employees,
    employeeGroups,
    loadSessions,
    loadSkills,
    models,
    navigate,
    prepareTabForNewTask,
    resolveSessionLaunchWorkDir,
    setCreateSessionError,
    setCreatingSession,
    setEmployeeAssistantSessionContexts,
    setPendingInitialMessage,
    setSelectedSkillId,
    skills,
  } = options;

  const handleStartTaskWithEmployee = useCallback(
    async (employeeId: string) => {
      if (creatingSession) return;

      const launchContext = resolveEmployeeDirectLaunchContext(
        employees,
        employeeId,
        defaultSkillId,
      );
      if (!launchContext) return;

      const modelId = getDefaultModelId(models);
      const targetTabId = prepareTabForNewTask();
      if (launchContext.skillId) {
        setSelectedSkillId(launchContext.skillId);
      }
      setCreateSessionError(null);
      navigate("start-task");

      if (!launchContext.skillId || !modelId) {
        return;
      }
      const skillId = launchContext.skillId;

      setCreatingSession(true);
      try {
        const workDir = await resolveSessionLaunchWorkDir(
          launchContext.defaultWorkDir,
        );
        const sessionId = await createRuntimeSession({
          skillId,
          modelId,
          workDir,
          employeeId: launchContext.employeeCode,
          title: launchContext.sessionTitle,
          sessionMode: "employee_direct",
        });
        appendOptimisticEmployeeSession({
          sessionId,
          skillId,
          modelId,
          title: launchContext.sessionTitle,
          employeeId: launchContext.employeeCode,
          sessionMode: "employee_direct",
          workDir,
        });
        activateSessionTab(sessionId, targetTabId);
        void loadSessions(skillId);
      } catch (error) {
        console.error("从员工页创建会话失败:", error);
        setCreateSessionError("创建会话失败，请稍后重试");
      } finally {
        setCreatingSession(false);
      }
    },
    [
      activateSessionTab,
      appendOptimisticEmployeeSession,
      createRuntimeSession,
      creatingSession,
      defaultSkillId,
      employees,
      loadSessions,
      models,
      navigate,
      prepareTabForNewTask,
      resolveSessionLaunchWorkDir,
      setCreateSessionError,
      setCreatingSession,
      setSelectedSkillId,
      skills,
    ],
  );

  const handleOpenEmployeeCreatorSkill = useCallback(
    async (launchOptions?: EmployeeAssistantLaunchOptions) => {
      if (creatingSession) return;

      const launchContext = resolveEmployeeAssistantLaunchContext(
        employees,
        launchOptions,
      );
      let nextSkills = skills;
      if (!hasEmployeeAssistantSkill(nextSkills)) {
        try {
          nextSkills = await loadSkills();
        } catch (error) {
          console.error(
            `加载${EMPLOYEE_ASSISTANT_DISPLAY_NAME}内置技能失败:`,
            error,
          );
        }
      }

      if (!hasEmployeeAssistantSkill(nextSkills)) {
        setCreateSessionError(
          `${EMPLOYEE_ASSISTANT_DISPLAY_NAME}暂未就绪，请稍后重试`,
        );
        navigate("experts");
        return;
      }

      const skillId = BUILTIN_EMPLOYEE_CREATOR_SKILL_ID;
      const modelId = getDefaultModelId(models);
      const targetTabId = prepareTabForNewTask();

      setSelectedSkillId(skillId);
      setCreateSessionError(null);
      navigate("start-task");

      if (!modelId) {
        return;
      }

      setCreatingSession(true);
      try {
        const workDir = await resolveSessionLaunchWorkDir(
          launchContext.defaultWorkDir,
        );
        const sessionId = await createRuntimeSession({
          skillId,
          modelId,
          workDir,
          employeeId: launchContext.employeeCode,
          title: launchContext.sessionTitle,
          sessionMode: launchContext.employeeCode
            ? "employee_direct"
            : "general",
        });
        activateSessionTab(sessionId, targetTabId);
        void loadSessions(skillId);
        setPendingInitialMessage({
          sessionId,
          message: launchContext.initialMessage,
        });
        setEmployeeAssistantSessionContexts((prev) => ({
          ...prev,
          [sessionId]: launchContext.sessionContext,
        }));
      } catch (error) {
        console.error(`打开${EMPLOYEE_ASSISTANT_DISPLAY_NAME}失败:`, error);
        setCreateSessionError("创建会话失败，请稍后重试");
      } finally {
        setCreatingSession(false);
      }
    },
    [
      activateSessionTab,
      createRuntimeSession,
      creatingSession,
      employees,
      loadSessions,
      loadSkills,
      models,
      navigate,
      prepareTabForNewTask,
      resolveSessionLaunchWorkDir,
      setCreateSessionError,
      setCreatingSession,
      setEmployeeAssistantSessionContexts,
      setPendingInitialMessage,
      setSelectedSkillId,
      skills,
    ],
  );

  const handleCreateTeamEntrySession = useCallback(
    async (input: { teamId: string; initialMessage?: string }) => {
      const teamId = (input.teamId || "").trim();
      const initialMessage = (input.initialMessage || "").trim();
      const modelId = getDefaultModelId(models);
      if (!teamId || !modelId || creatingSession) return;

      let nextGroups = employeeGroups;
      let group = nextGroups.find((item) => item.id === teamId);
      if (!group) {
        try {
          nextGroups = await loadEmployeeGroups();
          group = nextGroups.find((item) => item.id === teamId);
        } catch (error) {
          console.error("加载协作团队失败:", error);
        }
      }
      if (!group) {
        setCreateSessionError("未找到可用的协作团队");
        return;
      }

      const entryEmployeeCode = (
        group.entry_employee_id ||
        group.coordinator_employee_id ||
        ""
      ).trim();
      let nextEmployees = employees;
      let entryEmployee = nextEmployees.find((item) => {
        const code = (item.employee_id || item.role_id || "").trim();
        return code === entryEmployeeCode;
      });
      if (!entryEmployee && entryEmployeeCode) {
        try {
          nextEmployees = await loadEmployees();
          entryEmployee = nextEmployees.find((item) => {
            const code = (item.employee_id || item.role_id || "").trim();
            return code === entryEmployeeCode;
          });
        } catch (error) {
          console.error("加载员工列表失败:", error);
        }
      }
      const skillId = entryEmployee?.primary_skill_id || defaultSkillId;
      if (!skillId) return;

      const targetTabId = prepareTabForNewTask();
      setCreatingSession(true);
      setCreateSessionError(null);
      try {
        setSelectedSkillId(skillId);
        const workDir = await resolveSessionLaunchWorkDir(
          entryEmployee?.default_work_dir,
        );
        const sessionId = await createRuntimeSession({
          skillId,
          modelId,
          workDir,
          employeeId: entryEmployee?.employee_id || entryEmployee?.role_id || "",
          title: group.name || "团队协作",
          sessionMode: "team_entry",
          teamId,
        });
        appendOptimisticEmployeeSession({
          sessionId,
          skillId,
          modelId,
          title: group.name || "团队协作",
          employeeId: entryEmployee?.employee_id || entryEmployee?.role_id || "",
          sessionMode: "team_entry",
          teamId,
          workDir,
        });
        activateSessionTab(sessionId, targetTabId);
        void loadSessions(skillId);
        if (initialMessage) {
          setPendingInitialMessage({ sessionId, message: initialMessage });
        }
      } catch (error) {
        console.error("创建团队会话失败:", error);
        setCreateSessionError("创建团队会话失败，请稍后重试");
      } finally {
        setCreatingSession(false);
      }
    },
    [
      activateSessionTab,
      appendOptimisticEmployeeSession,
      createRuntimeSession,
      creatingSession,
      defaultSkillId,
      employeeGroups,
      employees,
      loadEmployeeGroups,
      loadEmployees,
      loadSessions,
      models,
      prepareTabForNewTask,
      resolveSessionLaunchWorkDir,
      setCreateSessionError,
      setCreatingSession,
      setPendingInitialMessage,
      setSelectedSkillId,
    ],
  );

  const handleOpenGroupRunSession = useCallback(
    async (sessionId: string, skillId: string) => {
      setSelectedSkillId(skillId);
      setCreateSessionError(null);
      activateExistingSession(sessionId);
      navigate("start-task");
      void loadSessions(skillId);
    },
    [
      activateExistingSession,
      loadSessions,
      navigate,
      setCreateSessionError,
      setSelectedSkillId,
    ],
  );

  return {
    handleCreateTeamEntrySession,
    handleOpenEmployeeCreatorSkill,
    handleOpenGroupRunSession,
    handleStartTaskWithEmployee,
  };
}
