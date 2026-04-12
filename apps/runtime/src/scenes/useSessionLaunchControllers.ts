import type { Dispatch, SetStateAction } from "react";
import type { MainView } from "./useAppUiState";
import { useEmployeeSessionLaunchCoordinator } from "./employees/useEmployeeSessionLaunchCoordinator";
import type { EmployeeAssistantSessionContext } from "./employees/employeeAssistantService";
import { useGeneralSessionLaunchCoordinator } from "./useGeneralSessionLaunchCoordinator";
import type {
  AgentEmployee,
  EmployeeGroup,
  LandingSessionLaunchInput,
  ModelConfig,
  PendingAttachment,
  SkillManifest,
} from "../types";

type CreateRuntimeSessionInput = {
  skillId: string;
  modelId: string;
  workDir?: string;
  employeeId?: string;
  title?: string;
  sessionMode: "general" | "employee_direct" | "team_entry";
  teamId?: string;
};

export function useSessionLaunchControllers(options: {
  activateRuntimeSessionTab: (sessionId: string, tabId: string) => void;
  appendRuntimeOptimisticSession: (input: {
    sessionId: string;
    skillId: string;
    modelId: string;
    title?: string;
    initialUserMessage?: string;
    employeeId?: string;
    sessionMode: "general" | "employee_direct" | "team_entry";
    teamId?: string;
    workDir?: string;
  }) => void;
  createRuntimeSession: (input: CreateRuntimeSessionInput) => Promise<string>;
  creatingSession: boolean;
  defaultSkillId: string | null;
  employeeGroups: EmployeeGroup[];
  employees: AgentEmployee[];
  loadEmployeeGroups: () => Promise<EmployeeGroup[]>;
  loadEmployees: () => Promise<AgentEmployee[]>;
  loadSessions: (skillId: string) => void | Promise<void>;
  loadSkills: () => Promise<SkillManifest[]>;
  models: ModelConfig[];
  navigate: (view: MainView) => void;
  openSessionInActiveTab: (sessionId: string) => void;
  prepareTabForNewTask: () => string;
  resolveSessionLaunchWorkDir: (preferredWorkDir?: string) => Promise<string>;
  setCreateSessionError: Dispatch<SetStateAction<string | null>>;
  setCreatingSession: Dispatch<SetStateAction<boolean>>;
  setEmployeeAssistantSessionContexts: Dispatch<
    SetStateAction<Record<string, EmployeeAssistantSessionContext>>
  >;
  setPendingInitialAttachments: Dispatch<
    SetStateAction<
      | {
          sessionId: string;
          attachments: PendingAttachment[];
        }
      | null
    >
  >;
  setPendingInitialMessage: Dispatch<
    SetStateAction<{ sessionId: string; message: string } | null>
  >;
  setSelectedSkillId: Dispatch<SetStateAction<string | null>>;
  skills: SkillManifest[];
}) {
  const {
    activateRuntimeSessionTab,
    appendRuntimeOptimisticSession,
    createRuntimeSession,
    creatingSession,
    defaultSkillId,
    employeeGroups,
    employees,
    loadEmployeeGroups,
    loadEmployees,
    loadSessions,
    loadSkills,
    models,
    navigate,
    openSessionInActiveTab,
    prepareTabForNewTask,
    resolveSessionLaunchWorkDir,
    setCreateSessionError,
    setCreatingSession,
    setEmployeeAssistantSessionContexts,
    setPendingInitialAttachments,
    setPendingInitialMessage,
    setSelectedSkillId,
    skills,
  } = options;

  const employeeSessionLaunch = useEmployeeSessionLaunchCoordinator({
    employees,
    employeeGroups,
    skills,
    models,
    defaultSkillId,
    creatingSession,
    loadEmployees,
    loadEmployeeGroups,
    loadSkills,
    loadSessions,
    navigate: (view) => navigate(view),
    prepareTabForNewTask,
    setSelectedSkillId,
    setCreateSessionError,
    setCreatingSession,
    resolveSessionLaunchWorkDir,
    createRuntimeSession,
    appendOptimisticEmployeeSession: (input) => appendRuntimeOptimisticSession(input),
    activateSessionTab: activateRuntimeSessionTab,
    activateExistingSession: (sessionId) => {
      openSessionInActiveTab(sessionId);
    },
    setPendingInitialMessage,
    setEmployeeAssistantSessionContexts,
  });

  const generalSessionLaunch = useGeneralSessionLaunchCoordinator({
    skills,
    models,
    defaultSkillId,
    creatingSession,
    prepareTabForNewTask,
    setSelectedSkillId,
    setCreateSessionError,
    setCreatingSession,
    resolveSessionLaunchWorkDir,
    createRuntimeSession,
    appendOptimisticGeneralSession: (input) =>
      appendRuntimeOptimisticSession({
        ...input,
        sessionMode: "general",
      }),
    activateSessionTab: activateRuntimeSessionTab,
    loadSessions,
    navigate: () => navigate("start-task"),
    setPendingInitialMessage,
    setPendingInitialAttachments,
  });

  return {
    ...employeeSessionLaunch,
    ...generalSessionLaunch,
  };
}
