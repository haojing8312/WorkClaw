import { useCallback } from "react";

import type { Dispatch, MutableRefObject, RefObject, SetStateAction } from "react";

import type { ChatMessagePart, PendingAttachment, SendMessageRequest, Message } from "../../types";
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
  const hasDocument = attachments.some((file) => file.kind === "text-file" || file.kind === "pdf-file");
  if (hasImage && hasDocument) {
    return "请结合这些图片和文档附件一起分析，并给出结论。";
  }
  if (hasImage) {
    return "请结合这些图片描述主要内容，并提取可见文字。";
  }
  return "请阅读这些文档附件并总结关键信息。";
}

export function buildMessageParts(message: string, attachments: PendingAttachment[]): ChatMessagePart[] {
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
      ...(file.kind === "pdf-file"
        ? {
            type: "pdf_file" as const,
            name: file.name,
            mimeType: file.mimeType,
            size: file.size,
            data: file.data,
            extractedText: file.extractedText,
            truncated: file.truncated,
          }
        : {
            type: "file_text" as const,
            name: file.name,
            mimeType: file.mimeType,
            size: file.size,
            text: file.text,
            truncated: file.truncated,
          }),
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
          contentParts: continuationRequest.parts,
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
