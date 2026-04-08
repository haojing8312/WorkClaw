import { useState, useEffect, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SkillManifest, ModelConfig, Message, StreamItem, PendingAttachment, ChatMessagePart, SendMessageRequest, EmployeeGroupRunSnapshot, SessionRunProjection, PersistedChatRuntimeState, ChatDelegationCardState } from "../types";
import { motion } from "framer-motion";
import { RiskConfirmDialog } from "./RiskConfirmDialog";
import { useImmersiveTranslation } from "../hooks/useImmersiveTranslation";
import { ChatWorkspaceSidePanel } from "./chat-side-panel/ChatWorkspaceSidePanel";
import { ChatExecutionContextBar } from "./chat/ChatExecutionContextBar";
import { ChatHeader } from "./chat/ChatHeader";
import { ChatComposer } from "./chat/ChatComposer";
import { ChatCollaborationStatusPanel } from "./chat/ChatCollaborationStatusPanel";
import { ChatMessageRail } from "./chat/ChatMessageRail";
import { ChatGroupRunSection } from "./chat/group-run/ChatGroupRunSection";
import { ChatShell } from "./chat/ChatShell";
import {
  clearSessionDraft,
  clonePersistedChatRuntimeState,
  persistSessionDraft,
  readSessionDraft,
} from "../scenes/chat/chatRuntimeState";
import { useChatDraftState } from "../scenes/chat/useChatDraftState";
import {
  buildTaskJourneyViewModel,
  buildTaskPanelViewModel,
  buildWebSearchViewModel,
  extractSessionTouchedFiles,
} from "./chat-side-panel/view-model";
import type { TaskJourneyViewModel } from "./chat-side-panel/view-model";
import { getDefaultModel } from "../lib/default-model";
import {
  answerUserQuestion,
  cancelAgent,
} from "../services/chat/chatSessionService";
import {
  resolveApproval as resolvePendingApproval,
} from "../services/chat/chatApprovalService";
import { useChatSessionController, type PendingApprovalView } from "../scenes/chat/useChatSessionController";
import { useChatCollaborationController } from "../scenes/chat/useChatCollaborationController";
import { buildMessageParts, useChatSendController } from "../scenes/chat/useChatSendController";
import {
  getModelErrorDisplay,
  inferModelErrorKindFromMessage,
  isModelErrorKind,
} from "../lib/model-error-display";
import { useChatStreamController } from "../scenes/chat/useChatStreamController";
import { openExternalUrl } from "../utils/openExternalUrl";

type ClawhubInstallCandidate = {
  slug: string;
  name: string;
  description?: string;
  stars?: number;
  githubUrl?: string | null;
  sourceUrl?: string | null;
};

type ChatSessionTimelineItem = {
  eventId?: string;
  linkedSessionId?: string;
  label: string;
  createdAt?: string;
  openSessionOptions?: ChatSessionOpenOptions;
};

type ChatSessionOpenOptions = {
  focusHint?: string;
  groupRunStepFocusId?: string;
  groupRunEventFocusId?: string;
  sourceSessionId?: string;
  sourceStepId?: string;
  sourceEmployeeId?: string;
  assigneeEmployeeId?: string;
  sourceStepTimeline?: ChatSessionTimelineItem[];
};

type ChatSessionExecutionContext = {
  sourceSessionId: string;
  sourceStepId: string;
  sourceEmployeeId?: string;
  assigneeEmployeeId?: string;
  sourceStepTimeline?: ChatSessionTimelineItem[];
};

type ChatLinkToastState = {
  variant: "success" | "error";
  message: string;
  url: string;
};

const TOOL_ACTION_LABELS: Record<string, string> = {
  file_delete: "删除文件",
  write_file: "写入文件",
  edit: "编辑文件",
  bash: "执行命令",
  web_search: "网页搜索",
  web_fetch: "获取网页",
};

function buildApprovalReasonText(
  approval: PendingApprovalView | null,
  toolLabel: string,
  readOnly: boolean,
  destructive: boolean,
  requiresApproval: boolean,
): string | undefined {
  if (!approval) return undefined;
  if (approval.irreversible || destructive || approval.toolName === "file_delete") {
    return `原因：这是不可逆的${toolLabel}操作，确认后会立即执行一次。`;
  }
  if (requiresApproval || approval.toolName === "bash") {
    return `原因：这是${readOnly ? "读取环境" : "会修改环境的"}${toolLabel}操作，确认后才会继续。`;
  }
  if (!readOnly) {
    return `原因：这是会修改环境的${toolLabel}操作。`;
  }
  return undefined;
}

function buildApprovalImpactText(
  approval: PendingApprovalView | null,
  readOnly: boolean,
  destructive: boolean,
): string | undefined {
  if (approval?.impact?.trim()) return approval.impact;
  if (approval?.irreversible || destructive || approval?.toolName === "file_delete") {
    return "这类操作可能直接删除或覆盖本地内容。";
  }
  if (readOnly) {
    return "这类操作通常只读取信息，不会直接修改本地内容。";
  }
  return "这类操作可能修改本地文件、命令环境或会话状态。";
}

const CONTINUE_MESSAGE_TEXT = "继续";
const CONTINUE_BUDGET_INCREMENT = 100;
const CHAT_SCROLL_EDGE_THRESHOLD = 48;

interface Props {
  skill: SkillManifest;
  models: ModelConfig[];
  sessionId: string;
  sessionModelId?: string;
  workDir?: string;
  onOpenSession?: (sessionId: string, options?: ChatSessionOpenOptions) => Promise<void> | void;
  sessionFocusRequest?: { nonce: number; snippet: string };
  groupRunStepFocusRequest?: { nonce: number; stepId: string; eventId?: string };
  sessionExecutionContext?: ChatSessionExecutionContext;
  onReturnToSourceSession?: (sessionId: string) => Promise<void> | void;
  onSessionUpdate?: () => void;
  onSessionBlockingStateChange?: (update: { blocking: boolean; status?: string | null }) => void;
  persistedRuntimeState?: PersistedChatRuntimeState;
  onPersistRuntimeState?: (state: PersistedChatRuntimeState) => void;
  initialMessage?: string;
  initialAttachments?: PendingAttachment[];
  onInitialMessageConsumed?: () => void;
  onInitialAttachmentsConsumed?: () => void;
  installedSkillIds?: string[];
  onSkillInstalled?: (skillId: string) => Promise<void> | void;
  suppressAskUserPrompt?: boolean;
  quickPrompts?: Array<{ label: string; prompt: string }>;
  employeeAssistantContext?: {
    mode: "create" | "update";
    employeeName?: string;
    employeeCode?: string;
  };
  sessionTitle?: string;
  sessionMode?: "general" | "employee_direct" | "team_entry" | string;
  sessionEmployeeName?: string;
  sessionSourceChannel?: string;
  sessionSourceLabel?: string;
  operationPermissionMode?: "standard" | "full_access" | string;
}

function shouldRenderCompletedJourneySummary(model: TaskJourneyViewModel) {
  if (model.deliverables.length === 0) return false;
  return model.status === "completed" || model.status === "partial";
}

function CopyActionIcon({ copied }: { copied: boolean }) {
  if (copied) {
    return (
      <svg aria-hidden="true" className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
      </svg>
    );
  }

  return (
    <svg aria-hidden="true" className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.8}>
      <rect x="9" y="9" width="10" height="10" rx="2" />
      <path strokeLinecap="round" strokeLinejoin="round" d="M15 9V7a2 2 0 00-2-2H7a2 2 0 00-2 2v6a2 2 0 002 2h2" />
    </svg>
  );
}

function getThinkingSupport(model: ModelConfig | null): {
  indicator: boolean;
  reasoning: boolean;
} {
  if (!model) {
    return { indicator: true, reasoning: false };
  }

  if (model.api_format === "openai") {
    return { indicator: true, reasoning: true };
  }

  if (model.api_format === "anthropic") {
    const normalizedBaseUrl = model.base_url.trim().toLowerCase();
    const normalizedModelName = model.model_name.trim().toLowerCase();
    const supportsExtendedAnthropicReasoning =
      normalizedBaseUrl.includes("api.anthropic.com/v1") &&
      (normalizedModelName.startsWith("claude-sonnet-4") || normalizedModelName.startsWith("claude-opus-4"));

    return {
      indicator: true,
      reasoning: supportsExtendedAnthropicReasoning,
    };
  }

  return { indicator: false, reasoning: false };
}

export function ChatView({
  skill,
  models,
  sessionId,
  sessionModelId,
  workDir,
  onOpenSession,
  sessionFocusRequest,
  groupRunStepFocusRequest,
  sessionExecutionContext,
  onReturnToSourceSession,
  onSessionUpdate,
  onSessionBlockingStateChange,
  persistedRuntimeState,
  onPersistRuntimeState,
  initialMessage,
  initialAttachments = [],
  onInitialMessageConsumed,
  onInitialAttachmentsConsumed,
  installedSkillIds = [],
  onSkillInstalled,
  suppressAskUserPrompt = false,
  quickPrompts = [],
  employeeAssistantContext,
  sessionTitle,
  sessionMode,
  sessionEmployeeName,
  sessionSourceChannel,
  sessionSourceLabel,
  operationPermissionMode = "standard",
}: Props) {
  const parseDuplicateSkillName = (error: unknown): string | null => {
    const message =
      typeof error === "string"
        ? error
        : error instanceof Error
        ? error.message
        : String(error ?? "");
    const prefix = "DUPLICATE_SKILL_NAME:";
    if (!message.includes(prefix)) return null;
    return message.split(prefix)[1]?.trim() || null;
  };
  const initialRuntimeState = clonePersistedChatRuntimeState(persistedRuntimeState);
  const [expandedRunDetailIds, setExpandedRunDetailIds] = useState<string[]>([]);
  const [pendingInstallSkill, setPendingInstallSkill] = useState<ClawhubInstallCandidate | null>(null);
  const [showInstallConfirm, setShowInstallConfirm] = useState(false);
  const [installingSlug, setInstallingSlug] = useState<string | null>(null);
  const [installError, setInstallError] = useState<string | null>(null);
  const installInFlightRef = useRef(false);
  const [mainRoleName, setMainRoleName] = useState(initialRuntimeState.mainRoleName);
  const [mainSummaryDelivered, setMainSummaryDelivered] = useState(initialRuntimeState.mainSummaryDelivered);
  const [highlightedMessageIndex, setHighlightedMessageIndex] = useState<number | null>(null);
  const [highlightedGroupRunStepId, setHighlightedGroupRunStepId] = useState<string | null>(null);
  const [highlightedGroupRunStepEventId, setHighlightedGroupRunStepEventId] = useState<string | null>(null);
  const [isNearTop, setIsNearTop] = useState(true);
  const [isNearBottom, setIsNearBottom] = useState(true);
  const [hasScrollableContent, setHasScrollableContent] = useState(false);
  const [delegationCards, setDelegationCards] = useState<ChatDelegationCardState[]>(initialRuntimeState.delegationCards);
  const bottomRef = useRef<HTMLDivElement>(null);
  const scrollRegionRef = useRef<HTMLDivElement>(null);
  const autoFollowScrollRef = useRef(true);
  const scrollAnimationFrameRef = useRef<number | null>(null);
  const scrollAnimationTargetRef = useRef<"top" | "bottom" | null>(null);
  const sessionIdRef = useRef(sessionId);
  const mainRoleNameRef = useRef("");
  const loadMessagesRef = useRef<(sid: string) => Promise<void>>(async () => {});
  const loadSessionRunsRef = useRef<(sid: string) => Promise<void>>(async () => {});
  const pendingApprovalsSnapshotRef = useRef<PendingApprovalView[]>([]);
  const resolvingApprovalSnapshotRef = useRef<string | null>(null);
  const lastHandledSessionFocusNonceRef = useRef<number | null>(null);
  const messageElementRefs = useRef<Record<number, HTMLDivElement | null>>({});
  const lastHandledGroupRunStepFocusNonceRef = useRef<number | null>(null);
  const groupRunStepElementRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const groupRunStepEventElementRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const {
    input,
    setInput,
    attachedFiles,
    composerError,
    setComposerError,
    textareaRef,
    handleComposerInputChange,
    handleFileSelect,
    removeAttachedFile,
    clearComposerState,
  } = useChatDraftState({
    sessionId,
    initialAttachments,
    consumeInitialAttachmentsImmediately: !initialMessage?.trim(),
    onInitialAttachmentsConsumed: onInitialAttachmentsConsumed
      ? () => onInitialAttachmentsConsumed()
      : undefined,
  });

  const upsertPendingApproval = (nextApproval: PendingApprovalView) => {
    setPendingApprovals((prev) => {
      const existingIndex = prev.findIndex((item) => item.approvalId === nextApproval.approvalId);
      if (existingIndex >= 0) {
        const updated = [...prev];
        updated[existingIndex] = {
          ...updated[existingIndex],
          ...nextApproval,
        };
        return updated;
      }
      return [...prev, nextApproval];
    });
  };

  const removePendingApproval = (approvalId: string) => {
    setPendingApprovals((prev) => prev.filter((item) => item.approvalId !== approvalId));
    setResolvingApprovalId((current) => (current === approvalId ? null : current));
  };

  const buildPendingApproval = (payload: {
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
  }): PendingApprovalView => ({
    approvalId: payload.approval_id || `${payload.tool_name}-${Date.now()}`,
    approvalRecordId: payload.approval_id || undefined,
    sessionId: payload.session_id,
    toolName: payload.tool_name,
    toolInput: payload.tool_input || payload.input || {},
    title: payload.title || "高危操作确认",
    summary: payload.summary || `将执行工具 ${payload.tool_name}`,
    impact: payload.impact || undefined,
    irreversible: payload.irreversible,
    status: payload.status,
    usesLegacyConfirm: !(payload.approval_id || "").trim(),
  });

  useEffect(() => {
    sessionIdRef.current = sessionId;
  }, [sessionId]);

  // 右侧面板状态
  const [sidePanelOpen, setSidePanelOpen] = useState(false);
  const [sidePanelTab, setSidePanelTab] = useState<"tasks" | "files" | "websearch">("tasks");
  const [expandedThinkingKeys, setExpandedThinkingKeys] = useState<string[]>([]);
  const [copiedAssistantMessageKey, setCopiedAssistantMessageKey] = useState<string | null>(null);
  const [chatLinkToast, setChatLinkToast] = useState<ChatLinkToastState | null>(null);

  const toggleThinkingBlock = (key: string) => {
    setExpandedThinkingKeys((prev) => (prev.includes(key) ? prev.filter((item) => item !== key) : [...prev, key]));
  };

  useEffect(() => {
    if (chatLinkToast?.variant !== "success") {
      return;
    }
    const timer = window.setTimeout(() => {
      setChatLinkToast((current) => (current?.variant === "success" ? null : current));
    }, 1200);
    return () => window.clearTimeout(timer);
  }, [chatLinkToast]);

  const collaborationControllerState = {
    resetForSessionSwitch: () => {},
  };

  const {
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
    applyPersistedRuntimeState: applyStreamRuntimeState,
    resetForSessionSwitch,
    prepareForSend,
    finishStreaming,
    interruptStreaming,
    clearAskUserPrompt,
  } = useChatStreamController({
    sessionId,
    suppressAskUserPrompt,
    initialRuntimeState,
    loadMessages: (sid) => loadMessagesRef.current(sid),
    loadSessionRuns: (sid) => loadSessionRunsRef.current(sid),
    pendingApprovalsRef: pendingApprovalsSnapshotRef,
    resolvingApprovalIdRef: resolvingApprovalSnapshotRef,
    buildPendingApproval,
    upsertPendingApproval,
    removePendingApproval,
    onResetForSessionSwitch: () => {
      collaborationControllerState.resetForSessionSwitch();
      setSidePanelTab("tasks");
      setExpandedThinkingKeys([]);
    },
  });

  const applyPersistedRuntimeState = (state?: PersistedChatRuntimeState | null) => {
    const next = clonePersistedChatRuntimeState(state);
    applyStreamRuntimeState(next);
    setMainRoleName(next.mainRoleName);
    mainRoleNameRef.current = next.mainRoleName;
    setMainSummaryDelivered(next.mainSummaryDelivered);
    setDelegationCards(next.delegationCards);
  };

  const runtimeSnapshot = useMemo<PersistedChatRuntimeState>(
    () => ({
      streaming,
      streamItems: [...streamItems],
      toolManifest: toolManifest.map((item) => ({ ...item })),
      streamReasoning: streamReasoning ? { ...streamReasoning } : null,
      agentState: agentState ? { ...agentState } : null,
      subAgentBuffer,
      subAgentRoleName,
      mainRoleName,
      mainSummaryDelivered,
      delegationCards: delegationCards.map((item) => ({ ...item })),
    }),
    [
      agentState,
      delegationCards,
      mainRoleName,
      mainSummaryDelivered,
      streamItems,
      toolManifest,
      streamReasoning,
      streaming,
      subAgentBuffer,
      subAgentRoleName,
    ],
  );

  const {
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
    updateWorkspace,
  } = useChatSessionController({
    sessionId,
    workDir,
    initialMessage,
    draftInput: input,
    persistedRuntimeState,
    runtimeSnapshot,
    onPersistRuntimeState,
    onApplyPersistedRuntimeState: applyPersistedRuntimeState,
    onDraftLoaded: setInput,
    onResetForSessionSwitch: resetForSessionSwitch,
    readSessionDraft,
    clearSessionDraft,
    persistSessionDraft,
    mapPendingApprovalRecord: (item) =>
      buildPendingApproval({
        approval_id: item.approval_id,
        session_id: item.session_id,
        tool_name: item.tool_name,
        input: item.input,
        title: "高危操作确认",
        summary: item.summary,
        impact: item.impact,
        irreversible: item.irreversible,
        status: item.status,
      }),
  });

  const {
    imRoleEvents,
    groupRunSnapshot,
    groupRunMemberEmployeeIds,
    groupRunCoordinatorEmployeeId,
    groupRunRules,
    expandedGroupRunStepIds,
    setExpandedGroupRunStepIds,
    groupRunActionLoading,
    resetForSessionSwitch: resetCollaborationForSessionSwitch,
    handleApproveGroupRunReview,
    handleRejectGroupRunReview,
    handlePauseGroupRun,
    handleResumeGroupRun,
    handleRetryFailedGroupRunSteps,
    handleReassignFailedGroupRunStep,
  } = useChatCollaborationController({
    sessionId,
    mainRoleName,
    getCurrentMainRoleName: () => mainRoleNameRef.current,
    onMainRoleNameChange: (roleName) => {
      mainRoleNameRef.current = roleName;
      setMainRoleName(roleName);
    },
    onMainSummaryDeliveredChange: setMainSummaryDelivered,
    onDelegationCardsChange: setDelegationCards,
    onMessagesAppend: (message) => {
      setMessages((prev) => [...prev, message]);
    },
    onResetForSessionSwitch: () => {
      setHighlightedMessageIndex(null);
      setHighlightedGroupRunStepId(null);
      setHighlightedGroupRunStepEventId(null);
      lastHandledGroupRunStepFocusNonceRef.current = null;
    },
  });
  collaborationControllerState.resetForSessionSwitch = resetCollaborationForSessionSwitch;

  loadMessagesRef.current = loadMessages;
  loadSessionRunsRef.current = loadSessionRuns;
  pendingApprovalsSnapshotRef.current = pendingApprovals;
  resolvingApprovalSnapshotRef.current = resolvingApprovalId;

  const renderUserContentParts = (parts: ChatMessagePart[]) => {
    const textParts = parts.filter((part): part is Extract<ChatMessagePart, { type: "text" }> => part.type === "text");
    const attachmentParts = parts.filter((part) => part.type !== "text");
    return (
      <div className="space-y-3">
        {textParts.map((part, index) => (
          <div key={`text-${index}`} className="whitespace-pre-wrap break-words">
            {part.text}
          </div>
        ))}
        {attachmentParts.length > 0 && (
          <div className="space-y-2">
            {attachmentParts.map((part, index) =>
              part.type === "image" ? (
                <div
                  key={`attachment-${part.name}-${index}`}
                  className="rounded-xl border border-white/20 bg-white/10 p-2"
                >
                  <img
                    src={part.data}
                    alt={part.name}
                    className="max-h-56 w-full rounded-lg object-cover"
                  />
                  <div className="mt-2 text-xs opacity-90">{part.name}</div>
                </div>
              ) : (
                <div
                  key={`attachment-${part.name}-${index}`}
                  className="rounded-xl border border-white/20 bg-white/10 p-3 text-xs"
                >
                  <div className="font-medium">{part.name}</div>
                  <div className="mt-1 opacity-80">
                    {part.type === "pdf_file" ? "PDF 附件" : "文本附件"}
                    {part.truncated ? " · 已截断" : ""}
                  </div>
                </div>
              ),
            )}
          </div>
        )}
      </div>
    );
  };

  const syncScrollMetrics = (element: HTMLDivElement | null) => {
    if (!element) {
      return;
    }
    const distanceFromBottom = Math.max(0, element.scrollHeight - element.scrollTop - element.clientHeight);
    const nextNearBottom = distanceFromBottom <= CHAT_SCROLL_EDGE_THRESHOLD;
    const nextNearTop = element.scrollTop <= CHAT_SCROLL_EDGE_THRESHOLD;
    const keepFollowingBottom = scrollAnimationTargetRef.current === "bottom";
    setIsNearBottom(nextNearBottom);
    setIsNearTop(nextNearTop);
    setHasScrollableContent(element.scrollHeight > element.clientHeight + 4);
    autoFollowScrollRef.current = keepFollowingBottom || nextNearBottom;
  };

  const stopScrollAnimation = () => {
    if (scrollAnimationFrameRef.current !== null) {
      cancelAnimationFrame(scrollAnimationFrameRef.current);
      scrollAnimationFrameRef.current = null;
    }
    scrollAnimationTargetRef.current = null;
  };

  const setScrollRegionTop = (scrollRegion: HTMLDivElement, top: number) => {
    if (typeof scrollRegion.scrollTo === "function") {
      scrollRegion.scrollTo({ top });
      return;
    }
    scrollRegion.scrollTop = top;
  };

  const animateScrollRegionTo = (targetTop: number, durationMs = 1000, target: "top" | "bottom" | null = null) => {
    const scrollRegion = scrollRegionRef.current;
    if (!scrollRegion) {
      return;
    }

    stopScrollAnimation();
    scrollAnimationTargetRef.current = target;

    const maxTop = Math.max(0, scrollRegion.scrollHeight - scrollRegion.clientHeight);
    const startTop = scrollRegion.scrollTop;
    const clampedTargetTop = Math.max(0, Math.min(targetTop, maxTop));
    const distance = clampedTargetTop - startTop;

    if (Math.abs(distance) < 1) {
      setScrollRegionTop(scrollRegion, clampedTargetTop);
      syncScrollMetrics(scrollRegion);
      if (target !== "bottom") {
        scrollAnimationTargetRef.current = null;
      }
      return;
    }

    const easeOutCubic = (t: number) => 1 - Math.pow(1 - t, 3);
    const initialTop = startTop + distance * 0.22;
    setScrollRegionTop(scrollRegion, initialTop);
    syncScrollMetrics(scrollRegion);
    let startTime: number | null = null;

    const step = (timestamp: number) => {
      if (startTime === null) {
        startTime = timestamp;
      }
      const progress = Math.min((timestamp - startTime) / durationMs, 1);
      const nextTop = startTop + distance * easeOutCubic(progress);
      setScrollRegionTop(scrollRegion, nextTop);
      syncScrollMetrics(scrollRegion);

      if (progress < 1) {
        scrollAnimationFrameRef.current = requestAnimationFrame(step);
        return;
      }

      scrollRegion.scrollTo({ top: clampedTargetTop });
      syncScrollMetrics(scrollRegion);
      scrollAnimationFrameRef.current = null;
      if (target !== "bottom") {
        scrollAnimationTargetRef.current = null;
      }
    };

    scrollAnimationFrameRef.current = requestAnimationFrame(step);
  };

  const handleScrollRegionScroll = () => {
    syncScrollMetrics(scrollRegionRef.current);
  };

  const handleScrollJump = () => {
    const scrollRegion = scrollRegionRef.current;
    if (!scrollRegion) {
      return;
    }

    if (isNearBottom) {
      autoFollowScrollRef.current = false;
      setIsNearBottom(false);
      setIsNearTop(true);
      animateScrollRegionTo(0, 1000, "top");
      return;
    }

    autoFollowScrollRef.current = true;
    setIsNearBottom(true);
    setIsNearTop(false);
    animateScrollRegionTo(scrollRegion.scrollHeight - scrollRegion.clientHeight, 1000, "bottom");
  };

  const {
    sendContent,
    handleSend,
  } = useChatSendController({
    sessionId,
    streaming,
    input,
    attachedFiles,
    clearComposerState,
    setComposerError,
    setMessages,
    loadMessages,
    loadSessionRuns,
    prepareForSend,
    finishStreaming,
    onSessionUpdate,
    autoFollowScrollRef,
    setIsNearBottom,
    setIsNearTop,
    animateScrollRegionTo,
    scrollRegionRef,
    shouldGrantContinuationBudget,
    continuationBudgetIncrement: CONTINUE_BUDGET_INCREMENT,
  });

  useEffect(() => {
    autoFollowScrollRef.current = true;
    setIsNearTop(true);
    setIsNearBottom(true);
    setHasScrollableContent(false);
  }, [sessionId]);

  useEffect(() => {
    if (autoFollowScrollRef.current) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
      return;
    }
    syncScrollMetrics(scrollRegionRef.current);
  }, [messages, streamItems, streamReasoning, askUserQuestion, pendingApprovals]);

  useEffect(() => {
    syncScrollMetrics(scrollRegionRef.current);
  }, []);

  useEffect(() => stopScrollAnimation, []);

  useEffect(() => {
    if (!sessionFocusRequest || !sessionFocusRequest.snippet.trim()) {
      return;
    }
    if (messages.length === 0) {
      return;
    }
    if (lastHandledSessionFocusNonceRef.current === sessionFocusRequest.nonce) {
      return;
    }

    const normalize = (value: string) => value.replace(/\s+/g, " ").trim().toLowerCase();
    const normalizedSnippet = normalize(sessionFocusRequest.snippet);
    const fallbackSnippet = normalizedSnippet.slice(0, 16);
    const assistantMessageIndexes = messages
      .map((message, index) => ({ message, index }))
      .filter(({ message }) => message.role === "assistant");

    let matchedIndex = -1;
    for (let i = assistantMessageIndexes.length - 1; i >= 0; i -= 1) {
      const candidate = assistantMessageIndexes[i];
      const normalizedContent = normalize(candidate.message.content || "");
      if (!normalizedContent) continue;
      if (
        normalizedContent.includes(normalizedSnippet) ||
        normalizedSnippet.includes(normalizedContent) ||
        (fallbackSnippet.length > 0 && normalizedContent.includes(fallbackSnippet))
      ) {
        matchedIndex = candidate.index;
        break;
      }
    }
    if (matchedIndex < 0 && assistantMessageIndexes.length > 0) {
      matchedIndex = assistantMessageIndexes[assistantMessageIndexes.length - 1].index;
    }

    lastHandledSessionFocusNonceRef.current = sessionFocusRequest.nonce;
    if (matchedIndex < 0) {
      return;
    }

    setHighlightedMessageIndex(matchedIndex);
    messageElementRefs.current[matchedIndex]?.scrollIntoView({ behavior: "smooth", block: "center" });
    const timer = setTimeout(() => {
      setHighlightedMessageIndex((current) => (current === matchedIndex ? null : current));
    }, 2400);
    return () => clearTimeout(timer);
  }, [messages, sessionFocusRequest, sessionId]);

  useEffect(() => {
    const targetStepId = (groupRunStepFocusRequest?.stepId || "").trim();
    if (!targetStepId || !groupRunSnapshot) {
      return;
    }
    if (lastHandledGroupRunStepFocusNonceRef.current === groupRunStepFocusRequest?.nonce) {
      return;
    }
    const matchedStep = (groupRunSnapshot.steps || []).find((step) => (step.id || "").trim() === targetStepId);
    if (!matchedStep) {
      return;
    }
    const targetEventId = (groupRunStepFocusRequest?.eventId || "").trim();
    if (targetEventId && !expandedGroupRunStepIds.includes(targetStepId)) {
      setExpandedGroupRunStepIds((prev) => (prev.includes(targetStepId) ? prev : [...prev, targetStepId]));
      return;
    }

    lastHandledGroupRunStepFocusNonceRef.current = groupRunStepFocusRequest?.nonce ?? null;
    setHighlightedGroupRunStepId(targetStepId);
    setHighlightedGroupRunStepEventId(targetEventId || null);
    const targetElement =
      (targetEventId ? groupRunStepEventElementRefs.current[targetEventId] : null) ||
      groupRunStepElementRefs.current[targetStepId];
    targetElement?.scrollIntoView({ behavior: "smooth", block: "center" });
    const timer = setTimeout(() => {
      setHighlightedGroupRunStepId((current) => (current === targetStepId ? null : current));
      setHighlightedGroupRunStepEventId((current) => (current === targetEventId ? null : current));
    }, 2400);
    return () => clearTimeout(timer);
  }, [expandedGroupRunStepIds, groupRunSnapshot, groupRunStepFocusRequest, sessionId]);

  useEffect(() => {
    const msg = initialMessage?.trim();
    if (!msg) return;

    const timer = setTimeout(() => {
      onInitialMessageConsumed?.();
      if (initialAttachments.length > 0) {
        onInitialAttachmentsConsumed?.();
        void sendContent({
          sessionId,
          parts: buildMessageParts(msg, initialAttachments),
        });
        return;
      }
      void sendContent(msg);
    }, 0);
    return () => clearTimeout(timer);
    // 仅依赖会话与初始消息，避免重复发送
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, initialAttachments, initialMessage]);

  async function handleCancel() {
    try {
      await cancelAgent();
    } catch (e) {
      console.error("取消任务失败:", e);
    }
    // 即时清除状态，不等待后端返回
    interruptStreaming();
  }

  async function handleAnswerUser(answer: string) {
    if (!answer.trim()) return;
    try {
      await answerUserQuestion(answer.trim());
    } catch (e) {
      console.error("回答用户问题失败:", e);
    }
    clearAskUserPrompt();
  }

  async function handleResolveApproval(decision: "allow_once" | "allow_always" | "deny") {
    const activeApproval = pendingApprovals[0];
    if (!activeApproval || resolvingApprovalId) return;
    try {
      setResolvingApprovalId(activeApproval.approvalId);
      await resolvePendingApproval(activeApproval.approvalId, decision, "desktop");
      removePendingApproval(activeApproval.approvalId);
    } catch (e) {
      console.error("工具确认失败:", e);
      setResolvingApprovalId(null);
    }
  }

  // 从 models 查找当前会话的模型名称
  const currentModel = useMemo(
    () => models.find((model) => model.id === sessionModelId) ?? getDefaultModel(models),
    [models, sessionModelId],
  );
  const thinkingSupport = useMemo(() => getThinkingSupport(currentModel), [currentModel]);
  const installedSkillSet = new Set(installedSkillIds);
  const recoverableSessionRun = useMemo(() => {
    return [...sessionRuns]
      .reverse()
      .find((run) => {
        const status = (run.status || "").trim().toLowerCase();
        const hasAssistantMessage = (run.assistant_message_id || "").trim().length > 0;
        const bufferedText = (run.buffered_text || "").trim();
        return (
          !hasAssistantMessage &&
          bufferedText.length > 0 &&
          ["thinking", "tool_calling", "waiting_approval"].includes(status)
        );
      }) ?? null;
  }, [sessionRuns]);
  const recoveredAssistantMessage = useMemo<Message | null>(() => {
    if (!recoverableSessionRun) return null;
    return {
      id: `recovered-run-${recoverableSessionRun.id}`,
      role: "assistant",
      content: recoverableSessionRun.buffered_text,
      created_at: recoverableSessionRun.updated_at || recoverableSessionRun.created_at,
      runId: recoverableSessionRun.id,
    };
  }, [recoverableSessionRun]);
  const renderedMessages = useMemo<Message[]>(
    () => (recoveredAssistantMessage ? [...messages, recoveredAssistantMessage] : messages),
    [messages, recoveredAssistantMessage]
  );
  const sidePanelMessages = useMemo<Message[]>(() => {
    if (streamItems.length === 0) return renderedMessages;
    return [
      ...renderedMessages,
      {
        role: "assistant",
        content: "",
        created_at: new Date().toISOString(),
        streamItems,
      },
    ];
  }, [renderedMessages, streamItems]);
  const showScrollJump = hasScrollableContent || !isNearBottom;
  const scrollJumpLabel = isNearBottom ? "跳转到顶部" : "跳转到底部";
  const scrollJumpHint = isNearBottom
    ? isNearTop
      ? "当前已在顶部"
      : "返回顶部"
    : "回到底部并继续跟随";
  const taskPanelModel = useMemo(() => buildTaskPanelViewModel(sidePanelMessages), [sidePanelMessages]);
  const webSearchEntries = useMemo(() => buildWebSearchViewModel(sidePanelMessages), [sidePanelMessages]);
  const failedSessionRuns = useMemo(
    () => sessionRuns.filter((run) => run.status === "failed" || run.status === "cancelled"),
    [sessionRuns]
  );
  const latestMaxTurnsRun = useMemo(
    () =>
      [...sessionRuns]
        .reverse()
        .find((run) => (run.error_kind || "").trim().toLowerCase() === "max_turns") ?? null,
    [sessionRuns]
  );
  const failedRunsByAssistantMessageId = useMemo(() => {
    const mapping = new Map<string, SessionRunProjection[]>();
    for (const run of failedSessionRuns) {
      const messageId = (run.assistant_message_id || "").trim();
      if (!messageId) continue;
      const current = mapping.get(messageId) ?? [];
      current.push(run);
      mapping.set(messageId, current);
    }
    return mapping;
  }, [failedSessionRuns]);
  const failedRunsByUserMessageId = useMemo(() => {
    const mapping = new Map<string, SessionRunProjection[]>();
    for (const run of failedSessionRuns) {
      if ((run.assistant_message_id || "").trim()) continue;
      const messageId = (run.user_message_id || "").trim();
      if (!messageId) continue;
      const current = mapping.get(messageId) ?? [];
      current.push(run);
      mapping.set(messageId, current);
    }
    return mapping;
  }, [failedSessionRuns]);
  const orphanFailedRuns = useMemo(() => {
    const anchoredMessageIds = new Set(
      messages
        .flatMap((message) => {
          const ids: string[] = [];
          const messageId = (message.id || "").trim();
          if (!messageId) return ids;
          ids.push(messageId);
          if ((message.runId || "").trim()) ids.push((message.runId || "").trim());
          return ids;
        })
    );
    return failedSessionRuns.filter((run) => {
      const userMessageId = (run.user_message_id || "").trim();
      const assistantMessageId = (run.assistant_message_id || "").trim();
      return (
        (!userMessageId || !anchoredMessageIds.has(userMessageId)) &&
        (!assistantMessageId || !anchoredMessageIds.has(assistantMessageId))
      );
    });
  }, [failedSessionRuns, messages]);
  const touchedFilePaths = useMemo(
    () => extractSessionTouchedFiles(sidePanelMessages).map((item) => item.path),
    [sidePanelMessages]
  );
  const normalizedSessionMode = (sessionMode || "").trim().toLowerCase();
  const isTeamEntrySession = normalizedSessionMode === "team_entry";
  const isEmployeeDirectSession = normalizedSessionMode === "employee_direct";
  const normalizedSessionTitle = (sessionTitle || "").trim();
  const normalizedSessionEmployeeName = (sessionEmployeeName || "").trim();
  const sessionDisplayTitle = isTeamEntrySession
    ? "团队协作"
    : isEmployeeDirectSession
    ? normalizedSessionEmployeeName || normalizedSessionTitle || skill.name
    : skill.name;
  const sessionDisplaySubtitle = isTeamEntrySession ? normalizedSessionTitle || "团队已连接" : "";
  const normalizedSessionSourceChannel = (sessionSourceChannel || "").trim().toLowerCase();
  const isImSource = normalizedSessionSourceChannel.length > 0 && normalizedSessionSourceChannel !== "local";
  const sessionSourceBadgeText =
    (sessionSourceLabel || "").trim() ||
    (normalizedSessionSourceChannel ? `${normalizedSessionSourceChannel} 同步` : "IM 同步");
  const displayWorkDirLabel = (workspace || "").trim() || "选择工作目录";
  const activePendingApproval = pendingApprovals[0] ?? null;
  const queuedApprovalCount = Math.max(0, pendingApprovals.length - 1);
  const activePendingApprovalDialog = useMemo(() => {
    if (!activePendingApproval) return null;
    const manifestEntry = toolManifest.find((item) => item.name === activePendingApproval.toolName) ?? null;
    const toolLabel =
      manifestEntry?.display_name ||
      TOOL_ACTION_LABELS[activePendingApproval.toolName] ||
      activePendingApproval.title ||
      activePendingApproval.toolName;
    const impact = buildApprovalImpactText(
      activePendingApproval,
      manifestEntry?.read_only ?? false,
      manifestEntry?.destructive ?? false,
    );
    const reason = buildApprovalReasonText(
      activePendingApproval,
      toolLabel,
      manifestEntry?.read_only ?? false,
      manifestEntry?.destructive ?? false,
      manifestEntry?.requires_approval ?? false,
    );
    const noteParts = [
      reason,
      queuedApprovalCount > 0 ? `还有 ${queuedApprovalCount} 条待审批` : undefined,
    ].filter((item): item is string => Boolean(item && item.trim()));
    return {
      title: activePendingApproval.title || "高危操作确认",
      summary: activePendingApproval.summary || `将执行工具 ${activePendingApproval.toolName}`,
      impact,
      note: noteParts.length > 0 ? noteParts.join(" · ") : undefined,
      irreversible: activePendingApproval.irreversible,
    };
  }, [activePendingApproval, queuedApprovalCount, toolManifest]);
  const activeDelegationCard = [...delegationCards]
    .reverse()
    .find((card) => card.status === "running");
  const primaryDelegationCard =
    activeDelegationCard || (delegationCards.length > 0 ? delegationCards[delegationCards.length - 1] : null);
  const delegationHistoryCards = primaryDelegationCard
    ? delegationCards.filter((card) => card.id !== primaryDelegationCard.id)
    : [];
  const runningDelegationCount = delegationCards.filter((card) => card.status === "running").length;
  const completedDelegationCount = delegationCards.filter((card) => card.status === "completed").length;
  const failedDelegationCount = delegationCards.filter((card) => card.status === "failed").length;
  const latestCompletedDelegation = [...delegationCards]
    .reverse()
    .find((card) => card.status === "completed");
  const groupPhaseFromEvents = mainSummaryDelivered
    ? "汇报"
    : delegationCards.length > 0
    ? "执行"
    : mainRoleName
    ? "计划"
    : null;
  const groupRoundFromEvents = delegationCards.length > 0 ? Math.max(1, Math.ceil(delegationCards.length / 3)) : 0;
  const groupMemberStatesFromEvents = (() => {
    const byRole = new Map<string, { status: "running" | "completed" | "failed"; stepType: string }>();
    for (const card of delegationCards) {
      byRole.set(card.toRole, { status: card.status, stepType: "execute" });
    }
    return Array.from(byRole.entries()).map(([role, info]) => ({ role, status: info.status, stepType: info.stepType }));
  })();
  const groupPhaseLabelFromSnapshot = (() => {
    const phase = (groupRunSnapshot?.current_phase || "").trim().toLowerCase();
    const state = (groupRunSnapshot?.state || "").trim().toLowerCase();
    if (state === "paused") return "已暂停";
    if (state === "failed") return "失败";
    if (state === "cancelled") return "已取消";
    const normalized = phase || state;
    if (!normalized) return null;
    if (normalized === "intake" || normalized === "plan" || normalized === "planning") return "计划";
    if (normalized === "review" || normalized === "waiting_review") return "审核";
    if (normalized === "dispatch" || normalized === "execute" || normalized === "executing") return "执行";
    if (normalized === "synthesize" || normalized === "finalize" || normalized === "done" || normalized === "completed") return "汇报";
    if (normalized === "failed") return "失败";
    if (normalized === "paused") return "已暂停";
    if (normalized === "cancelled") return "已取消";
    return "执行";
  })();
  const groupPhaseFromSnapshot = (() => {
    const state = (groupRunSnapshot?.state || "").trim().toLowerCase();
    if (!state) return null;
    if (state === "planning") return "计划";
    if (state === "executing") return "执行";
    if (state === "done" || state === "completed") return "汇报";
    if (state === "failed") return "失败";
    if (state === "cancelled") return "已取消";
    return "执行";
  })();
  const groupRoundFromSnapshot = groupRunSnapshot?.current_round || 0;
  const groupReviewRound = groupRunSnapshot?.review_round || 0;
  const groupRunState = (groupRunSnapshot?.state || "").trim().toLowerCase();
  const groupWaitingLabel = groupRunSnapshot?.waiting_for_user
    ? "等待用户"
    : (groupRunSnapshot?.waiting_for_employee_id || "").trim();
  const groupStatusReason = (groupRunSnapshot?.status_reason || "").trim();
  const recentGroupEvents = (groupRunSnapshot?.events || []).slice(-4).reverse();
  const failedGroupRunSteps = (groupRunSnapshot?.steps || []).filter(
    (step) =>
      ((step.status || "").trim().toLowerCase() === "failed") &&
      ((step.step_type || "").trim().toLowerCase() === "execute"),
  );
  const groupRunAssignees = Array.from(
    new Set(
      (groupRunSnapshot?.steps || [])
        .map((step) => (step.assignee_employee_id || "").trim())
        .filter((value) => value.length > 0),
    ),
  );
  const groupRunStepMap = new Map(
    (groupRunSnapshot?.steps || []).map((step) => [step.id, step] as const),
  );
  const parseGroupRunEventPayload = (event: EmployeeGroupRunSnapshot["events"][number]) => {
    try {
      return event.payload_json ? (JSON.parse(event.payload_json) as Record<string, unknown>) : {};
    } catch {
      return {};
    }
  };
  const latestStepReassignPayloadByStepId = (() => {
    const byStepId = new Map<string, Record<string, unknown>>();
    for (const event of groupRunSnapshot?.events || []) {
      if (event.event_type !== "step_reassigned" || !event.step_id) continue;
      byStepId.set(event.step_id, parseGroupRunEventPayload(event));
    }
    return byStepId;
  })();
  const latestGroupEventByStepId = (() => {
    const byStepId = new Map<string, EmployeeGroupRunSnapshot["events"][number]>();
    for (const event of groupRunSnapshot?.events || []) {
      if (!event.step_id) continue;
      byStepId.set(event.step_id, event);
    }
    return byStepId;
  })();
  const formatGroupRunEventLabel = (event: EmployeeGroupRunSnapshot["events"][number]) => {
    const payload = parseGroupRunEventPayload(event);
    const relatedStep = groupRunStepMap.get(event.step_id);
    const assigneeEmployeeId = String(
      payload.assignee_employee_id || relatedStep?.assignee_employee_id || "",
    ).trim();
    const dispatchSourceEmployeeId = String(
      payload.dispatch_source_employee_id || relatedStep?.dispatch_source_employee_id || "",
    ).trim();
    if (
      ["step_created", "step_dispatched", "step_completed", "step_failed", "step_reassigned"].includes(
        event.event_type,
      )
    ) {
      if (dispatchSourceEmployeeId && assigneeEmployeeId) {
        return `${event.event_type} · ${dispatchSourceEmployeeId} -> ${assigneeEmployeeId}`;
      }
      if (assigneeEmployeeId) {
        return `${event.event_type} · ${assigneeEmployeeId}`;
      }
    }
    return event.event_type;
  };
  const groupRunEventTimelineByStepId = (() => {
    const byStepId = new Map<string, ChatSessionTimelineItem[]>();
    for (const event of groupRunSnapshot?.events || []) {
      if (!event.step_id) continue;
      const label = formatGroupRunEventLabel(event).trim();
      if (!label) continue;
      const payload = parseGroupRunEventPayload(event);
      const relatedStep = groupRunStepMap.get(event.step_id);
      const list = byStepId.get(event.step_id) || [];
      list.push({
        eventId: String(event.id || "").trim() || undefined,
        linkedSessionId: String(payload.session_id || relatedStep?.session_id || "").trim() || undefined,
        label,
        createdAt: String(event.created_at || "").trim() || undefined,
        openSessionOptions:
          event.step_id || String(payload.session_id || relatedStep?.session_id || "").trim()
            ? {
                focusHint: String(payload.output_summary || relatedStep?.output_summary || label || "").trim() || undefined,
                sourceSessionId: sessionId,
                sourceStepId: event.step_id || undefined,
                sourceEmployeeId: String(
                  payload.dispatch_source_employee_id || relatedStep?.dispatch_source_employee_id || "",
                ).trim() || undefined,
                assigneeEmployeeId: String(payload.assignee_employee_id || relatedStep?.assignee_employee_id || "").trim() || undefined,
              }
            : undefined,
      });
      byStepId.set(event.step_id, list);
    }
    for (const [stepId, items] of byStepId.entries()) {
      byStepId.set(stepId, items.slice(-3));
    }
    return byStepId;
  })();
  const groupRunExecuteStepCards = (groupRunSnapshot?.steps || [])
    .filter((step) => (step.step_type || "").trim().toLowerCase() === "execute")
    .map((step) => {
      const reassignPayload = latestStepReassignPayloadByStepId.get(step.id) || {};
      const latestEvent = latestGroupEventByStepId.get(step.id) || null;
      const latestEventPayload = latestEvent ? parseGroupRunEventPayload(latestEvent) : {};
      const currentAssigneeEmployeeId = String(
        reassignPayload.assignee_employee_id || step.assignee_employee_id || "",
      ).trim();
      const dispatchSourceEmployeeId = String(
        reassignPayload.dispatch_source_employee_id || step.dispatch_source_employee_id || "",
      ).trim();
      const previousAssigneeEmployeeId = String(
        reassignPayload.previous_assignee_employee_id || "",
      ).trim();
      const latestFailureSummary = String(
        reassignPayload.previous_output_summary ||
          (String(step.status || "").trim().toLowerCase() === "failed"
            ? step.output_summary || step.output || ""
            : ""),
      ).trim();
      const attemptNo =
        typeof step.attempt_no === "number" && Number.isFinite(step.attempt_no) && step.attempt_no > 0
          ? step.attempt_no
          : 1;
      const detailSessionId = String(step.session_id || latestEventPayload.session_id || "").trim();
      const detailOutputSummary = String(
        step.output_summary || latestEventPayload.output_summary || step.output || "",
      ).trim();
      const latestEventCreatedAt = String(latestEvent?.created_at || "").trim();
      const sourceStepTimeline = groupRunEventTimelineByStepId.get(step.id) || [];
      const sourceStepTimelineForOpenSession = sourceStepTimeline.map(({ eventId, linkedSessionId, label, createdAt }) => ({
        eventId,
        linkedSessionId,
        label,
        createdAt,
      }));
      return {
        step,
        currentAssigneeEmployeeId,
        dispatchSourceEmployeeId,
        previousAssigneeEmployeeId,
        latestFailureSummary,
        attemptNo,
        detailSessionId,
        detailOutputSummary,
        latestEventCreatedAt,
        sourceStepTimeline: sourceStepTimeline.map((item) => ({
          ...item,
          openSessionOptions: item.linkedSessionId
            ? {
                ...(item.openSessionOptions || {}),
                focusHint: detailOutputSummary || item.label || undefined,
                sourceSessionId: sessionId,
                sourceStepId: step.id,
                sourceEmployeeId: dispatchSourceEmployeeId || undefined,
                assigneeEmployeeId: currentAssigneeEmployeeId || undefined,
                sourceStepTimeline:
                  sourceStepTimelineForOpenSession.length > 0 ? sourceStepTimelineForOpenSession : undefined,
              }
            : item.openSessionOptions,
        })),
        detailSessionOpenOptions: detailSessionId
          ? {
              focusHint: detailOutputSummary || undefined,
              sourceSessionId: sessionId,
              sourceStepId: step.id,
              sourceEmployeeId: dispatchSourceEmployeeId || undefined,
              assigneeEmployeeId: currentAssigneeEmployeeId || undefined,
              sourceStepTimeline:
                sourceStepTimelineForOpenSession.length > 0 ? sourceStepTimelineForOpenSession : undefined,
            }
          : undefined,
      };
    });
  const toggleGroupRunStepDetails = (stepId: string) => {
    setExpandedGroupRunStepIds((prev) =>
      prev.includes(stepId) ? prev.filter((id) => id !== stepId) : [...prev, stepId],
    );
  };
  const groupRunExecuteRuleTargets = (dispatchSourceEmployeeId?: string) => {
    const coordinatorEmployeeId = groupRunCoordinatorEmployeeId.trim().toLowerCase();
    const normalizedDispatchSourceEmployeeId = (dispatchSourceEmployeeId || "").trim().toLowerCase();
    const memberSet = new Set(
      groupRunMemberEmployeeIds
        .map((value) => value.trim().toLowerCase())
        .filter((value) => value.length > 0),
    );
    const exactTargets = new Map<string, string>();
    const coordinatorTargets = new Map<string, string>();
    const fallbackTargets = new Map<string, string>();
    for (const rule of groupRunRules) {
      const relationType = (rule.relation_type || "").trim().toLowerCase();
      const phaseScope = (rule.phase_scope || "").trim().toLowerCase();
      if (!["delegate", "handoff"].includes(relationType)) continue;
      if (phaseScope.length > 0 && !["execute", "all", "*"].includes(phaseScope)) continue;
      const targetEmployeeId = (rule.to_employee_id || "").trim();
      const normalizedTargetEmployeeId = targetEmployeeId.toLowerCase();
      if (!targetEmployeeId || (memberSet.size > 0 && !memberSet.has(normalizedTargetEmployeeId))) {
        continue;
      }
      if (!fallbackTargets.has(normalizedTargetEmployeeId)) {
        fallbackTargets.set(normalizedTargetEmployeeId, targetEmployeeId);
      }
      const fromEmployeeId = (rule.from_employee_id || "").trim().toLowerCase();
      if (
        normalizedDispatchSourceEmployeeId &&
        fromEmployeeId === normalizedDispatchSourceEmployeeId &&
        !exactTargets.has(normalizedTargetEmployeeId)
      ) {
        exactTargets.set(normalizedTargetEmployeeId, targetEmployeeId);
      }
      if (
        coordinatorEmployeeId &&
        fromEmployeeId === coordinatorEmployeeId &&
        !coordinatorTargets.has(normalizedTargetEmployeeId)
      ) {
        coordinatorTargets.set(normalizedTargetEmployeeId, targetEmployeeId);
      }
    }
    const preferredTargets =
      exactTargets.size > 0
        ? exactTargets
        : coordinatorTargets.size > 0
          ? coordinatorTargets
          : fallbackTargets;
    return {
      hasExecuteRules: fallbackTargets.size > 0,
      ids: Array.from(preferredTargets.values()),
    };
  };
  const groupRunCandidateEmployeeIds = (step?: EmployeeGroupRunSnapshot["steps"][number]) =>
    Array.from(
      new Set(
        (
          groupRunExecuteRuleTargets(step?.dispatch_source_employee_id).hasExecuteRules
            ? groupRunExecuteRuleTargets(step?.dispatch_source_employee_id).ids
            : [...groupRunMemberEmployeeIds, ...groupRunAssignees]
        )
          .map((value) => value.trim())
          .filter((value) => value.length > 0),
      ),
    );
  const failedGroupRunReassignOptions = failedGroupRunSteps
    .map((step) => ({
      step,
      candidateEmployeeIds: groupRunCandidateEmployeeIds(step).filter(
        (employeeId) =>
          employeeId.trim().toLowerCase() !== (step.assignee_employee_id || "").trim().toLowerCase(),
      ),
    }))
    .filter((entry) => entry.candidateEmployeeIds.length > 0);
  const canPauseGroupRun =
    !!groupRunSnapshot &&
    !["paused", "done", "completed", "cancelled", "failed"].includes(groupRunState);
  const canResumeGroupRun = !!groupRunSnapshot && groupRunState === "paused";
  const canRetryFailedGroupRunSteps = failedGroupRunSteps.length > 0;
  const canReassignFailedGroupRunStep = failedGroupRunReassignOptions.length > 0;
  const showStreamingThinkingState =
    Boolean(streamReasoning) || (agentState?.state === "thinking" && thinkingSupport.indicator);
  const showStreamingAssistantBubble =
    showStreamingThinkingState || streamItems.length > 0 || subAgentBuffer.length > 0;
  const handleToggleRunDetail = (runId: string) => {
    setExpandedRunDetailIds((prev) =>
      prev.includes(runId) ? prev.filter((item) => item !== runId) : [...prev, runId],
    );
  };
  const handleContinueExecution = () =>
    sendContent({
      sessionId,
      parts: [{ type: "text", text: CONTINUE_MESSAGE_TEXT }],
      maxIterations: CONTINUE_BUDGET_INCREMENT,
    });
  const liveBlockingStatus = useMemo(() => {
    if (pendingApprovals.length > 0 || agentState?.state === "waiting_approval") {
      return "waiting_approval";
    }
    if (agentState?.state === "thinking" || streamReasoning?.status === "thinking") {
      return "thinking";
    }
    if (agentState?.state === "tool_calling") {
      return "tool_calling";
    }
    if (streaming || streamItems.length > 0 || subAgentBuffer.trim()) {
      return "running";
    }
    return null;
  }, [agentState?.state, pendingApprovals.length, streamItems.length, streamReasoning?.status, streaming, subAgentBuffer]);
  const shouldShowTeamEntryEmptyState =
    isTeamEntrySession &&
    !initialMessage?.trim() &&
    messages.length === 0 &&
    streamItems.length === 0 &&
    !subAgentBuffer.trim() &&
    !streaming &&
    !groupRunSnapshot;
  const groupMemberStatesFromSnapshot = (() => {
    const byRole = new Map<string, { status: string; stepType: string }>();
    for (const step of groupRunSnapshot?.steps || []) {
      const role = (step.assignee_employee_id || "").trim();
      if (!role) continue;
      byRole.set(role, {
        status: step.status || "running",
        stepType: (step.step_type || "").trim(),
      });
    }
    return Array.from(byRole.entries()).map(([role, info]) => ({
      role,
      status: info.status,
      stepType: info.stepType,
    }));
  })();
  const groupPhaseLabel = groupPhaseLabelFromSnapshot || groupPhaseFromSnapshot || groupPhaseFromEvents;
  const groupRound = groupRoundFromSnapshot || groupRoundFromEvents;
  const groupMemberStates =
    groupMemberStatesFromSnapshot.length > 0 ? groupMemberStatesFromSnapshot : groupMemberStatesFromEvents;
  const collaborationStatusText =
    mainSummaryDelivered
      ? `${mainRoleName || "主员工"} 已输出最终汇总`
      : runningDelegationCount > 0 && primaryDelegationCard
      ? `${mainRoleName || "主员工"} 正在处理，已委派 ${primaryDelegationCard.toRole}`
      : latestCompletedDelegation
      ? `${latestCompletedDelegation.toRole} 已完成，${mainRoleName || "主员工"} 正在汇总最终答复`
      : `${mainRoleName || "主员工"} 正在处理`;

  useEffect(() => {
    onSessionBlockingStateChange?.({
      blocking: Boolean(liveBlockingStatus),
      status: liveBlockingStatus,
    });
  }, [liveBlockingStatus, onSessionBlockingStateChange]);

  function parseClawhubCandidatesFromOutput(output?: string): ClawhubInstallCandidate[] {
    if (!output) return [];
    try {
      const parsed = JSON.parse(output);
      if (parsed?.source !== "clawhub" || !Array.isArray(parsed?.items)) return [];
      return parsed.items
        .map((item: any) => {
          const slug = typeof item?.slug === "string" ? item.slug.trim() : "";
          const name = typeof item?.name === "string" ? item.name.trim() : "";
          if (!slug || !name) return null;
          return {
            slug,
            name,
            description: typeof item?.description === "string" ? item.description : "",
            stars: typeof item?.stars === "number" ? item.stars : undefined,
            githubUrl: typeof item?.github_url === "string" ? item.github_url : null,
            sourceUrl: typeof item?.source_url === "string" ? item.source_url : null,
            sourceKind: "clawhub",
          } as ClawhubInstallCandidate;
        })
        .filter(Boolean) as ClawhubInstallCandidate[];
    } catch {
      return [];
    }
  }

  function mergeInstallCandidate(map: Map<string, ClawhubInstallCandidate>, candidate: ClawhubInstallCandidate) {
    const key = `${candidate.slug}:${candidate.githubUrl ?? ""}`;
    const exists = map.get(key);
    if (!exists) {
      map.set(key, candidate);
      return;
    }
    const existingLen = exists.description?.length ?? 0;
    const currentLen = candidate.description?.length ?? 0;
    if (currentLen > existingLen || (candidate.stars ?? 0) > (exists.stars ?? 0)) {
      map.set(key, candidate);
    }
  }

  function extractInstallCandidates(items: StreamItem[] | undefined, content?: string): ClawhubInstallCandidate[] {
    const map = new Map<string, ClawhubInstallCandidate>();
    if (items && items.length > 0) {
      for (const item of items) {
        if (item.type !== "tool_call" || !item.toolCall) continue;
        const name = item.toolCall.name;
        if (name !== "clawhub_search" && name !== "clawhub_recommend") continue;
        const parsed = parseClawhubCandidatesFromOutput(item.toolCall.output);
        for (const candidate of parsed) {
          mergeInstallCandidate(map, candidate);
        }
      }
    }
    return Array.from(map.values());
  }

  const candidateTranslationTexts = useMemo(
    () => [
      ...messages.flatMap((m) =>
        extractInstallCandidates(m.streamItems, m.content).flatMap((candidate) => [
          candidate.name,
          candidate.description ?? "",
        ]),
      ),
      ...extractInstallCandidates(streamItems).flatMap((candidate) => [
        candidate.name,
        candidate.description ?? "",
      ]),
    ],
    [messages, streamItems],
  );
  const { renderDisplayText: renderCandidateText } = useImmersiveTranslation(
    candidateTranslationTexts,
    {
      scene: "experts-finder",
      batchSize: 80,
    },
  );

  function renderInstallCandidates(rawCandidates: unknown[]) {
    const candidates = rawCandidates as ClawhubInstallCandidate[];
    if (candidates.length === 0) return null;
    return (
      <div className="mt-3 border border-blue-100 bg-blue-50/40 rounded-xl p-3">
        <div className="text-xs font-medium text-blue-700 mb-2">可安装技能</div>
        <div className="space-y-2">
          {candidates.map((candidate) => {
            const installed = installedSkillSet.has(`clawhub-${candidate.slug}`);
            const isInstalling = installingSlug === candidate.slug;
            return (
              <div key={`${candidate.slug}:${candidate.githubUrl ?? ""}`} className="rounded-lg border border-blue-100 bg-white p-2.5">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="text-sm font-medium text-gray-800 truncate">
                      {renderCandidateText(candidate.name)}
                    </div>
                    <div className="text-[11px] text-gray-400">slug: {candidate.slug}</div>
                  </div>
                  <button
                    onClick={() => {
                      if (installed || isInstalling) return;
                      setInstallError(null);
                      setPendingInstallSkill(candidate);
                      setShowInstallConfirm(true);
                    }}
                    disabled={installed || isInstalling}
                    className={`h-7 px-2.5 rounded text-xs font-medium transition-colors ${
                      installed
                        ? "bg-gray-100 text-gray-400 cursor-not-allowed"
                        : isInstalling
                        ? "bg-blue-100 text-blue-400 cursor-not-allowed"
                        : "bg-blue-500 hover:bg-blue-600 text-white"
                    }`}
                  >
                    {installed ? "已安装" : isInstalling ? "安装中..." : "立即安装"}
                  </button>
                </div>
                {candidate.description && (
                  <div className="mt-1.5 text-xs text-gray-600 line-clamp-2">
                    {renderCandidateText(candidate.description)}
                  </div>
                )}
                <div className="mt-1.5 text-[11px] text-gray-400">stars: {candidate.stars ?? 0}</div>
              </div>
            );
          })}
        </div>
        {installError && <div className="mt-2 text-xs text-red-500">{installError}</div>}
      </div>
    );
  }

  async function handleConfirmInstall() {
    if (!pendingInstallSkill || installInFlightRef.current) return;
    installInFlightRef.current = true;
    setInstallError(null);
    setInstallingSlug(pendingInstallSkill.slug);
    try {
      const result = await invoke<{ manifest: { id: string } }>("install_clawhub_skill", {
        slug: pendingInstallSkill.slug,
        githubUrl: pendingInstallSkill.githubUrl ?? pendingInstallSkill.sourceUrl ?? null,
      });
      if (result?.manifest?.id) {
        await onSkillInstalled?.(result.manifest.id);
      }
      setShowInstallConfirm(false);
      setPendingInstallSkill(null);
    } catch (e) {
      const duplicateName = parseDuplicateSkillName(e);
      if (duplicateName) {
        setInstallError(`技能名称冲突：已存在「${duplicateName}」，请先重命名后再安装。`);
      } else {
        setInstallError("安装失败，请重试。");
      }
      console.error("安装 ClawHub 技能失败:", e);
    } finally {
      installInFlightRef.current = false;
      setInstallingSlug(null);
    }
  }

  function handleCancelInstallConfirm() {
    if (installInFlightRef.current) return;
    setShowInstallConfirm(false);
    setPendingInstallSkill(null);
  }

  function handleComposerWorkdirClick() {
    invoke<string | null>("select_directory", {
      defaultPath: workspace || undefined,
    }).then((newDir) => {
      if (newDir) {
        updateWorkspace(newDir);
      }
    });
  }

  function handleComposerRemoveAttachment(fileId: string) {
    removeAttachedFile(fileId);
  }

  async function handleCopyAssistantMessage(messageKey: string, content: string) {
    const trimmed = content.trim();
    if (!trimmed) return;
    await globalThis.navigator?.clipboard?.writeText?.(trimmed);
    setCopiedAssistantMessageKey(messageKey);
    window.setTimeout(() => {
      setCopiedAssistantMessageKey((current) => (current === messageKey ? null : current));
    }, 1600);
  }

  async function handleOpenChatExternalLink(url: string) {
    try {
      await openExternalUrl(url);
      setChatLinkToast({
        variant: "success",
        message: "已在浏览器打开",
        url,
      });
    } catch (error) {
      console.error("打开会话外链失败:", error);
      setChatLinkToast({
        variant: "error",
        message: "链接打开失败",
        url,
      });
    }
  }

  async function handleCopyChatLink(url: string) {
    const trimmed = url.trim();
    if (!trimmed) return;
    try {
      await globalThis.navigator?.clipboard?.writeText?.(trimmed);
      setChatLinkToast({
        variant: "success",
        message: "链接已复制",
        url: trimmed,
      });
    } catch (error) {
      console.error("复制会话外链失败:", error);
      setChatLinkToast({
        variant: "error",
        message: "复制链接失败",
        url: trimmed,
      });
    }
  }

  function getAgentStateLabel() {
    if (!agentState) return "";
    if (agentState.state === "thinking") return "正在分析任务";
    if (agentState.state === "retrying") {
      return agentState.detail || "网络异常，正在自动重试";
    }
    if (agentState.state === "tool_calling") {
      return agentState.detail ? `正在处理步骤：${agentState.detail}` : "正在处理步骤";
    }
    if (agentState.state === "stopped") {
      return agentState.stopReasonTitle || agentState.stopReasonMessage || agentState.detail || "任务已停止";
    }
    if (agentState.state === "error") {
      return `执行异常：${agentState.detail || "未知错误"}`;
    }
    return agentState.detail || agentState.state;
  }

  function renderAgentStateIndicator() {
    if (!agentState) return null;
    if (agentState.state === "stopped") {
      return <span className="inline-flex h-3 w-3 rounded-full bg-amber-400" />;
    }
    if (agentState.state === "error") {
      return <span className="inline-flex h-3 w-3 rounded-full bg-red-400" />;
    }
    return <span className="animate-spin h-3 w-3 border-2 border-blue-400 border-t-transparent rounded-full" />;
  }

  function renderAgentStateSecondaryText() {
    if (!agentState || agentState.state !== "stopped") {
      return null;
    }

    const secondaryLines: string[] = [];
    if (agentState.stopReasonMessage && agentState.stopReasonMessage !== agentState.stopReasonTitle) {
      secondaryLines.push(agentState.stopReasonMessage);
    }
    if (
      agentState.detail &&
      agentState.detail !== agentState.stopReasonTitle &&
      agentState.detail !== agentState.stopReasonMessage
    ) {
      secondaryLines.push(agentState.detail);
    }
    if (agentState.stopReasonLastCompletedStep) {
      secondaryLines.push(`最后完成步骤：${agentState.stopReasonLastCompletedStep}`);
    }
    if (secondaryLines.length === 0) {
      return null;
    }

    return (
      <div className="flex min-w-0 flex-col gap-0.5 text-[11px] text-amber-700">
        {secondaryLines.map((line) => (
          <span key={line} className="whitespace-pre-wrap">
            {line}
          </span>
        ))}
      </div>
    );
  }

  function handleViewFilesFromDelivery() {
    setSidePanelOpen(true);
    setSidePanelTab("files");
  }

  function getRunFailureDisplay(run: SessionRunProjection) {
    const networkRecoverySuffix =
      "\n已经保留当前任务的历史消息和部分输出。网络恢复后可直接输入“继续”，从当前上下文继续完成任务。";

    if (run.error_kind === "cancelled") {
      return {
        title: "任务已取消",
        message: run.error_message || "",
        rawMessage: null as string | null,
      };
    }

    if (run.error_kind === "max_turns") {
      return {
        title: "任务达到执行步数上限",
        message:
          run.error_message || "已达到执行步数上限，系统已自动停止。\n你可以点击下方“继续执行”，或直接发送“继续”来再追加 100 步预算。",
        rawMessage: null as string | null,
      };
    }

    if (run.error_kind === "loop_detected") {
      return {
        title: "任务疑似卡住，已自动停止",
        message: run.error_message || "系统检测到重复执行模式，已自动停止本轮任务。",
        rawMessage: null as string | null,
      };
    }

    if (run.error_kind === "no_progress") {
      return {
        title: "任务长时间没有进展",
        message: run.error_message || "系统检测到任务在多轮执行后没有明显进展，已自动停止。",
        rawMessage: null as string | null,
      };
    }

    if (run.error_kind === "policy_blocked") {
      return {
        title: "当前任务无法继续执行",
        message: run.error_message || "本次请求触发了安全或工作区限制，系统已停止继续尝试。",
        rawMessage: null as string | null,
      };
    }

    if (isModelErrorKind(run.error_kind)) {
      const display = getModelErrorDisplay(run.error_kind);
      return {
        title: display.title,
        message: run.error_kind === "network" ? `${display.message}${networkRecoverySuffix}` : display.message,
        rawMessage:
          run.error_kind === "unknown" &&
          run.error_message &&
          run.error_message !== display.title &&
          run.error_message !== display.message
            ? run.error_message
            : null,
      };
    }

    const inferredModelErrorKind = run.error_message
      ? inferModelErrorKindFromMessage(run.error_message)
      : null;
    if (inferredModelErrorKind) {
      const display = getModelErrorDisplay(inferredModelErrorKind);
      return {
        title: display.title,
        message:
          inferredModelErrorKind === "network"
            ? `${display.message}${networkRecoverySuffix}`
            : display.message,
        rawMessage: null as string | null,
      };
    }

    return {
      title: run.error_message || "本轮执行失败",
      message: "",
      rawMessage: null as string | null,
    };
  }

  function isContinuationText(text: string) {
    const normalized = text.trim().toLowerCase();
    return normalized === "继续" || normalized === "继续执行" || normalized === "continue";
  }

  function shouldGrantContinuationBudget(request: SendMessageRequest) {
    if (!latestMaxTurnsRun) return false;
    if (request.parts.length !== 1) return false;
    const [part] = request.parts;
    if (part.type !== "text") return false;
    return isContinuationText(part.text);
  }

  const shouldShowAgentStateBanner = !(
    agentState?.state === "error" &&
    failedSessionRuns.some((run) => {
      if (run.status !== "failed") {
        return false;
      }
      if (isModelErrorKind(run.error_kind)) {
        return true;
      }
      const errorMessage = (run.error_message || "").trim();
      return errorMessage ? inferModelErrorKindFromMessage(errorMessage) !== null : false;
    })
  );
  return (
    <ChatShell
      header={
        <ChatHeader
          sessionDisplayTitle={sessionDisplayTitle}
          sessionDisplaySubtitle={sessionDisplaySubtitle}
          isImSource={isImSource}
          sessionSourceBadgeText={sessionSourceBadgeText}
          sidePanelOpen={sidePanelOpen}
          onToggleSidePanel={() => setSidePanelOpen(!sidePanelOpen)}
        />
      }
      executionContextBar={
        sessionExecutionContext ? (
          <ChatExecutionContextBar
            sessionExecutionContext={sessionExecutionContext}
            onOpenSession={onOpenSession}
            onReturnToSourceSession={onReturnToSourceSession}
          />
        ) : undefined
      }
      mainContent={
        <>
        {/* 消息列表 */}
        <div className="relative flex-1 bg-[#f7f7f4]">
        <div
          ref={scrollRegionRef}
          data-testid="chat-scroll-region"
          onScroll={handleScrollRegionScroll}
          className="h-full overflow-y-auto bg-transparent px-4 py-6 sm:px-6 xl:px-8"
        >
        <div data-testid="chat-content-rail" className="mx-auto flex w-full max-w-[76rem] flex-col gap-5">
        {employeeAssistantContext && (
          <div className="space-y-3">
            <div
              data-testid="chat-employee-assistant-context"
              className="rounded-xl border border-blue-200 bg-blue-50 px-4 py-2 text-xs text-blue-800"
            >
              {employeeAssistantContext.mode === "update"
                ? `正在修改：${employeeAssistantContext.employeeName || "目标员工"}${
                    employeeAssistantContext.employeeCode
                      ? `（${employeeAssistantContext.employeeCode}）`
                      : ""
                  }`
                : "正在创建：新智能体员工"}
            </div>
            {employeeAssistantContext.mode === "create" && (
              <div className="max-w-[80%] rounded-2xl border border-blue-100 bg-white px-5 py-4 text-sm text-slate-700 shadow-sm">
                我会先问 1-2 个关键问题，再给出配置草案，确认后执行创建。
              </div>
            )}
          </div>
        )}
        {agentState && agentState.state !== "thinking" && shouldShowAgentStateBanner && (
          <div
            className={`sticky top-0 z-10 flex items-center gap-2 bg-white/80 backdrop-blur-lg px-4 py-2 rounded-xl text-xs border shadow-sm mx-4 mt-2 ${
              agentState.state === "stopped"
                ? "text-amber-800 border-amber-200"
                : agentState.state === "error"
                ? "text-red-700 border-red-200"
                : agentState.state === "retrying"
                ? "text-blue-700 border-blue-200"
                : "text-gray-600 border-gray-200"
            }`}
          >
            {renderAgentStateIndicator()}
            <div className="flex min-w-0 flex-col">
              <span
                className={
                  agentState.state === "error"
                    ? "text-red-500"
                    : agentState.state === "retrying"
                    ? "text-blue-700"
                    : undefined
                }
              >
                {getAgentStateLabel()}
              </span>
              {renderAgentStateSecondaryText()}
            </div>
          </div>
        )}
        <ChatCollaborationStatusPanel
          mainRoleName={mainRoleName}
          primaryDelegationCard={primaryDelegationCard}
          delegationHistoryCards={delegationHistoryCards}
          collaborationStatusText={collaborationStatusText}
          completedDelegationCount={completedDelegationCount}
          failedDelegationCount={failedDelegationCount}
        />
        <ChatGroupRunSection
          groupPhaseLabel={groupPhaseLabel}
          groupRound={groupRound}
          groupReviewRound={groupReviewRound}
          groupWaitingLabel={groupWaitingLabel}
          groupStatusReason={groupStatusReason}
          groupRunSnapshot={groupRunSnapshot}
          onApproveGroupRunReview={() => void handleApproveGroupRunReview()}
          onRejectGroupRunReview={() => void handleRejectGroupRunReview()}
          onPauseGroupRun={() => void handlePauseGroupRun()}
          onResumeGroupRun={() => void handleResumeGroupRun()}
          onRetryFailedGroupRunSteps={() => void handleRetryFailedGroupRunSteps()}
          onReassignFailedGroupRunStep={handleReassignFailedGroupRunStep}
          groupRunActionLoading={groupRunActionLoading}
          canPauseGroupRun={canPauseGroupRun}
          canResumeGroupRun={canResumeGroupRun}
          canRetryFailedGroupRunSteps={canRetryFailedGroupRunSteps}
          canReassignFailedGroupRunStep={canReassignFailedGroupRunStep}
          failedGroupRunReassignOptions={failedGroupRunReassignOptions}
          groupMemberStates={groupMemberStates}
          recentGroupEvents={recentGroupEvents.map((event) => ({
            id: event.id,
            label: formatGroupRunEventLabel(event),
          }))}
          groupRunExecuteStepCards={groupRunExecuteStepCards}
          highlightedGroupRunStepId={highlightedGroupRunStepId}
          highlightedGroupRunStepEventId={highlightedGroupRunStepEventId}
          expandedGroupRunStepIds={expandedGroupRunStepIds}
          groupRunStepElementRefs={groupRunStepElementRefs}
          groupRunStepEventElementRefs={groupRunStepEventElementRefs}
          onToggleGroupRunStepDetails={toggleGroupRunStepDetails}
          onOpenSession={onOpenSession}
          sessionId={sessionId}
          shouldShowTeamEntryEmptyState={shouldShowTeamEntryEmptyState}
          sessionDisplaySubtitle={sessionDisplaySubtitle}
        />
        <ChatMessageRail
          renderedMessages={renderedMessages}
          highlightedMessageIndex={highlightedMessageIndex}
          messageElementRefs={messageElementRefs}
          expandedThinkingKeys={expandedThinkingKeys}
          onToggleThinkingBlock={toggleThinkingBlock}
          buildTaskJourneyModel={buildTaskJourneyViewModel}
          shouldRenderCompletedJourneySummary={shouldRenderCompletedJourneySummary}
          failedRunsByAssistantMessageId={failedRunsByAssistantMessageId}
          failedRunsByUserMessageId={failedRunsByUserMessageId}
          renderInstallCandidates={renderInstallCandidates}
          extractInstallCandidates={extractInstallCandidates}
          renderUserContentParts={renderUserContentParts}
          copiedAssistantMessageKey={copiedAssistantMessageKey}
          onCopyAssistantMessage={handleCopyAssistantMessage}
          CopyActionIcon={CopyActionIcon}
          onViewFilesFromDelivery={handleViewFilesFromDelivery}
          expandedRunDetailIds={expandedRunDetailIds}
          streaming={streaming}
          onToggleRunDetail={handleToggleRunDetail}
          onContinueExecution={handleContinueExecution}
          getRunFailureDisplay={getRunFailureDisplay}
          orphanFailedRuns={orphanFailedRuns}
          showStreamingAssistantBubble={showStreamingAssistantBubble}
          showStreamingThinkingState={showStreamingThinkingState}
          streamReasoning={streamReasoning}
          streamItems={streamItems}
          toolManifest={toolManifest}
          subAgentBuffer={subAgentBuffer}
          subAgentRoleName={subAgentRoleName}
          askUserQuestion={askUserQuestion}
          askUserOptions={askUserOptions}
          askUserAnswer={askUserAnswer}
          onAskUserAnswerChange={setAskUserAnswer}
          onAnswerUser={handleAnswerUser}
          onOpenExternalLink={handleOpenChatExternalLink}
        />
        {chatLinkToast && (
          <div className="pointer-events-none absolute inset-x-0 bottom-5 z-20 flex justify-center px-4">
            <div
              data-testid="chat-link-toast"
              className={
                "pointer-events-auto flex max-w-[36rem] items-center gap-3 rounded-2xl border px-4 py-3 text-sm shadow-lg backdrop-blur-sm " +
                (chatLinkToast.variant === "success"
                  ? "border-emerald-200 bg-white/95 text-emerald-700"
                  : "border-rose-200 bg-white/95 text-rose-700")
              }
            >
              <span className="font-medium">{chatLinkToast.message}</span>
              {chatLinkToast.variant === "error" && (
                <>
                  <button
                    type="button"
                    className="rounded-lg border border-rose-200 bg-rose-50 px-3 py-1.5 text-xs font-medium text-rose-700 transition-colors hover:bg-rose-100"
                    onClick={() => void handleOpenChatExternalLink(chatLinkToast.url)}
                  >
                    重试
                  </button>
                  <button
                    type="button"
                    className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 text-xs font-medium text-slate-700 transition-colors hover:bg-slate-100"
                    onClick={() => void handleCopyChatLink(chatLinkToast.url)}
                  >
                    复制链接
                  </button>
                </>
              )}
              <button
                type="button"
                aria-label="关闭链接提示"
                className="rounded-lg px-2 py-1 text-xs text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-700"
                onClick={() => setChatLinkToast(null)}
              >
                关闭
              </button>
            </div>
          </div>
        )}
        <RiskConfirmDialog
          open={Boolean(activePendingApproval)}
          level="high"
          title={activePendingApprovalDialog?.title || "高危操作确认"}
          summary={activePendingApprovalDialog?.summary || "请确认是否继续执行。"}
          impact={activePendingApprovalDialog?.impact}
          note={activePendingApprovalDialog?.note}
          irreversible={activePendingApprovalDialog?.irreversible}
          confirmLabel="允许一次"
          secondaryActionLabel="始终允许"
          cancelLabel="取消"
          loading={Boolean(resolvingApprovalId)}
          onConfirm={() => void handleResolveApproval("allow_once")}
          onSecondaryAction={() => void handleResolveApproval("allow_always")}
          onCancel={() => void handleResolveApproval("deny")}
        />
        <RiskConfirmDialog
          open={showInstallConfirm && Boolean(pendingInstallSkill)}
          level="medium"
          title="安装技能"
          summary={
            pendingInstallSkill
              ? `是否安装「${renderCandidateText(pendingInstallSkill.name)}」？`
              : "是否安装该技能？"
          }
          impact={pendingInstallSkill ? `slug: ${pendingInstallSkill.slug}` : undefined}
          irreversible={false}
          confirmLabel="确认安装"
          cancelLabel="取消"
          loading={Boolean(installingSlug)}
          onConfirm={handleConfirmInstall}
          onCancel={handleCancelInstallConfirm}
        />
        <div ref={bottomRef} />
        </div>
      </div>
      {showScrollJump && (
        <div className="pointer-events-none absolute inset-x-0 bottom-5 z-20 flex justify-center">
          <motion.button
            type="button"
            data-testid="chat-scroll-jump-button"
            aria-label={scrollJumpLabel}
            title={scrollJumpHint}
            onClick={handleScrollJump}
            initial={false}
            animate={{
              opacity: isNearBottom ? 0.94 : 0.88,
              y: isNearBottom ? 0 : -20,
              scale: isNearBottom ? 1 : 0.985,
            }}
            transition={{ type: "spring", stiffness: 240, damping: 28, mass: 0.8 }}
            className="pointer-events-auto flex h-9 w-9 items-center justify-center rounded-full border border-slate-200/85 bg-[#f4f4f1]/92 text-slate-500 shadow-[0_6px_16px_rgba(15,23,42,0.08)] transition-all duration-200 hover:border-slate-300 hover:bg-[#f7f7f4] hover:text-slate-700 hover:shadow-[0_10px_22px_rgba(15,23,42,0.1)]"
          >
            <motion.span
              aria-hidden="true"
              initial={false}
              animate={{ rotate: isNearBottom ? 0 : 180 }}
              transition={{ duration: 0.22, ease: "easeInOut" }}
              className="translate-y-[-1px] text-[20px] leading-none"
            >
              ↑
            </motion.span>
          </motion.button>
        </div>
      )}
      </div>
      </>
      }
      sidePanel={<ChatWorkspaceSidePanel
        open={sidePanelOpen}
        tab={sidePanelTab}
        onTabChange={setSidePanelTab}
        onClose={() => setSidePanelOpen(false)}
        workspace={workspace}
        touchedFiles={touchedFilePaths}
        active={sidePanelOpen}
        taskModel={taskPanelModel}
        webSearchEntries={webSearchEntries}
      />}
      composer={
        <ChatComposer
          operationPermissionMode={operationPermissionMode}
          quickPrompts={quickPrompts}
          streaming={streaming}
          sendContent={sendContent}
          attachedFiles={attachedFiles}
          onFileSelect={handleFileSelect}
          composerError={composerError}
          input={input}
          textareaRef={textareaRef}
          onInputChange={handleComposerInputChange}
          onSubmit={handleSend}
          onWorkdirClick={handleComposerWorkdirClick}
          displayWorkDirLabel={displayWorkDirLabel}
          currentModel={currentModel}
          onRemoveAttachment={handleComposerRemoveAttachment}
          onCancel={handleCancel}
        />
      }
    />
  );
}
