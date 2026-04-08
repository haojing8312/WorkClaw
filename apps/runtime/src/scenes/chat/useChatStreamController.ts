import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";
import { listen } from "@tauri-apps/api/event";

import type {
  ChatRuntimeAgentState,
  PersistedChatRuntimeState,
  SessionToolManifestEntry,
  StreamItem,
} from "../../types";
import { confirmLegacyToolExecution, resolveApproval } from "../../services/chat/chatApprovalService";
import {
  subscribeChatStreamEvent,
  type AssistantReasoningCompletedEvent,
  type AssistantReasoningDeltaEvent,
  type AssistantReasoningInterruptedEvent,
  type AssistantReasoningStartedEvent,
  type SessionToolManifestEvent,
  type StreamTokenEvent,
  type ToolCallEvent,
} from "../../lib/chat-stream-events";
import type { PendingApprovalView } from "./useChatSessionController";

export type ChatStreamReasoningState = {
  status: "thinking" | "completed" | "interrupted";
  content: string;
  durationMs?: number;
} | null;

type PendingApprovalEventPayload = {
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

type AskUserEventPayload = {
  session_id: string;
  question: string;
  options: string[];
};

type AgentStateEventPayload = {
  session_id: string;
  state: string;
  detail: string | null;
  iteration: number;
  stop_reason_kind?: string | null;
  stop_reason_title?: string | null;
  stop_reason_message?: string | null;
  stop_reason_last_completed_step?: string | null;
};

type UseChatStreamControllerArgs = {
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

export function useChatStreamController({
  sessionId,
  suppressAskUserPrompt,
  initialRuntimeState,
  loadMessages,
  loadSessionRuns,
  pendingApprovalsRef,
  resolvingApprovalIdRef,
  buildPendingApproval,
  upsertPendingApproval,
  removePendingApproval,
  onResetForSessionSwitch,
}: UseChatStreamControllerArgs) {
  const buildPendingApprovalRef = useRef(buildPendingApproval);
  const upsertPendingApprovalRef = useRef(upsertPendingApproval);
  const removePendingApprovalRef = useRef(removePendingApproval);

  useEffect(() => {
    buildPendingApprovalRef.current = buildPendingApproval;
    upsertPendingApprovalRef.current = upsertPendingApproval;
    removePendingApprovalRef.current = removePendingApproval;
  }, [buildPendingApproval, upsertPendingApproval, removePendingApproval]);

  const [streaming, setStreaming] = useState(initialRuntimeState.streaming);
  const [streamItems, setStreamItems] = useState<StreamItem[]>(initialRuntimeState.streamItems);
  const streamItemsRef = useRef<StreamItem[]>(initialRuntimeState.streamItems);
  const [toolManifest, setToolManifest] = useState<SessionToolManifestEntry[]>(initialRuntimeState.toolManifest);
  const [streamReasoning, setStreamReasoning] = useState<ChatStreamReasoningState>(
    initialRuntimeState.streamReasoning ?? null,
  );
  const streamReasoningRef = useRef<ChatStreamReasoningState>(initialRuntimeState.streamReasoning ?? null);
  const [askUserQuestion, setAskUserQuestion] = useState<string | null>(null);
  const [askUserOptions, setAskUserOptions] = useState<string[]>([]);
  const [askUserAnswer, setAskUserAnswer] = useState("");
  const [agentState, setAgentState] = useState<ChatRuntimeAgentState | null>(initialRuntimeState.agentState);
  const [subAgentBuffer, setSubAgentBuffer] = useState(initialRuntimeState.subAgentBuffer);
  const [subAgentRoleName, setSubAgentRoleName] = useState(initialRuntimeState.subAgentRoleName);
  const subAgentBufferRef = useRef(initialRuntimeState.subAgentBuffer);

  const updateStreamReasoning = (updater: (prev: ChatStreamReasoningState) => ChatStreamReasoningState) => {
    setStreamReasoning((prev) => {
      const next = updater(prev);
      streamReasoningRef.current = next;
      return next;
    });
  };

  const clearAskUserPrompt = () => {
    setAskUserQuestion(null);
    setAskUserOptions([]);
    setAskUserAnswer("");
  };

  const resetForSessionSwitch = () => {
    clearAskUserPrompt();
    setAgentState(null);
    onResetForSessionSwitch();
  };

  const applyPersistedRuntimeState = (state?: PersistedChatRuntimeState | null) => {
    const next: PersistedChatRuntimeState = {
      streaming: state?.streaming ?? false,
      streamItems: state?.streamItems ? [...state.streamItems] : [],
      toolManifest: state?.toolManifest ? state.toolManifest.map((item) => ({ ...item })) : [],
      streamReasoning: state?.streamReasoning ? { ...state.streamReasoning } : null,
      agentState: state?.agentState ? { ...state.agentState } : null,
      subAgentBuffer: state?.subAgentBuffer ?? "",
      subAgentRoleName: state?.subAgentRoleName ?? "",
      mainRoleName: state?.mainRoleName ?? "",
      mainSummaryDelivered: state?.mainSummaryDelivered ?? false,
      delegationCards: state?.delegationCards ? state.delegationCards.map((item) => ({ ...item })) : [],
    };
    setStreaming(next.streaming);
    setStreamItems(next.streamItems);
    streamItemsRef.current = next.streamItems;
    setToolManifest(next.toolManifest);
    setStreamReasoning(next.streamReasoning ?? null);
    streamReasoningRef.current = next.streamReasoning ?? null;
    setAgentState(next.agentState ?? null);
    setSubAgentBuffer(next.subAgentBuffer);
    subAgentBufferRef.current = next.subAgentBuffer;
    setSubAgentRoleName(next.subAgentRoleName);
  };

  const prepareForSend = () => {
    setStreaming(true);
    streamItemsRef.current = [];
    setStreamItems([]);
    setStreamReasoning(null);
    streamReasoningRef.current = null;
    subAgentBufferRef.current = "";
    setSubAgentBuffer("");
    setSubAgentRoleName("");
  };

  const finishStreaming = () => {
    setStreaming(false);
  };

  const interruptStreaming = () => {
    setStreaming(false);
    setAgentState(null);
    updateStreamReasoning((prev) => (prev ? { ...prev, status: "interrupted" } : prev));
    const items = streamItemsRef.current.map((item) => {
      if (item.type === "tool_call" && item.toolCall?.status === "running") {
        return {
          ...item,
          toolCall: {
            ...item.toolCall,
            output: "已取消",
            status: "error" as const,
          },
        };
      }
      return item;
    });
    streamItemsRef.current = items;
    setStreamItems([...items]);
  };

  useLayoutEffect(() => {
    let currentSessionId: string | null = sessionId;
    const unsubscribe = subscribeChatStreamEvent("stream-token", (payload: StreamTokenEvent) => {
      if (payload.session_id !== currentSessionId) return;
      if (payload.done) {
        streamItemsRef.current = [];
        setStreamItems([]);
        setStreamReasoning(null);
        streamReasoningRef.current = null;
        subAgentBufferRef.current = "";
        setSubAgentBuffer("");
        setSubAgentRoleName("");
        setStreaming(false);
        if (currentSessionId) {
          void Promise.all([loadMessages(currentSessionId), loadSessionRuns(currentSessionId)]);
        }
        return;
      }
      if (payload.sub_agent) {
        const delegatedRole = (payload.role_name || payload.role_id || "").trim();
        if (delegatedRole) {
          setSubAgentRoleName(delegatedRole);
        }
        subAgentBufferRef.current += payload.token;
        setSubAgentBuffer(subAgentBufferRef.current);
        return;
      }
      const items = [...streamItemsRef.current];
      const last = items[items.length - 1];
      if (last && last.type === "text") {
        last.content = mergeStreamingTextChunk(last.content || "", payload.token);
      } else {
        items.push({ type: "text", content: payload.token });
      }
      streamItemsRef.current = items;
      setStreamItems([...items]);
    });
    return () => {
      currentSessionId = null;
      unsubscribe();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId]);

  useEffect(() => {
    const unlistenPromise = listen<AskUserEventPayload>("ask-user-event", ({ payload }) => {
      if (payload.session_id !== sessionId) return;
      if (suppressAskUserPrompt) {
        setAskUserQuestion(null);
        setAskUserOptions([]);
        return;
      }
      setAskUserQuestion(payload.question);
      setAskUserOptions(payload.options);
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [sessionId, suppressAskUserPrompt]);

  useEffect(() => {
    const unlistenPromise = listen<AgentStateEventPayload>("agent-state-event", ({ payload }) => {
      if (payload.session_id !== sessionId) return;
      if (payload.state === "finished") {
        setAgentState(null);
        return;
      }
      setAgentState({
        state: payload.state,
        detail: payload.detail ?? undefined,
        iteration: payload.iteration,
        stopReasonKind: payload.stop_reason_kind ?? undefined,
        stopReasonTitle: payload.stop_reason_title ?? undefined,
        stopReasonMessage: payload.stop_reason_message ?? undefined,
        stopReasonLastCompletedStep: payload.stop_reason_last_completed_step ?? undefined,
      });
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [sessionId]);

  useEffect(() => {
    const unlistenCreatedPromise = listen<PendingApprovalEventPayload>("approval-created", ({ payload }) => {
      if (payload.session_id !== sessionId) return;
      upsertPendingApprovalRef.current(buildPendingApprovalRef.current(payload));
    });

    const unlistenResolvedPromise = listen<{
      approval_id?: string;
      session_id?: string;
      status?: string;
    }>("approval-resolved", ({ payload }) => {
      if ((payload.session_id || "").trim() && payload.session_id !== sessionId) return;
      const approvalId = (payload.approval_id || "").trim();
      if (!approvalId) return;
      removePendingApprovalRef.current(approvalId);
    });

    const unlistenLegacyPromise = listen<PendingApprovalEventPayload>("tool-confirm-event", ({ payload }) => {
      if (payload.session_id !== sessionId) return;
      if ((payload.approval_id || "").trim()) return;
      upsertPendingApprovalRef.current(buildPendingApprovalRef.current(payload));
    });

    return () => {
      unlistenCreatedPromise.then((fn) => fn());
      unlistenResolvedPromise.then((fn) => fn());
      unlistenLegacyPromise.then((fn) => fn());
    };
  }, [sessionId]);

  useEffect(() => {
    const cleanupSessionId = sessionId;
    return () => {
      const resolvingId = resolvingApprovalIdRef.current;
      const staleApprovals = (pendingApprovalsRef.current ?? []).filter(
        (item) => item.sessionId === cleanupSessionId && item.approvalId.trim() && item.approvalId !== resolvingId,
      );
      for (const approval of staleApprovals) {
        if (approval.approvalRecordId) {
          void resolveApproval(approval.approvalRecordId, "deny", "desktop_cleanup").catch((error) => {
            console.error("自动拒绝待审批操作失败:", error);
          });
          continue;
        }
        if (approval.usesLegacyConfirm) {
          void confirmLegacyToolExecution(false).catch((error) => {
            console.error("自动拒绝旧版待审批操作失败:", error);
          });
        }
      }
    };
  }, [pendingApprovalsRef, resolvingApprovalIdRef, sessionId]);

  useEffect(() => {
    const unlisteners = [
      subscribeChatStreamEvent("assistant-reasoning-started", (payload: AssistantReasoningStartedEvent) => {
        if (payload.session_id !== sessionId) return;
        updateStreamReasoning((prev) => ({
          status: "thinking",
          content: prev?.content || "",
          durationMs: prev?.durationMs,
        }));
      }),
      subscribeChatStreamEvent("assistant-reasoning-delta", (payload: AssistantReasoningDeltaEvent) => {
        if (payload.session_id !== sessionId) return;
        updateStreamReasoning((prev) => ({
          status: "thinking",
          content: `${prev?.content || ""}${payload.text || ""}`,
          durationMs: prev?.durationMs,
        }));
      }),
      subscribeChatStreamEvent("assistant-reasoning-completed", (payload: AssistantReasoningCompletedEvent) => {
        if (payload.session_id !== sessionId) return;
        updateStreamReasoning((prev) =>
          prev
            ? {
                ...prev,
                status: "completed",
                durationMs: payload.duration_ms,
              }
            : {
                status: "completed",
                content: "",
                durationMs: payload.duration_ms,
              },
        );
      }),
      subscribeChatStreamEvent("assistant-reasoning-interrupted", (payload: AssistantReasoningInterruptedEvent) => {
        if (payload.session_id !== sessionId) return;
        updateStreamReasoning((prev) => ({
          status: "interrupted",
          content: prev?.content || "",
          durationMs: prev?.durationMs,
        }));
      }),
    ];
    return () => {
      unlisteners.forEach((dispose) => dispose());
    };
  }, [sessionId]);

  useEffect(() => {
    const unsubscribe = subscribeChatStreamEvent("session-tool-manifest", (payload: SessionToolManifestEvent) => {
      if (payload.session_id !== sessionId) return;
      setToolManifest(payload.manifest.map((item) => ({ ...item })));
    });
    return () => {
      unsubscribe();
    };
  }, [sessionId]);

  useEffect(() => {
    const unsubscribe = subscribeChatStreamEvent("tool-call-event", (payload: ToolCallEvent) => {
      if (payload.session_id !== sessionId) return;
      if (payload.status === "started") {
        const items = streamItemsRef.current;
        items.push({
          type: "tool_call",
          toolCall: {
            id: `${payload.tool_name}-${Date.now()}`,
            name: payload.tool_name,
            input: payload.tool_input,
            status: "running" as const,
          },
        });
        streamItemsRef.current = items;
        setStreamItems([...items]);
        return;
      }
      const items = streamItemsRef.current.map((item) => {
        if (item.type === "tool_call" && item.toolCall?.name === payload.tool_name && item.toolCall?.status === "running") {
          return {
            ...item,
            toolCall: {
              ...item.toolCall,
              output: payload.tool_output ?? undefined,
              status: (payload.status === "completed" ? "completed" : "error") as "completed" | "error",
            },
          };
        }
        return item;
      });
      streamItemsRef.current = items;
      setStreamItems([...items]);
    });
    return () => {
      unsubscribe();
    };
  }, [sessionId]);

  return {
    streaming,
    streamItems,
    toolManifest,
    streamReasoning,
    askUserQuestion,
    askUserOptions,
    askUserAnswer,
    setAskUserAnswer,
    agentState,
    subAgentBuffer,
    subAgentRoleName,
    applyPersistedRuntimeState,
    resetForSessionSwitch,
    prepareForSend,
    finishStreaming,
    interruptStreaming,
    clearAskUserPrompt,
  };
}

function mergeStreamingTextChunk(currentText: string, incomingText: string): string {
  if (!incomingText) return currentText;
  if (!currentText) return incomingText;
  if (currentText.endsWith(incomingText)) return currentText;
  if (incomingText.startsWith(currentText)) return incomingText;

  const maxOverlap = Math.min(currentText.length, incomingText.length);
  for (let overlap = maxOverlap; overlap > 0; overlap -= 1) {
    if (currentText.slice(-overlap) === incomingText.slice(0, overlap)) {
      return currentText + incomingText.slice(overlap);
    }
  }

  return currentText + incomingText;
}
