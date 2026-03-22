import { useCallback } from "react";
import type { Dispatch, SetStateAction } from "react";
import type { PersistedChatRuntimeState, SessionInfo } from "../types";

type SessionBlockingStateUpdate = {
  blocking: boolean;
  status?: string | null;
};

function normalizeRuntimeStatus(status?: string | null): string {
  return (status || "").trim().toLowerCase();
}

function isBlockingRuntimeStatus(status?: string | null): boolean {
  return ["thinking", "running", "tool_calling", "waiting_approval"].includes(normalizeRuntimeStatus(status));
}

export function useSessionRuntimeStateCoordinator(options: {
  selectedSessionId: string | null;
  liveSessionRuntimeStatusById: Record<string, string>;
  setLiveSessionRuntimeStatusById: Dispatch<SetStateAction<Record<string, string>>>;
  setSessionRuntimeStateById: Dispatch<SetStateAction<Record<string, PersistedChatRuntimeState>>>;
}) {
  const {
    liveSessionRuntimeStatusById,
    selectedSessionId,
    setLiveSessionRuntimeStatusById,
    setSessionRuntimeStateById,
  } = options;

  const getEffectiveSessionRuntimeStatus = useCallback(
    (sessionId?: string | null, runtimeStatus?: string | null): string | null => {
      const normalizedSessionId = (sessionId || "").trim();
      if (normalizedSessionId && liveSessionRuntimeStatusById[normalizedSessionId]) {
        return liveSessionRuntimeStatusById[normalizedSessionId];
      }
      return runtimeStatus ?? null;
    },
    [liveSessionRuntimeStatusById],
  );

  const isSessionBlockingStartTaskReuse = useCallback(
    (session: SessionInfo | null | undefined, sessionId?: string | null): boolean => {
      return isBlockingRuntimeStatus(
        getEffectiveSessionRuntimeStatus(sessionId || session?.id, session?.runtime_status),
      );
    },
    [getEffectiveSessionRuntimeStatus],
  );

  const handleSelectedSessionBlockingStateChange = useCallback(
    (update: SessionBlockingStateUpdate) => {
      const sessionId = (selectedSessionId || "").trim();
      if (!sessionId) return;
      const nextStatus = normalizeRuntimeStatus(update.status);
      setLiveSessionRuntimeStatusById((prev) => {
        if (update.blocking && nextStatus) {
          if (prev[sessionId] === nextStatus) return prev;
          return {
            ...prev,
            [sessionId]: nextStatus,
          };
        }
        if (!prev[sessionId]) return prev;
        const next = { ...prev };
        delete next[sessionId];
        return next;
      });
    },
    [selectedSessionId, setLiveSessionRuntimeStatusById],
  );

  const handlePersistSessionRuntimeState = useCallback(
    (sessionId: string, state: PersistedChatRuntimeState) => {
      const normalizedSessionId = sessionId.trim();
      if (!normalizedSessionId) return;
      setSessionRuntimeStateById((prev) => ({
        ...prev,
        [normalizedSessionId]: state,
      }));
    },
    [setSessionRuntimeStateById],
  );

  return {
    getEffectiveSessionRuntimeStatus,
    handlePersistSessionRuntimeState,
    handleSelectedSessionBlockingStateChange,
    isSessionBlockingStartTaskReuse,
  };
}
