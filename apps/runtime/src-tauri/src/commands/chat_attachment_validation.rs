use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

use super::chat::AttachmentInput;
use super::chat_attachment_policy::{
    attachment_is_pdf, attachment_is_text_document, attachment_size_limit_bytes,
    AttachmentCapabilityPolicy, AttachmentPolicy,
};

pub fn validate_attachment_input(
    policy: &AttachmentPolicy,
    attachment: &AttachmentInput,
) -> Result<(), String> {
    match attachment.kind.as_str() {
        "image" => {
            validate_capability_enabled("image", attachment, policy.image.enabled)?;
            validate_source_allowed("image", attachment, policy.image.allow_sources)?;
            validate_size_limit(policy, attachment)?;
            if attachment.source_payload.is_none() {
                return Err(format!("图片附件 {} 缺少 sourcePayload", attachment.name));
            }
            Ok(())
        }
        "document" => {
            validate_document_enabled(policy, attachment)?;
            validate_source_allowed("document", attachment, policy.document.allow_sources)?;
            validate_size_limit(policy, attachment)?;
            if attachment_is_pdf(attachment) {
                if attachment.source_payload.is_none() && attachment.extracted_text.is_none() {
                    return Err(format!(
                        "PDF 附件 {} 缺少 sourcePayload 或 extractedText",
                        attachment.name
                    ));
                }
            } else if attachment_is_text_document(attachment)
                && attachment.source_payload.is_none()
                && attachment.extracted_text.is_none()
            {
                return Err(format!(
                    "文档附件 {} 缺少 sourcePayload 或 extractedText",
                    attachment.name
                ));
            }
            Ok(())
        }
        "audio" => validate_capability_enabled("audio", attachment, policy.audio.enabled),
        "video" => validate_capability_enabled("video", attachment, policy.video.enabled),
        other => Err(format!("未知附件类型 {other} {}", attachment.name)),
    }
}

pub fn validate_attachment_inputs(
    policy: &AttachmentPolicy,
    attachments: &[AttachmentInput],
) -> Result<(), String> {
    if attachments.len() > policy.global_max_attachments {
        return Err(format!(
            "附件数量超过当前阶段限制 {}",
            policy.global_max_attachments
        ));
    }

    let mut counts = HashMap::<&str, usize>::new();
    for attachment in attachments {
        *counts.entry(attachment.kind.as_str()).or_insert(0) += 1;
        validate_attachment_input(policy, attachment)?;
    }

    validate_kind_count(
        "image",
        counts.get("image").copied().unwrap_or(0),
        &policy.image,
    )?;
    validate_kind_count(
        "document",
        counts.get("document").copied().unwrap_or(0),
        &AttachmentCapabilityPolicy {
            enabled: policy.document.enabled,
            max_attachments: policy.document.max_attachments,
            max_bytes: policy.document.max_text_bytes,
            allow_sources: policy.document.allow_sources,
            fallback_behavior: policy.document.fallback_behavior,
        },
    )?;
    validate_kind_count(
        "audio",
        counts.get("audio").copied().unwrap_or(0),
        &policy.audio,
    )?;
    validate_kind_count(
        "video",
        counts.get("video").copied().unwrap_or(0),
        &policy.video,
    )?;

    Ok(())
}

fn validate_document_enabled(
    policy: &AttachmentPolicy,
    attachment: &AttachmentInput,
) -> Result<(), String> {
    if !policy.document.enabled {
        return Err(format!(
            "当前阶段暂不支持 document 类型附件 {}",
            attachment.name
        ));
    }
    Ok(())
}

fn validate_capability_enabled(
    kind: &str,
    attachment: &AttachmentInput,
    enabled: bool,
) -> Result<(), String> {
    if enabled {
        Ok(())
    } else {
        Err(format!(
            "当前阶段暂不支持 {kind} 类型附件 {}",
            attachment.name
        ))
    }
}

fn validate_source_allowed(
    kind: &str,
    attachment: &AttachmentInput,
    allow_sources: &[&str],
) -> Result<(), String> {
    if allow_sources.contains(&attachment.source_type.as_str()) {
        Ok(())
    } else {
        Err(format!(
            "{kind} 附件 {} 的 sourceType {} 不在允许范围内",
            attachment.name, attachment.source_type
        ))
    }
}

fn validate_size_limit(
    policy: &AttachmentPolicy,
    attachment: &AttachmentInput,
) -> Result<(), String> {
    let Some(size_bytes) = resolve_effective_size_bytes(attachment)? else {
        return Ok(());
    };
    let max_bytes = attachment_size_limit_bytes(policy, attachment)?;
    if size_bytes > max_bytes {
        Err(format!(
            "附件 {} 超过 {} 字节限制",
            attachment.name, max_bytes
        ))
    } else {
        Ok(())
    }
}

fn resolve_effective_size_bytes(attachment: &AttachmentInput) -> Result<Option<usize>, String> {
    let derived_size = derive_size_bytes_from_payload(attachment)?;
    Ok(match (attachment.size_bytes, derived_size) {
        (Some(claimed), Some(derived)) => Some(claimed.max(derived)),
        (Some(claimed), None) => Some(claimed),
        (None, Some(derived)) => Some(derived),
        (None, None) => None,
    })
}

fn derive_size_bytes_from_payload(attachment: &AttachmentInput) -> Result<Option<usize>, String> {
    match attachment.kind.as_str() {
        "image" => attachment
            .source_payload
            .as_deref()
            .map(decode_base64_payload_len)
            .transpose(),
        "document" if attachment_is_pdf(attachment) => {
            if let Some(payload) = attachment.source_payload.as_deref() {
                return decode_base64_payload_len(payload).map(Some);
            }
            Ok(attachment.extracted_text.as_ref().map(|text| text.len()))
        }
        "document" => Ok(attachment
            .source_payload
            .as_ref()
            .or(attachment.extracted_text.as_ref())
            .map(|text| text.len())),
        _ => Ok(attachment
            .source_payload
            .as_ref()
            .or(attachment.extracted_text.as_ref())
            .map(|content| content.len())),
    }
}

fn decode_base64_payload_len(payload: &str) -> Result<usize, String> {
    let encoded = payload
        .split_once("base64,")
        .map(|(_, payload)| payload)
        .unwrap_or(payload);
    let bytes = BASE64.decode(encoded).map_err(|err| err.to_string())?;
    Ok(bytes.len())
}

fn validate_kind_count(
    kind: &str,
    count: usize,
    capability: &AttachmentCapabilityPolicy,
) -> Result<(), String> {
    if count > capability.max_attachments {
        Err(format!(
            "{kind} 附件数量超过当前阶段限制 {}",
            capability.max_attachments
        ))
    } else {
        Ok(())
    }
}
