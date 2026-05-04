import { useState, useEffect, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SkillManifest, ModelConfig, PendingAttachment, SendMessageRequest, EmployeeGroupRunSnapshot, PersistedChatRuntimeState, ChatDelegationCardState } from "../types";
import { ChatWorkspaceSidePanel } from "./chat-side-panel/ChatWorkspaceSidePanel";
import { ChatActionDialogs } from "./chat/ChatActionDialogs";
import { ChatExecutionContextBar } from "./chat/ChatExecutionContextBar";
import { ChatHeader } from "./chat/ChatHeader";
import { ChatComposer } from "./chat/ChatComposer";
import { ChatCollaborationStatusPanel } from "./chat/ChatCollaborationStatusPanel";
import { ChatEmployeeAssistantContext } from "./chat/ChatEmployeeAssistantContext";
import { ChatAgentStateBanner } from "./chat/ChatAgentStateBanner";
import { ChatLinkToast } from "./chat/ChatLinkToast";
import { ChatMessageRail } from "./chat/ChatMessageRail";
import { useChatInstallCandidatesController } from "./chat/useChatInstallCandidatesController";
import { useChatLinkActions } from "./chat/useChatLinkActions";
import { useChatDerivedViewModels } from "./chat/useChatDerivedViewModels";
import { ChatScrollJumpButton } from "./chat/ChatScrollJumpButton";
import { useChatViewportController } from "./chat/useChatViewportController";
import { ChatGroupRunSection } from "./chat/group-run/ChatGroupRunSection";
import { ChatShell } from "./chat/ChatShell";
import {
  buildApprovalImpactText,
  buildApprovalReasonText,
  buildChatAgentBannerViewModel,
  CopyActionIcon,
  extractInstallCandidates,
  getRunFailureDisplay,
  getThinkingSupport,
  shouldRenderCompletedJourneySummary,
  TOOL_ACTION_LABELS,
} from "./chat/chatViewHelpers";
import { deriveDelegationState, deriveGroupRunState } from "./chat/chatGroupRunHelpers";
import {
  clearSessionDraft,
  clonePersistedChatRuntimeState,
  persistSessionDraft,
  readSessionDraft,
} from "../scenes/chat/chatRuntimeState";
import { useChatDraftState } from "../scenes/chat/useChatDraftState";
import { buildTaskJourneyViewModel } from "./chat-side-panel/view-model";
import type { TaskJourneyViewModel } from "./chat-side-panel/view-model";
import { getDefaultModel } from "../lib/default-model";
import { answerUserQuestion, cancelAgent } from "../services/chat/chatSessionService";
import { resolveApproval as resolvePendingApproval } from "../services/chat/chatApprovalService";
import { useChatSessionController, type PendingApprovalView } from "../scenes/chat/useChatSessionController";
import { useChatCollaborationController } from "../scenes/chat/useChatCollaborationController";
import {
  buildMessageParts,
  useChatSendController,
} from "../scenes/chat/useChatSendController";
import { useLocalChatCommandRunner } from "../scenes/chat/useLocalChatCommandRunner";
import { useChatStreamController } from "../scenes/chat/useChatStreamController";

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

const CONTINUE_MESSAGE_TEXT = "继续";
const CONTINUE_BUDGET_INCREMENT = 100;

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
  const initialRuntimeState = clonePersistedChatRuntimeState(persistedRuntimeState);
  const [expandedRunDetailIds, setExpandedRunDetailIds] = useState<string[]>([]);
  const [mainRoleName, setMainRoleName] = useState(initialRuntimeState.mainRoleName);
  const [mainSummaryDelivered, setMainSummaryDelivered] = useState(initialRuntimeState.mainSummaryDelivered);
  const [delegationCards, setDelegationCards] = useState<ChatDelegationCardState[]>(initialRuntimeState.delegationCards);
  const sessionIdRef = useRef(sessionId);
  const mainRoleNameRef = useRef("");
  const loadMessagesRef = useRef<(sid: string) => Promise<void>>(async () => {});
  const loadSessionRunsRef = useRef<(sid: string) => Promise<void>>(async () => {});
  const pendingApprovalsSnapshotRef = useRef<PendingApprovalView[]>([]);
  const resolvingApprovalSnapshotRef = useRef<string | null>(null);
  const {
    input,
    setInput,
    attachedFiles,
    composerError,
    setComposerError,
    isAddingFiles,
    textareaRef,
    handleComposerInputChange,
    addFiles,
    hasPendingAttachmentIntake,
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
  const {
    copiedAssistantMessageKey,
    chatLinkToast,
    handleCopyAssistantMessage,
    handleOpenChatExternalLink,
    handleCopyChatLink,
    closeChatLinkToast,
  } = useChatLinkActions();

  const toggleThinkingBlock = (key: string) => {
    setExpandedThinkingKeys((prev) => (prev.includes(key) ? prev.filter((item) => item !== key) : [...prev, key]));
  };

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
    compactionStatus,
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
      compactionStatus: compactionStatus ? { ...compactionStatus } : null,
      subAgentBuffer,
      subAgentRoleName,
      mainRoleName,
      mainSummaryDelivered,
      delegationCards: delegationCards.map((item) => ({ ...item })),
    }),
    [
      agentState,
      compactionStatus,
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
    onResetForSessionSwitch: () => {},
  });
  collaborationControllerState.resetForSessionSwitch = resetCollaborationForSessionSwitch;

  loadMessagesRef.current = loadMessages;
  loadSessionRunsRef.current = loadSessionRuns;
  pendingApprovalsSnapshotRef.current = pendingApprovals;
  resolvingApprovalSnapshotRef.current = resolvingApprovalId;

  const {
    highlightedMessageIndex,
    highlightedGroupRunStepId,
    highlightedGroupRunStepEventId,
    isNearTop,
    isNearBottom,
    setIsNearTop,
    setIsNearBottom,
    hasScrollableContent,
    scrollTop,
    viewportHeight,
    bottomRef,
    scrollRegionRef,
    autoFollowScrollRef,
    messageElementRefs,
    groupRunStepElementRefs,
    groupRunStepEventElementRefs,
    animateScrollRegionTo,
    handleScrollRegionScroll,
    handleScrollJump,
  } = useChatViewportController({
    sessionId,
    messages,
    streamItems,
    streamReasoning,
    askUserQuestion,
    pendingApprovals,
    sessionFocusRequest,
    groupRunStepFocusRequest,
    groupRunSnapshot,
    expandedGroupRunStepIds,
    onExpandGroupRunStep: (stepId) => {
      setExpandedGroupRunStepIds((prev) => (prev.includes(stepId) ? prev : [...prev, stepId]));
    },
  });

  async function handleCancel() {
    try {
      await cancelAgent(sessionId);
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
  const {
    renderedMessages,
    virtualWindow,
    virtualizedRenderedMessages,
    taskPanelModel,
    webSearchEntries,
    touchedFilePaths,
    failedSessionRuns,
    latestMaxTurnsRun,
    failedRunsByAssistantMessageId,
    failedRunsByUserMessageId,
    orphanFailedRuns,
  } = useChatDerivedViewModels({
    messages,
    sessionRuns,
    streamItems,
    highlightedMessageIndex,
    scrollTop,
    viewportHeight,
  });
  const showScrollJump = hasScrollableContent || !isNearBottom;
  const scrollJumpLabel = isNearBottom ? "跳转到顶部" : "跳转到底部";
  const scrollJumpHint = isNearBottom
    ? isNearTop
      ? "当前已在顶部"
      : "返回顶部"
    : "回到底部并继续跟随";
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
  const localStatusSummary = useMemo(() => {
    const lines = [
      "当前会话状态：",
      `- 模型：${currentModel?.name || "未配置"}`,
      `- 工作目录：${(workspace || "").trim() || "未设置"}`,
      `- 会话类型：${normalizedSessionMode || "general"}`,
      `- 来源：${isImSource ? sessionSourceBadgeText : "本地"}`,
      `- 权限模式：${operationPermissionMode}`,
    ];
    if (sessionDisplayTitle.trim()) {
      lines.push(`- 标题：${sessionDisplayTitle.trim()}`);
    }
    if (sessionDisplaySubtitle.trim()) {
      lines.push(`- 副标题：${sessionDisplaySubtitle.trim()}`);
    }
    return lines.join("\n");
  }, [
    currentModel?.name,
    workspace,
    normalizedSessionMode,
    isImSource,
    sessionSourceBadgeText,
    operationPermissionMode,
    sessionDisplayTitle,
    sessionDisplaySubtitle,
  ]);
  const {
    parseDuplicateSkillName,
    renderInstallCandidates,
    setInstallError,
    installDialog,
  } = useChatInstallCandidatesController({
    messages,
    streamItems,
    installedSkillIds,
    onSkillInstalled,
  });
  const handleLocalSendRequest = useLocalChatCommandRunner({
    hasAttachments: attachedFiles.length > 0,
    installedSkillIds,
    onSkillInstalled,
    setInstallError,
    setMessages,
    parseDuplicateSkillName,
    buildStatusSummary: () => localStatusSummary,
  });
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
    handleLocalSendRequest,
    hasPendingAttachmentIntake,
  });
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
  const {
    primaryDelegationCard,
    delegationHistoryCards,
    completedDelegationCount,
    failedDelegationCount,
    groupPhaseFromEvents,
    groupRoundFromEvents,
    groupMemberStatesFromEvents,
    collaborationStatusText,
  } = useMemo(
    () =>
      deriveDelegationState({
        delegationCards,
        mainRoleName,
        mainSummaryDelivered,
      }),
    [delegationCards, mainRoleName, mainSummaryDelivered],
  );
  const toggleGroupRunStepDetails = (stepId: string) => {
    setExpandedGroupRunStepIds((prev) =>
      prev.includes(stepId) ? prev.filter((id) => id !== stepId) : [...prev, stepId],
    );
  };
  const {
    groupPhaseLabelFromSnapshot,
    groupPhaseFromSnapshot,
    groupRoundFromSnapshot,
    groupReviewRound,
    groupRunState,
    groupWaitingLabel,
    groupStatusReason,
    recentGroupEvents,
    groupRunExecuteStepCards,
    groupMemberStatesFromSnapshot,
    failedGroupRunReassignOptions,
    canPauseGroupRun,
    canResumeGroupRun,
    canRetryFailedGroupRunSteps,
    canReassignFailedGroupRunStep,
  } = useMemo(
    () =>
      deriveGroupRunState({
        groupRunSnapshot,
        sessionId,
        groupRunMemberEmployeeIds,
        groupRunCoordinatorEmployeeId,
        groupRunRules,
      }),
    [groupRunCoordinatorEmployeeId, groupRunMemberEmployeeIds, groupRunRules, groupRunSnapshot, sessionId],
  );
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
    if (compactionStatus?.phase === "started" || compactionStatus?.phase === "completed") {
      return "thinking";
    }
    if (streaming || streamItems.length > 0 || subAgentBuffer.trim()) {
      return "running";
    }
    return null;
  }, [
    agentState?.state,
    compactionStatus?.phase,
    pendingApprovals.length,
    streamItems.length,
    streamReasoning?.status,
    streaming,
    subAgentBuffer,
  ]);
  const shouldShowTeamEntryEmptyState =
    isTeamEntrySession &&
    !initialMessage?.trim() &&
    messages.length === 0 &&
    streamItems.length === 0 &&
    !subAgentBuffer.trim() &&
    !streaming &&
    !groupRunSnapshot;
  const groupPhaseLabel = groupPhaseLabelFromSnapshot || groupPhaseFromSnapshot || groupPhaseFromEvents;
  const groupRound = groupRoundFromSnapshot || groupRoundFromEvents;
  const groupMemberStates =
    groupMemberStatesFromSnapshot.length > 0 ? groupMemberStatesFromSnapshot : groupMemberStatesFromEvents;

  useEffect(() => {
    onSessionBlockingStateChange?.({
      blocking: Boolean(liveBlockingStatus),
      status: liveBlockingStatus,
    });
  }, [liveBlockingStatus, onSessionBlockingStateChange]);

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

  function handleViewFilesFromDelivery() {
    setSidePanelOpen(true);
    setSidePanelTab("files");
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

  const agentBanner = buildChatAgentBannerViewModel({
    agentState,
    compactionStatus,
    failedSessionRuns,
  });
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
        <ChatEmployeeAssistantContext employeeAssistantContext={employeeAssistantContext} />
        <ChatAgentStateBanner
          visible={agentBanner.visible}
          state={agentBanner.state}
          label={agentBanner.label}
          indicator={agentBanner.indicator}
          secondary={agentBanner.secondary}
        />
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
          recentGroupEvents={recentGroupEvents}
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
          renderedMessages={virtualizedRenderedMessages}
          visibleStartIndex={virtualWindow.startIndex}
          topSpacerHeight={virtualWindow.topSpacerHeight}
          bottomSpacerHeight={virtualWindow.bottomSpacerHeight}
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
        <ChatLinkToast
          toast={chatLinkToast}
          onRetry={(url) => void handleOpenChatExternalLink(url)}
          onCopy={(url) => void handleCopyChatLink(url)}
          onClose={closeChatLinkToast}
        />
        <ChatActionDialogs
          approvalOpen={Boolean(activePendingApproval)}
          approvalDialog={activePendingApprovalDialog}
          approvalLoading={Boolean(resolvingApprovalId)}
          onAllowOnce={() => void handleResolveApproval("allow_once")}
          onAllowAlways={() => void handleResolveApproval("allow_always")}
          onDeny={() => void handleResolveApproval("deny")}
          installOpen={installDialog.open}
          installSummary={installDialog.summary}
          installImpact={installDialog.impact}
          installLoading={installDialog.loading}
          onConfirmInstall={installDialog.onConfirm}
          onCancelInstall={installDialog.onCancel}
        />
        <div ref={bottomRef} />
        </div>
      </div>
      <ChatScrollJumpButton
        visible={showScrollJump}
        isNearBottom={isNearBottom}
        label={scrollJumpLabel}
        hint={scrollJumpHint}
        onClick={handleScrollJump}
      />
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
          isAddingFiles={isAddingFiles}
          onFilesAdd={addFiles}
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
