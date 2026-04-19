use super::chat::AttachmentInput;

pub const PHASE_ONE_GLOBAL_MAX_ATTACHMENTS: usize = 5;
pub const PHASE_ONE_MAX_IMAGE_ATTACHMENTS: usize = 3;
pub const PHASE_ONE_MAX_DOCUMENT_ATTACHMENTS: usize = 5;
pub const PHASE_ONE_MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
pub const PHASE_ONE_MAX_TEXT_DOCUMENT_BYTES: usize = 20 * 1024 * 1024;
pub const PHASE_ONE_MAX_PDF_BYTES: usize = 20 * 1024 * 1024;
pub const PHASE_ONE_MAX_PDF_EXTRACTED_TEXT_CHARS: usize = 200_000;

#[derive(Debug, Clone, Copy)]
pub struct AttachmentCapabilityPolicy {
    pub enabled: bool,
    pub max_attachments: usize,
    pub max_bytes: usize,
    pub allow_sources: &'static [&'static str],
    pub fallback_behavior: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct DocumentAttachmentPolicy {
    pub enabled: bool,
    pub max_attachments: usize,
    pub max_text_bytes: usize,
    pub max_pdf_bytes: usize,
    pub allow_sources: &'static [&'static str],
    pub fallback_behavior: &'static str,
    pub max_extracted_text_chars: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct AttachmentPolicy {
    pub global_max_attachments: usize,
    pub image: AttachmentCapabilityPolicy,
    pub audio: AttachmentCapabilityPolicy,
    pub video: AttachmentCapabilityPolicy,
    pub document: DocumentAttachmentPolicy,
}

pub fn default_attachment_policy() -> AttachmentPolicy {
    AttachmentPolicy {
        global_max_attachments: PHASE_ONE_GLOBAL_MAX_ATTACHMENTS,
        image: AttachmentCapabilityPolicy {
            enabled: true,
            max_attachments: PHASE_ONE_MAX_IMAGE_ATTACHMENTS,
            max_bytes: PHASE_ONE_MAX_IMAGE_BYTES,
            allow_sources: &["browser_file"],
            fallback_behavior: "native",
        },
        audio: AttachmentCapabilityPolicy {
            enabled: true,
            max_attachments: 2,
            max_bytes: 25 * 1024 * 1024,
            allow_sources: &["browser_file"],
            fallback_behavior: "transcribe",
        },
        video: AttachmentCapabilityPolicy {
            enabled: true,
            max_attachments: 1,
            max_bytes: 100 * 1024 * 1024,
            allow_sources: &["browser_file"],
            fallback_behavior: "summarize",
        },
        document: DocumentAttachmentPolicy {
            enabled: true,
            max_attachments: PHASE_ONE_MAX_DOCUMENT_ATTACHMENTS,
            max_text_bytes: PHASE_ONE_MAX_TEXT_DOCUMENT_BYTES,
            max_pdf_bytes: PHASE_ONE_MAX_PDF_BYTES,
            allow_sources: &["browser_file"],
            fallback_behavior: "extract_text",
            max_extracted_text_chars: PHASE_ONE_MAX_PDF_EXTRACTED_TEXT_CHARS,
        },
    }
}

pub fn attachment_is_pdf(attachment: &AttachmentInput) -> bool {
    attachment.declared_mime_type.as_deref() == Some("application/pdf")
        || attachment.name.to_ascii_lowercase().ends_with(".pdf")
}

pub fn attachment_is_text_document(attachment: &AttachmentInput) -> bool {
    if attachment.kind != "document" || attachment_is_pdf(attachment) {
        return false;
    }

    let mime = attachment
        .declared_mime_type
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if mime.starts_with("text/") {
        return true;
    }
    if matches!(mime.as_str(), "application/json" | "text/csv") {
        return true;
    }

    matches!(
        attachment
            .name
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "txt"
            | "md"
            | "json"
            | "yaml"
            | "yml"
            | "xml"
            | "csv"
            | "tsv"
            | "log"
            | "ini"
            | "conf"
            | "env"
            | "js"
            | "jsx"
            | "ts"
            | "tsx"
            | "py"
            | "rs"
            | "go"
            | "java"
            | "c"
            | "cpp"
            | "h"
            | "cs"
            | "sh"
            | "ps1"
            | "sql"
    )
}

pub fn attachment_size_limit_bytes(
    policy: &AttachmentPolicy,
    attachment: &AttachmentInput,
) -> Result<usize, String> {
    match attachment.kind.as_str() {
        "image" => Ok(policy.image.max_bytes),
        "audio" => Ok(policy.audio.max_bytes),
        "video" => Ok(policy.video.max_bytes),
        "document" => {
            if attachment_is_pdf(attachment) {
                Ok(policy.document.max_pdf_bytes)
            } else {
                Ok(policy.document.max_text_bytes)
            }
        }
        other => Err(format!("未知附件类型 {other} {}", attachment.name)),
    }
}
