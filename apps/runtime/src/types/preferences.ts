export interface RuntimePreferences {
  default_work_dir: string;
  default_language: string;
  immersive_translation_enabled: boolean;
  immersive_translation_display: "translated_only" | "bilingual_inline" | string;
  immersive_translation_trigger: "auto" | "manual" | string;
  translation_engine: "model_then_free" | "model_only" | "free_only" | string;
  translation_model_id: string;
  launch_at_login: boolean;
  launch_minimized: boolean;
  close_to_tray: boolean;
  operation_permission_mode: "standard" | "full_access" | string;
}

export type AttachmentDraftKind = "image" | "audio" | "video" | "document";

export type AttachmentDraftSourceType =
  | "browser_file"
  | "local_path"
  | "file_url"
  | "remote_url"
  | "data_url"
  | "base64";

export interface AttachmentDraftInputBase {
  sourceType: AttachmentDraftSourceType;
}

export interface BrowserFileAttachmentDraftInput extends AttachmentDraftInputBase {
  sourceType: "browser_file";
  file: Pick<File, "name" | "type" | "size">;
}

export type AttachmentDraftNonBrowserInput = AttachmentDraftInputBase & {
  sourceType: Exclude<AttachmentDraftSourceType, "browser_file">;
};

export type AttachmentDraftInput = BrowserFileAttachmentDraftInput | AttachmentDraftNonBrowserInput;

export interface AttachmentDraft {
  sourceType: AttachmentDraftSourceType;
  kind: AttachmentDraftKind;
  name: string;
  mimeType: string;
  size: number;
}

export type AttachmentDraftRejectionReason =
  | "unsupported_source_type"
  | "unrecognized_file_type"
  | "kind_disabled"
  | "size_exceeded"
  | "total_size_exceeded"
  | "kind_limit_exceeded"
  | "batch_limit_exceeded";

export interface AttachmentDraftRejection {
  input: AttachmentDraftInput;
  reason: AttachmentDraftRejectionReason;
  kind?: AttachmentDraftKind;
  message: string;
}

export interface AttachmentDraftNormalizationResult {
  accepted: AttachmentDraft[];
  rejected: AttachmentDraftRejection[];
}

export type AttachmentKind = AttachmentDraftKind;

export type AttachmentSourceType = AttachmentDraftSourceType;

export interface AttachmentInput {
  id: string;
  kind: AttachmentKind;
  sourceType: AttachmentSourceType;
  name: string;
  declaredMimeType?: string;
  sizeBytes?: number;
  sourcePayload?: string;
  sourceUri?: string;
  mediaRef?: string;
  extractedText?: string;
  previewChars?: number;
  transcript?: string;
  summary?: string;
  warnings?: string[];
  truncated?: boolean;
}

export type PendingAttachment =
  | {
      id: string;
      kind: "image";
      name: string;
      mimeType: string;
      size: number;
      data: string;
      previewUrl: string;
    }
  | {
      id: string;
      kind: "text-file";
      name: string;
      mimeType: string;
      size: number;
      text: string;
      truncated?: boolean;
    }
  | {
      id: string;
      kind: "audio";
      name: string;
      mimeType: string;
      size: number;
      data: string;
    }
  | {
      id: string;
      kind: "video";
      name: string;
      mimeType: string;
      size: number;
      data?: string;
      mediaRef?: string;
    }
  | {
      id: string;
      kind: "document-file";
      name: string;
      mimeType: string;
      size: number;
      data: string;
      summary?: string;
      warnings?: string[];
    }
  | {
      id: string;
      kind: "pdf-file";
      name: string;
      mimeType: string;
      size: number;
      data: string;
      extractedText?: string;
      truncated?: boolean;
    };

export interface LandingSessionLaunchInput {
  initialMessage: string;
  attachments: PendingAttachment[];
  workDir: string;
}

export type ChatMessagePart =
  | { type: "text"; text: string }
  | { type: "attachment"; attachment: AttachmentInput }
  | {
      type: "image";
      name: string;
      mimeType: string;
      size: number;
      data?: string;
      mediaRef?: string;
    }
  | {
      type: "file_text";
      name: string;
      mimeType: string;
      size: number;
      text: string;
      truncated?: boolean;
      mediaRef?: string;
      previewChars?: number;
    }
  | {
      type: "pdf_file";
      name: string;
      mimeType: string;
      size: number;
      data?: string;
      extractedText?: string;
      truncated?: boolean;
      mediaRef?: string;
      previewChars?: number;
    };

export interface SendMessageRequest {
  sessionId: string;
  parts: ChatMessagePart[];
  maxIterations?: number;
}

/// 兼容旧附件实现，待迁移到 PendingAttachment/ChatMessagePart。
export interface FileAttachment {
  name: string;
  size: number;
  type: string;
  content: string;
}
