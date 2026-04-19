use super::chat::AttachmentInput;
use super::chat_attachment_policy::{attachment_is_pdf, attachment_is_text_document, AttachmentPolicy};
use super::chat_attachment_validation::validate_attachment_input;

const VIDEO_NO_AUDIO_TRACK: &str = "VIDEO_NO_AUDIO_TRACK";
const VIDEO_AUDIO_EXTRACTION_UNAVAILABLE: &str = "VIDEO_AUDIO_EXTRACTION_UNAVAILABLE";
const VIDEO_AUDIO_EXTRACTION_FAILED: &str = "VIDEO_AUDIO_EXTRACTION_FAILED";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAttachment {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub source_type: String,
    pub resolved_mime_type: String,
    pub size_bytes: Option<usize>,
    pub source_payload: Option<String>,
    pub source_uri: Option<String>,
    pub extracted_text: Option<String>,
    pub transcript: Option<String>,
    pub summary: Option<String>,
    pub truncated: bool,
    pub is_pdf: bool,
    pub warnings: Vec<String>,
}

pub fn resolve_attachment_input(
    policy: &AttachmentPolicy,
    attachment: &AttachmentInput,
) -> Result<ResolvedAttachment, String> {
    validate_attachment_input(policy, attachment)?;

    let is_pdf = attachment.kind == "document" && attachment_is_pdf(attachment);
    let is_text_document = attachment.kind == "document" && attachment_is_text_document(attachment);
    let resolved_mime_type = match attachment.kind.as_str() {
        "image" => attachment
            .declared_mime_type
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_string()),
        "document" if is_pdf => attachment
            .declared_mime_type
            .clone()
            .unwrap_or_else(|| "application/pdf".to_string()),
        "document" => attachment
            .declared_mime_type
            .clone()
            .unwrap_or_else(|| "text/plain".to_string()),
        "audio" => attachment
            .declared_mime_type
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_string()),
        "video" => attachment
            .declared_mime_type
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_string()),
        other => {
            return Err(format!(
                "当前阶段暂不支持 {other} 类型附件 {}",
                attachment.name
            ))
        }
    };

    Ok(ResolvedAttachment {
        id: attachment.id.clone(),
        kind: attachment.kind.clone(),
        name: attachment.name.clone(),
        source_type: attachment.source_type.clone(),
        resolved_mime_type,
        size_bytes: attachment.size_bytes,
        source_payload: attachment.source_payload.clone(),
        source_uri: attachment.source_uri.clone(),
        extracted_text: attachment.extracted_text.clone(),
        transcript: match attachment.kind.as_str() {
            "audio" => attachment
                .extracted_text
                .as_ref()
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty())
                .or_else(|| Some("TRANSCRIPTION_REQUIRED".to_string())),
            _ => None,
        },
        summary: match attachment.kind.as_str() {
            "video" => resolve_video_summary(attachment),
            "document" if !is_pdf && !is_text_document => Some("EXTRACTION_REQUIRED".to_string()),
            _ => None,
        },
        truncated: attachment.truncated.unwrap_or(false),
        is_pdf,
        warnings: match attachment.kind.as_str() {
            "audio" => {
                let has_transcript = attachment
                    .extracted_text
                    .as_ref()
                    .map(|text| !text.trim().is_empty())
                    .unwrap_or(false);
                if has_transcript {
                    Vec::new()
                } else {
                    vec!["transcription_pending".to_string()]
                }
            }
            "video" => resolve_video_warnings(attachment),
            "document" if !is_pdf && !is_text_document => {
                vec!["document_extraction_pending".to_string()]
            }
            _ => Vec::new(),
        },
    })
}

fn resolve_video_summary(attachment: &AttachmentInput) -> Option<String> {
    let summary = attachment
        .extracted_text
        .as_ref()
        .map(|text| text.trim())
        .filter(|text| !text.is_empty());
    match summary {
        Some(VIDEO_NO_AUDIO_TRACK) => Some(VIDEO_NO_AUDIO_TRACK.to_string()),
        Some(VIDEO_AUDIO_EXTRACTION_UNAVAILABLE) => Some(VIDEO_AUDIO_EXTRACTION_UNAVAILABLE.to_string()),
        Some(VIDEO_AUDIO_EXTRACTION_FAILED) => Some(VIDEO_AUDIO_EXTRACTION_FAILED.to_string()),
        Some(value) => Some(value.to_string()),
        None => Some("SUMMARY_REQUIRED".to_string()),
    }
}

fn resolve_video_warnings(attachment: &AttachmentInput) -> Vec<String> {
    match attachment
        .extracted_text
        .as_ref()
        .map(|text| text.trim())
        .filter(|text| !text.is_empty())
    {
        Some(VIDEO_NO_AUDIO_TRACK) => vec!["video_no_audio_track".to_string()],
        Some(VIDEO_AUDIO_EXTRACTION_UNAVAILABLE) => {
            vec!["video_audio_extraction_unavailable".to_string()]
        }
        Some(VIDEO_AUDIO_EXTRACTION_FAILED) => vec!["video_audio_extraction_failed".to_string()],
        Some(_) => Vec::new(),
        None => vec!["summary_pending".to_string()],
    }
}
