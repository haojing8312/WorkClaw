import type {
  AttachmentDraft,
  AttachmentDraftInput,
  AttachmentDraftKind,
  AttachmentDraftNormalizationResult,
  AttachmentDraftRejection,
  AttachmentDraftRejectionReason,
  BrowserFileAttachmentDraftInput,
} from "../types";
import {
  DEFAULT_ATTACHMENT_POLICY,
  AUDIO_EXTENSIONS,
  BINARY_DOCUMENT_EXTENSIONS,
  IMAGE_EXTENSIONS,
  PDF_EXTENSIONS,
  TEXT_FILE_EXTENSIONS,
  VIDEO_EXTENSIONS,
  type AttachmentPolicy,
} from "./attachmentPolicy";

type BrowserFileRecord = BrowserFileAttachmentDraftInput["file"];

const MIME_BY_EXTENSION = new Map<string, string>([
  ...IMAGE_EXTENSIONS.map((extension) => [
    extension,
    extension === "jpg" || extension === "jpeg" ? "image/jpeg" : extension === "webp" ? "image/webp" : `image/${extension}`,
  ] as const),
  ...AUDIO_EXTENSIONS.map((extension) => [
    extension,
    extension === "mp3"
      ? "audio/mpeg"
      : extension === "m4a"
        ? "audio/mp4"
        : extension === "wav"
          ? "audio/wav"
          : extension === "ogg"
            ? "audio/ogg"
            : extension === "flac"
              ? "audio/flac"
              : extension === "aac"
                ? "audio/aac"
                : "audio/webm",
  ] as const),
  ...VIDEO_EXTENSIONS.map((extension) => [
    extension,
    extension === "mov"
      ? "video/quicktime"
      : extension === "mkv"
        ? "video/x-matroska"
        : extension === "avi"
          ? "video/x-msvideo"
          : extension === "m4v"
            ? "video/x-m4v"
            : "video/mp4",
  ] as const),
  ...TEXT_FILE_EXTENSIONS.map((extension) => [extension, "text/plain"] as const),
  ...PDF_EXTENSIONS.map((extension) => [extension, "application/pdf"] as const),
  ...BINARY_DOCUMENT_EXTENSIONS.map((extension) => [
    extension,
    extension === "doc"
      ? "application/msword"
      : extension === "docx"
        ? "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        : extension === "xls"
          ? "application/vnd.ms-excel"
          : "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  ] as const),
]);

interface KindMatch {
  kind: AttachmentDraftKind;
  maxSizeBytes: number;
  resolvedMimeType: string;
}

type MatchSource = "mime" | "extension";

function getFileExtension(fileName: string): string {
  return fileName.split(".").pop()?.toLowerCase() ?? "";
}

function normalizeMimeType(mimeType: string): string {
  return mimeType.trim().toLowerCase();
}

function matchesMimeType(fileMimeType: string, candidateMimeType: string): boolean {
  if (candidateMimeType.endsWith("/*")) {
    return fileMimeType.startsWith(candidateMimeType.slice(0, -1));
  }
  return fileMimeType === candidateMimeType;
}

function getMatchedKind(file: BrowserFileRecord, policy: AttachmentPolicy): KindMatch | null {
  const mimeType = normalizeMimeType(file.type);
  const extension = getFileExtension(file.name);

  const findMatchByMimeType = (): KindMatch | null => {
    for (const kind of ["image", "audio", "video", "document"] as const) {
      const kindPolicy = policy.kinds[kind];
      if (!kindPolicy) {
        continue;
      }
      for (const fileType of kindPolicy.fileTypes) {
        if (
          fileType.mimeTypes.some((candidateMimeType) => matchesMimeType(mimeType, candidateMimeType.toLowerCase()))
        ) {
          return {
            kind,
            maxSizeBytes: fileType.maxSizeBytes,
            resolvedMimeType: resolveCanonicalMimeType(kind, extension, mimeType, fileType.mimeTypes[0], "mime"),
          };
        }
      }
    }
    return null;
  };

  if (mimeType) {
    return findMatchByMimeType();
  }

  for (const kind of ["image", "audio", "video", "document"] as const) {
    const kindPolicy = policy.kinds[kind];
    if (!kindPolicy) {
      continue;
    }
    for (const fileType of kindPolicy.fileTypes) {
      if (fileType.extensions.includes(extension)) {
        return {
          kind,
          maxSizeBytes: fileType.maxSizeBytes,
          resolvedMimeType: resolveCanonicalMimeType(kind, extension, mimeType, fileType.mimeTypes[0], "extension"),
        };
      }
    }
  }

  return null;
}

function resolveCanonicalMimeType(
  kind: AttachmentDraftKind,
  extension: string,
  sourceMimeType: string,
  declaredMimeType?: string,
  matchSource?: MatchSource,
): string {
  if (matchSource === "mime") {
    if (sourceMimeType) {
      return sourceMimeType;
    }
    if (declaredMimeType && !declaredMimeType.endsWith("/*")) {
      return declaredMimeType.toLowerCase();
    }
  }

  const mimeFromExtension = MIME_BY_EXTENSION.get(extension);
  if (mimeFromExtension) {
    return mimeFromExtension;
  }

  if (matchSource === "extension") {
    if (declaredMimeType && !declaredMimeType.endsWith("/*")) {
      return declaredMimeType.toLowerCase();
    }
  }

  if (sourceMimeType) {
    if (kind !== "document") {
      if (sourceMimeType.startsWith(`${kind}/`)) {
        return sourceMimeType;
      }
    } else if (
      sourceMimeType === "text/plain" ||
      sourceMimeType === "text/markdown" ||
      sourceMimeType === "application/json" ||
      sourceMimeType === "text/csv" ||
      sourceMimeType === "application/pdf" ||
      sourceMimeType === "application/msword" ||
      sourceMimeType === "application/vnd.openxmlformats-officedocument.wordprocessingml.document" ||
      sourceMimeType === "application/vnd.ms-excel" ||
      sourceMimeType === "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    ) {
      return sourceMimeType;
    }
  }

  return kind === "document" ? "application/octet-stream" : `${kind}/octet-stream`;
}

function createRejection(
  input: AttachmentDraftInput,
  reason: AttachmentDraftRejectionReason,
  message: string,
  kind?: AttachmentDraftKind,
): AttachmentDraftRejection {
  return { input, reason, message, kind };
}

function isKindEnabled(policy: AttachmentPolicy, kind: AttachmentDraftKind): boolean {
  return Boolean(policy.kinds[kind]?.enabled);
}

function getKindLimit(policy: AttachmentPolicy, kind: AttachmentDraftKind): number | undefined {
  return policy.kinds[kind]?.maxCount;
}

function getKindTotalSizeLimit(policy: AttachmentPolicy, kind: AttachmentDraftKind): number | undefined {
  return policy.kinds[kind]?.maxTotalSizeBytes;
}

function formatMegabyteLimit(bytes: number): string {
  const megabytes = bytes / (1024 * 1024);
  return Number.isInteger(megabytes) ? `${megabytes}MB` : `${megabytes.toFixed(1)}MB`;
}

function formatKindTotalSizeLabel(kind: AttachmentDraftKind): string {
  if (kind === "image") {
    return "图片附件";
  }
  return `${kind} 附件`;
}

function isBrowserFileAttachmentDraftInput(input: AttachmentDraftInput): input is BrowserFileAttachmentDraftInput {
  return input.sourceType === "browser_file" && "file" in input;
}

export function normalizeBrowserFileAttachmentDrafts(
  inputs: readonly AttachmentDraftInput[],
  policy: AttachmentPolicy = DEFAULT_ATTACHMENT_POLICY,
): AttachmentDraftNormalizationResult {
  const accepted: AttachmentDraft[] = [];
  const rejected: AttachmentDraftRejection[] = [];
  const kindCounts = new Map<AttachmentDraftKind, number>();
  const kindTotalSizes = new Map<AttachmentDraftKind, number>();

  for (const input of inputs) {
    if (!isBrowserFileAttachmentDraftInput(input)) {
      rejected.push(
        createRejection(input, "unsupported_source_type", `不支持的附件来源：${input.sourceType}`),
      );
      continue;
    }

    const matchedKind = getMatchedKind(input.file, policy);
    if (!matchedKind) {
      rejected.push(
        createRejection(input, "unrecognized_file_type", `无法识别文件类型：${input.file.name}`),
      );
      continue;
    }

    if (!isKindEnabled(policy, matchedKind.kind)) {
      rejected.push(
        createRejection(input, "kind_disabled", `当前策略未启用 ${matchedKind.kind} 附件`, matchedKind.kind),
      );
      continue;
    }

    const kindLimit = getKindLimit(policy, matchedKind.kind);
    const kindCount = kindCounts.get(matchedKind.kind) ?? 0;
    if (kindLimit !== undefined && kindCount >= kindLimit) {
      rejected.push(
        createRejection(input, "kind_limit_exceeded", `${matchedKind.kind} 附件数量已达上限`, matchedKind.kind),
      );
      continue;
    }

    if (input.file.size > matchedKind.maxSizeBytes) {
      rejected.push(
        createRejection(
          input,
          "size_exceeded",
          `${input.file.name} 超过 ${matchedKind.kind} 附件大小限制`,
          matchedKind.kind,
        ),
      );
      continue;
    }

    if (accepted.length >= policy.maxFiles) {
      rejected.push(createRejection(input, "batch_limit_exceeded", "附件总数已达上限", matchedKind.kind));
      continue;
    }

    const kindTotalSizeLimit = getKindTotalSizeLimit(policy, matchedKind.kind);
    const kindTotalSize = kindTotalSizes.get(matchedKind.kind) ?? 0;
    if (kindTotalSizeLimit !== undefined && kindTotalSize + input.file.size > kindTotalSizeLimit) {
      rejected.push(
        createRejection(
          input,
          "total_size_exceeded",
          `${formatKindTotalSizeLabel(matchedKind.kind)}总大小超过 ${formatMegabyteLimit(kindTotalSizeLimit)} 限制`,
          matchedKind.kind,
        ),
      );
      continue;
    }

    accepted.push({
      sourceType: input.sourceType,
      kind: matchedKind.kind,
      name: input.file.name,
      mimeType: matchedKind.resolvedMimeType,
      size: input.file.size,
    });
    kindCounts.set(matchedKind.kind, kindCount + 1);
    kindTotalSizes.set(matchedKind.kind, kindTotalSize + input.file.size);
  }

  return { accepted, rejected };
}
