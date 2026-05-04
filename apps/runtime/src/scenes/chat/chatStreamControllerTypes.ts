import type { RefObject } from "react";

import type { PersistedChatRuntimeState } from "../../types";
import type { PendingApprovalView } from "./useChatSessionController";

export type ChatStreamReasoningState = {
  status: "thinking" | "completed" | "interrupted";
  content: string;
  durationMs?: number;
} | null;

export type PendingApprovalEventPayload = {
  approval_id?: string;
  session_id: string;
  tool_name: string;
  tool_input?: Record<string, unknown>;
  input?: Record<string, unknown>;
  title?: string;
  summary?: string;
  impact?: string | null;
  irreversible?: boolean;
  status?: string;
};

export type AskUserEventPayload = {
  session_id: string;
  question: string;
  options: string[];
};

export type AgentStateEventPayload = {
  session_id: string;
  state: string;
  detail: string | null;
  iteration: number;
  stop_reason_kind?: string | null;
  stop_reason_title?: string | null;
  stop_reason_message?: string | null;
  stop_reason_last_completed_step?: string | null;
};

export type UseChatStreamControllerArgs = {
  sessionId: string;
  suppressAskUserPrompt: boolean;
  initialRuntimeState: PersistedChatRuntimeState;
  loadMessages: (sessionId: string) => Promise<void>;
  loadSessionRuns: (sessionId: string) => Promise<void>;
  pendingApprovalsRef: RefObject<PendingApprovalView[]>;
  resolvingApprovalIdRef: RefObject<string | null>;
  buildPendingApproval: (payload: PendingApprovalEventPayload) => PendingApprovalView;
  upsertPendingApproval: (approval: PendingApprovalView) => void;
  removePendingApproval: (approvalId: string) => void;
  onResetForSessionSwitch: () => void;
};
