import { Fragment, useState, useEffect, useLayoutEffect, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import { SkillManifest, ModelConfig, Message, StreamItem, PendingAttachment, ChatMessagePart, SendMessageRequest, ImRoleTimelineEvent, ImRoleDispatchRequest, EmployeeGroupRunSnapshot, EmployeeGroup, EmployeeGroupRule, SessionRunProjection, PersistedChatRuntimeState, ChatRuntimeAgentState, ChatDelegationCardState } from "../types";
import { motion } from "framer-motion";
import { ToolIsland } from "./ToolIsland";
import { RiskConfirmDialog } from "./RiskConfirmDialog";
import { useImmersiveTranslation } from "../hooks/useImmersiveTranslation";
import { ChatWorkspaceSidePanel } from "./chat-side-panel/ChatWorkspaceSidePanel";
import { ThinkingBlock } from "./ThinkingBlock";
import {
  buildTaskJourneyViewModel,
  buildTaskPanelViewModel,
  buildWebSearchViewModel,
  extractSessionTouchedFiles,
} from "./chat-side-panel/view-model";
import type { TaskJourneyViewModel } from "./chat-side-panel/view-model";
import { TaskJourneySummary } from "./chat-journey/TaskJourneySummary";
import { getDefaultModel } from "../lib/default-model";
import { getModelErrorDisplay, isModelErrorKind } from "../lib/model-error-display";
import {
  subscribeChatStreamEvent,
  type AssistantReasoningCompletedEvent,
  type AssistantReasoningDeltaEvent,
  type AssistantReasoningInterruptedEvent,
  type AssistantReasoningStartedEvent,
  type StreamTokenEvent,
  type ToolCallEvent,
} from "../lib/chat-stream-events";

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

const SESSION_DRAFT_STORAGE_PREFIX = "workclaw:session-draft:";

function getSessionDraftStorageKey(sessionId: string): string {
  return `${SESSION_DRAFT_STORAGE_PREFIX}${sessionId}`;
}

function readSessionDraft(sessionId: string): string {
  if (typeof window === "undefined" || !sessionId) {
    return "";
  }
  try {
    return window.localStorage.getItem(getSessionDraftStorageKey(sessionId)) ?? "";
  } catch {
    return "";
  }
}

function persistSessionDraft(sessionId: string, value: string) {
  if (typeof window === "undefined" || !sessionId) {
    return;
  }
  try {
    if (value.length > 0) {
      window.localStorage.setItem(getSessionDraftStorageKey(sessionId), value);
      return;
    }
    window.localStorage.removeItem(getSessionDraftStorageKey(sessionId));
  } catch {
    // ignore localStorage failures
  }
}

function clearSessionDraft(sessionId: string) {
  persistSessionDraft(sessionId, "");
}

const CONTINUE_MESSAGE_TEXT = "继续";
const CONTINUE_BUDGET_INCREMENT = 100;
const CHAT_SCROLL_EDGE_THRESHOLD = 48;

interface Props {
  skill: SkillManifest;
  models: ModelConfig[];
  sessionId: string;
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

function clonePersistedChatRuntimeState(state?: PersistedChatRuntimeState | null): PersistedChatRuntimeState {
  return {
    streaming: state?.streaming ?? false,
    streamItems: state?.streamItems ? [...state.streamItems] : [],
    streamReasoning: state?.streamReasoning ? { ...state.streamReasoning } : null,
    agentState: state?.agentState ? { ...state.agentState } : null,
    subAgentBuffer: state?.subAgentBuffer ?? "",
    subAgentRoleName: state?.subAgentRoleName ?? "",
    mainRoleName: state?.mainRoleName ?? "",
    mainSummaryDelivered: state?.mainSummaryDelivered ?? false,
    delegationCards: state?.delegationCards ? state.delegationCards.map((item) => ({ ...item })) : [],
  };
}

interface PendingApprovalView {
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

function shouldRenderCompletedJourneySummary(model: TaskJourneyViewModel) {
  if (model.deliverables.length === 0) return false;
  return model.status === "completed" || model.status === "partial";
}

function extractNodeText(node: unknown): string {
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map((item) => extractNodeText(item)).join("");
  if (node && typeof node === "object" && "props" in node) {
    return extractNodeText((node as { props?: { children?: unknown } }).props?.children);
  }
  return "";
}

function extractPlainTextFromStreamItems(items: StreamItem[]): string {
  return items
    .filter((item) => item.type === "text")
    .map((item) => item.content || "")
    .join("");
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
  const [messages, setMessages] = useState<Message[]>([]);
  const [sessionRuns, setSessionRuns] = useState<SessionRunProjection[]>([]);
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(initialRuntimeState.streaming);
  // 有序的流式输出项：文字和工具调用按时间顺序排列
  const [streamItems, setStreamItems] = useState<StreamItem[]>(initialRuntimeState.streamItems);
  const streamItemsRef = useRef<StreamItem[]>(initialRuntimeState.streamItems);
  const [streamReasoning, setStreamReasoning] = useState<{
    status: "thinking" | "completed" | "interrupted";
    content: string;
    durationMs?: number;
  } | null>(initialRuntimeState.streamReasoning);
  const streamReasoningRef = useRef<{
    status: "thinking" | "completed" | "interrupted";
    content: string;
    durationMs?: number;
  } | null>(initialRuntimeState.streamReasoning);
  const [askUserQuestion, setAskUserQuestion] = useState<string | null>(null);
  const [askUserOptions, setAskUserOptions] = useState<string[]>([]);
  const [askUserAnswer, setAskUserAnswer] = useState("");
  const [agentState, setAgentState] = useState<ChatRuntimeAgentState | null>(initialRuntimeState.agentState);
  const [pendingApprovals, setPendingApprovals] = useState<PendingApprovalView[]>([]);
  const [resolvingApprovalId, setResolvingApprovalId] = useState<string | null>(null);
  const [pendingInstallSkill, setPendingInstallSkill] = useState<ClawhubInstallCandidate | null>(null);
  const [showInstallConfirm, setShowInstallConfirm] = useState(false);
  const [installingSlug, setInstallingSlug] = useState<string | null>(null);
  const [installError, setInstallError] = useState<string | null>(null);
  const [composerError, setComposerError] = useState<string | null>(null);
  const installInFlightRef = useRef(false);
  const [subAgentBuffer, setSubAgentBuffer] = useState(initialRuntimeState.subAgentBuffer);
  const [subAgentRoleName, setSubAgentRoleName] = useState(initialRuntimeState.subAgentRoleName);
  const [mainRoleName, setMainRoleName] = useState(initialRuntimeState.mainRoleName);
  const [mainSummaryDelivered, setMainSummaryDelivered] = useState(initialRuntimeState.mainSummaryDelivered);
  const [highlightedMessageIndex, setHighlightedMessageIndex] = useState<number | null>(null);
  const [highlightedGroupRunStepId, setHighlightedGroupRunStepId] = useState<string | null>(null);
  const [highlightedGroupRunStepEventId, setHighlightedGroupRunStepEventId] = useState<string | null>(null);
  const [showDelegationHistory, setShowDelegationHistory] = useState(false);
  const [isNearTop, setIsNearTop] = useState(true);
  const [isNearBottom, setIsNearBottom] = useState(true);
  const [hasScrollableContent, setHasScrollableContent] = useState(false);
  const [delegationCards, setDelegationCards] = useState<ChatDelegationCardState[]>(initialRuntimeState.delegationCards);
  const bottomRef = useRef<HTMLDivElement>(null);
  const scrollRegionRef = useRef<HTMLDivElement>(null);
  const autoFollowScrollRef = useRef(true);
  const scrollAnimationFrameRef = useRef<number | null>(null);
  const scrollAnimationTargetRef = useRef<"top" | "bottom" | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const pendingApprovalsRef = useRef<PendingApprovalView[]>([]);
  const resolvingApprovalIdRef = useRef<string | null>(null);
  const subAgentBufferRef = useRef("");
  const sessionIdRef = useRef(sessionId);
  const mainRoleNameRef = useRef("");
  const lastHandledSessionFocusNonceRef = useRef<number | null>(null);
  const messageElementRefs = useRef<Record<number, HTMLDivElement | null>>({});
  const lastHandledGroupRunStepFocusNonceRef = useRef<number | null>(null);
  const groupRunStepElementRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const groupRunStepEventElementRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const seededInitialAttachmentsSessionRef = useRef<string | null>(null);

  // File Upload: 附件状态
  const [attachedFiles, setAttachedFiles] = useState<PendingAttachment[]>([]);
  const MAX_FILES = 5;
  const MAX_IMAGE_FILES = 3;
  const MAX_IMAGE_SIZE = 5 * 1024 * 1024;
  const MAX_TEXT_FILE_SIZE = 1 * 1024 * 1024;
  const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "webp"]);
  const TEXT_FILE_EXTENSIONS = new Set([
    "txt",
    "md",
    "json",
    "yaml",
    "yml",
    "xml",
    "csv",
    "tsv",
    "log",
    "ini",
    "conf",
    "env",
    "js",
    "jsx",
    "ts",
    "tsx",
    "py",
    "rs",
    "go",
    "java",
    "c",
    "cpp",
    "h",
    "cs",
    "sh",
    "ps1",
    "sql",
  ]);

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
    pendingApprovalsRef.current = pendingApprovals;
    resolvingApprovalIdRef.current = resolvingApprovalId;
  }, [pendingApprovals, resolvingApprovalId]);

  useEffect(() => {
    sessionIdRef.current = sessionId;
  }, [sessionId]);

  function syncComposerHeight() {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
  }

  // 右侧面板状态
  const [sidePanelOpen, setSidePanelOpen] = useState(false);
  const [sidePanelTab, setSidePanelTab] = useState<"tasks" | "files" | "websearch">("tasks");
  const [imRoleEvents, setImRoleEvents] = useState<ImRoleTimelineEvent[]>([]);
  const [groupRunSnapshot, setGroupRunSnapshot] = useState<EmployeeGroupRunSnapshot | null>(null);
  const [groupRunMemberEmployeeIds, setGroupRunMemberEmployeeIds] = useState<string[]>([]);
  const [groupRunCoordinatorEmployeeId, setGroupRunCoordinatorEmployeeId] = useState("");
  const [groupRunRules, setGroupRunRules] = useState<EmployeeGroupRule[]>([]);
  const [expandedGroupRunStepIds, setExpandedGroupRunStepIds] = useState<string[]>([]);
  const [groupRunActionLoading, setGroupRunActionLoading] = useState<
    "approve" | "reject" | "pause" | "resume" | "retry" | "reassign" | null
  >(null);
  const [expandedThinkingKeys, setExpandedThinkingKeys] = useState<string[]>([]);
  const [copiedAssistantMessageKey, setCopiedAssistantMessageKey] = useState<string | null>(null);

  const toggleThinkingBlock = (key: string) => {
    setExpandedThinkingKeys((prev) => (prev.includes(key) ? prev.filter((item) => item !== key) : [...prev, key]));
  };

  const updateStreamReasoning = (
    updater: (
      prev: {
        status: "thinking" | "completed" | "interrupted";
        content: string;
        durationMs?: number;
      } | null,
    ) => {
      status: "thinking" | "completed" | "interrupted";
      content: string;
      durationMs?: number;
    } | null,
  ) => {
    setStreamReasoning((prev) => {
      const next = updater(prev);
      streamReasoningRef.current = next;
      return next;
    });
  };

  const applyPersistedRuntimeState = (state?: PersistedChatRuntimeState | null) => {
    const next = clonePersistedChatRuntimeState(state);
    setStreaming(next.streaming);
    setStreamItems(next.streamItems);
    streamItemsRef.current = next.streamItems;
    setStreamReasoning(next.streamReasoning);
    streamReasoningRef.current = next.streamReasoning;
    setAgentState(next.agentState);
    setSubAgentBuffer(next.subAgentBuffer);
    subAgentBufferRef.current = next.subAgentBuffer;
    setSubAgentRoleName(next.subAgentRoleName);
    setMainRoleName(next.mainRoleName);
    mainRoleNameRef.current = next.mainRoleName;
    setMainSummaryDelivered(next.mainSummaryDelivered);
    setDelegationCards(next.delegationCards);
  };

  // File Upload: 读取文件为文本
  const readFileAsText = (file: File): Promise<string> => {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as string);
      reader.onerror = reject;
      reader.readAsText(file);
    });
  };

  const readFileAsDataUrl = (file: File): Promise<string> => {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as string);
      reader.onerror = reject;
      reader.readAsDataURL(file);
    });
  };

  const getFileExtension = (fileName: string): string => fileName.split(".").pop()?.toLowerCase() ?? "";

  const isImageFile = (file: File): boolean =>
    file.type.startsWith("image/") || IMAGE_EXTENSIONS.has(getFileExtension(file.name));

  const isTextFile = (file: File): boolean => TEXT_FILE_EXTENSIONS.has(getFileExtension(file.name));

  // File Upload: 处理文件选择
  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(e.target.files || []);
    const currentImageCount = attachedFiles.filter((file) => file.kind === "image").length;

    if (attachedFiles.length + files.length > MAX_FILES) {
      alert(`最多只能上传 ${MAX_FILES} 个文件`);
      e.target.value = "";
      return;
    }

    const newFiles: PendingAttachment[] = [];
    let nextImageCount = currentImageCount;
    for (const file of files) {
      if (isImageFile(file)) {
        if (nextImageCount >= MAX_IMAGE_FILES) {
          alert(`最多只能上传 ${MAX_IMAGE_FILES} 张图片`);
          continue;
        }
        if (file.size > MAX_IMAGE_SIZE) {
          alert(`图片 ${file.name} 超过 5MB 限制`);
          continue;
        }
        const data = await readFileAsDataUrl(file);
        newFiles.push({
          id: crypto.randomUUID(),
          kind: "image",
          name: file.name,
          mimeType: file.type,
          size: file.size,
          data,
          previewUrl: data,
        });
        nextImageCount += 1;
        continue;
      }

      if (!isTextFile(file)) {
        alert(`暂不支持附件类型 ${file.name}`);
        continue;
      }
      if (file.size > MAX_TEXT_FILE_SIZE) {
        alert(`文本文件 ${file.name} 超过 1MB 限制`);
        continue;
      }
      const text = await readFileAsText(file);
      newFiles.push({
        id: crypto.randomUUID(),
        kind: "text-file",
        name: file.name,
        mimeType: file.type || "text/plain",
        size: file.size,
        text,
      });
    }

    setAttachedFiles((prev) => [...prev, ...newFiles]);
    e.target.value = ""; // 重置 input
  };

  // File Upload: 删除附件
  const removeAttachedFile = (index: number) => {
    setAttachedFiles((prev) => prev.filter((_, i) => i !== index));
  };

  const buildDefaultAttachmentPrompt = (attachments: PendingAttachment[]): string => {
    const hasImage = attachments.some((file) => file.kind === "image");
    const hasTextFile = attachments.some((file) => file.kind === "text-file");
    if (hasImage && hasTextFile) {
      return "请结合这些图片和文本附件一起分析，并给出结论。";
    }
    if (hasImage) {
      return "请结合这些图片描述主要内容，并提取可见文字。";
    }
    return "请阅读这些附件并总结关键信息。";
  };

  const buildMessageParts = (message: string, attachments: PendingAttachment[]): ChatMessagePart[] => {
    const normalizedMessage = message.trim() || buildDefaultAttachmentPrompt(attachments);
    const parts: ChatMessagePart[] = [{ type: "text", text: normalizedMessage }];
    attachments.forEach((file) => {
      if (file.kind === "image") {
        parts.push({
          type: "image",
          name: file.name,
          mimeType: file.mimeType,
          size: file.size,
          data: file.data,
        });
        return;
      }
      parts.push({
        type: "file_text",
        name: file.name,
        mimeType: file.mimeType,
        size: file.size,
        text: file.text,
        truncated: file.truncated,
      });
    });
    return parts;
  };

  const buildOptimisticUserContent = (parts: ChatMessagePart[]): string => {
    const textPart = parts.find((part) => part.type === "text");
    const nonTextParts = parts.filter((part) => part.type !== "text");
    if (nonTextParts.length === 0) {
      return textPart?.text ?? "";
    }
    const attachmentSummary = nonTextParts
      .map((part) => (part.type === "image" ? `[图片] ${part.name}` : `[文本文件] ${part.name}`))
      .join("\n");
    return [textPart?.text ?? "", attachmentSummary].filter(Boolean).join("\n\n");
  };

  const toUserFacingSendError = (error: unknown): string => {
    const raw =
      typeof error === "string"
        ? error
        : error instanceof Error
        ? error.message
        : String(error ?? "");
    if (raw.includes("VISION_MODEL_NOT_CONFIGURED")) {
      return "请先在设置中配置图片理解模型";
    }
    return `错误: ${raw}`;
  };

  const shouldPreserveInlineSendError = (error: unknown): boolean => {
    const raw =
      typeof error === "string"
        ? error
        : error instanceof Error
        ? error.message
        : String(error ?? "");
    return raw.includes("VISION_MODEL_NOT_CONFIGURED");
  };

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
                    文本附件
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

  // Secure Workspace: 工作空间状态
  const [workspace, setWorkspace] = useState<string>((workDir || "").trim());

  // Secure Workspace: 加载会话的工作空间
  const loadWorkspace = async (sid: string) => {
    try {
      const sessions = await invoke<any[]>("list_sessions");
      const list = Array.isArray(sessions) ? sessions : [];
      const current = list.find((s: any) => s.id === sid);
      if (current) {
        setWorkspace(current.work_dir || "");
      }
    } catch (e) {
      console.error("加载工作空间失败:", e);
    }
  };

  useEffect(() => {
    setWorkspace((workDir || "").trim());
  }, [sessionId, workDir]);

  // Secure Workspace: 更新会话的工作空间
  const updateWorkspace = async (newWorkspace: string) => {
    try {
      await invoke("update_session_workspace", {
        sessionId,
        workspace: newWorkspace,
      });
      setWorkspace(newWorkspace);
    } catch (e) {
      console.error("更新工作空间失败:", e);
    }
  };

  // sessionId 变化时加载历史消息
  useEffect(() => {
    // 新建会话带首条自动消息时，先发送首条，避免历史加载覆盖本地首句显示
    if (!initialMessage?.trim()) {
      loadMessages(sessionId);
      setInput(readSessionDraft(sessionId));
    } else {
      setMessages([]);
      clearSessionDraft(sessionId);
      setInput("");
    }
    loadSessionRuns(sessionId);
    void loadPendingApprovals(sessionId);
    loadWorkspace(sessionId);
    // 切换会话时重置流式状态
    applyPersistedRuntimeState(persistedRuntimeState);
    setShowDelegationHistory(false);
    setAskUserQuestion(null);
    setAskUserOptions([]);
    setAskUserAnswer("");
    setAgentState(null);
    setPendingApprovals([]);
    setResolvingApprovalId(null);
    setSidePanelTab("tasks");
    setImRoleEvents([]);
    setGroupRunSnapshot(null);
    setGroupRunMemberEmployeeIds([]);
    setGroupRunCoordinatorEmployeeId("");
    setGroupRunRules([]);
    setExpandedGroupRunStepIds([]);
    setHighlightedMessageIndex(null);
    setHighlightedGroupRunStepId(null);
    setHighlightedGroupRunStepEventId(null);
    setSessionRuns([]);
    setExpandedThinkingKeys([]);
    lastHandledGroupRunStepFocusNonceRef.current = null;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId]);

  useEffect(() => {
    onPersistRuntimeState?.({
      streaming,
      streamItems: [...streamItems],
      streamReasoning: streamReasoning ? { ...streamReasoning } : null,
      agentState: agentState ? { ...agentState } : null,
      subAgentBuffer,
      subAgentRoleName,
      mainRoleName,
      mainSummaryDelivered,
      delegationCards: delegationCards.map((item) => ({ ...item })),
    });
  }, [
    agentState,
    delegationCards,
    mainRoleName,
    mainSummaryDelivered,
    onPersistRuntimeState,
    streamItems,
    streamReasoning,
    streaming,
    subAgentBuffer,
    subAgentRoleName,
  ]);

  useEffect(() => {
    persistSessionDraft(sessionId, input);
  }, [input]);

  useEffect(() => {
    if (initialAttachments.length === 0) {
      return;
    }
    if (seededInitialAttachmentsSessionRef.current === sessionId) {
      return;
    }

    seededInitialAttachmentsSessionRef.current = sessionId;
    setAttachedFiles(initialAttachments);

    if (!initialMessage?.trim()) {
      onInitialAttachmentsConsumed?.();
    }
  }, [initialAttachments, initialMessage, onInitialAttachmentsConsumed, sessionId]);

  useEffect(() => {
    syncComposerHeight();
  }, [input, sessionId]);

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
      scrollRegion.scrollTo({ top: clampedTargetTop });
      syncScrollMetrics(scrollRegion);
      if (target !== "bottom") {
        scrollAnimationTargetRef.current = null;
      }
      return;
    }

    const easeOutCubic = (t: number) => 1 - Math.pow(1 - t, 3);
    const initialTop = startTop + distance * 0.22;
    scrollRegion.scrollTo({ top: initialTop });
    syncScrollMetrics(scrollRegion);
    let startTime: number | null = null;

    const step = (timestamp: number) => {
      if (startTime === null) {
        startTime = timestamp;
      }
      const progress = Math.min((timestamp - startTime) / durationMs, 1);
      const nextTop = startTop + distance * easeOutCubic(progress);
      scrollRegion.scrollTo({ top: nextTop });
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

  // stream-token 事件监听
  useLayoutEffect(() => {
    let currentSessionId: string | null = sessionId;
    const unsubscribe = subscribeChatStreamEvent(
      "stream-token",
      (payload: StreamTokenEvent) => {
        if (payload.session_id !== currentSessionId) return;
        if (payload.done) {
          // 流结束：将 streamItems 转为历史消息
          const items = streamItemsRef.current;
          const finalText = items
            .filter((i) => i.type === "text")
            .map((i) => i.content || "")
            .join("");
          const toolCalls = items
            .filter((i) => i.type === "tool_call" && i.toolCall)
            .map((i) => i.toolCall!);
          if (finalText || toolCalls.length > 0) {
            setMessages((prev) => [
              ...prev,
              {
                role: "assistant",
                content: finalText,
                created_at: new Date().toISOString(),
                toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
                reasoning: streamReasoningRef.current
                  ? {
                      status: streamReasoningRef.current.status,
                      duration_ms: streamReasoningRef.current.durationMs,
                      content: streamReasoningRef.current.content,
                    }
                  : undefined,
                streamItems: items.length > 0 ? [...items] : undefined,
              },
            ]);
          }
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
        } else if (payload.sub_agent) {
          // 子 Agent 的 token 单独缓冲
          const delegatedRole = (payload.role_name || payload.role_id || "").trim();
          if (delegatedRole) {
            setSubAgentRoleName(delegatedRole);
          }
          subAgentBufferRef.current += payload.token;
          setSubAgentBuffer(subAgentBufferRef.current);
        } else {
          // 主 Agent 的文字 token → 追加到最后一个 text 项或新建
          const items = [...streamItemsRef.current];
          const last = items[items.length - 1];
          if (last && last.type === "text") {
            last.content = mergeStreamingTextChunk(last.content || "", payload.token);
          } else {
            items.push({ type: "text", content: payload.token });
          }
          streamItemsRef.current = items;
          setStreamItems([...items]);
        }
      },
    );
    return () => {
      currentSessionId = null;
      unsubscribe();
    };
  }, [sessionId]);

  useEffect(() => {
    const unlistenPromise = listen<ImRoleTimelineEvent>("im-role-event", ({ payload }) => {
      if (payload.session_id !== sessionId) return;
      setImRoleEvents((prev) => [...prev, payload]);
      const roleLabel = (payload.role_name || payload.role_id || "").trim();
      if (payload.sender_role === "main_agent" && roleLabel) {
        mainRoleNameRef.current = roleLabel;
        setMainRoleName(roleLabel);
      }
      if (payload.sender_role === "main_agent") {
        if (payload.status === "completed") {
          setMainSummaryDelivered(true);
        } else if (payload.status === "running") {
          setMainSummaryDelivered(false);
        }
      }
      if (
        payload.sender_role === "sub_agent" &&
        roleLabel &&
        (payload.status === "completed" || payload.status === "failed")
      ) {
        setDelegationCards((prev) => {
          const next = [...prev];
          let matchedIndex = -1;
          for (let i = next.length - 1; i >= 0; i -= 1) {
            const item = next[i];
            const byTaskId = payload.task_id && item.taskId === payload.task_id;
            const byRole = item.toRole === roleLabel;
            if (item.status === "running" && (byTaskId || byRole)) {
              matchedIndex = i;
              break;
            }
          }
          if (matchedIndex >= 0) {
            next[matchedIndex] = {
              ...next[matchedIndex],
              status: payload.status === "failed" ? "failed" : "completed",
            };
          }
          return next;
        });
      }
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [sessionId]);

  useEffect(() => {
    const unlistenPromise = listen<ImRoleDispatchRequest>("im-role-dispatch-request", ({ payload }) => {
      if (payload.session_id !== sessionId) return;
      const cleanPrompt = (payload.prompt || "")
        .replace(/@_[A-Za-z0-9_]+/g, " ")
        .replace(/\s+/g, " ")
        .trim();
      const roleLabel = payload.role_name || payload.role_id;
      setMessages((prev) => [
        ...prev,
        {
          role: "user",
          content: `【${roleLabel}】${cleanPrompt || payload.prompt || ""}`,
          created_at: new Date().toISOString(),
        },
      ]);
      setImRoleEvents((prev) => [
        ...prev,
        {
          session_id: payload.session_id,
          thread_id: payload.thread_id,
          role_id: payload.role_id,
          role_name: roleLabel,
          sender_role: payload.sender_role ?? "main_agent",
          sender_employee_id: payload.sender_employee_id ?? payload.role_id,
          target_employee_id: payload.target_employee_id ?? payload.role_id,
          task_id: payload.task_id,
          parent_task_id: payload.parent_task_id,
          message_type: payload.message_type ?? "delegate_request",
          source_channel: payload.source_channel ?? "app",
          status: "running",
          summary: `任务已分发(${payload.agent_type}) -> ${roleLabel}`,
        },
      ]);
      const delegationId = (payload.task_id || "").trim() || `${payload.thread_id}-${Date.now()}`;
      setMainSummaryDelivered(false);
      setDelegationCards((prev) => {
        const next = prev.filter((item) => item.id !== delegationId);
        next.push({
          id: delegationId,
          fromRole: mainRoleNameRef.current || mainRoleName || "主员工",
          toRole: roleLabel,
          status: "running",
          taskId: payload.task_id,
        });
        return next.slice(-8);
      });
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [mainRoleName, sessionId]);

  useEffect(() => {
    let disposed = false;
    const loadSnapshot = async () => {
      try {
        const snapshot = await invoke<EmployeeGroupRunSnapshot | null>("get_employee_group_run_snapshot", { sessionId });
        if (!disposed && snapshot) {
          setGroupRunSnapshot(snapshot);
        }
      } catch {
        if (!disposed) {
          setGroupRunSnapshot(null);
        }
      }
    };
    void loadSnapshot();
    const timer = setInterval(() => {
      void loadSnapshot();
    }, 3000);
    return () => {
      disposed = true;
      clearInterval(timer);
    };
  }, [sessionId]);

  useEffect(() => {
    let disposed = false;
    const groupId = (groupRunSnapshot?.group_id || "").trim();
    if (!groupId) {
      setGroupRunMemberEmployeeIds([]);
      setGroupRunCoordinatorEmployeeId("");
      setGroupRunRules([]);
      return () => {
        disposed = true;
      };
    }
    const loadGroupMembers = async () => {
      try {
        const [groups, rules] = await Promise.all([
          invoke<EmployeeGroup[] | null>("list_employee_groups"),
          invoke<EmployeeGroupRule[] | null>("list_employee_group_rules", { groupId }),
        ]);
        if (disposed) return;
        const matchedGroup = Array.isArray(groups)
          ? groups.find((group) => (group.id || "").trim() === groupId)
          : null;
        const memberIds = (matchedGroup?.member_employee_ids || [])
          .map((value) => (value || "").trim())
          .filter((value) => value.length > 0);
        setGroupRunMemberEmployeeIds(memberIds);
        setGroupRunCoordinatorEmployeeId((matchedGroup?.coordinator_employee_id || "").trim());
        setGroupRunRules(Array.isArray(rules) ? rules : []);
      } catch {
        if (!disposed) {
          setGroupRunMemberEmployeeIds([]);
          setGroupRunCoordinatorEmployeeId("");
          setGroupRunRules([]);
        }
      }
    };
    void loadGroupMembers();
    return () => {
      disposed = true;
    };
  }, [groupRunSnapshot?.group_id]);

  // ask-user-event 事件监听
  useEffect(() => {
    const unlistenPromise = listen<{
      session_id: string;
      question: string;
      options: string[];
    }>("ask-user-event", ({ payload }) => {
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

  // agent-state-event 事件监听
  useEffect(() => {
    const unlistenPromise = listen<{
      session_id: string;
      state: string;
      detail: string | null;
      iteration: number;
      stop_reason_kind?: string | null;
      stop_reason_title?: string | null;
      stop_reason_message?: string | null;
      stop_reason_last_completed_step?: string | null;
    }>("agent-state-event", ({ payload }) => {
      if (payload.session_id !== sessionId) return;
      if (payload.state === "finished") {
        setAgentState(null);
      } else {
        setAgentState({
          state: payload.state,
          detail: payload.detail ?? undefined,
          iteration: payload.iteration,
          stopReasonKind: payload.stop_reason_kind ?? undefined,
          stopReasonTitle: payload.stop_reason_title ?? undefined,
          stopReasonMessage: payload.stop_reason_message ?? undefined,
          stopReasonLastCompletedStep: payload.stop_reason_last_completed_step ?? undefined,
        });
      }
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [sessionId]);

  // approval-created / approval-resolved 事件监听（权限审批）
  useEffect(() => {
    const unlistenCreatedPromise = listen<{
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
    }>("approval-created", ({ payload }) => {
      if (payload.session_id !== sessionId) return;
      upsertPendingApproval(buildPendingApproval(payload));
    });

    const unlistenResolvedPromise = listen<{
      approval_id?: string;
      session_id?: string;
      status?: string;
    }>("approval-resolved", ({ payload }) => {
      if ((payload.session_id || "").trim() && payload.session_id !== sessionId) return;
      const approvalId = (payload.approval_id || "").trim();
      if (!approvalId) return;
      removePendingApproval(approvalId);
    });

    const unlistenLegacyPromise = listen<{
      approval_id?: string;
      session_id: string;
      tool_name: string;
      tool_input: Record<string, unknown>;
      title?: string;
      summary?: string;
      impact?: string;
      irreversible?: boolean;
    }>("tool-confirm-event", ({ payload }) => {
      if (payload.session_id !== sessionId) return;
      if ((payload.approval_id || "").trim()) return;
      upsertPendingApproval(buildPendingApproval(payload));
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
      const staleApprovals = pendingApprovalsRef.current.filter(
        (item) => item.sessionId === cleanupSessionId && item.approvalId.trim() && item.approvalId !== resolvingId,
      );

      for (const approval of staleApprovals) {
        if (approval.approvalRecordId) {
          void invoke("resolve_approval", {
            approvalId: approval.approvalRecordId,
            decision: "deny",
            source: "desktop_cleanup",
          }).catch((error) => {
            console.error("自动拒绝待审批操作失败:", error);
          });
          continue;
        }
        if (approval.usesLegacyConfirm) {
          void invoke("confirm_tool_execution", {
            confirmed: false,
          }).catch((error) => {
            console.error("自动拒绝旧版待审批操作失败:", error);
          });
        }
      }
    };
  }, [sessionId]);

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
              }
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

  // tool-call-event 事件监听：按顺序插入到 streamItems
  useEffect(() => {
    const unsubscribe = subscribeChatStreamEvent("tool-call-event", (payload: ToolCallEvent) => {
      if (payload.session_id !== sessionId) return;
      if (payload.status === "started") {
        // 新的工具调用 → 直接追加到 streamItems（文字和工具按时间排列）
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
      } else {
        // 工具完成/出错 → 更新对应项
        const items = streamItemsRef.current.map((item) => {
          if (
            item.type === "tool_call" &&
            item.toolCall?.name === payload.tool_name &&
            item.toolCall?.status === "running"
          ) {
            return {
              ...item,
              toolCall: {
                ...item.toolCall,
                output: payload.tool_output ?? undefined,
                status: (payload.status === "completed"
                  ? "completed"
                  : "error") as "completed" | "error",
              },
            };
          }
          return item;
        });
        streamItemsRef.current = items;
        setStreamItems([...items]);
      }
    });
    return () => {
      unsubscribe();
    };
  }, [sessionId]);

  async function loadMessages(sid: string) {
    try {
      const list = await invoke<Message[]>("get_messages", { sessionId: sid });
      setMessages(list);
    } catch (e) {
      console.error("加载历史消息失败:", e);
      setMessages([]);
    }
  }

  async function loadSessionRuns(sid: string) {
    if (!sid) {
      setSessionRuns([]);
      return;
    }
    try {
      const runs = await invoke<SessionRunProjection[]>("list_session_runs", {
        sessionId: sid,
      });
      setSessionRuns(Array.isArray(runs) ? runs : []);
    } catch (e) {
      console.error("加载会话运行记录失败:", e);
      setSessionRuns([]);
    }
  }

  async function loadPendingApprovals(sid: string) {
    if (!sid) {
      setPendingApprovals([]);
      return;
    }
    try {
      const approvals = await invoke<
        Array<{
          approval_id: string;
          session_id: string;
          tool_name: string;
          input?: Record<string, unknown>;
          summary: string;
          impact?: string | null;
          irreversible?: boolean;
          status?: string;
        }>
      >("list_pending_approvals", {
        sessionId: sid,
      });
      setPendingApprovals(
        Array.isArray(approvals)
          ? approvals.map((item) =>
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
              })
            )
          : []
      );
    } catch (error) {
      console.error("加载待审批列表失败:", error);
      setPendingApprovals([]);
    }
  }

  async function handleSend() {
    if (!input.trim() && attachedFiles.length === 0) return;
    if (streaming || !sessionId) return;

    const parts = buildMessageParts(input, attachedFiles);
    await sendContent({
      sessionId,
      parts,
    });
  }

  async function sendContent(request: SendMessageRequest | string) {
    if (streaming || !sessionId) return;

    const normalizedRequest: SendMessageRequest =
      typeof request === "string"
        ? {
            sessionId,
            parts: [{ type: "text", text: request.trim() }],
          }
        : request;
    const continuationRequest =
      shouldGrantContinuationBudget(normalizedRequest) &&
      normalizedRequest.maxIterations === undefined
        ? {
            ...normalizedRequest,
            maxIterations: CONTINUE_BUDGET_INCREMENT,
          }
        : normalizedRequest;
    const optimisticContent = buildOptimisticUserContent(continuationRequest.parts);
    if (!continuationRequest.parts.length || !optimisticContent.trim()) return;

    setInput("");
    setAttachedFiles([]); // 发送后清空附件
    setComposerError(null);
    autoFollowScrollRef.current = true;
    setIsNearBottom(true);
    setIsNearTop(false);
    animateScrollRegionTo(
      (scrollRegionRef.current?.scrollHeight ?? 0) - (scrollRegionRef.current?.clientHeight ?? 0),
      1000,
      "bottom",
    );
    setMessages((prev) => [
      ...prev,
      {
        role: "user",
        content: optimisticContent,
        contentParts: continuationRequest.parts,
        created_at: new Date().toISOString(),
      },
    ]);
    setStreaming(true);
    streamItemsRef.current = [];
    setStreamItems([]);
    setStreamReasoning(null);
    streamReasoningRef.current = null;
    subAgentBufferRef.current = "";
    setSubAgentBuffer("");
    setSubAgentRoleName("");
    try {
      await invoke("send_message", { request: continuationRequest });
      onSessionUpdate?.();
    } catch (e) {
      const preserveInlineError = shouldPreserveInlineSendError(e);
      const userFacingError = toUserFacingSendError(e);
      setComposerError(userFacingError);
      if (preserveInlineError) {
        return;
      }
      setMessages((prev) => [
        ...prev,
        {
          role: "assistant",
          content: userFacingError,
          created_at: new Date().toISOString(),
        },
      ]);
      await Promise.all([loadMessages(sessionId), loadSessionRuns(sessionId)]);
    } finally {
      setStreaming(false);
    }
  }

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
      await invoke("cancel_agent");
    } catch (e) {
      console.error("取消任务失败:", e);
    }
    // 即时清除状态，不等待后端返回
    setStreaming(false);
    setAgentState(null);
    updateStreamReasoning((prev) => (prev ? { ...prev, status: "interrupted" } : prev));
    // 将所有 running 状态的工具标记为 error，避免永远转圈
    const items = streamItemsRef.current.map((item) => {
      if (
        item.type === "tool_call" &&
        item.toolCall?.status === "running"
      ) {
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
  }

  async function handleAnswerUser(answer: string) {
    if (!answer.trim()) return;
    try {
      await invoke("answer_user_question", { answer: answer.trim() });
    } catch (e) {
      console.error("回答用户问题失败:", e);
    }
    setAskUserQuestion(null);
    setAskUserOptions([]);
    setAskUserAnswer("");
  }

  async function handleResolveApproval(decision: "allow_once" | "allow_always" | "deny") {
    const activeApproval = pendingApprovals[0];
    if (!activeApproval || resolvingApprovalId) return;
    try {
      setResolvingApprovalId(activeApproval.approvalId);
      await invoke("resolve_approval", {
        approvalId: activeApproval.approvalId,
        decision,
        source: "desktop",
      });
      removePendingApproval(activeApproval.approvalId);
    } catch (e) {
      console.error("工具确认失败:", e);
      setResolvingApprovalId(null);
    }
  }

  async function handleApproveGroupRunReview() {
    if (!groupRunSnapshot?.run_id || groupRunActionLoading) return;
    setGroupRunActionLoading("approve");
    try {
      await invoke("review_group_run_step", {
        runId: groupRunSnapshot.run_id,
        action: "approve",
        comment: "前端确认通过",
      });
      const snapshot = await invoke<EmployeeGroupRunSnapshot>("continue_employee_group_run", {
        runId: groupRunSnapshot.run_id,
      });
      setGroupRunSnapshot(snapshot);
    } catch (e) {
      console.error("审核通过失败:", e);
    } finally {
      setGroupRunActionLoading(null);
    }
  }

  async function refreshGroupRunSnapshot(targetSessionId?: string) {
    const snapshotSessionId = (targetSessionId || groupRunSnapshot?.session_id || sessionId || "").trim();
    if (!snapshotSessionId) return;
    const snapshot = await invoke<EmployeeGroupRunSnapshot | null>("get_employee_group_run_snapshot", {
      sessionId: snapshotSessionId,
    });
    if (snapshot) {
      setGroupRunSnapshot(snapshot);
    }
  }

  async function handleRejectGroupRunReview() {
    if (!groupRunSnapshot?.run_id || groupRunActionLoading) return;
    setGroupRunActionLoading("reject");
    try {
      await invoke("review_group_run_step", {
        runId: groupRunSnapshot.run_id,
        action: "reject",
        comment: "前端要求补充方案",
      });
      const snapshot = await invoke<EmployeeGroupRunSnapshot>("continue_employee_group_run", {
        runId: groupRunSnapshot.run_id,
      });
      setGroupRunSnapshot(snapshot);
    } catch (e) {
      console.error("审核打回失败:", e);
    } finally {
      setGroupRunActionLoading(null);
    }
  }

  async function handlePauseGroupRun() {
    if (!groupRunSnapshot?.run_id || groupRunActionLoading) return;
    setGroupRunActionLoading("pause");
    try {
      await invoke("pause_employee_group_run", {
        runId: groupRunSnapshot.run_id,
        reason: "前端人工暂停",
      });
      await refreshGroupRunSnapshot(groupRunSnapshot.session_id);
    } catch (e) {
      console.error("暂停协作失败:", e);
    } finally {
      setGroupRunActionLoading(null);
    }
  }

  async function handleResumeGroupRun() {
    if (!groupRunSnapshot?.run_id || groupRunActionLoading) return;
    setGroupRunActionLoading("resume");
    try {
      await invoke("resume_employee_group_run", {
        runId: groupRunSnapshot.run_id,
      });
      const snapshot = await invoke<EmployeeGroupRunSnapshot>("continue_employee_group_run", {
        runId: groupRunSnapshot.run_id,
      });
      setGroupRunSnapshot(snapshot);
    } catch (e) {
      console.error("继续协作失败:", e);
    } finally {
      setGroupRunActionLoading(null);
    }
  }

  async function handleRetryFailedGroupRunSteps() {
    if (!groupRunSnapshot?.run_id || groupRunActionLoading) return;
    setGroupRunActionLoading("retry");
    try {
      await invoke("retry_employee_group_run_failed_steps", {
        runId: groupRunSnapshot.run_id,
      });
      await refreshGroupRunSnapshot(groupRunSnapshot.session_id);
    } catch (e) {
      console.error("重试失败步骤失败:", e);
    } finally {
      setGroupRunActionLoading(null);
    }
  }

  async function handleReassignFailedGroupRunStep(stepId: string, assigneeEmployeeId: string) {
    if (!groupRunSnapshot?.run_id || groupRunActionLoading) return;
    setGroupRunActionLoading("reassign");
    try {
      await invoke("reassign_group_run_step", {
        stepId,
        assigneeEmployeeId,
      });
      const snapshot = await invoke<EmployeeGroupRunSnapshot>("continue_employee_group_run", {
        runId: groupRunSnapshot.run_id,
      });
      setGroupRunSnapshot(snapshot);
    } catch (e) {
      console.error("改派失败步骤失败:", e);
    } finally {
      setGroupRunActionLoading(null);
    }
  }

  // 从 models 查找当前会话的模型名称
  const currentModel = getDefaultModel(models);
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
  const formatGroupRunStepStatusLabel = (status?: string) => {
    const normalized = (status || "").trim().toLowerCase();
    if (normalized === "completed" || normalized === "done") return "已完成";
    if (normalized === "failed") return "失败";
    if (normalized === "running" || normalized === "executing") return "执行中";
    if (normalized === "pending") return "待执行";
    if (normalized === "paused") return "已暂停";
    if (normalized === "cancelled") return "已取消";
    return status?.trim() || "待执行";
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
        sourceStepTimeline,
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
  const streamText = useMemo(() => extractPlainTextFromStreamItems(streamItems), [streamItems]);
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

  function renderInstallCandidates(candidates: ClawhubInstallCandidate[]) {
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

  async function handleCopyAssistantMessage(messageKey: string, content: string) {
    const trimmed = content.trim();
    if (!trimmed) return;
    await globalThis.navigator?.clipboard?.writeText?.(trimmed);
    setCopiedAssistantMessageKey(messageKey);
    window.setTimeout(() => {
      setCopiedAssistantMessageKey((current) => (current === messageKey ? null : current));
    }, 1600);
  }

  // Markdown 渲染组件配置
  const markdownComponents = {
    // 代码块
    code({ className, children, ...props }: any) {
      const match = /language-(\w+)/.exec(className || "");
      const codeString = String(children).replace(/\n$/, "");
      return match ? (
        <SyntaxHighlighter
          style={oneDark}
          language={match[1]}
          PreTag="div"
          customStyle={{ margin: 0, borderRadius: "0.375rem", fontSize: "0.8125rem" }}
        >
          {codeString}
        </SyntaxHighlighter>
      ) : (
        <code className={"bg-gray-200/60 px-1.5 py-0.5 rounded text-sm text-gray-800 font-mono " + (className || "")} {...props}>
          {children}
        </code>
      );
    },
    // 标题
    h1: ({ children }: any) => (
      <h1 className="mt-7 mb-4 border-b border-slate-200 pb-3 text-[1.75rem] font-semibold tracking-[-0.02em] text-slate-950">
        {children}
      </h1>
    ),
    h2: ({ children }: any) => (
      <h2 className="mt-6 mb-3 border-b border-slate-150 pb-2 text-[1.35rem] font-semibold tracking-[-0.015em] text-slate-900">
        {children}
      </h2>
    ),
    h3: ({ children }: any) => <h3 className="mt-5 mb-2.5 text-[1.1rem] font-semibold text-slate-900">{children}</h3>,
    h4: ({ children }: any) => <h4 className="mt-4 mb-2 text-base font-semibold text-slate-800">{children}</h4>,
    h5: ({ children }: any) => <h5 className="mt-3 mb-1.5 text-sm font-semibold uppercase tracking-[0.01em] text-slate-700">{children}</h5>,
    h6: ({ children }: any) => <h6 className="mt-3 mb-1 text-sm font-medium text-slate-600">{children}</h6>,
    // 段落
    p: ({ children }: any) => {
      const text = extractNodeText(children).trim();
      const isSummaryBlock = /^(共计|总计|总结[:：]?|结论[:：]?)/.test(text);
      return isSummaryBlock ? (
        <p
          data-testid="assistant-result-summary"
          className="mb-4 rounded-2xl border border-slate-200/90 bg-slate-50/80 px-4 py-3 text-[15px] font-medium leading-7 text-slate-800"
        >
          {children}
        </p>
      ) : (
        <p className="mb-4 text-[15px] leading-7 text-slate-700">{children}</p>
      );
    },
    // 列表
    ul: ({ children }: any) => <ul className="mb-4 list-disc space-y-1.5 pl-5 text-[15px] text-slate-700">{children}</ul>,
    ol: ({ children }: any) => <ol className="mb-4 list-decimal space-y-1.5 pl-5 text-[15px] text-slate-700">{children}</ol>,
    li: ({ children }: any) => <li className="leading-7 text-slate-700">{children}</li>,
    // 链接
    a: ({ href, children }: any) => (
      <a
        href={href}
        className="text-blue-500 hover:text-blue-600 underline underline-offset-2 text-sm"
        target="_blank"
        rel="noopener noreferrer"
      >
        {children}
      </a>
    ),
    // 引用块
    blockquote: ({ children }: any) => (
      <blockquote className="my-4 rounded-r-lg border-l-[3px] border-slate-300 bg-slate-50/70 py-1 pl-4">
        <div className="text-[15px] italic text-slate-600">{children}</div>
      </blockquote>
    ),
    // 表格
    table: ({ children }: any) => (
      <div className="my-4 overflow-x-auto rounded-2xl border border-slate-200/90 bg-white/90 shadow-[0_1px_2px_rgba(15,23,42,0.04)]">
        <table className="min-w-full text-sm">{children}</table>
      </div>
    ),
    thead: ({ children }: any) => <thead className="bg-slate-50/90">{children}</thead>,
    tbody: ({ children }: any) => <tbody className="divide-y divide-slate-200/80 bg-white">{children}</tbody>,
    tr: ({ children }: any) => <tr className="transition-colors hover:bg-slate-50/70">{children}</tr>,
    th: ({ children }: any) => (
      <th className="bg-slate-50/90 px-4 py-3 text-left text-xs font-semibold uppercase tracking-[0.08em] text-slate-500">
        {children}
      </th>
    ),
    td: ({ children }: any) => {
      const text = extractNodeText(children).trim();
      const isNumericLike = /^(?:[\d,]+(?:\.\d+)?(?:\s*(?:字节|KB|MB|GB|%))?|[\d]{4}\/[\d]{1,2}\/[\d]{1,2}.*)$/.test(text);
      return (
        <td className={"px-4 py-3 text-[15px] text-slate-700 " + (isNumericLike ? "whitespace-nowrap tabular-nums" : "")}>
          {children}
        </td>
      );
    },
    // 水平线
    hr: () => <hr className="my-7 border-slate-200" />,
    // 强调
    strong: ({ children }: any) => <strong className="font-semibold text-slate-950">{children}</strong>,
    em: ({ children }: any) => <em className="italic text-slate-700">{children}</em>,
    input: ({ type, checked, disabled }: any) => {
      if (type === "checkbox" && disabled) {
        return (
          <span
            aria-hidden="true"
            className={
              "mr-2 inline-flex h-4 w-4 translate-y-[1px] items-center justify-center rounded border " +
              (checked
                ? "border-emerald-200 bg-emerald-50 text-emerald-600"
                : "border-slate-300 bg-white text-transparent")
            }
          >
            {checked ? (
              <svg className="h-3 w-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
              </svg>
            ) : (
              <span className="h-3 w-3" />
            )}
          </span>
        );
      }
      return <input type={type} checked={checked} disabled={disabled} readOnly />;
    },
  };

  /** 渲染有序的 StreamItem 列表（将连续的工具调用合并到一个 ToolIsland） */
  function renderStreamItems(items: StreamItem[], isStreaming: boolean) {
    const groups: { type: "text" | "tools"; items: StreamItem[] }[] = [];
    for (const item of items) {
      if (item.type === "tool_call") {
        const last = groups[groups.length - 1];
        if (last && last.type === "tools") {
          last.items.push(item);
        } else {
          groups.push({ type: "tools", items: [item] });
        }
      } else {
        groups.push({ type: "text", items: [item] });
      }
    }

    return groups.map((g, i) => {
      if (g.type === "tools") {
        const toolCalls = g.items
          .filter((it) => it.toolCall)
          .map((it) => it.toolCall!);
        const hasRunning = toolCalls.some((tc) => tc.status === "running");
        return (
          <ToolIsland
            key={`island-${i}`}
            toolCalls={toolCalls}
            isRunning={hasRunning}
            subAgentBuffer={hasRunning ? subAgentBuffer : undefined}
          />
        );
      }
      const text = g.items.map((it) => it.content || "").join("");
      if (!text) return null;
      return (
        <div key={`txt-${i}`}>
          <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>{text}</ReactMarkdown>
        </div>
      );
    });
  }

  function getAgentStateLabel() {
    if (!agentState) return "";
    if (agentState.state === "thinking") return "正在分析任务";
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
      const rawMessage =
        run.error_message &&
        run.error_message !== display.title &&
        run.error_message !== display.message
          ? run.error_message
          : null;
      return {
        title: display.title,
        message: display.message,
        rawMessage,
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

  function renderRunFailureCard(run: SessionRunProjection) {
    const display = getRunFailureDisplay(run);
    const showContinueAction = run.error_kind === "max_turns";

    return (
      <motion.div
        key={`run-failure-${run.id}`}
        data-testid={`run-failure-card-${run.id}`}
        initial={{ opacity: 0, y: 8 }}
        animate={{ opacity: 1, y: 0 }}
        className="mr-auto max-w-[80%] rounded-2xl border border-amber-200 bg-amber-50 px-5 py-4 text-sm text-amber-900 shadow-sm"
      >
        <div className="text-xs font-medium tracking-wide text-amber-700">本轮执行结果</div>
        <div className="mt-1 text-lg font-semibold">{display.title}</div>
        {display.message ? (
          <div className="mt-2 whitespace-pre-wrap text-sm text-amber-800">{display.message}</div>
        ) : null}
        {display.rawMessage ? (
          <div className="mt-2 whitespace-pre-wrap break-all rounded-xl border border-white/70 bg-white/70 px-4 py-3 font-mono text-xs text-amber-900/90">
            {display.rawMessage}
          </div>
        ) : null}
        {run.buffered_text && (
          <div className="mt-3 rounded-xl border border-white/70 bg-white/70 px-4 py-3 text-sm text-gray-700">
            <div className="mb-1 text-xs font-medium tracking-wide text-gray-500">已保留的部分输出</div>
            <div className="whitespace-pre-wrap">{run.buffered_text}</div>
          </div>
        )}
        {showContinueAction ? (
          <div className="mt-3">
            <button
              type="button"
              onClick={() =>
                void sendContent({
                  sessionId,
                  parts: [{ type: "text", text: CONTINUE_MESSAGE_TEXT }],
                  maxIterations: CONTINUE_BUDGET_INCREMENT,
                })
              }
              disabled={streaming}
              className="inline-flex items-center rounded-lg bg-amber-600 px-3 py-2 text-sm font-medium text-white transition-colors hover:bg-amber-700 disabled:cursor-not-allowed disabled:bg-amber-300"
            >
              继续执行
            </button>
          </div>
        ) : null}
      </motion.div>
    );
  }
  return (
    <div className="flex flex-col h-full">
      {/* 头部 */}
      <div className="px-4 pt-4 sm:px-6 xl:px-8">
        <div className="flex w-full items-start justify-between gap-4">
          <div className="min-w-0">
            <div
              data-testid="chat-session-display-title"
              className="truncate text-[22px] font-semibold tracking-tight text-[var(--sm-text)]"
            >
              {sessionDisplayTitle}
            </div>
            {(sessionDisplaySubtitle || isImSource) && (
              <div className="mt-1 flex flex-wrap items-center gap-2 text-[11px] text-[var(--sm-text-muted)]">
                {sessionDisplaySubtitle ? (
                  <div
                    data-testid="chat-session-display-subtitle"
                    className="truncate"
                  >
                    {sessionDisplaySubtitle}
                  </div>
                ) : null}
                {isImSource && (
                  <span
                    data-testid="chat-session-source-badge"
                    title={`该会话由${sessionSourceBadgeText}触发`}
                    className="inline-flex items-center rounded-full border border-[var(--sm-border)] bg-[var(--sm-surface)] px-2 py-0.5 font-medium text-[var(--sm-text-muted)]"
                  >
                    {sessionSourceBadgeText}
                  </span>
                )}
              </div>
            )}
          </div>
          <button
            type="button"
            aria-label="面板"
            title="面板"
            data-testid="chat-side-panel-trigger"
            aria-pressed={sidePanelOpen}
            onClick={() => setSidePanelOpen(!sidePanelOpen)}
            className={
              "sm-btn ml-auto h-10 w-10 rounded-xl border transition-colors " +
              (sidePanelOpen
                ? "border-[var(--sm-primary-soft)] bg-[var(--sm-primary-soft)] text-[var(--sm-primary-strong)]"
                : "border-[var(--sm-border)] bg-[var(--sm-surface)] text-[var(--sm-text-muted)] hover:bg-[var(--sm-surface-soft)] hover:text-[var(--sm-text)]")
            }
          >
            <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M9 17V7m0 10a2 2 0 01-2 2H5a2 2 0 01-2-2V7a2 2 0 012-2h2a2 2 0 012 2m0 10a2 2 0 002 2h2a2 2 0 002-2M9 7a2 2 0 012-2h2a2 2 0 012 2m0 10V7m0 10a2 2 0 002 2h2a2 2 0 002-2V7a2 2 0 00-2-2h-2a2 2 0 00-2 2" />
            </svg>
          </button>
        </div>
      </div>
      {sessionExecutionContext && (
        <div
          data-testid="chat-session-execution-context-bar"
          className="border-b border-sky-100 bg-sky-50/80 text-[11px] text-sky-900"
        >
          <div className="mx-auto flex w-full max-w-[76rem] flex-wrap items-center justify-between gap-2 px-5 py-2 lg:px-8">
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <div className="flex flex-wrap items-center gap-3">
              <span>{`来源 step：${sessionExecutionContext.sourceStepId}`}</span>
              {sessionExecutionContext.sourceEmployeeId && (
                <span>{`来源员工：${sessionExecutionContext.sourceEmployeeId}`}</span>
              )}
              {sessionExecutionContext.assigneeEmployeeId && (
                <span>{`当前负责人：${sessionExecutionContext.assigneeEmployeeId}`}</span>
              )}
            </div>
            {(sessionExecutionContext.sourceStepTimeline || []).length > 0 && (
              <div
                data-testid="chat-session-execution-context-timeline"
                className="space-y-1 text-[10px] text-sky-800/90"
              >
                {(sessionExecutionContext.sourceStepTimeline || []).map((item, index) => {
                  const label = item.createdAt ? `${item.label} · ${item.createdAt}` : item.label;
                  return onOpenSession ? (
                    <button
                      key={`${item.label}-${item.createdAt || index}`}
                      type="button"
                      data-testid={`chat-session-execution-context-timeline-item-${index}`}
                      onClick={() =>
                        void onOpenSession(sessionExecutionContext.sourceSessionId, {
                          groupRunStepFocusId: sessionExecutionContext.sourceStepId,
                          groupRunEventFocusId: item.eventId,
                        })
                      }
                      className="block text-left underline underline-offset-2 hover:text-sky-900"
                    >
                      {label}
                    </button>
                  ) : (
                    <div
                      key={`${item.label}-${item.createdAt || index}`}
                      data-testid={`chat-session-execution-context-timeline-item-${index}`}
                    >
                      {label}
                    </div>
                  );
                })}
              </div>
            )}
          </div>
          <button
            type="button"
            data-testid="chat-session-execution-context-back"
            onClick={() => void onReturnToSourceSession?.(sessionExecutionContext.sourceSessionId)}
            className="text-[11px] font-medium text-sky-700 underline underline-offset-2 hover:text-sky-800"
          >
            返回协作看板
          </button>
          </div>
        </div>
      )}

      {/* 主内容区：消息列表 + 右侧面板 */}
      <div className="flex-1 flex overflow-hidden bg-[#f7f7f4]">
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
        {agentState && agentState.state !== "thinking" && (
          <div
            className={`sticky top-0 z-10 flex items-center gap-2 bg-white/80 backdrop-blur-lg px-4 py-2 rounded-xl text-xs border shadow-sm mx-4 mt-2 ${
              agentState.state === "stopped"
                ? "text-amber-800 border-amber-200"
                : agentState.state === "error"
                ? "text-red-700 border-red-200"
                : "text-gray-600 border-gray-200"
            }`}
          >
            {renderAgentStateIndicator()}
            <div className="flex min-w-0 flex-col">
              <span className={agentState.state === "error" ? "text-red-500" : undefined}>{getAgentStateLabel()}</span>
              {renderAgentStateSecondaryText()}
            </div>
          </div>
        )}
        {(mainRoleName || primaryDelegationCard) && (
          <div
            data-testid="team-collab-status-bar"
            className="sticky top-0 z-10 max-w-[80%] rounded-xl border border-sky-200 bg-sky-50 px-4 py-2 text-xs text-sky-800"
          >
            <div className="flex items-center gap-2">
              <span className="inline-flex h-5 w-5 items-center justify-center rounded-full bg-sky-500 text-[10px] font-semibold text-white">
                主
              </span>
              <span>{collaborationStatusText}</span>
            </div>
            {(completedDelegationCount > 0 || failedDelegationCount > 0) && (
              <div className="mt-1 text-[11px] text-sky-700/90">
                {completedDelegationCount > 0 && <span>已完成 {completedDelegationCount} 次协作</span>}
                {completedDelegationCount > 0 && failedDelegationCount > 0 && <span> · </span>}
                {failedDelegationCount > 0 && <span>待处理失败 {failedDelegationCount} 次</span>}
              </div>
            )}
          </div>
        )}
        {(groupPhaseLabel || groupMemberStates.length > 0 || groupRunSnapshot) && (
          <div
            data-testid="group-orchestration-board"
            className="sticky top-0 z-10 max-w-[80%] rounded-xl border border-indigo-200 bg-indigo-50 px-4 py-2 text-xs text-indigo-900"
          >
            <div className="font-medium">{`阶段：${groupPhaseLabel || "计划"}`}</div>
            <div className="mt-1">{`轮次：第 ${groupRound || 1} 轮`}</div>
            {groupReviewRound > 0 && <div className="mt-1">{`审议轮次：${groupReviewRound}`}</div>}
            {groupWaitingLabel && <div className="mt-1">{`等待：${groupWaitingLabel}`}</div>}
            {groupStatusReason && <div className="mt-1 text-amber-700">{groupStatusReason}</div>}
            {groupRunSnapshot && (groupRunSnapshot.state || "").trim().toLowerCase() === "waiting_review" && (
              <div className="mt-2 flex items-center gap-2">
                <button
                  type="button"
                  data-testid="group-run-review-reject"
                  onClick={() => void handleRejectGroupRunReview()}
                  disabled={groupRunActionLoading !== null}
                  className="rounded bg-rose-600 px-2.5 py-1 text-[11px] text-white hover:bg-rose-700 disabled:bg-rose-300"
                >
                  {groupRunActionLoading === "reject" ? "打回中..." : "打回重审"}
                </button>
                <button
                  type="button"
                  data-testid="group-run-review-approve"
                  onClick={() => void handleApproveGroupRunReview()}
                  disabled={groupRunActionLoading !== null}
                  className="rounded bg-emerald-600 px-2.5 py-1 text-[11px] text-white hover:bg-emerald-700 disabled:bg-emerald-300"
                >
                  {groupRunActionLoading === "approve" ? "通过中..." : "通过审议"}
                </button>
              </div>
            )}
            {groupRunSnapshot && (
              <div className="mt-2 flex flex-wrap items-center gap-2">
                {canPauseGroupRun && (
                  <button
                    type="button"
                    data-testid="group-run-pause"
                    onClick={() => void handlePauseGroupRun()}
                    disabled={groupRunActionLoading !== null}
                    className="rounded bg-slate-600 px-2.5 py-1 text-[11px] text-white hover:bg-slate-700 disabled:bg-slate-300"
                  >
                    {groupRunActionLoading === "pause" ? "暂停中..." : "暂停协作"}
                  </button>
                )}
                {canResumeGroupRun && (
                  <button
                    type="button"
                    data-testid="group-run-resume"
                    onClick={() => void handleResumeGroupRun()}
                    disabled={groupRunActionLoading !== null}
                    className="rounded bg-sky-600 px-2.5 py-1 text-[11px] text-white hover:bg-sky-700 disabled:bg-sky-300"
                  >
                    {groupRunActionLoading === "resume" ? "继续中..." : "继续协作"}
                  </button>
                )}
                {canRetryFailedGroupRunSteps && (
                  <button
                    type="button"
                    data-testid="group-run-retry-failed"
                    onClick={() => void handleRetryFailedGroupRunSteps()}
                    disabled={groupRunActionLoading !== null}
                    className="rounded bg-amber-600 px-2.5 py-1 text-[11px] text-white hover:bg-amber-700 disabled:bg-amber-300"
                  >
                    {groupRunActionLoading === "retry" ? "重试中..." : "重试失败步骤"}
                  </button>
                )}
                {canReassignFailedGroupRunStep && (
                  <div className="w-full space-y-1.5">
                    {failedGroupRunReassignOptions.map(({ step, candidateEmployeeIds }) => (
                      <div
                        key={step.id}
                        data-testid={`group-run-reassign-row-${step.id}`}
                        className="rounded border border-indigo-200 bg-white/70 px-2.5 py-2"
                      >
                        <div className="text-[11px] font-medium text-indigo-800">
                          {`失败步骤：${step.assignee_employee_id || step.id}`}
                        </div>
                        {(step.dispatch_source_employee_id || "").trim().length > 0 && (
                          <div className="mt-1 text-[10px] text-indigo-700/80">
                            {`来源：${step.dispatch_source_employee_id}`}
                          </div>
                        )}
                        {(step.output || "").trim().length > 0 && (
                          <div className="mt-1 text-[10px] text-indigo-700/80">{step.output}</div>
                        )}
                        <div className="mt-1.5 flex flex-wrap gap-2">
                          {candidateEmployeeIds.map((employeeId) => (
                            <button
                              key={`${step.id}-${employeeId}`}
                              type="button"
                              data-testid={`group-run-reassign-${step.id}-${employeeId}`}
                              onClick={() => void handleReassignFailedGroupRunStep(step.id, employeeId)}
                              disabled={groupRunActionLoading !== null}
                              className="rounded bg-fuchsia-600 px-2.5 py-1 text-[11px] text-white hover:bg-fuchsia-700 disabled:bg-fuchsia-300"
                            >
                              {groupRunActionLoading === "reassign" ? "改派中..." : `改派给${employeeId}`}
                            </button>
                          ))}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}
            {groupMemberStates.length > 0 && (
              <div className="mt-2 space-y-1">
                {groupMemberStates.map((member) => (
                  <div key={member.role} className="text-[11px] text-indigo-800">
                    {member.role}
                    {member.stepType ? ` · ${member.stepType}` : ""}
                    {` · ${member.status}`}
                  </div>
                ))}
              </div>
            )}
            {groupRunExecuteStepCards.length > 0 && (
              <div className="mt-2 border-t border-indigo-100 pt-2">
                <div className="text-[11px] font-medium text-indigo-800">步骤链路</div>
                <div className="mt-1 space-y-1.5">
                  {groupRunExecuteStepCards.map(
                    ({
                      step,
                      currentAssigneeEmployeeId,
                      dispatchSourceEmployeeId,
                      previousAssigneeEmployeeId,
                      latestFailureSummary,
                      attemptNo,
                      detailSessionId,
                      detailOutputSummary,
                      latestEventCreatedAt,
                      sourceStepTimeline,
                    }) => {
                      const isGroupRunStepFocusTarget = highlightedGroupRunStepId === step.id;
                      return (
                      <div
                        key={step.id}
                        ref={(node) => {
                          groupRunStepElementRefs.current[step.id] = node;
                        }}
                        data-testid={`group-run-step-card-${step.id}`}
                        data-group-run-step-highlighted={isGroupRunStepFocusTarget ? "true" : "false"}
                        className={
                          "rounded border border-indigo-200 bg-white/70 px-2.5 py-2 transition-all " +
                          (isGroupRunStepFocusTarget ? "ring-2 ring-amber-300 bg-amber-50/80 " : "")
                        }
                      >
                        <div className="text-[11px] font-medium text-indigo-800">
                          {step.assignee_employee_id || step.id}
                        </div>
                        <div className="mt-1 text-[10px] text-indigo-700/80">
                          {`当前负责人：${currentAssigneeEmployeeId || "未分配"}`}
                        </div>
                        <div className="mt-1 text-[10px] text-indigo-700/80">
                          {`当前状态：${formatGroupRunStepStatusLabel(step.status)}`}
                        </div>
                        <div className="mt-1 text-[10px] text-indigo-700/80">
                          {`尝试次数：${attemptNo}`}
                        </div>
                        {dispatchSourceEmployeeId && (
                          <div className="mt-1 text-[10px] text-indigo-700/80">
                            {`来源人：${dispatchSourceEmployeeId}`}
                          </div>
                        )}
                        {previousAssigneeEmployeeId &&
                          previousAssigneeEmployeeId.toLowerCase() !==
                            currentAssigneeEmployeeId.toLowerCase() && (
                            <div className="mt-1 text-[10px] text-indigo-700/80">
                              {`原负责人：${previousAssigneeEmployeeId}`}
                            </div>
                          )}
                        {latestFailureSummary && (
                          <div className="mt-1 text-[10px] text-amber-700/90">
                            {`最近失败：${latestFailureSummary}`}
                          </div>
                        )}
                        <button
                          type="button"
                          data-testid={`group-run-step-card-${step.id}-toggle`}
                          onClick={() => toggleGroupRunStepDetails(step.id)}
                          className="mt-2 text-[10px] text-indigo-700 underline underline-offset-2 hover:text-indigo-800"
                        >
                          {expandedGroupRunStepIds.includes(step.id) ? "收起详情" : "查看详情"}
                        </button>
                        {expandedGroupRunStepIds.includes(step.id) && (
                          <div
                            data-testid={`group-run-step-card-${step.id}-details`}
                            className="mt-2 space-y-1 rounded border border-indigo-100 bg-indigo-50/60 px-2 py-1.5 text-[10px] text-indigo-800"
                          >
                            <div>{`session_id：${detailSessionId || "暂无"}`}</div>
                            <div>{`输出摘要：${detailOutputSummary || "暂无"}`}</div>
                            <div>{`最近事件时间：${latestEventCreatedAt || "暂无"}`}</div>
                            {sourceStepTimeline.length > 0 && (
                              <div className="space-y-1">
                                <div className="font-medium text-indigo-800">步骤事件</div>
                                {sourceStepTimeline.map((item, index) => {
                                  const eventId = (item.eventId || "").trim();
                                  const linkedSessionId = (item.linkedSessionId || "").trim();
                                  const isGroupRunEventFocusTarget =
                                    eventId.length > 0 && highlightedGroupRunStepEventId === eventId;
                                  const eventLabel = item.createdAt ? `${item.label} · ${item.createdAt}` : item.label;
                                  const eventKey = `${eventId || item.label}-${item.createdAt || index}`;
                                  const commonProps = {
                                    ref: (node: HTMLDivElement | HTMLButtonElement | null) => {
                                      if (eventId) {
                                        groupRunStepEventElementRefs.current[eventId] = node as HTMLDivElement | null;
                                      }
                                    },
                                    "data-testid": `group-run-step-card-${step.id}-event-${eventId || index}`,
                                    "data-group-run-step-event-linkable":
                                      linkedSessionId && onOpenSession ? "true" : "false",
                                    "data-group-run-step-event-highlighted": isGroupRunEventFocusTarget ? "true" : "false",
                                    className:
                                      "rounded px-1.5 py-1 transition-all flex items-center justify-between gap-2 " +
                                      (isGroupRunEventFocusTarget ? "bg-amber-100 ring-1 ring-amber-300 " : "") +
                                      (linkedSessionId && onOpenSession
                                        ? " w-full text-left border border-sky-200 bg-white text-sky-900 underline underline-offset-2 hover:bg-sky-50"
                                        : " border border-indigo-100 bg-white/60 text-indigo-700/90"),
                                  } as const;
                                  return linkedSessionId && onOpenSession ? (
                                    <button
                                      key={eventKey}
                                      {...commonProps}
                                      type="button"
                                      onClick={() =>
                                        void onOpenSession(linkedSessionId, {
                                          focusHint: detailOutputSummary || item.label || undefined,
                                          sourceSessionId: sessionId,
                                          sourceStepId: step.id,
                                          sourceEmployeeId: dispatchSourceEmployeeId || undefined,
                                          assigneeEmployeeId: currentAssigneeEmployeeId || undefined,
                                          sourceStepTimeline:
                                            sourceStepTimeline.length > 0 ? sourceStepTimeline : undefined,
                                        })
                                      }
                                    >
                                      <span className="min-w-0 flex-1 truncate">{eventLabel}</span>
                                      <span className="shrink-0 rounded bg-sky-100 px-1.5 py-0.5 text-[9px] font-medium text-sky-700">
                                        执行会话
                                      </span>
                                    </button>
                                  ) : (
                                    <div key={eventKey} {...commonProps}>
                                      <span className="min-w-0 flex-1 truncate">{eventLabel}</span>
                                      <span className="shrink-0 rounded bg-indigo-100 px-1.5 py-0.5 text-[9px] font-medium text-indigo-700">
                                        日志
                                      </span>
                                    </div>
                                  );
                                })}
                              </div>
                            )}
                            {onOpenSession && detailSessionId && (
                              <button
                                type="button"
                            data-testid={`group-run-step-card-${step.id}-open-session`}
                            onClick={() =>
                              void onOpenSession(detailSessionId, {
                                focusHint: detailOutputSummary || undefined,
                                sourceSessionId: sessionId,
                                sourceStepId: step.id,
                                sourceEmployeeId: dispatchSourceEmployeeId || undefined,
                                assigneeEmployeeId: currentAssigneeEmployeeId || undefined,
                                sourceStepTimeline:
                                  sourceStepTimeline.length > 0 ? sourceStepTimeline : undefined,
                              })
                            }
                            className="text-[10px] text-indigo-700 underline underline-offset-2 hover:text-indigo-800"
                          >
                                查看执行会话
                              </button>
                            )}
                          </div>
                        )}
                      </div>
                    );
                    },
                  )}
                </div>
              </div>
            )}
            {recentGroupEvents.length > 0 && (
              <div className="mt-2 border-t border-indigo-100 pt-2">
                <div className="text-[11px] font-medium text-indigo-800">最近事件</div>
                <div className="mt-1 space-y-1">
                  {recentGroupEvents.map((event) => (
                    <div key={event.id} className="text-[11px] text-indigo-800">
                      {formatGroupRunEventLabel(event)}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
        {shouldShowTeamEntryEmptyState && (
          <div
            data-testid="team-entry-empty-state"
            className="max-w-[80%] rounded-2xl border border-sky-200 bg-sky-50 px-5 py-4 text-sm text-sky-950 shadow-sm"
          >
            <div className="text-sm font-semibold">团队已就绪</div>
            <div className="mt-1 text-xs text-sky-800">
              {sessionDisplaySubtitle || "当前团队"} 已进入协作模式，等待你下达第一条任务。
            </div>
            <div className="mt-3 rounded-xl border border-sky-100 bg-white/80 px-3 py-2 text-[11px] text-sky-900">
              适合提交需要拆分、审核、执行和汇总的复杂任务。直接在下方输入目标即可开始团队协作。
            </div>
          </div>
        )}
        {primaryDelegationCard && (
          <div className="space-y-2">
            <div
              data-testid={`delegation-card-${primaryDelegationCard.id}`}
              className="max-w-[80%] rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-2 text-xs text-emerald-800"
            >
              <div className="font-medium">{`${primaryDelegationCard.fromRole} 已将任务分配给 ${primaryDelegationCard.toRole}`}</div>
              <div className="mt-1">
                {primaryDelegationCard.status === "running" && "执行中"}
                {primaryDelegationCard.status === "completed" && "已完成"}
                {primaryDelegationCard.status === "failed" && "失败"}
              </div>
            </div>
            {delegationHistoryCards.length > 0 && (
              <>
                <button
                  data-testid="delegation-history-toggle"
                  onClick={() => setShowDelegationHistory((prev) => !prev)}
                  className="text-[11px] text-emerald-700 hover:text-emerald-800 underline underline-offset-2"
                >
                  历史协作（{delegationHistoryCards.length}）
                </button>
                {showDelegationHistory && (
                  <div data-testid="delegation-history-panel" className="space-y-2">
                    {delegationHistoryCards.map((card) => (
                      <div
                        key={card.id}
                        data-testid={`delegation-card-${card.id}`}
                        className="max-w-[80%] rounded-lg border border-gray-200 bg-white px-3 py-2 text-[11px] text-gray-700"
                      >
                        <div>{`${card.fromRole} -> ${card.toRole}`}</div>
                        <div className="mt-0.5 text-gray-500">
                          {card.status === "running" && "执行中"}
                          {card.status === "completed" && "已完成"}
                          {card.status === "failed" && "失败"}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </>
            )}
          </div>
        )}
        {renderedMessages.map((m, i) => {
          const isLatest = i === renderedMessages.length - 1;
          const isSessionFocusTarget = highlightedMessageIndex === i;
          const messageJourneyModel = m.role === "assistant" ? buildTaskJourneyViewModel([m]) : null;
          const shouldRenderJourneySummary =
            messageJourneyModel !== null && shouldRenderCompletedJourneySummary(messageJourneyModel);
          const messageSummaryKey = (m.runId || m.id || `message-${i}`).trim();
          const inlineFailedRuns =
            m.role === "assistant" && (m.id || "").trim()
              ? failedRunsByAssistantMessageId.get((m.id || "").trim()) ?? []
              : m.role === "user" && (m.id || "").trim()
              ? failedRunsByUserMessageId.get((m.id || "").trim()) ?? []
              : [];
          return (
            <Fragment key={m.id || `${i}-${m.created_at}`}>
              <motion.div
                ref={(node) => {
                  messageElementRefs.current[i] = node;
                }}
                data-testid={`chat-message-${i}`}
                data-recovered-run-message={m.id?.startsWith("recovered-run-") ? "true" : "false"}
                data-session-focus-highlighted={isSessionFocusTarget ? "true" : "false"}
                initial={isLatest ? { opacity: 0, x: m.role === "user" ? 20 : -20 } : false}
                animate={{ opacity: 1, x: 0 }}
                transition={{ type: "spring", stiffness: 300, damping: 24 }}
                className={"flex " + (m.role === "user" ? "justify-end" : "justify-start")}
              >
                <div
                  data-testid={`chat-message-bubble-${m.id || i}`}
                  className={
                    "text-sm transition-all " +
                    (isSessionFocusTarget ? "ring-2 ring-amber-300 " : "") +
                    (m.role === "user"
                      ? "max-w-[28rem] rounded-[1.4rem] bg-slate-100 px-4 py-2.5 text-slate-800 shadow-[0_1px_2px_rgba(15,23,42,0.04)] md:max-w-[32rem]"
                      : "w-full max-w-[92%] px-0 py-1 text-slate-800 sm:max-w-[88%] md:max-w-[48rem] xl:max-w-[52rem]")
                  }
                >
                  {m.role === "assistant" && m.reasoning && (
                    <ThinkingBlock
                      status={m.reasoning.status}
                      content={m.reasoning.content}
                      durationMs={m.reasoning.duration_ms}
                      expanded={expandedThinkingKeys.includes(`message-${m.id || i}`)}
                      onToggle={
                        m.reasoning.content.trim()
                          ? () => toggleThinkingBlock(`message-${m.id || i}`)
                          : undefined
                      }
                      toggleTestId={`thinking-block-toggle-${m.id || i}`}
                    />
                  )}
                  {m.role === "assistant" && m.streamItems ? (
                    <>
                      {renderStreamItems(m.streamItems, false)}
                      {renderInstallCandidates(extractInstallCandidates(m.streamItems, m.content))}
                    </>
                  ) : m.role === "assistant" && m.toolCalls ? (
                    <>
                      <ToolIsland toolCalls={m.toolCalls} isRunning={false} />
                      <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>{m.content}</ReactMarkdown>
                    </>
                  ) : m.role === "assistant" ? (
                    <>
                      <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>{m.content}</ReactMarkdown>
                    </>
                  ) : m.role === "user" && m.contentParts?.length ? (
                    renderUserContentParts(m.contentParts)
                  ) : (
                    m.content
                  )}
                  {m.role === "assistant" && m.content.trim() && (
                    <div className="mt-3 flex items-center justify-end gap-2">
                      <button
                        type="button"
                        data-testid={`assistant-copy-action-${m.id || i}`}
                        aria-label="复制回答"
                        title={copiedAssistantMessageKey === `message-${m.id || i}` ? "已复制" : "复制回答"}
                        onClick={() => void handleCopyAssistantMessage(`message-${m.id || i}`, m.content)}
                        className="inline-flex h-9 w-9 items-center justify-center rounded-full text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 focus-visible:ring-offset-2"
                      >
                        <CopyActionIcon copied={copiedAssistantMessageKey === `message-${m.id || i}`} />
                      </button>
                    </div>
                  )}
                </div>
              </motion.div>
              {shouldRenderJourneySummary && messageJourneyModel && (
                <div data-testid={`task-journey-summary-${messageSummaryKey}`}>
                  <TaskJourneySummary
                    model={messageJourneyModel}
                    onViewFiles={handleViewFilesFromDelivery}
                  />
                </div>
              )}
              {inlineFailedRuns.map((run) => renderRunFailureCard(run))}
            </Fragment>
          );
        })}
        {orphanFailedRuns.map((run) => renderRunFailureCard(run))}
        {/* 流式输出区域：按时间顺序渲染 */}
        {showStreamingAssistantBubble && (
          <motion.div
            initial={{ opacity: 0, x: -20 }}
            animate={{ opacity: 1, x: 0 }}
            className="flex justify-start"
          >
            <div
              data-testid="chat-streaming-bubble"
              className="w-full max-w-[92%] px-0 py-1 text-sm text-slate-800 sm:max-w-[88%] md:max-w-[48rem] xl:max-w-[52rem]"
            >
              {showStreamingThinkingState && (
                <ThinkingBlock
                  status={streamReasoning?.status || "thinking"}
                  content={streamReasoning?.content || ""}
                  durationMs={streamReasoning?.durationMs}
                  expanded={expandedThinkingKeys.includes("stream")}
                  onToggle={
                    (streamReasoning?.content || "").trim()
                      ? () => toggleThinkingBlock("stream")
                      : undefined
                  }
                />
              )}
              {streamItems.length > 0 && renderStreamItems(streamItems, true)}
              {subAgentBuffer && (
                <div
                  data-testid="sub-agent-stream-buffer"
                  className="mt-2 rounded-xl border border-slate-200/80 bg-slate-50/80 px-3 py-2"
                >
                  <div className="mb-1 text-[11px] font-semibold text-slate-600">
                    {subAgentRoleName ? `子员工 · ${subAgentRoleName}` : "子员工"}
                  </div>
                  <div className="prose prose-xs max-w-none text-slate-700">
                    <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>{subAgentBuffer}</ReactMarkdown>
                    <span className="animate-pulse text-slate-400">|</span>
                  </div>
                </div>
              )}
              {streamItems.length > 0 && (
                <>
                  {/* 光标闪烁效果 */}
                  <span className="inline-block w-0.5 h-4 bg-blue-400 ml-0.5 align-middle animate-[blink_1s_infinite]" />
                </>
              )}
              {streamText.trim() && (
                <div className="mt-3 flex items-center justify-end gap-2">
                  <button
                    type="button"
                    data-testid="assistant-copy-action-stream"
                    aria-label="复制回答"
                    title={copiedAssistantMessageKey === "stream" ? "已复制" : "复制回答"}
                    onClick={() => void handleCopyAssistantMessage("stream", streamText)}
                    className="inline-flex h-9 w-9 items-center justify-center rounded-full text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 focus-visible:ring-offset-2"
                  >
                    <CopyActionIcon copied={copiedAssistantMessageKey === "stream"} />
                  </button>
                </div>
              )}
            </div>
          </motion.div>
        )}
        {/* AskUser 问答卡片 */}
        {askUserQuestion && (
          <div className="sticky top-0 z-20 flex justify-start">
            <div
              data-testid="ask-user-action-card"
              className="max-w-[80%] bg-amber-50 border border-amber-300 rounded-2xl px-4 py-3 text-sm shadow-sm"
            >
              <div className="font-semibold text-amber-800 mb-1">需要你的确认</div>
              <div className="font-medium text-amber-700 mb-2">{askUserQuestion}</div>
              {askUserOptions.length > 0 && (
                <div className="flex flex-wrap gap-2 mb-2">
                  {askUserOptions.map((opt, i) => (
                    <button
                      key={i}
                      onClick={() => handleAnswerUser(opt)}
                      className="bg-amber-100 hover:bg-amber-200 text-amber-800 px-3 py-1 rounded text-xs transition-colors border border-amber-300"
                    >
                      {opt}
                    </button>
                  ))}
                </div>
              )}
              <div className="flex gap-2">
                <input
                  value={askUserAnswer}
                  onChange={(e) => setAskUserAnswer(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      handleAnswerUser(askUserAnswer);
                    }
                  }}
                  placeholder="输入回答..."
                  className="flex-1 bg-white border border-gray-200 rounded px-2 py-1 text-xs focus:outline-none focus:border-amber-500"
                />
                <button
                  onClick={() => handleAnswerUser(askUserAnswer)}
                  disabled={!askUserAnswer.trim()}
                  className="bg-amber-500 hover:bg-amber-600 disabled:bg-gray-200 disabled:text-gray-400 px-3 py-1 rounded text-xs transition-colors"
                >
                  回答
                </button>
              </div>
            </div>
          </div>
        )}
        <RiskConfirmDialog
          open={Boolean(activePendingApproval)}
          level="high"
          title={activePendingApproval?.title || "高危操作确认"}
          summary={activePendingApproval?.summary || "请确认是否继续执行。"}
          impact={activePendingApproval?.impact}
          note={queuedApprovalCount > 0 ? `还有 ${queuedApprovalCount} 条待审批` : undefined}
          irreversible={activePendingApproval?.irreversible}
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

      {/* 右侧面板 */}
      <ChatWorkspaceSidePanel
        open={sidePanelOpen}
        tab={sidePanelTab}
        onTabChange={setSidePanelTab}
        onClose={() => setSidePanelOpen(false)}
        workspace={workspace}
        touchedFiles={touchedFilePaths}
        active={sidePanelOpen}
        taskModel={taskPanelModel}
        webSearchEntries={webSearchEntries}
      />
      </div>

      {/* 输入区域 */}
      <div className="border-t border-slate-200/80 bg-[#f4f4f1]/92 px-4 py-3 sm:px-6 xl:px-8">
        <div className="mx-auto w-full max-w-[76rem]">
        <div
          data-testid="chat-composer-shell"
          className="mx-auto max-w-3xl rounded-[26px] border border-[var(--sm-border)] bg-white px-4 pt-4 pb-3 shadow-[0_8px_24px_-20px_rgba(59,130,246,0.35)] transition-all focus-within:border-[var(--sm-primary)] focus-within:shadow-[var(--sm-focus-ring)]"
        >
          {operationPermissionMode === "full_access" && (
            <div className="pb-3">
              <div
                data-testid="full-access-badge"
                className="inline-flex items-center rounded-full border border-red-200 bg-red-50 px-2.5 py-1 text-[11px] font-medium text-red-700"
              >
                全自动模式
              </div>
            </div>
          )}
          {quickPrompts.length > 0 && (
            <div data-testid="chat-quick-prompts" className="flex flex-wrap gap-2 border-b border-gray-100 pb-2">
              {quickPrompts.map((item, index) => (
                <button
                  key={`${item.label}-${index}`}
                  data-testid={`chat-quick-prompt-${index}`}
                  type="button"
                  disabled={streaming}
                  title={item.prompt}
                  onClick={() => void sendContent(item.prompt)}
                  className="h-7 px-2.5 rounded border border-blue-200 hover:bg-blue-50 disabled:bg-gray-100 disabled:text-gray-400 text-blue-700 text-[11px]"
                >
                  {item.label}
                </button>
              ))}
            </div>
          )}
          {/* 隐藏的文件输入 */}
          <input
            type="file"
            multiple
            onChange={handleFileSelect}
            className="hidden"
            id="file-upload"
          />

          {attachedFiles.length > 0 && (
            <div className="pb-2 space-y-2">
              <div className="text-[11px] text-gray-500">
                已添加 {attachedFiles.length} 个附件
              </div>
              <div className="space-y-2">
                {attachedFiles.map((file) => (
                  <div
                    key={file.id}
                    className="flex items-center gap-3 rounded-lg border border-gray-200 bg-white px-3 py-2"
                  >
                    {file.kind === "image" ? (
                      <img
                        src={file.previewUrl}
                        alt={file.name}
                        className="h-10 w-10 rounded object-cover border border-gray-200"
                      />
                    ) : (
                      <div className="flex h-10 w-10 items-center justify-center rounded border border-gray-200 bg-gray-50 text-[11px] text-gray-600">
                        TXT
                      </div>
                    )}
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm text-gray-800">{file.name}</div>
                      <div className="flex items-center gap-2 text-[11px] text-gray-500">
                        <span>{file.kind === "image" ? "图片" : "文本"}</span>
                        <span>{Math.ceil(file.size / 1024)} KB</span>
                        {file.kind === "text-file" && file.truncated && <span>已截断</span>}
                      </div>
                    </div>
                    <button
                      type="button"
                      aria-label="移除附件"
                      onClick={() => removeAttachedFile(attachedFiles.findIndex((item) => item.id === file.id))}
                      className="rounded-md px-2 py-1 text-xs text-gray-500 hover:bg-gray-100"
                    >
                      删除
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}

          {composerError && (
            <div className="pb-2">
              <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
                {composerError}
              </div>
            </div>
          )}

          {/* 输入框主体 */}
          <textarea
            ref={textareaRef}
            value={input}
            onChange={(e) => {
              if (composerError) setComposerError(null);
              setInput(e.target.value);
              // auto-expand
              const el = e.target;
              el.style.height = "auto";
              el.style.height = Math.min(el.scrollHeight, 200) + "px";
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleSend();
              }
            }}
            placeholder="输入消息，Shift+Enter 换行..."
            rows={3}
            className="sm-textarea w-full min-h-[88px] max-h-[200px] border-0 bg-transparent px-0 py-0 focus:border-0 focus:shadow-none"
          />
          {/* 底部工具栏 */}
          <div className="mt-4 flex flex-wrap items-center justify-between gap-3 border-t border-gray-100 pt-3">
            <div className="flex flex-wrap items-center gap-2">
              <button
                type="button"
                data-testid="chat-composer-workdir-button"
                onClick={() => {
                  invoke<string | null>("select_directory", {
                    defaultPath: workspace || undefined,
                  }).then((newDir) => {
                    if (newDir) {
                      updateWorkspace(newDir);
                    }
                  });
                }}
                className="inline-flex items-center gap-1.5 rounded-lg bg-gray-100 px-2.5 py-1 text-xs text-gray-600 transition-colors hover:bg-gray-200"
                title={displayWorkDirLabel}
              >
                <svg className="h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-6l-2-2H5a2 2 0 0 0-2 2Z" />
                </svg>
                <span data-testid="chat-composer-workdir-label" className="max-w-[180px] truncate">
                  {displayWorkDirLabel}
                </span>
              </button>
              {currentModel && (
                <span
                  data-testid="chat-composer-model-chip"
                  className="inline-flex items-center rounded-lg bg-gray-100 px-2.5 py-1 text-xs text-gray-600"
                >
                  {currentModel.name}
                </span>
              )}
              <label
                htmlFor="file-upload"
                className="sm-btn sm-btn-secondary h-8 gap-1.5 rounded-lg px-3 text-xs cursor-pointer"
              >
                <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13" />
                </svg>
                附件
              </label>
            </div>
            <div className="flex items-center gap-2">
              {streaming ? (
                <button
                  onClick={handleCancel}
                  className="sm-btn sm-btn-danger h-8 px-3 gap-1.5 rounded-lg text-xs"
                >
                  <svg className="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24">
                    <rect x="6" y="6" width="12" height="12" rx="2" />
                  </svg>
                  停止
                </button>
              ) : (
                <button
                  onClick={handleSend}
                  disabled={!input.trim() && attachedFiles.length === 0}
                  className="sm-btn sm-btn-primary h-8 px-3 gap-1.5 rounded-lg text-xs disabled:bg-[var(--sm-surface-muted)] disabled:text-[var(--sm-text-muted)]"
                >
                  <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                    <path strokeLinecap="round" strokeLinejoin="round" d="M5 12h14M12 5l7 7-7 7" />
                  </svg>
                  发送
                </button>
              )}
            </div>
          </div>
        </div>
        </div>
      </div>
    </div>
  );
}
