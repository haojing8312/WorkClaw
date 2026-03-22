import { useCallback, useState } from "react";

type SessionFocusRequest = {
  sessionId: string;
  snippet: string;
  nonce: number;
};

type GroupRunStepFocusRequest = {
  sessionId: string;
  stepId: string;
  eventId?: string;
  nonce: number;
};

type SessionExecutionTimelineItem = {
  eventId?: string;
  label: string;
  createdAt?: string;
};

type SessionExecutionContext = {
  targetSessionId: string;
  sourceSessionId: string;
  sourceStepId: string;
  sourceEmployeeId?: string;
  assigneeEmployeeId?: string;
  sourceStepTimeline?: SessionExecutionTimelineItem[];
};

type OpenSessionOptions = {
  focusHint?: string;
  groupRunStepFocusId?: string;
  groupRunEventFocusId?: string;
  sourceSessionId?: string;
  sourceStepId?: string;
  sourceEmployeeId?: string;
  assigneeEmployeeId?: string;
  sourceStepTimeline?: Array<{
    eventId?: string | null;
    label?: string | null;
    createdAt?: string | null;
  }>;
};

export function useChatSessionBridge(options: {
  openGroupRunSession: (sessionId: string, skillId: string) => void | Promise<void>;
}) {
  const { openGroupRunSession } = options;
  const [pendingSessionFocusRequest, setPendingSessionFocusRequest] =
    useState<SessionFocusRequest | null>(null);
  const [pendingSessionExecutionContext, setPendingSessionExecutionContext] =
    useState<SessionExecutionContext | null>(null);
  const [pendingGroupRunStepFocusRequest, setPendingGroupRunStepFocusRequest] =
    useState<GroupRunStepFocusRequest | null>(null);

  const handleOpenSession = useCallback(
    (nextSessionId: string, skillId: string, options?: OpenSessionOptions) => {
      const focusHint = (options?.focusHint || "").trim();
      const groupRunStepFocusId = (options?.groupRunStepFocusId || "").trim();
      const groupRunEventFocusId = (options?.groupRunEventFocusId || "").trim();
      setPendingSessionFocusRequest(
        focusHint
          ? {
              sessionId: nextSessionId,
              snippet: focusHint,
              nonce: Date.now(),
            }
          : null,
      );
      setPendingGroupRunStepFocusRequest(
        groupRunStepFocusId
          ? {
              sessionId: nextSessionId,
              stepId: groupRunStepFocusId,
              eventId: groupRunEventFocusId || undefined,
              nonce: Date.now(),
            }
          : null,
      );

      const sourceSessionId = (options?.sourceSessionId || "").trim();
      const sourceStepId = (options?.sourceStepId || "").trim();
      const sourceStepTimeline = (options?.sourceStepTimeline || [])
        .map((item) => ({
          eventId: (item?.eventId || "").trim() || undefined,
          label: (item?.label || "").trim(),
          createdAt: (item?.createdAt || "").trim() || undefined,
        }))
        .filter((item) => item.label.length > 0);

      setPendingSessionExecutionContext(
        sourceSessionId && sourceStepId
          ? {
              targetSessionId: nextSessionId,
              sourceSessionId,
              sourceStepId,
              sourceEmployeeId:
                (options?.sourceEmployeeId || "").trim() || undefined,
              assigneeEmployeeId:
                (options?.assigneeEmployeeId || "").trim() || undefined,
              sourceStepTimeline:
                sourceStepTimeline.length > 0 ? sourceStepTimeline : undefined,
            }
          : null,
      );

      return openGroupRunSession(nextSessionId, skillId);
    },
    [openGroupRunSession],
  );

  const handleReturnToSourceSession = useCallback(
    (sourceSessionId: string, skillId: string) => {
      setPendingGroupRunStepFocusRequest(null);
      setPendingSessionExecutionContext(null);
      return openGroupRunSession(sourceSessionId, skillId);
    },
    [openGroupRunSession],
  );

  const resolveSessionFocusRequest = useCallback(
    (selectedSessionId: string | null) => {
      if (
        !selectedSessionId ||
        !pendingSessionFocusRequest ||
        pendingSessionFocusRequest.sessionId !== selectedSessionId
      ) {
        return undefined;
      }
      return {
        nonce: pendingSessionFocusRequest.nonce,
        snippet: pendingSessionFocusRequest.snippet,
      };
    },
    [pendingSessionFocusRequest],
  );

  const resolveGroupRunStepFocusRequest = useCallback(
    (selectedSessionId: string | null) => {
      if (
        !selectedSessionId ||
        !pendingGroupRunStepFocusRequest ||
        pendingGroupRunStepFocusRequest.sessionId !== selectedSessionId
      ) {
        return undefined;
      }
      return {
        nonce: pendingGroupRunStepFocusRequest.nonce,
        stepId: pendingGroupRunStepFocusRequest.stepId,
        eventId: pendingGroupRunStepFocusRequest.eventId,
      };
    },
    [pendingGroupRunStepFocusRequest],
  );

  const resolveSessionExecutionContext = useCallback(
    (selectedSessionId: string | null) => {
      if (
        !selectedSessionId ||
        !pendingSessionExecutionContext ||
        pendingSessionExecutionContext.targetSessionId !== selectedSessionId
      ) {
        return undefined;
      }
      return {
        sourceSessionId: pendingSessionExecutionContext.sourceSessionId,
        sourceStepId: pendingSessionExecutionContext.sourceStepId,
        sourceEmployeeId: pendingSessionExecutionContext.sourceEmployeeId,
        assigneeEmployeeId: pendingSessionExecutionContext.assigneeEmployeeId,
        sourceStepTimeline: pendingSessionExecutionContext.sourceStepTimeline,
      };
    },
    [pendingSessionExecutionContext],
  );

  return {
    handleOpenSession,
    handleReturnToSourceSession,
    resolveGroupRunStepFocusRequest,
    resolveSessionExecutionContext,
    resolveSessionFocusRequest,
  };
}
