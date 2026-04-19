export type AttachmentCapability = "image" | "audio" | "video" | "document";

export interface AttachmentFileTypePolicy {
  mimeTypes: readonly string[];
  extensions: readonly string[];
  maxSizeBytes: number;
}

export interface AttachmentCapabilityPolicy {
  enabled: boolean;
  maxCount?: number;
  fileTypes: readonly AttachmentFileTypePolicy[];
}

export interface AttachmentPolicy {
  maxFiles: number;
  kinds: Record<AttachmentCapability, AttachmentCapabilityPolicy>;
}

export const MAX_FILES = 5;
export const MAX_IMAGE_FILES = 3;
export const MAX_IMAGE_SIZE = 5 * 1024 * 1024;
export const MAX_TEXT_FILE_SIZE = 20 * 1024 * 1024;
export const MAX_PDF_FILE_SIZE = 20 * 1024 * 1024;
export const MAX_TEXT_PREVIEW_CHARS = 200_000;

export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp"] as const;
export const TEXT_FILE_EXTENSIONS = [
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
] as const;
export const PDF_EXTENSIONS = ["pdf"] as const;
export const BINARY_DOCUMENT_EXTENSIONS = [
  "doc",
  "docx",
  "xls",
  "xlsx",
] as const;
export const AUDIO_EXTENSIONS = ["mp3", "m4a", "wav", "ogg", "flac", "aac", "webm"] as const;
export const VIDEO_EXTENSIONS = ["mp4", "mov", "webm", "mkv", "avi", "m4v"] as const;

const ATTACHMENT_KIND_ORDER: AttachmentCapability[] = ["image", "audio", "video", "document"];

export const DEFAULT_ATTACHMENT_POLICY: AttachmentPolicy = {
  maxFiles: MAX_FILES,
  kinds: {
    image: {
      enabled: true,
      maxCount: MAX_IMAGE_FILES,
      fileTypes: [
        {
          mimeTypes: ["image/*"],
          extensions: IMAGE_EXTENSIONS,
          maxSizeBytes: MAX_IMAGE_SIZE,
        },
      ],
    },
    audio: {
      enabled: true,
      maxCount: 2,
      fileTypes: [
        {
          mimeTypes: ["audio/*"],
          extensions: AUDIO_EXTENSIONS,
          maxSizeBytes: 25 * 1024 * 1024,
        },
      ],
    },
    video: {
      enabled: true,
      maxCount: 1,
      fileTypes: [
        {
          mimeTypes: ["video/*"],
          extensions: VIDEO_EXTENSIONS,
          maxSizeBytes: 100 * 1024 * 1024,
        },
      ],
    },
    document: {
      enabled: true,
      fileTypes: [
        {
          mimeTypes: ["text/plain", "text/markdown", "application/json", "text/csv"],
          extensions: TEXT_FILE_EXTENSIONS,
          maxSizeBytes: MAX_TEXT_FILE_SIZE,
        },
        {
          mimeTypes: ["application/pdf"],
          extensions: PDF_EXTENSIONS,
          maxSizeBytes: MAX_PDF_FILE_SIZE,
        },
        {
          mimeTypes: [
            "application/msword",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "application/vnd.ms-excel",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
          ],
          extensions: BINARY_DOCUMENT_EXTENSIONS,
          maxSizeBytes: MAX_TEXT_FILE_SIZE,
        },
      ],
    },
  },
};

export function buildFileInputAccept(policy: AttachmentPolicy = DEFAULT_ATTACHMENT_POLICY): string {
  const acceptTokens: string[] = [];
  const seen = new Set<string>();

  for (const kind of ATTACHMENT_KIND_ORDER) {
    const kindPolicy = policy.kinds[kind];
    if (!kindPolicy?.enabled) {
      continue;
    }
    for (const fileType of kindPolicy.fileTypes) {
      for (const mimeType of fileType.mimeTypes) {
        if (!seen.has(mimeType)) {
          seen.add(mimeType);
          acceptTokens.push(mimeType);
        }
      }
      for (const extension of fileType.extensions) {
        const token = `.${extension}`;
        if (!seen.has(token)) {
          seen.add(token);
          acceptTokens.push(token);
        }
      }
    }
  }

  return acceptTokens.join(",");
}
