import { useCallback } from "react";

interface RuntimeSessionCoordinatorLike {
  activateSessionTab: (sessionId: string, tabId: string) => void;
  appendOptimisticSession: (input: {
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
  createRuntimeSession: (input: {
    skillId: string;
    modelId: string;
    workDir?: string;
    employeeId?: string;
    title?: string;
    sessionMode: "general" | "employee_direct" | "team_entry";
    teamId?: string;
  }) => Promise<unknown>;
  loadSessions: (_skillId: string, options?: { requestId?: number; attempt?: number }) => Promise<unknown>;
  prepareTabForNewTask: () => string;
}

export function useRuntimeSessionBindings(input: {
  runtimeSessionCoordinator: RuntimeSessionCoordinatorLike;
  selectedSkillId: string | null;
}) {
  const { runtimeSessionCoordinator, selectedSkillId } = input;

  const appendRuntimeOptimisticSession = useCallback(
    (options: Parameters<RuntimeSessionCoordinatorLike["appendOptimisticSession"]>[0]) => {
      runtimeSessionCoordinator.appendOptimisticSession(options);
    },
    [runtimeSessionCoordinator],
  );

  const activateRuntimeSessionTab = useCallback(
    (sessionId: string, tabId: string) => {
      runtimeSessionCoordinator.activateSessionTab(sessionId, tabId);
    },
    [runtimeSessionCoordinator],
  );

  const createRuntimeSession = useCallback(
    (options: Parameters<RuntimeSessionCoordinatorLike["createRuntimeSession"]>[0]) =>
      runtimeSessionCoordinator.createRuntimeSession(options),
    [runtimeSessionCoordinator],
  );

  const loadSessions = useCallback(
    (_skillId: string, options?: { requestId?: number; attempt?: number }) =>
      runtimeSessionCoordinator.loadSessions(_skillId, options),
    [runtimeSessionCoordinator],
  );

  const prepareTabForNewTask = useCallback(
    () => runtimeSessionCoordinator.prepareTabForNewTask(),
    [runtimeSessionCoordinator],
  );

  const refreshImSessionList = useCallback(() => {
    void loadSessions(selectedSkillId ?? "");
  }, [loadSessions, selectedSkillId]);

  return {
    activateRuntimeSessionTab,
    appendRuntimeOptimisticSession,
    createRuntimeSession,
    loadSessions,
    prepareTabForNewTask,
    refreshImSessionList,
  };
}
