import type { ChatMessagePart } from "../../types";
import { getAttachmentPhaseOneDisplayKind } from "../../scenes/chat/useChatSendController";

type AttachmentDisplayMeta = {
  label: string;
  detail?: string;
};

function describeAttachmentCard(
  part: Exclude<ChatMessagePart, { type: "text" | "image" | "file_text" | "pdf_file" }>,
): AttachmentDisplayMeta {
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
}

export function renderUserContentParts(parts: ChatMessagePart[]) {
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
              const hasInlineData = Boolean(part.data?.trim());
              return (
                <div
                  key={`attachment-${part.name}-${index}`}
                  className="rounded-xl border border-white/20 bg-white/10 p-2 text-xs"
                >
                  {hasInlineData ? (
                    <img
                      src={part.data}
                      alt={part.name}
                      className="max-h-56 w-full rounded-lg object-cover"
                    />
                  ) : (
                    <div className="rounded-lg border border-white/15 bg-white/10 p-3">
                      <div className="font-medium">{part.name}</div>
                      <div className="mt-1 opacity-80">图片附件 · 已保存</div>
                    </div>
                  )}
                  {hasInlineData && <div className="mt-2 opacity-90">{part.name}</div>}
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
                    detail: part.mediaRef
                      ? part.truncated
                        ? "已保存 · 已截断"
                        : "已保存"
                      : part.truncated
                        ? "已截断"
                        : undefined,
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
}
