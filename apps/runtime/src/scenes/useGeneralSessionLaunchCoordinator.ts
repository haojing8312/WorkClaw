import { useCallback } from "react";
import type { Dispatch, SetStateAction } from "react";
import { getDefaultModelId } from "../lib/default-model";
import type {
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

function normalizeLandingSessionLaunchInput(
  input?: string | LandingSessionLaunchInput,
): LandingSessionLaunchInput {
  if (typeof input === "string" || typeof input === "undefined") {
    return {
      initialMessage: typeof input === "string" ? input : "",
      attachments: [],
      workDir: "",
    };
  }

  return {
    initialMessage:
      typeof input.initialMessage === "string" ? input.initialMessage : "",
    attachments: Array.isArray(input.attachments) ? input.attachments : [],
    workDir: typeof input.workDir === "string" ? input.workDir : "",
  };
}

export function useGeneralSessionLaunchCoordinator(options: {
  skills: SkillManifest[];
  models: ModelConfig[];
  defaultSkillId: string | null;
  creatingSession: boolean;
  prepareTabForNewTask: () => string;
  setSelectedSkillId: Dispatch<SetStateAction<string | null>>;
  setCreateSessionError: Dispatch<SetStateAction<string | null>>;
  setCreatingSession: Dispatch<SetStateAction<boolean>>;
  resolveSessionLaunchWorkDir: (preferredWorkDir?: string) => Promise<string>;
  createRuntimeSession: (input: CreateRuntimeSessionInput) => Promise<string>;
  appendOptimisticGeneralSession: (input: {
    sessionId: string;
    skillId: string;
    modelId: string;
    title?: string;
    initialUserMessage?: string;
    workDir: string;
  }) => void;
  activateSessionTab: (sessionId: string, tabId: string) => void;
  loadSessions: (skillId: string) => void | Promise<void>;
  navigate: (view: "start-task") => void;
  setPendingInitialMessage: Dispatch<
    SetStateAction<{ sessionId: string; message: string } | null>
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
}) {
  const {
    activateSessionTab,
    appendOptimisticGeneralSession,
    createRuntimeSession,
    creatingSession,
    defaultSkillId,
    loadSessions,
    models,
    navigate,
    prepareTabForNewTask,
    resolveSessionLaunchWorkDir,
    setCreateSessionError,
    setCreatingSession,
    setPendingInitialAttachments,
    setPendingInitialMessage,
    setSelectedSkillId,
    skills,
  } = options;

  const handleCreateSession = useCallback(
    async (initialInput: string | LandingSessionLaunchInput = "") => {
      const skillId = defaultSkillId;
      const modelId = getDefaultModelId(models);
      if (!skillId || !modelId || creatingSession) return;

      const launchInput = normalizeLandingSessionLaunchInput(initialInput);
      const targetTabId = prepareTabForNewTask();
      setCreatingSession(true);
      setCreateSessionError(null);
      try {
        setSelectedSkillId(skillId);
        const workDir = await resolveSessionLaunchWorkDir(launchInput.workDir);
        const sessionId = await createRuntimeSession({
          skillId,
          modelId,
          workDir,
          sessionMode: "general",
        });
        appendOptimisticGeneralSession({
          sessionId,
          skillId,
          modelId,
          initialUserMessage: launchInput.initialMessage,
          workDir,
        });
        activateSessionTab(sessionId, targetTabId);
        void loadSessions(skillId);

        const firstMessage = launchInput.initialMessage.trim();
        if (firstMessage) {
          setPendingInitialMessage({ sessionId, message: firstMessage });
        }
        if (launchInput.attachments.length > 0) {
          setPendingInitialAttachments({
            sessionId,
            attachments: launchInput.attachments,
          });
        }
      } catch (error) {
        console.error("创建会话失败:", error);
        setCreateSessionError("创建会话失败，请稍后重试");
      } finally {
        setCreatingSession(false);
      }
    },
    [
      activateSessionTab,
      appendOptimisticGeneralSession,
      createRuntimeSession,
      creatingSession,
      defaultSkillId,
      loadSessions,
      models,
      prepareTabForNewTask,
      resolveSessionLaunchWorkDir,
      setCreateSessionError,
      setCreatingSession,
      setPendingInitialAttachments,
      setPendingInitialMessage,
      setSelectedSkillId,
    ],
  );

  const handleStartTaskWithSkill = useCallback(
    async (skillId: string) => {
      if (creatingSession) return;

      const skill = skills.find((item) => item.id === skillId);
      if (!skill) {
        setCreateSessionError("未找到可用技能");
        return;
      }

      const modelId = getDefaultModelId(models);
      if (!modelId) {
        setCreateSessionError("请先在设置中配置模型和 API Key");
        return;
      }

      const targetTabId = prepareTabForNewTask();
      setSelectedSkillId(skill.id);
      setCreateSessionError(null);
      setCreatingSession(true);

      try {
        const workDir = await resolveSessionLaunchWorkDir();
        const sessionId = await createRuntimeSession({
          skillId: skill.id,
          modelId,
          workDir,
          title: skill.name,
          sessionMode: "general",
        });
        appendOptimisticGeneralSession({
          sessionId,
          skillId: skill.id,
          modelId,
          title: skill.name,
          workDir,
        });
        activateSessionTab(sessionId, targetTabId);
        navigate("start-task");
        void loadSessions(skill.id);
      } catch (error) {
        console.error("从专家技能页创建会话失败:", error);
        setCreateSessionError("创建会话失败，请稍后重试");
      } finally {
        setCreatingSession(false);
      }
    },
    [
      activateSessionTab,
      appendOptimisticGeneralSession,
      createRuntimeSession,
      creatingSession,
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

  return {
    handleCreateSession,
    handleStartTaskWithSkill,
  };
}
