import { useState, useEffect, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SkillManifest, ModelConfig, StreamItem, PendingAttachment, ChatMessagePart, SendMessageRequest, EmployeeGroupRunSnapshot, PersistedChatRuntimeState, ChatDelegationCardState } from "../types";
import { ChatWorkspaceSidePanel } from "./chat-side-panel/ChatWorkspaceSidePanel";
import { ChatActionDialogs } from "./chat/ChatActionDialogs";
import { ChatExecutionContextBar } from "./chat/ChatExecutionContextBar";
import { ChatHeader } from "./chat/ChatHeader";
import { ChatComposer } from "./chat/ChatComposer";
import { ChatCollaborationStatusPanel } from "./chat/ChatCollaborationStatusPanel";
import { ChatEmployeeAssistantContext } from "./chat/ChatEmployeeAssistantContext";
import { ChatAgentStateBanner } from "./chat/ChatAgentStateBanner";
import { ChatInstallCandidatesPanel } from "./chat/ChatInstallCandidatesPanel";
import { ChatLinkToast } from "./chat/ChatLinkToast";
import { ChatMessageRail } from "./chat/ChatMessageRail";
import { useChatDerivedViewModels } from "./chat/useChatDerivedViewModels";
import { ChatScrollJumpButton } from "./chat/ChatScrollJumpButton";
import { useChatViewportController } from "./chat/useChatViewportController";
import { ChatGroupRunSection } from "./chat/group-run/ChatGroupRunSection";
import { ChatShell } from "./chat/ChatShell";
import {
  buildApprovalImpactText,
  buildApprovalReasonText,
  ClawhubInstallCandidate,
  CopyActionIcon,
  extractInstallCandidates,
  extractInstallCandidatesWithContent,
  getRunFailureDisplay,
  getThinkingSupport,
  renderAgentStateIndicator,
  renderAgentStateSecondaryText,
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
import {
  buildTaskJourneyViewModel,
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
import {
  buildMessageParts,
  getAttachmentPhaseOneDisplayKind,
  useChatSendController,
} from "../scenes/chat/useChatSendController";
import {
  getModelErrorDisplay,
  inferModelErrorKindFromMessage,
  isModelErrorKind,
} from "../lib/model-error-display";
import { useChatStreamController } from "../scenes/chat/useChatStreamController";
import { useImmersiveTranslation } from "../hooks/useImmersiveTranslation";
import { openExternalUrl } from "../utils/openExternalUrl";

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
    onResetForSessionSwitch: () => {},
  });
  collaborationControllerState.resetForSessionSwitch = resetCollaborationForSessionSwitch;

  loadMessagesRef.current = loadMessages;
  loadSessionRunsRef.current = loadSessionRuns;
  pendingApprovalsSnapshotRef.current = pendingApprovals;
  resolvingApprovalSnapshotRef.current = resolvingApprovalId;

  const renderUserContentParts = (parts: ChatMessagePart[]) => {
    const describeAttachmentCard = (
      part: Exclude<ChatMessagePart, { type: "text" | "image" | "file_text" | "pdf_file" }>,
    ): { label: string; detail?: string } => {
      const attachmentDisplayKind = getAttachmentPhaseOneDisplayKind(part.attachment);
      const warnings = part.attachment.warnings ?? [];
      if (attachmentDisplayKind === "pdf") {
        return {
          label: "PDF 附件",
          detail: part.attachment.truncated ? "已截断" : undefined,
        };
      }
      if (attachmentDisplayKind === "text") {
        return {
          label: "文本附件",
          detail: part.attachment.truncated ? "已截断" : undefined,
        };
      }
      if (part.attachment.kind === "audio") {
        const transcriptPending =
          part.attachment.transcript === "TRANSCRIPTION_REQUIRED" ||
          warnings.includes("transcription_pending");
        return {
          label: "音频附件",
          detail: transcriptPending ? "待转写" : "已转写",
        };
      }
      if (part.attachment.kind === "video") {
        if (
          part.attachment.summary === "VIDEO_NO_AUDIO_TRACK" ||
          warnings.includes("video_no_audio_track")
        ) {
          return {
            label: "视频附件",
            detail: "无音轨",
          };
        }
        if (
          part.attachment.summary === "VIDEO_AUDIO_EXTRACTION_UNAVAILABLE" ||
          warnings.includes("video_audio_extraction_unavailable")
        ) {
          return {
            label: "视频附件",
            detail: "缺少转写环境",
          };
        }
        if (
          part.attachment.summary === "VIDEO_AUDIO_EXTRACTION_FAILED" ||
          warnings.includes("video_audio_extraction_failed")
        ) {
          return {
            label: "视频附件",
            detail: "提取失败",
          };
        }
        const summaryPending =
          part.attachment.summary === "SUMMARY_REQUIRED" ||
          warnings.includes("summary_pending");
        return {
          label: "视频附件",
          detail: summaryPending ? "待摘要" : "已摘要",
        };
      }
      if (part.attachment.kind === "document") {
        const extractionPending =
          part.attachment.summary === "EXTRACTION_REQUIRED" ||
          warnings.includes("document_extraction_pending");
        return {
          label: "文档附件",
          detail: extractionPending ? "待提取" : "已提取",
        };
      }
      return {
        label: "附件暂不支持预览",
      };
    };

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
            {attachmentParts.map((part, index) => {
              if (part.type === "image") {
                return (
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
                );
              }

              if (part.type === "attachment" && getAttachmentPhaseOneDisplayKind(part.attachment) === "image") {
                return (
                  <div
                    key={`attachment-${part.attachment.name}-${index}`}
                    className="rounded-xl border border-white/20 bg-white/10 p-2"
                  >
                    <img
                      src={part.attachment.sourcePayload || ""}
                      alt={part.attachment.name}
                      className="max-h-56 w-full rounded-lg object-cover"
                    />
                    <div className="mt-2 text-xs opacity-90">{part.attachment.name}</div>
                  </div>
                );
              }

              const attachmentName = part.type === "attachment" ? part.attachment.name : part.name;
              const attachmentMeta =
                part.type === "attachment"
                  ? describeAttachmentCard(part)
                  : {
                      label: part.type === "pdf_file" ? "PDF 附件" : "文本附件",
                      detail: part.truncated ? "已截断" : undefined,
                    };

              return (
                <div
                  key={`attachment-${attachmentName}-${index}`}
                  className="rounded-xl border border-white/20 bg-white/10 p-3 text-xs"
                >
                  <div className="font-medium">{attachmentName}</div>
                  <div className="mt-1 opacity-80">
                    {attachmentMeta.label}
                    {attachmentMeta.detail ? ` · ${attachmentMeta.detail}` : ""}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    );
  };

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
  const installedSkillSet = new Set(installedSkillIds);
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

  const candidateTranslationTexts = useMemo(
    () => [
      ...messages.flatMap((m) =>
        extractInstallCandidatesWithContent(m.streamItems, m.content).flatMap((candidate) => [
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
      batchSize: 40,
    },
  );

  function renderInstallCandidates(rawCandidates: unknown[]) {
    return (
      <ChatInstallCandidatesPanel
        candidates={rawCandidates as ClawhubInstallCandidate[]}
        installError={installError}
        installedSkillSet={installedSkillSet}
        installingSlug={installingSlug}
        renderCandidateText={renderCandidateText}
        onInstallRequest={(candidate) => {
          setInstallError(null);
          setPendingInstallSkill(candidate);
          setShowInstallConfirm(true);
        }}
      />
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
        <ChatEmployeeAssistantContext employeeAssistantContext={employeeAssistantContext} />
        <ChatAgentStateBanner
          visible={Boolean(agentState && agentState.state !== "thinking" && shouldShowAgentStateBanner)}
          state={agentState?.state}
          label={getAgentStateLabel()}
          indicator={renderAgentStateIndicator(agentState)}
          secondary={renderAgentStateSecondaryText(agentState)}
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
        <ChatLinkToast
          toast={chatLinkToast}
          onRetry={(url) => void handleOpenChatExternalLink(url)}
          onCopy={(url) => void handleCopyChatLink(url)}
          onClose={() => setChatLinkToast(null)}
        />
        <ChatActionDialogs
          approvalOpen={Boolean(activePendingApproval)}
          approvalDialog={activePendingApprovalDialog}
          approvalLoading={Boolean(resolvingApprovalId)}
          onAllowOnce={() => void handleResolveApproval("allow_once")}
          onAllowAlways={() => void handleResolveApproval("allow_always")}
          onDeny={() => void handleResolveApproval("deny")}
          installOpen={showInstallConfirm && Boolean(pendingInstallSkill)}
          installSummary={
            pendingInstallSkill
              ? `是否安装「${renderCandidateText(pendingInstallSkill.name)}」？`
              : "是否安装该技能？"
          }
          installImpact={pendingInstallSkill ? `slug: ${pendingInstallSkill.slug}` : undefined}
          installLoading={Boolean(installingSlug)}
          onConfirmInstall={handleConfirmInstall}
          onCancelInstall={handleCancelInstallConfirm}
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
