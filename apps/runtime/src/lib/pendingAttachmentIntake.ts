import {
  createAttachmentId,
  getFileExtension,
  isBinaryDocumentFile,
  readFileAsBase64,
  readFileAsDataUrl,
  readFileAsText,
} from "./chatAttachments";
import { normalizeBrowserFileAttachmentDrafts } from "./attachmentDrafts";
import {
  DEFAULT_ATTACHMENT_POLICY,
  MAX_PDF_FILE_SIZE,
  MAX_TEXT_FILE_SIZE,
  MAX_TEXT_PREVIEW_CHARS,
} from "./attachmentPolicy";
import type { AttachmentDraftRejection, PendingAttachment } from "../types";

type TextOversizeMode = "reject" | "truncate";

export interface NormalizePendingAttachmentsFromBrowserFilesArgs {
  files: File[];
  existingAttachments?: PendingAttachment[];
  textOversizeMode?: TextOversizeMode;
}

function buildAttachmentPolicyForExistingFiles(
  existingAttachments: PendingAttachment[],
  textOversizeMode: TextOversizeMode,
) {
  const existingImageCount = existingAttachments.filter((file) => file.kind === "image").length;
  const existingAudioCount = existingAttachments.filter((file) => file.kind === "audio").length;
  const existingVideoCount = existingAttachments.filter((file) => file.kind === "video").length;
  const existingDocumentCount = existingAttachments.filter(
    (file) => file.kind === "text-file" || file.kind === "pdf-file",
  ).length;
  const textDocumentPolicy = DEFAULT_ATTACHMENT_POLICY.kinds.document.fileTypes[0];

  return {
    ...DEFAULT_ATTACHMENT_POLICY,
    maxFiles: Math.max(0, DEFAULT_ATTACHMENT_POLICY.maxFiles - existingAttachments.length),
    kinds: {
      ...DEFAULT_ATTACHMENT_POLICY.kinds,
      image: {
        ...DEFAULT_ATTACHMENT_POLICY.kinds.image,
        maxCount: Math.max(
          0,
          (DEFAULT_ATTACHMENT_POLICY.kinds.image.maxCount ?? 0) - existingImageCount,
        ),
      },
      audio: {
        ...DEFAULT_ATTACHMENT_POLICY.kinds.audio,
        maxCount: Math.max(
          0,
          (DEFAULT_ATTACHMENT_POLICY.kinds.audio.maxCount ?? 0) - existingAudioCount,
        ),
      },
      video: {
        ...DEFAULT_ATTACHMENT_POLICY.kinds.video,
        maxCount: Math.max(
          0,
          (DEFAULT_ATTACHMENT_POLICY.kinds.video.maxCount ?? 0) - existingVideoCount,
        ),
      },
      document: {
        ...DEFAULT_ATTACHMENT_POLICY.kinds.document,
        ...(DEFAULT_ATTACHMENT_POLICY.kinds.document.maxCount !== undefined
          ? {
              maxCount: Math.max(
                0,
                DEFAULT_ATTACHMENT_POLICY.kinds.document.maxCount - existingDocumentCount,
              ),
            }
          : {}),
        fileTypes: DEFAULT_ATTACHMENT_POLICY.kinds.document.fileTypes.map((fileType) =>
          fileType === textDocumentPolicy && textOversizeMode === "truncate"
            ? {
                ...fileType,
                maxSizeBytes: Number.MAX_SAFE_INTEGER,
              }
            : fileType,
        ),
      },
    },
  };
}

function getAttachmentRejectionFileName(rejection: AttachmentDraftRejection): string {
  if (rejection.input.sourceType === "browser_file") {
    return rejection.input.file.name;
  }
  return "该附件";
}

function formatAttachmentRejection(rejection: AttachmentDraftRejection): string {
  const fileName = getAttachmentRejectionFileName(rejection);

  switch (rejection.reason) {
    case "batch_limit_exceeded":
      return `最多只能上传 ${DEFAULT_ATTACHMENT_POLICY.maxFiles} 个文件`;
    case "kind_limit_exceeded":
      if (rejection.kind === "image") {
        return `最多只能上传 ${DEFAULT_ATTACHMENT_POLICY.kinds.image.maxCount ?? 0} 张图片`;
      }
      if (rejection.kind === "audio") {
        return `最多只能上传 ${DEFAULT_ATTACHMENT_POLICY.kinds.audio.maxCount ?? 0} 个音频附件`;
      }
      if (rejection.kind === "video") {
        return `最多只能上传 ${DEFAULT_ATTACHMENT_POLICY.kinds.video.maxCount ?? 0} 个视频附件`;
      }
      return `${fileName} 超出当前附件数量限制`;
    case "size_exceeded":
      if (rejection.kind === "image") {
        return `图片 ${fileName} 超过 5MB 限制`;
      }
      if (fileName.toLowerCase().endsWith(".pdf")) {
        return `PDF 文件 ${fileName} 超过 ${Math.floor(MAX_PDF_FILE_SIZE / 1024 / 1024)}MB 限制`;
      }
      return `文档文件 ${fileName} 超过 ${Math.floor(MAX_TEXT_FILE_SIZE / 1024 / 1024)}MB 限制`;
    case "total_size_exceeded":
      return rejection.message;
    case "unrecognized_file_type":
    case "kind_disabled":
      return `暂不支持附件类型 ${fileName}`;
    case "unsupported_source_type":
      return rejection.message;
    default:
      return rejection.message;
  }
}

function isTextLikeDocumentDraft(draft: { mimeType: string; name: string }): boolean {
  const mimeType = draft.mimeType.trim().toLowerCase();
  const extension = getFileExtension(draft.name);
  if (mimeType === "application/pdf") {
    return false;
  }
  if (mimeType.startsWith("text/")) {
    return true;
  }
  return ["txt", "md", "json", "yaml", "yml", "xml", "csv", "tsv", "log", "ini", "conf", "env", "js", "jsx", "ts", "tsx", "py", "rs", "go", "java", "c", "cpp", "h", "cs", "sh", "ps1", "sql"].includes(extension);
}

function buildAttachmentRejectionMessage(rejections: AttachmentDraftRejection[]): string | null {
  if (rejections.length === 0) {
    return null;
  }

  return rejections.map(formatAttachmentRejection).join("；");
}

export async function normalizePendingAttachmentsFromBrowserFiles({
  files,
  existingAttachments = [],
  textOversizeMode = "reject",
}: NormalizePendingAttachmentsFromBrowserFilesArgs): Promise<{
  accepted: PendingAttachment[];
  rejectionMessage: string | null;
}> {
  if (files.length === 0) {
    return { accepted: [], rejectionMessage: null };
  }

  const draftInputs = files.map((file) => ({
    sourceType: "browser_file" as const,
    file,
  }));
  const policy = buildAttachmentPolicyForExistingFiles(existingAttachments, textOversizeMode);
  const { accepted, rejected } = normalizeBrowserFileAttachmentDrafts(draftInputs, policy);
  const rejectedFiles = new Set(
    rejected.flatMap((item) =>
      item.input.sourceType === "browser_file" ? [item.input.file as File] : [],
    ),
  );

  const nextAccepted: PendingAttachment[] = [];
  let acceptedDraftIndex = 0;

  for (const file of files) {
    if (rejectedFiles.has(file)) {
      continue;
    }

    const draft = accepted[acceptedDraftIndex];
    acceptedDraftIndex += 1;
    if (!draft) {
      continue;
    }

    if (draft.kind === "image") {
      const data = await readFileAsDataUrl(file);
      nextAccepted.push({
        id: createAttachmentId(),
        kind: "image",
        name: draft.name,
        mimeType: draft.mimeType || file.type || "image/png",
        size: file.size,
        data,
        previewUrl: data,
      });
      continue;
    }

    if (draft.kind === "audio" || draft.kind === "video") {
      const data = await readFileAsBase64(file);
      nextAccepted.push({
        id: createAttachmentId(),
        kind: draft.kind,
        name: draft.name,
        mimeType: draft.mimeType || file.type || "application/octet-stream",
        size: file.size,
        data,
      });
      continue;
    }

    if (draft.mimeType === "application/pdf") {
      const data = await readFileAsBase64(file);
      nextAccepted.push({
        id: createAttachmentId(),
        kind: "pdf-file",
        name: draft.name,
        mimeType: draft.mimeType || file.type || "application/pdf",
        size: file.size,
        data,
      });
      continue;
    }

    if (draft.kind === "document" && !isTextLikeDocumentDraft(draft)) {
      const data = await readFileAsBase64(file);
      nextAccepted.push({
        id: createAttachmentId(),
        kind: "document-file",
        name: draft.name,
        mimeType: draft.mimeType || file.type || "application/octet-stream",
        size: file.size,
        data,
        summary: "EXTRACTION_REQUIRED",
        warnings: isBinaryDocumentFile(file) ? ["document_extraction_pending"] : [],
      });
      continue;
    }

    const text = await readFileAsText(file);
    const shouldTruncate =
      textOversizeMode === "truncate" &&
      text.length > MAX_TEXT_PREVIEW_CHARS;
    nextAccepted.push({
      id: createAttachmentId(),
      kind: "text-file",
      name: draft.name,
      mimeType: draft.mimeType || file.type || "text/plain",
      size: file.size,
      text: shouldTruncate ? text.slice(0, MAX_TEXT_PREVIEW_CHARS) : text,
      ...(shouldTruncate ? { truncated: true } : {}),
    });
  }

  return {
    accepted: nextAccepted,
    rejectionMessage: buildAttachmentRejectionMessage(rejected),
  };
}
