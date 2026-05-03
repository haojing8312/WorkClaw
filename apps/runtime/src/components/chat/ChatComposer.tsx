import { memo, type ChangeEvent, type ClipboardEvent, type DragEvent, type KeyboardEvent, type RefObject } from "react";

import { buildFileInputAccept, DEFAULT_ATTACHMENT_POLICY } from "../../lib/attachmentPolicy";
import { buildPendingAttachmentMeta } from "../../lib/chatAttachments";
import type { ModelConfig, PendingAttachment } from "../../types";

type ChatComposerProps = {
  operationPermissionMode?: "standard" | "full_access" | string;
  quickPrompts: Array<{ label: string; prompt: string }>;
  streaming: boolean;
  sendContent: (request: string) => Promise<void> | void;
  attachedFiles: PendingAttachment[];
  onFilesAdd: (files: File[]) => Promise<void> | void;
  onFileSelect: (event: ChangeEvent<HTMLInputElement>) => void;
  composerError: string | null;
  input: string;
  textareaRef: RefObject<HTMLTextAreaElement>;
  onInputChange: (value: string) => void;
  onSubmit: () => void;
  onWorkdirClick: () => void;
  displayWorkDirLabel: string;
  currentModel: ModelConfig | null;
  onRemoveAttachment: (fileId: string) => void;
  onCancel: () => void;
};

function ChatComposerImpl({
  operationPermissionMode = "standard",
  quickPrompts,
  streaming,
  sendContent,
  attachedFiles,
  onFilesAdd,
  onFileSelect,
  composerError,
  input,
  textareaRef,
  onInputChange,
  onSubmit,
  onWorkdirClick,
  displayWorkDirLabel,
  currentModel,
  onRemoveAttachment,
  onCancel,
}: ChatComposerProps) {
  const addTransferredFiles = (files: File[]) => {
    if (files.length === 0) {
      return false;
    }
    void onFilesAdd(files);
    return true;
  };

  const handleDragOver = (event: DragEvent<HTMLDivElement>) => {
    if (Array.from(event.dataTransfer.types).includes("Files")) {
      event.preventDefault();
    }
  };

  const handleDrop = (event: DragEvent<HTMLDivElement>) => {
    const files = Array.from(event.dataTransfer.files || []);
    if (addTransferredFiles(files)) {
      event.preventDefault();
    }
  };

  const handlePaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const filesFromList = Array.from(event.clipboardData.files || []);
    const filesFromItems = Array.from(event.clipboardData.items || [])
      .filter((item) => item.kind === "file")
      .map((item) => item.getAsFile())
      .filter((file): file is File => Boolean(file));
    const files = filesFromList.length > 0 ? filesFromList : filesFromItems;
    if (addTransferredFiles(files)) {
      event.preventDefault();
    }
  };

  return (
    <div className="border-t border-slate-200/80 bg-[#f4f4f1]/92 px-4 py-3 sm:px-6 xl:px-8">
      <div className="mx-auto w-full max-w-[76rem]">
        <div
          data-testid="chat-composer-shell"
          onDragOver={handleDragOver}
          onDrop={handleDrop}
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
                  className="h-7 rounded border border-blue-200 px-2.5 text-[11px] text-blue-700 hover:bg-blue-50 disabled:bg-gray-100 disabled:text-gray-400"
                >
                  {item.label}
                </button>
              ))}
            </div>
          )}

          <input
            type="file"
            multiple
            accept={buildFileInputAccept(DEFAULT_ATTACHMENT_POLICY)}
            onChange={onFileSelect}
            className="hidden"
            id="file-upload"
          />

          {attachedFiles.length > 0 && (
            <div className="space-y-2 pb-2">
              <div className="text-[11px] text-gray-500">已添加 {attachedFiles.length} 个附件</div>
              <div className="space-y-2">
                {attachedFiles.map((file) => (
                  (() => {
                    const meta = buildPendingAttachmentMeta(file);
                    return (
                  <div
                    key={file.id}
                    className="flex items-center gap-3 rounded-lg border border-gray-200 bg-white px-3 py-2"
                  >
                    {file.kind === "image" ? (
                      <img
                        src={file.previewUrl}
                        alt={file.name}
                        className="h-10 w-10 rounded border border-gray-200 object-cover"
                      />
                    ) : (
                      <div className="flex h-10 w-10 items-center justify-center rounded border border-gray-200 bg-gray-50 text-[11px] text-gray-600">
                        {file.kind === "audio"
                          ? "AUD"
                          : file.kind === "video"
                            ? "VID"
                            : file.kind === "pdf-file"
                              ? "PDF"
                              : file.kind === "document-file"
                                ? "DOC"
                                : "TXT"}
                      </div>
                    )}
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm text-gray-800">{file.name}</div>
                      <div className="flex items-center gap-2 text-[11px] text-gray-500">
                        <span>{meta.badge}</span>
                        <span>{Math.ceil(file.size / 1024)} KB</span>
                        {meta.truncated && <span>已截断</span>}
                      </div>
                    </div>
                    <button
                      type="button"
                      aria-label="移除附件"
                      onClick={() => onRemoveAttachment(file.id)}
                      className="rounded-md px-2 py-1 text-xs text-gray-500 hover:bg-gray-100"
                    >
                      删除
                    </button>
                  </div>
                    );
                  })()
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

          <textarea
            ref={textareaRef}
            value={input}
            onChange={(event) => onInputChange(event.target.value)}
            onPaste={handlePaste}
            onKeyDown={(event: KeyboardEvent<HTMLTextAreaElement>) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                onSubmit();
              }
            }}
            placeholder="输入消息，Shift+Enter 换行..."
            rows={3}
            className="sm-textarea w-full min-h-[88px] max-h-[200px] border-0 bg-transparent px-0 py-0 focus:border-0 focus:shadow-none"
          />

          <div className="mt-4 flex flex-wrap items-center justify-between gap-3 border-t border-gray-100 pt-3">
            <div className="flex flex-wrap items-center gap-2">
              <button
                type="button"
                data-testid="chat-composer-workdir-button"
                onClick={onWorkdirClick}
                className="inline-flex items-center gap-1.5 rounded-lg bg-gray-100 px-2.5 py-1 text-xs text-gray-600 transition-colors hover:bg-gray-200"
                title={displayWorkDirLabel}
              >
                <svg className="h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-6l-2-2H5a2 2 0 0 0-2 2Z"
                  />
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
              <label htmlFor="file-upload" className="sm-btn sm-btn-secondary h-8 cursor-pointer gap-1.5 rounded-lg px-3 text-xs">
                <svg className="h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13"
                  />
                </svg>
                附件
              </label>
            </div>
            <div className="flex items-center gap-2">
              {streaming ? (
                <button onClick={onCancel} className="sm-btn sm-btn-danger h-8 gap-1.5 rounded-lg px-3 text-xs">
                  <svg className="h-3.5 w-3.5" fill="currentColor" viewBox="0 0 24 24">
                    <rect x="6" y="6" width="12" height="12" rx="2" />
                  </svg>
                  停止
                </button>
              ) : (
                <button
                  onClick={onSubmit}
                  disabled={!input.trim() && attachedFiles.length === 0}
                  className="sm-btn sm-btn-primary h-8 gap-1.5 rounded-lg px-3 text-xs disabled:bg-[var(--sm-surface-muted)] disabled:text-[var(--sm-text-muted)]"
                >
                  <svg className="h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
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
  );
}

export const ChatComposer = memo(ChatComposerImpl, (prev, next) => {
  return (
    prev.operationPermissionMode === next.operationPermissionMode &&
    prev.streaming === next.streaming &&
    prev.composerError === next.composerError &&
    prev.input === next.input &&
    prev.displayWorkDirLabel === next.displayWorkDirLabel &&
    prev.currentModel?.id === next.currentModel?.id &&
    prev.quickPrompts === next.quickPrompts &&
    prev.attachedFiles === next.attachedFiles
  );
});
