import { useEffect, useRef, useState, type ChangeEvent } from "react";

import { normalizePendingAttachmentsFromBrowserFiles } from "../../lib/pendingAttachmentIntake";
import type { PendingAttachment } from "../../types";

type UseChatDraftStateArgs = {
  sessionId: string;
  initialAttachments?: PendingAttachment[];
  consumeInitialAttachmentsImmediately?: boolean;
  onInitialAttachmentsConsumed?: () => void;
};

export function useChatDraftState({
  sessionId,
  initialAttachments = [],
  consumeInitialAttachmentsImmediately = false,
  onInitialAttachmentsConsumed,
}: UseChatDraftStateArgs) {
  const [input, setInput] = useState("");
  const [attachedFiles, setAttachedFiles] = useState<PendingAttachment[]>([]);
  const [composerError, setComposerError] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const seededInitialAttachmentsSessionRef = useRef<string | null>(null);

  const syncComposerHeight = () => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
  };

  useEffect(() => {
    syncComposerHeight();
  }, [input, sessionId]);

  useEffect(() => {
    if (initialAttachments.length === 0) {
      return;
    }
    if (seededInitialAttachmentsSessionRef.current === sessionId) {
      return;
    }

    seededInitialAttachmentsSessionRef.current = sessionId;
    setAttachedFiles(initialAttachments);

    if (consumeInitialAttachmentsImmediately) {
      onInitialAttachmentsConsumed?.();
    }
  }, [consumeInitialAttachmentsImmediately, initialAttachments, onInitialAttachmentsConsumed, sessionId]);

  const handleFileSelect = async (e: ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(e.target.files || []);
    try {
      const { accepted, rejectionMessage } = await normalizePendingAttachmentsFromBrowserFiles({
        files,
        existingAttachments: attachedFiles,
        textOversizeMode: "reject",
      });

      if (accepted.length > 0) {
        setAttachedFiles((prev) => [...prev, ...accepted]);
      }
      setComposerError(rejectionMessage);
    } catch (error) {
      console.error("处理附件失败:", error);
      setComposerError("附件读取失败，请重试");
    }
    e.target.value = "";
  };

  const removeAttachedFile = (fileId: string) => {
    setAttachedFiles((prev) => prev.filter((item) => item.id !== fileId));
  };

  const handleComposerInputChange = (nextValue: string) => {
    if (composerError) setComposerError(null);
    setInput(nextValue);
  };

  const clearComposerState = () => {
    setInput("");
    setAttachedFiles([]);
    setComposerError(null);
  };

  return {
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
  };
}
