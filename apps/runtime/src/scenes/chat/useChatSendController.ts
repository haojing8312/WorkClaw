import { useCallback } from "react";

import type { Dispatch, MutableRefObject, RefObject, SetStateAction } from "react";

import type { AttachmentInput, ChatMessagePart, PendingAttachment, SendMessageRequest, Message } from "../../types";
import { getModelErrorDisplay, inferModelErrorKindFromMessage } from "../../lib/model-error-display";
import { sendMessage } from "../../services/chat/chatSessionService";

type UseChatSendControllerArgs = {
  sessionId: string;
  streaming: boolean;
  input: string;
  attachedFiles: PendingAttachment[];
  clearComposerState: () => void;
  setComposerError: (value: string | null) => void;
  setMessages: Dispatch<SetStateAction<Message[]>>;
  loadMessages: (sessionId: string) => Promise<void>;
  loadSessionRuns: (sessionId: string) => Promise<void>;
  prepareForSend: () => void;
  finishStreaming: () => void;
  onSessionUpdate?: () => void;
  autoFollowScrollRef: MutableRefObject<boolean>;
  setIsNearBottom: (value: boolean) => void;
  setIsNearTop: (value: boolean) => void;
  animateScrollRegionTo: (targetTop: number, durationMs: number, target: "top" | "bottom") => void;
  scrollRegionRef: RefObject<HTMLDivElement>;
  shouldGrantContinuationBudget: (request: SendMessageRequest) => boolean;
  continuationBudgetIncrement: number;
};

export function buildDefaultAttachmentPrompt(attachments: PendingAttachment[]): string {
  const hasImage = attachments.some((file) => file.kind === "image");
  const hasDocument = attachments.some(
    (file) => file.kind === "text-file" || file.kind === "pdf-file" || file.kind === "document-file",
  );
  const hasAudio = attachments.some((file) => file.kind === "audio");
  const hasVideo = attachments.some((file) => file.kind === "video");
  if (hasImage && hasDocument) {
    return "请结合这些图片和文档附件一起分析，并给出结论。";
  }
  if (hasImage && (hasAudio || hasVideo)) {
    return "请结合这些图片和音视频附件一起分析，并给出结论。";
  }
  if (hasImage) {
    return "请结合这些图片描述主要内容，并提取可见文字。";
  }
  if (hasAudio || hasVideo) {
    return "请结合这些音频和视频附件总结关键信息，并说明当前仍需补充的处理步骤。";
  }
  return "请阅读这些文档附件并总结关键信息。";
}

function toAttachmentInput(file: PendingAttachment): AttachmentInput {
  if (file.kind === "image") {
    return {
      id: file.id,
      kind: "image",
      sourceType: "browser_file",
      name: file.name,
      declaredMimeType: file.mimeType,
      sizeBytes: file.size,
      sourcePayload: file.data,
    };
  }
  if (file.kind === "audio" || file.kind === "video") {
    return {
      id: file.id,
      kind: file.kind,
      sourceType: "browser_file",
      name: file.name,
      declaredMimeType: file.mimeType,
      sizeBytes: file.size,
      sourcePayload: file.data,
    };
  }
  if (file.kind === "document-file") {
    return {
      id: file.id,
      kind: "document",
      sourceType: "browser_file",
      name: file.name,
      declaredMimeType: file.mimeType,
      sizeBytes: file.size,
      sourcePayload: file.data,
      summary: file.summary,
      warnings: file.warnings,
    };
  }
  if (file.kind === "pdf-file") {
    return {
      id: file.id,
      kind: "document",
      sourceType: "browser_file",
      name: file.name,
      declaredMimeType: file.mimeType,
      sizeBytes: file.size,
      sourcePayload: file.data,
      extractedText: file.extractedText,
      truncated: file.truncated,
    };
  }
  return {
    id: file.id,
    kind: "document",
    sourceType: "browser_file",
    name: file.name,
    declaredMimeType: file.mimeType,
    sizeBytes: file.size,
    sourcePayload: file.text,
    truncated: file.truncated,
  };
}

function isPdfAttachment(attachment: AttachmentInput): boolean {
  return (
    attachment.declaredMimeType === "application/pdf" ||
    attachment.name.toLowerCase().endsWith(".pdf")
  );
}

export function getAttachmentPhaseOneDisplayKind(
  attachment: AttachmentInput,
): "image" | "pdf" | "text" | "unsupported" {
  if (attachment.sourceType !== "browser_file") {
    return "unsupported";
  }
  if (attachment.kind === "image") {
    return attachment.sourcePayload ? "image" : "unsupported";
  }
  if (attachment.kind !== "document") {
    return "unsupported";
  }
  if (!attachment.sourcePayload && !attachment.extractedText) {
    return "unsupported";
  }
  return isPdfAttachment(attachment) ? "pdf" : "text";
}

export function toOptimisticDisplayPart(part: ChatMessagePart): ChatMessagePart {
  if (part.type !== "attachment") {
    return part;
  }

  const { attachment } = part;
  switch (getAttachmentPhaseOneDisplayKind(attachment)) {
    case "image":
      return {
        type: "image",
        name: attachment.name,
        mimeType: attachment.declaredMimeType ?? "application/octet-stream",
        size: attachment.sizeBytes ?? 0,
        data: attachment.sourcePayload ?? "",
      };
    case "pdf":
      return {
        type: "pdf_file",
        name: attachment.name,
        mimeType: attachment.declaredMimeType ?? "application/pdf",
        size: attachment.sizeBytes ?? 0,
        data: attachment.sourcePayload,
        extractedText: attachment.extractedText,
        truncated: attachment.truncated,
      };
    case "text":
      return {
        type: "file_text",
        name: attachment.name,
        mimeType: attachment.declaredMimeType ?? "text/plain",
        size: attachment.sizeBytes ?? 0,
        text: attachment.sourcePayload ?? attachment.extractedText ?? "",
        truncated: attachment.truncated,
      };
    default:
      return part;
  }
}

export function buildMessageParts(message: string, attachments: PendingAttachment[]): ChatMessagePart[] {
  const normalizedMessage = message.trim() || buildDefaultAttachmentPrompt(attachments);
  const parts: ChatMessagePart[] = [{ type: "text", text: normalizedMessage }];
  attachments.forEach((file) => {
    parts.push({
      type: "attachment",
      attachment: toAttachmentInput(file),
    });
  });
  return parts;
}

function buildOptimisticUserContent(parts: ChatMessagePart[]): string {
  return parts
    .map((part) => {
      if (part.type === "text") {
        return part.text;
      }
      if (part.type === "attachment") {
        const displayKind = getAttachmentPhaseOneDisplayKind(part.attachment);
        if (displayKind === "image") {
          return `[图片: ${part.attachment.name}]`;
        }
        if (displayKind === "pdf") {
          return `[PDF: ${part.attachment.name}]`;
        }
        if (displayKind === "text") {
          return `[文件: ${part.attachment.name}]`;
        }
        return `[附件: ${part.attachment.name}]`;
      }
      if (part.type === "image") {
        return `[图片: ${part.name}]`;
      }
      if (part.type === "file_text") {
        return `[文件: ${part.name}]`;
      }
      if (part.type === "pdf_file") {
        return `[PDF: ${part.name}]`;
      }
      return "";
    })
    .join("\n");
}

function toUserFacingSendError(error: unknown): string {
  const raw =
    typeof error === "string" ? error : error instanceof Error ? error.message : String(error ?? "");
  if (raw.includes("VISION_MODEL_NOT_CONFIGURED")) {
    return "请先在设置中配置图片理解模型";
  }
  const modelErrorKind = inferModelErrorKindFromMessage(raw);
  if (modelErrorKind) {
    const display = getModelErrorDisplay(modelErrorKind);
    return `${display.title}：${display.message}`;
  }
  return `错误: ${raw}`;
}

function shouldPreserveInlineSendError(error: unknown): boolean {
  const raw =
    typeof error === "string" ? error : error instanceof Error ? error.message : String(error ?? "");
  return raw.includes("VISION_MODEL_NOT_CONFIGURED");
}

function isModelRouteFailureError(error: unknown): boolean {
  const raw =
    typeof error === "string" ? error : error instanceof Error ? error.message : String(error ?? "");
  return inferModelErrorKindFromMessage(raw) !== null;
}

export function useChatSendController({
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
  continuationBudgetIncrement,
}: UseChatSendControllerArgs) {
  const sendContent = useCallback(
    async (request: SendMessageRequest | string) => {
      if (streaming || !sessionId) return;

      const normalizedRequest: SendMessageRequest =
        typeof request === "string"
          ? {
              sessionId,
              parts: [{ type: "text", text: request.trim() }],
            }
          : request;
      const continuationRequest =
        shouldGrantContinuationBudget(normalizedRequest) && normalizedRequest.maxIterations === undefined
          ? {
              ...normalizedRequest,
              maxIterations: continuationBudgetIncrement,
            }
          : normalizedRequest;
      const optimisticContent = buildOptimisticUserContent(continuationRequest.parts);
      if (!continuationRequest.parts.length || !optimisticContent.trim()) return;

      clearComposerState();
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
          contentParts: continuationRequest.parts.map(toOptimisticDisplayPart),
          created_at: new Date().toISOString(),
        },
      ]);
      prepareForSend();
      try {
        await sendMessage(continuationRequest);
        onSessionUpdate?.();
      } catch (e) {
        const preserveInlineError = shouldPreserveInlineSendError(e);
        const modelRouteFailureError = isModelRouteFailureError(e);
        const userFacingError = toUserFacingSendError(e);
        setComposerError(modelRouteFailureError ? null : userFacingError);
        if (preserveInlineError) {
          return;
        }
        await Promise.all([loadMessages(sessionId), loadSessionRuns(sessionId)]);
        if (!modelRouteFailureError) {
          setMessages((prev) => [
            ...prev,
            {
              role: "assistant",
              content: userFacingError,
              created_at: new Date().toISOString(),
            },
          ]);
        }
      } finally {
        finishStreaming();
      }
    },
    [
      autoFollowScrollRef,
      clearComposerState,
      continuationBudgetIncrement,
      animateScrollRegionTo,
      finishStreaming,
      loadMessages,
      loadSessionRuns,
      onSessionUpdate,
      prepareForSend,
      scrollRegionRef,
      sessionId,
      setComposerError,
      setIsNearBottom,
      setIsNearTop,
      setMessages,
      shouldGrantContinuationBudget,
      streaming,
    ],
  );

  const handleSend = useCallback(async () => {
    if (!input.trim() && attachedFiles.length === 0) return;
    if (streaming || !sessionId) return;

    const parts = buildMessageParts(input, attachedFiles);
    await sendContent({
      sessionId,
      parts,
    });
  }, [attachedFiles, input, sendContent, sessionId, streaming]);

  return {
    sendContent,
    handleSend,
  };
}
