import { useEffect, useRef, useState } from "react";

import type { PersistedChatRuntimeState, SessionRunProjection, Message } from "../../types";
import type { PendingApprovalRecord } from "../../services/chat/chatApprovalService";
import { getMessages, listSessionRuns, listSessions, updateSessionWorkspace } from "../../services/chat/chatSessionService";
import { listPendingApprovals as listPendingApprovalRecords } from "../../services/chat/chatApprovalService";

export interface PendingApprovalView {
  approvalId: string;
  approvalRecordId?: string;
  sessionId: string;
  toolName: string;
  toolInput: Record<string, unknown>;
  title: string;
  summary: string;
  impact?: string;
  irreversible?: boolean;
  status?: string;
  usesLegacyConfirm?: boolean;
}

type UseChatSessionControllerArgs = {
  sessionId: string;
  workDir?: string;
  initialMessage?: string;
  draftInput: string;
  persistedRuntimeState?: PersistedChatRuntimeState;
  runtimeSnapshot: PersistedChatRuntimeState;
  onPersistRuntimeState?: (state: PersistedChatRuntimeState) => void;
  onApplyPersistedRuntimeState: (state?: PersistedChatRuntimeState | null) => void;
  onDraftLoaded: (draft: string) => void;
  onResetForSessionSwitch: () => void;
  readSessionDraft: (sessionId: string) => string;
  clearSessionDraft: (sessionId: string) => void;
  persistSessionDraft: (sessionId: string, value: string) => void;
  mapPendingApprovalRecord: (record: PendingApprovalRecord) => PendingApprovalView;
};

export function useChatSessionController({
  sessionId,
  workDir,
  initialMessage,
  draftInput,
  persistedRuntimeState,
  runtimeSnapshot,
  onPersistRuntimeState,
  onApplyPersistedRuntimeState,
  onDraftLoaded,
  onResetForSessionSwitch,
  readSessionDraft,
  clearSessionDraft,
  persistSessionDraft,
  mapPendingApprovalRecord,
}: UseChatSessionControllerArgs) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [sessionRuns, setSessionRuns] = useState<SessionRunProjection[]>([]);
  const [pendingApprovals, setPendingApprovals] = useState<PendingApprovalView[]>([]);
  const [resolvingApprovalId, setResolvingApprovalId] = useState<string | null>(null);
  const [workspace, setWorkspace] = useState<string>((workDir || "").trim());
  const pendingApprovalsRef = useRef<PendingApprovalView[]>([]);
  const resolvingApprovalIdRef = useRef<string | null>(null);

  const loadWorkspace = async (sid: string) => {
    try {
      const sessions = await listSessions();
      const current = sessions.find((session) => session.id === sid);
      if (current) {
        setWorkspace(current.work_dir || "");
      }
    } catch (error) {
      console.error("加载工作空间失败:", error);
    }
  };

  const setSessionWorkspace = async (nextWorkspace: string) => {
    try {
      await updateSessionWorkspace(sessionId, nextWorkspace);
      setWorkspace(nextWorkspace);
    } catch (error) {
      console.error("更新工作空间失败:", error);
    }
  };

  const loadMessages = async (sid: string) => {
    try {
      const nextMessages = await getMessages(sid);
      setMessages(nextMessages);
    } catch (error) {
      console.error("加载历史消息失败:", error);
    }
  };

  const loadSessionRuns = async (sid: string) => {
    if (!sid) {
      setSessionRuns([]);
      return;
    }
    try {
      const runs = await listSessionRuns(sid);
      setSessionRuns(runs);
    } catch (error) {
      console.error("加载会话运行记录失败:", error);
    }
  };

  const loadPendingApprovals = async (sid: string) => {
    if (!sid) {
      setPendingApprovals([]);
      return;
    }
    try {
      const approvals = await listPendingApprovalRecords(sid);
      const fetchedApprovals = approvals.map(mapPendingApprovalRecord);
      setPendingApprovals((prev) => {
        const merged = [...fetchedApprovals];
        for (const approval of prev) {
          if (approval.sessionId !== sid) continue;
          if (merged.some((item) => item.approvalId === approval.approvalId)) continue;
          merged.push(approval);
        }
        return merged;
      });
    } catch (error) {
      console.error("加载待审批列表失败:", error);
    }
  };

  useEffect(() => {
    pendingApprovalsRef.current = pendingApprovals;
    resolvingApprovalIdRef.current = resolvingApprovalId;
  }, [pendingApprovals, resolvingApprovalId]);

  useEffect(() => {
    setWorkspace((workDir || "").trim());
  }, [sessionId, workDir]);

  useEffect(() => {
    if (!initialMessage?.trim()) {
      void loadMessages(sessionId);
      onDraftLoaded(readSessionDraft(sessionId));
    } else {
      setMessages([]);
      clearSessionDraft(sessionId);
      onDraftLoaded("");
    }
    void loadSessionRuns(sessionId);
    void loadPendingApprovals(sessionId);
    void loadWorkspace(sessionId);
    onApplyPersistedRuntimeState(persistedRuntimeState);
    onResetForSessionSwitch();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId]);

  useEffect(() => {
    onPersistRuntimeState?.(runtimeSnapshot);
  }, [onPersistRuntimeState, runtimeSnapshot]);

  useEffect(() => {
    persistSessionDraft(sessionId, draftInput);
  }, [draftInput, persistSessionDraft, sessionId]);

  return {
    messages,
    setMessages,
    sessionRuns,
    setSessionRuns,
    pendingApprovals,
    setPendingApprovals,
    pendingApprovalsRef,
    resolvingApprovalId,
    setResolvingApprovalId,
    resolvingApprovalIdRef,
    workspace,
    loadMessages,
    loadSessionRuns,
    loadPendingApprovals,
    loadWorkspace,
    updateWorkspace: setSessionWorkspace,
  };
}
