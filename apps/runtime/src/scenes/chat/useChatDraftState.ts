import { useEffect, useRef, useState, type ChangeEvent } from "react";

import {
  isImageFile,
  isPdfFile,
  isTextFile,
  MAX_FILES,
  MAX_IMAGE_FILES,
  MAX_IMAGE_SIZE,
  MAX_PDF_FILE_SIZE,
  MAX_TEXT_FILE_SIZE,
  readFileAsBase64,
  readFileAsDataUrl,
  readFileAsText,
} from "../../lib/chatAttachments";
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
        if (!isPdfFile(file)) {
          alert(`暂不支持附件类型 ${file.name}`);
          continue;
        }
        if (file.size > MAX_PDF_FILE_SIZE) {
          alert(`PDF 文件 ${file.name} 超过 10MB 限制`);
          continue;
        }
        const data = await readFileAsBase64(file);
        newFiles.push({
          id: crypto.randomUUID(),
          kind: "pdf-file",
          name: file.name,
          mimeType: file.type || "application/pdf",
          size: file.size,
          data,
        });
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
