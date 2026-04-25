use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use regex::Regex;
use reqwest::Client;
use runtime_chat_app::{
    parse_fallback_chain_targets, ChatSettingsRepository, PreparedRouteCandidate,
};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::process::Command;
use zip::ZipArchive;

use crate::agent::runtime::repo::PoolChatSettingsRepository;
use crate::model_transport::{resolve_model_transport, ModelTransportKind};

use super::chat::{AttachmentInput, SendMessagePart};
use super::chat_attachment_policy::{
    attachment_is_text_document, default_attachment_policy, PHASE_ONE_MAX_PDF_EXTRACTED_TEXT_CHARS,
};
use super::chat_attachment_resolution::{resolve_attachment_input, ResolvedAttachment};
use super::chat_attachment_validation::validate_attachment_inputs;

const VIDEO_NO_AUDIO_TRACK: &str = "VIDEO_NO_AUDIO_TRACK";
const VIDEO_AUDIO_EXTRACTION_UNAVAILABLE: &str = "VIDEO_AUDIO_EXTRACTION_UNAVAILABLE";
const VIDEO_AUDIO_EXTRACTION_FAILED: &str = "VIDEO_AUDIO_EXTRACTION_FAILED";
const VIDEO_VISUAL_SUMMARY_PROMPT: &str = "请基于这些视频关键帧，用简洁中文总结视频里正在发生的事情、主要对象和场景。若信息有限，请明确说明。";

pub(crate) fn normalize_message_parts(parts: &[SendMessagePart]) -> Result<Vec<Value>, String> {
    let policy = default_attachment_policy();
    let attachments = collect_attachment_inputs(parts);
    validate_attachment_inputs(&policy, &attachments)?;
    parts
        .iter()
        .map(|part| normalize_message_part(part, &policy))
        .collect()
}

pub(crate) async fn normalize_message_parts_with_pool(
    parts: &[SendMessagePart],
    pool: &sqlx::SqlitePool,
) -> Result<Vec<Value>, String> {
    let policy = default_attachment_policy();
    let attachments = collect_attachment_inputs(parts);
    validate_attachment_inputs(&policy, &attachments)?;
    let prepared_attachments = preprocess_attachment_inputs_with_pool(&attachments, pool).await?;
    let prepared_by_id = prepared_attachments
        .into_iter()
        .map(|attachment| (attachment.id.clone(), attachment))
        .collect::<HashMap<_, _>>();

    parts
        .iter()
        .map(|part| match part {
            SendMessagePart::Attachment { attachment } => {
                let normalized_attachment =
                    prepared_by_id.get(&attachment.id).unwrap_or(attachment);
                normalize_attachment_part(&policy, normalized_attachment)
            }
            _ => normalize_message_part(part, &policy),
        })
        .collect()
}

async fn preprocess_attachment_inputs_with_pool(
    attachments: &[AttachmentInput],
    pool: &sqlx::SqlitePool,
) -> Result<Vec<AttachmentInput>, String> {
    let audio_candidate = resolve_audio_stt_route_candidate(pool).await?;
    let vision_candidate = resolve_vision_route_candidate(pool).await?;
    if audio_candidate.is_none() && vision_candidate.is_none() {
        return Ok(attachments.to_vec());
    }

    let mut prepared = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        if attachment.kind != "audio" && attachment.kind != "video" {
            prepared.push(attachment.clone());
            continue;
        }

        if attachment.kind == "audio" {
            let Some(audio_candidate) = audio_candidate.as_ref() else {
                prepared.push(attachment.clone());
                continue;
            };
            prepared.push(
                preprocess_audio_attachment_with_candidate(attachment, audio_candidate).await?,
            );
        } else {
            prepared.push(
                preprocess_video_attachment_with_candidates(
                    attachment,
                    audio_candidate.as_ref(),
                    vision_candidate.as_ref(),
                )
                .await?,
            );
        }
    }
    Ok(prepared)
}

async fn resolve_audio_stt_route_candidate(
    pool: &sqlx::SqlitePool,
) -> Result<Option<PreparedRouteCandidate>, String> {
    resolve_capability_route_candidate(pool, "audio_stt", supports_audio_stt_provider_candidate)
        .await
}

pub(crate) async fn resolve_vision_route_candidate(
    pool: &sqlx::SqlitePool,
) -> Result<Option<PreparedRouteCandidate>, String> {
    resolve_capability_route_candidate(pool, "vision", supports_vision_provider_candidate).await
}

async fn resolve_capability_route_candidate(
    pool: &sqlx::SqlitePool,
    capability: &str,
    supports_candidate: fn(&str, &str, &str, &str) -> bool,
) -> Result<Option<PreparedRouteCandidate>, String> {
    let repo = PoolChatSettingsRepository::new(pool);
    let Some(policy) = repo
        .load_route_policy(capability)
        .await?
        .filter(|policy| policy.enabled)
    else {
        return Ok(None);
    };

    let mut provider_targets = vec![(policy.primary_provider_id, policy.primary_model)];
    provider_targets.extend(parse_fallback_chain_targets(&policy.fallback_chain_json));

    for (provider_id, preferred_model) in provider_targets {
        let Some(provider) = repo.get_provider_connection(&provider_id).await? else {
            continue;
        };
        if !supports_candidate(
            &provider.protocol_type,
            &provider.base_url,
            &provider.provider_key,
            &provider.api_key,
        ) {
            continue;
        }
        return Ok(Some(PreparedRouteCandidate {
            provider_key: provider.provider_key,
            protocol_type: provider.protocol_type,
            base_url: provider.base_url,
            model_name: preferred_model,
            api_key: provider.api_key,
        }));
    }

    Ok(None)
}

fn supports_audio_stt_provider_candidate(
    protocol_type: &str,
    base_url: &str,
    provider_key: &str,
    api_key: &str,
) -> bool {
    if api_key.trim().is_empty() {
        return false;
    }

    let provider_key = if provider_key.trim().is_empty() {
        None
    } else {
        Some(provider_key)
    };
    let transport = resolve_model_transport(protocol_type, base_url, provider_key);
    matches!(
        transport.kind,
        ModelTransportKind::OpenAiCompletions | ModelTransportKind::OpenAiResponses
    )
}

fn supports_vision_provider_candidate(
    protocol_type: &str,
    base_url: &str,
    provider_key: &str,
    api_key: &str,
) -> bool {
    supports_audio_stt_provider_candidate(protocol_type, base_url, provider_key, api_key)
}

async fn preprocess_audio_attachment_with_candidate(
    attachment: &AttachmentInput,
    candidate: &PreparedRouteCandidate,
) -> Result<AttachmentInput, String> {
    if attachment
        .extracted_text
        .as_ref()
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false)
    {
        return Ok(attachment.clone());
    }

    let Some(payload) = attachment.source_payload.as_deref() else {
        return Ok(attachment.clone());
    };

    let Some(transcript) =
        transcribe_audio_attachment_with_candidate(attachment, payload, candidate).await?
    else {
        return Ok(attachment.clone());
    };

    let mut prepared = attachment.clone();
    prepared.extracted_text = Some(transcript);
    Ok(prepared)
}

async fn preprocess_video_attachment_with_candidates(
    attachment: &AttachmentInput,
    audio_candidate: Option<&PreparedRouteCandidate>,
    vision_candidate: Option<&PreparedRouteCandidate>,
) -> Result<AttachmentInput, String> {
    if attachment
        .extracted_text
        .as_ref()
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false)
    {
        return Ok(attachment.clone());
    }

    let Some(payload) = attachment.source_payload.as_deref() else {
        return Ok(attachment.clone());
    };

    if let Some(candidate) = audio_candidate {
        let extraction = extract_audio_bytes_from_video_payload(attachment, payload);
        let audio_bytes = match extraction {
            Ok(VideoAudioExtractionOutcome::Extracted(bytes)) => Some(bytes),
            Ok(VideoAudioExtractionOutcome::NoAudioTrack) => None,
            Ok(VideoAudioExtractionOutcome::Unavailable) => {
                if vision_candidate.is_none() {
                    let mut prepared = attachment.clone();
                    prepared.extracted_text = Some(VIDEO_AUDIO_EXTRACTION_UNAVAILABLE.to_string());
                    return Ok(prepared);
                }
                None
            }
            Ok(VideoAudioExtractionOutcome::Failed) | Err(_) => {
                if vision_candidate.is_none() {
                    let mut prepared = attachment.clone();
                    prepared.extracted_text = Some(VIDEO_AUDIO_EXTRACTION_FAILED.to_string());
                    return Ok(prepared);
                }
                None
            }
        };
        if let Some(audio_bytes) = audio_bytes {
            let Some(transcript) = transcribe_audio_bytes_with_candidate(
                "audio.mp3",
                "audio/mpeg",
                &attachment.name,
                &audio_bytes,
                candidate,
            )
            .await?
            else {
                return Ok(attachment.clone());
            };

            let mut prepared = attachment.clone();
            prepared.extracted_text = Some(format!("音轨转写：{transcript}"));
            return Ok(prepared);
        }
    }

    if let Some(candidate) = vision_candidate {
        if let Some(summary) =
            summarize_video_frames_with_candidate(attachment, payload, candidate).await?
        {
            let mut prepared = attachment.clone();
            prepared.extracted_text = Some(format!("视频画面摘要：{summary}"));
            return Ok(prepared);
        }
    }

    let mut prepared = attachment.clone();
    prepared.extracted_text = Some(VIDEO_NO_AUDIO_TRACK.to_string());
    Ok(prepared)
}

async fn transcribe_audio_attachment_with_candidate(
    attachment: &AttachmentInput,
    payload: &str,
    candidate: &PreparedRouteCandidate,
) -> Result<Option<String>, String> {
    let normalized_base_url = candidate.base_url.trim();
    if normalized_base_url.eq_ignore_ascii_case("http://mock-audio-stt-success") {
        return Ok(Some(format!("MOCK_TRANSCRIPT: {}", attachment.name)));
    }
    if normalized_base_url.eq_ignore_ascii_case("http://mock-audio-stt-empty") {
        return Ok(None);
    }
    if normalized_base_url.eq_ignore_ascii_case("http://mock-audio-stt-error") {
        return Err(format!(
            "音频附件 {} 转写失败: mock upstream error",
            attachment.name
        ));
    }

    let bytes = decode_base64_payload_bytes(payload)
        .map_err(|err| format!("音频附件 {} 读取失败: {err}", attachment.name))?;
    transcribe_audio_bytes_with_candidate(
        &attachment.name,
        attachment
            .declared_mime_type
            .as_deref()
            .unwrap_or("application/octet-stream"),
        &attachment.name,
        &bytes,
        candidate,
    )
    .await
}

async fn transcribe_audio_bytes_with_candidate(
    upload_file_name: &str,
    upload_mime_type: &str,
    display_name: &str,
    bytes: &[u8],
    candidate: &PreparedRouteCandidate,
) -> Result<Option<String>, String> {
    let normalized_base_url = candidate.base_url.trim();
    if normalized_base_url.eq_ignore_ascii_case("http://mock-audio-stt-success") {
        return Ok(Some(format!("MOCK_TRANSCRIPT: {display_name}")));
    }
    if normalized_base_url.eq_ignore_ascii_case("http://mock-audio-stt-empty") {
        return Ok(None);
    }
    if normalized_base_url.eq_ignore_ascii_case("http://mock-audio-stt-error") {
        return Err(format!(
            "音频附件 {} 转写失败: mock upstream error",
            display_name
        ));
    }

    let boundary = "workclaw-audio-stt-boundary";
    let multipart_body = build_audio_transcription_multipart_body(
        boundary,
        upload_file_name,
        upload_mime_type,
        &candidate.model_name,
        bytes,
    );
    let url = format!(
        "{}/audio/transcriptions",
        normalized_base_url.trim_end_matches('/')
    );
    let response = Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|err| format!("创建音频转写客户端失败: {err}"))?
        .post(url)
        .bearer_auth(&candidate.api_key)
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(multipart_body)
        .send()
        .await
        .map_err(|err| format!("音频附件 {} 转写请求失败: {err}", display_name))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("音频附件 {} 转写响应读取失败: {err}", display_name))?;
    if !status.is_success() {
        let preview = body.chars().take(240).collect::<String>();
        return Err(format!(
            "音频附件 {} 转写失败: HTTP {} {}",
            display_name, status, preview
        ));
    }

    parse_audio_transcription_body(&body)
        .map(|transcript| Some(transcript))
        .ok_or_else(|| {
            format!(
                "音频附件 {} 转写响应缺少 transcript/text 字段",
                display_name
            )
        })
}

fn parse_audio_transcription_body(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return value
            .get("text")
            .and_then(Value::as_str)
            .or_else(|| value.get("transcript").and_then(Value::as_str))
            .or_else(|| {
                value
                    .get("result")
                    .and_then(|result| result.get("text"))
                    .and_then(Value::as_str)
            })
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToString::to_string);
    }

    Some(trimmed.to_string())
}

fn extract_audio_bytes_from_video_payload(
    attachment: &AttachmentInput,
    payload: &str,
) -> Result<VideoAudioExtractionOutcome, String> {
    let video_bytes = decode_base64_payload_bytes(payload)
        .map_err(|err| format!("视频附件 {} 读取失败: {err}", attachment.name))?;
    let Some(ffmpeg) = resolve_ffmpeg_command() else {
        return Ok(VideoAudioExtractionOutcome::Unavailable);
    };
    let temp_dir = tempfile::tempdir()
        .map_err(|err| format!("视频附件 {} 创建临时目录失败: {err}", attachment.name))?;
    let input_extension = attachment
        .name
        .rsplit('.')
        .next()
        .filter(|ext| !ext.trim().is_empty())
        .unwrap_or("mp4");
    let input_path = temp_dir.path().join(format!("input.{input_extension}"));
    let output_path = temp_dir.path().join("audio.mp3");
    std::fs::write(&input_path, &video_bytes)
        .map_err(|err| format!("视频附件 {} 写入临时文件失败: {err}", attachment.name))?;

    let output = Command::new(&ffmpeg)
        .args([
            "-y",
            "-i",
            input_path.to_string_lossy().as_ref(),
            "-vn",
            "-acodec",
            "mp3",
            output_path.to_string_lossy().as_ref(),
        ])
        .output()
        .map_err(|err| format!("视频附件 {} 调用 ffmpeg 失败: {err}", attachment.name))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Output file #0 does not contain any stream")
            || stderr.contains("Stream map '0:a' matches no streams")
            || stderr.contains("does not contain any stream")
        {
            return Ok(VideoAudioExtractionOutcome::NoAudioTrack);
        }
        return Ok(VideoAudioExtractionOutcome::Failed);
    }

    let audio_bytes = std::fs::read(&output_path)
        .map_err(|err| format!("视频附件 {} 读取音轨失败: {err}", attachment.name))?;
    if audio_bytes.is_empty() {
        return Ok(VideoAudioExtractionOutcome::NoAudioTrack);
    }
    Ok(VideoAudioExtractionOutcome::Extracted(audio_bytes))
}

enum VideoAudioExtractionOutcome {
    Extracted(Vec<u8>),
    NoAudioTrack,
    Unavailable,
    Failed,
}

async fn summarize_video_frames_with_candidate(
    attachment: &AttachmentInput,
    payload: &str,
    candidate: &PreparedRouteCandidate,
) -> Result<Option<String>, String> {
    let Some(frame_data_urls) = extract_video_frame_data_urls(payload, attachment)? else {
        return Ok(None);
    };
    if frame_data_urls.is_empty() {
        return Ok(None);
    }

    request_vision_summary_with_candidate(
        &attachment.name,
        &frame_data_urls,
        candidate,
        VIDEO_VISUAL_SUMMARY_PROMPT,
    )
    .await
}

fn extract_video_frame_data_urls(
    payload: &str,
    attachment: &AttachmentInput,
) -> Result<Option<Vec<String>>, String> {
    let video_bytes = decode_base64_payload_bytes(payload)
        .map_err(|err| format!("视频附件 {} 读取失败: {err}", attachment.name))?;
    let Some(ffmpeg) = resolve_ffmpeg_command() else {
        return Ok(None);
    };
    let temp_dir = tempfile::tempdir()
        .map_err(|err| format!("视频附件 {} 创建临时目录失败: {err}", attachment.name))?;
    let input_extension = attachment
        .name
        .rsplit('.')
        .next()
        .filter(|ext| !ext.trim().is_empty())
        .unwrap_or("mp4");
    let input_path = temp_dir.path().join(format!("input.{input_extension}"));
    let frame_pattern = temp_dir.path().join("frame-%02d.jpg");
    std::fs::write(&input_path, &video_bytes)
        .map_err(|err| format!("视频附件 {} 写入临时文件失败: {err}", attachment.name))?;

    let output = Command::new(&ffmpeg)
        .args([
            "-y",
            "-i",
            input_path.to_string_lossy().as_ref(),
            "-vf",
            "fps=1,scale=768:-1",
            "-frames:v",
            "3",
            frame_pattern.to_string_lossy().as_ref(),
        ])
        .output()
        .map_err(|err| format!("视频附件 {} 调用 ffmpeg 抽帧失败: {err}", attachment.name))?;
    if !output.status.success() {
        return Ok(None);
    }

    let mut frames = Vec::new();
    for index in 1..=3 {
        let frame_path = temp_dir.path().join(format!("frame-{index:02}.jpg"));
        if !frame_path.exists() {
            continue;
        }
        let bytes = std::fs::read(&frame_path)
            .map_err(|err| format!("视频附件 {} 读取关键帧失败: {err}", attachment.name))?;
        if bytes.is_empty() {
            continue;
        }
        frames.push(format!("data:image/jpeg;base64,{}", BASE64.encode(bytes)));
    }

    if frames.is_empty() {
        Ok(None)
    } else {
        Ok(Some(frames))
    }
}

pub(crate) async fn request_vision_summary_with_candidate(
    display_name: &str,
    frame_data_urls: &[String],
    candidate: &PreparedRouteCandidate,
    prompt: &str,
) -> Result<Option<String>, String> {
    request_visual_summary_with_candidate(
        "视频附件",
        "画面摘要",
        "frames",
        display_name,
        frame_data_urls,
        candidate,
        prompt,
    )
    .await
}

pub(crate) async fn request_image_vision_summary_with_candidate(
    display_name: &str,
    image_data_urls: &[String],
    candidate: &PreparedRouteCandidate,
    prompt: &str,
) -> Result<Option<String>, String> {
    request_visual_summary_with_candidate(
        "图片",
        "视觉分析",
        "images",
        display_name,
        image_data_urls,
        candidate,
        prompt,
    )
    .await
}

async fn request_visual_summary_with_candidate(
    subject_label: &str,
    action_label: &str,
    count_label: &str,
    display_name: &str,
    image_data_urls: &[String],
    candidate: &PreparedRouteCandidate,
    prompt: &str,
) -> Result<Option<String>, String> {
    let normalized_base_url = candidate.base_url.trim();
    if normalized_base_url.eq_ignore_ascii_case("http://mock-vision-summary-success") {
        return Ok(Some(format!(
            "MOCK_VISION_SUMMARY: {} ({} {})",
            display_name,
            image_data_urls.len(),
            count_label
        )));
    }
    if normalized_base_url.eq_ignore_ascii_case("http://mock-vision-summary-empty") {
        return Ok(None);
    }
    if normalized_base_url.eq_ignore_ascii_case("http://mock-vision-summary-error") {
        return Err(format!(
            "{subject_label} {display_name} {action_label}失败: mock upstream error"
        ));
    }

    let transport = resolve_model_transport(
        &candidate.protocol_type,
        &candidate.base_url,
        Some(candidate.provider_key.as_str()).filter(|value| !value.trim().is_empty()),
    );
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|err| format!("创建视频摘要客户端失败: {err}"))?;
    let (url, body) = match transport.kind {
        ModelTransportKind::OpenAiResponses => (
            format!("{}/responses", normalized_base_url.trim_end_matches('/')),
            build_video_vision_responses_request_body(
                &candidate.model_name,
                prompt,
                image_data_urls,
            ),
        ),
        ModelTransportKind::OpenAiCompletions => (
            format!(
                "{}/chat/completions",
                normalized_base_url.trim_end_matches('/')
            ),
            build_video_vision_chat_completions_request_body(
                &candidate.model_name,
                prompt,
                image_data_urls,
            ),
        ),
        ModelTransportKind::AnthropicMessages => return Ok(None),
    };

    let response = client
        .post(url)
        .bearer_auth(&candidate.api_key)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("{subject_label} {display_name} {action_label}请求失败: {err}"))?;
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        format!("{subject_label} {display_name} {action_label}响应读取失败: {err}")
    })?;
    if !status.is_success() {
        let preview = body.chars().take(240).collect::<String>();
        return Err(format!(
            "{subject_label} {display_name} {action_label}失败: HTTP {} {}",
            status, preview
        ));
    }

    parse_vision_summary_body(&body)
        .map(Some)
        .ok_or_else(|| format!("{subject_label} {display_name} {action_label}响应缺少文本内容"))
}

fn build_video_vision_responses_request_body(
    model_name: &str,
    prompt: &str,
    frame_data_urls: &[String],
) -> Value {
    let mut content = vec![json!({
        "type": "input_text",
        "text": prompt,
    })];
    content.extend(frame_data_urls.iter().map(|image_url| {
        json!({
            "type": "input_image",
            "image_url": image_url,
        })
    }));
    json!({
        "model": model_name,
        "input": [{
            "role": "user",
            "content": content,
        }],
        "stream": false,
    })
}

fn build_video_vision_chat_completions_request_body(
    model_name: &str,
    prompt: &str,
    frame_data_urls: &[String],
) -> Value {
    let mut content = vec![json!({
        "type": "text",
        "text": prompt,
    })];
    content.extend(frame_data_urls.iter().map(|image_url| {
        json!({
            "type": "image_url",
            "image_url": { "url": image_url },
        })
    }));
    json!({
        "model": model_name,
        "messages": [{
            "role": "user",
            "content": content,
        }],
        "stream": false,
    })
}

fn parse_vision_summary_body(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }
    let value = serde_json::from_str::<Value>(trimmed).ok()?;
    extract_text_from_openai_like_body(&value)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn extract_text_from_openai_like_body(value: &Value) -> Option<String> {
    if let Some(text) = value.get("output_text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(output) = value.get("output").and_then(Value::as_array) {
        let collected = output
            .iter()
            .flat_map(|item| {
                item.get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|part| {
                        part.get("text")
                            .and_then(Value::as_str)
                            .or_else(|| part.get("output_text").and_then(Value::as_str))
                    })
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        if !collected.is_empty() {
            return Some(collected.join("\n"));
        }
    }
    if let Some(choices) = value.get("choices").and_then(Value::as_array) {
        for choice in choices {
            if let Some(content) = choice
                .get("message")
                .and_then(|message| message.get("content"))
            {
                match content {
                    Value::String(text) if !text.trim().is_empty() => {
                        return Some(text.trim().to_string());
                    }
                    Value::Array(parts) => {
                        let collected = parts
                            .iter()
                            .filter_map(|part| part.get("text").and_then(Value::as_str))
                            .map(str::trim)
                            .filter(|text| !text.is_empty())
                            .collect::<Vec<_>>();
                        if !collected.is_empty() {
                            return Some(collected.join("\n"));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    None
}

fn resolve_ffmpeg_command() -> Option<PathBuf> {
    resolve_ffmpeg_command_from_env_and_candidates(
        &["WORKCLAW_FFMPEG_PATH", "FFMPEG_PATH"],
        &collect_ffmpeg_command_candidates(),
    )
}

fn resolve_ffmpeg_command_from_env_and_candidates(
    env_keys: &[&str],
    candidates: &[PathBuf],
) -> Option<PathBuf> {
    let mut ordered = Vec::new();
    for key in env_keys {
        if let Some(value) = std::env::var_os(key).filter(|value| !value.is_empty()) {
            ordered.push(PathBuf::from(value));
        }
    }
    ordered.extend(candidates.iter().cloned());
    ordered
        .into_iter()
        .find(|candidate| probe_command_available(candidate))
}

fn probe_command_available(command: &PathBuf) -> bool {
    Command::new(command)
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn collect_ffmpeg_command_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from("ffmpeg"), PathBuf::from("ffmpeg.exe")];
    #[cfg(target_os = "windows")]
    {
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            let base = PathBuf::from(program_files);
            candidates.push(base.join("ffmpeg").join("bin").join("ffmpeg.exe"));
            candidates.push(base.join("FFmpeg").join("bin").join("ffmpeg.exe"));
        }
        if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
            let base = PathBuf::from(program_files_x86);
            candidates.push(base.join("ffmpeg").join("bin").join("ffmpeg.exe"));
            candidates.push(base.join("FFmpeg").join("bin").join("ffmpeg.exe"));
        }
        if let Some(chocolatey) = std::env::var_os("ChocolateyInstall") {
            let base = PathBuf::from(chocolatey);
            candidates.push(base.join("bin").join("ffmpeg.exe"));
        }
        if let Some(local_app_data) = std::env::var_os("LocalAppData") {
            let base = PathBuf::from(local_app_data);
            candidates.push(
                base.join("Microsoft")
                    .join("WinGet")
                    .join("Links")
                    .join("ffmpeg.exe"),
            );
            candidates.push(
                base.join("Programs")
                    .join("ffmpeg")
                    .join("bin")
                    .join("ffmpeg.exe"),
            );
        }
        if let Some(user_profile) = std::env::var_os("USERPROFILE") {
            let base = PathBuf::from(user_profile);
            candidates.push(base.join("scoop").join("shims").join("ffmpeg.exe"));
        }
    }
    candidates
}

fn build_audio_transcription_multipart_body(
    boundary: &str,
    file_name: &str,
    mime_type: &str,
    model_name: &str,
    bytes: &[u8],
) -> Vec<u8> {
    let mut body = Vec::with_capacity(bytes.len() + 512);
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n{model_name}\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: {mime_type}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

fn collect_attachment_inputs(parts: &[SendMessagePart]) -> Vec<AttachmentInput> {
    parts
        .iter()
        .filter_map(project_part_to_attachment_input)
        .collect()
}

fn project_part_to_attachment_input(part: &SendMessagePart) -> Option<AttachmentInput> {
    match part {
        SendMessagePart::Text { .. } => None,
        SendMessagePart::Attachment { attachment } => Some(attachment.clone()),
        SendMessagePart::Image {
            name,
            mime_type,
            size,
            data,
        } => Some(AttachmentInput {
            id: format!("legacy-image-{name}"),
            kind: "image".to_string(),
            source_type: "browser_file".to_string(),
            name: name.clone(),
            declared_mime_type: Some(mime_type.clone()),
            size_bytes: Some(*size),
            source_payload: Some(data.clone()),
            source_uri: None,
            extracted_text: None,
            truncated: None,
        }),
        SendMessagePart::FileText {
            name,
            mime_type,
            size,
            text,
            truncated,
        } => Some(AttachmentInput {
            id: format!("legacy-file-{name}"),
            kind: "document".to_string(),
            source_type: "browser_file".to_string(),
            name: name.clone(),
            declared_mime_type: Some(mime_type.clone()),
            size_bytes: Some(*size),
            source_payload: Some(text.clone()),
            source_uri: None,
            extracted_text: Some(text.clone()),
            truncated: Some(truncated.unwrap_or(false)),
        }),
        SendMessagePart::PdfFile {
            name,
            mime_type,
            size,
            data,
        } => Some(AttachmentInput {
            id: format!("legacy-pdf-{name}"),
            kind: "document".to_string(),
            source_type: "browser_file".to_string(),
            name: name.clone(),
            declared_mime_type: Some(mime_type.clone()),
            size_bytes: Some(*size),
            source_payload: Some(data.clone()),
            source_uri: None,
            extracted_text: None,
            truncated: None,
        }),
    }
}

fn normalize_message_part(
    part: &SendMessagePart,
    policy: &super::chat_attachment_policy::AttachmentPolicy,
) -> Result<Value, String> {
    match part {
        SendMessagePart::Text { text } => Ok(json!({
            "type": "text",
            "text": text,
        })),
        SendMessagePart::Attachment { attachment } => normalize_attachment_part(policy, attachment),
        SendMessagePart::Image {
            name,
            mime_type,
            size,
            data,
        } => Ok(json!({
            "type": "image",
            "name": name,
            "mimeType": mime_type,
            "size": size,
            "data": data,
        })),
        SendMessagePart::FileText {
            name,
            mime_type,
            size,
            text,
            truncated,
        } => Ok(json!({
            "type": "file_text",
            "name": name,
            "mimeType": mime_type,
            "size": size,
            "text": text,
            "truncated": truncated.unwrap_or(false),
        })),
        SendMessagePart::PdfFile {
            name,
            mime_type,
            size,
            data,
        } => {
            let (extracted_text, truncated) =
                extract_pdf_text(data, PHASE_ONE_MAX_PDF_EXTRACTED_TEXT_CHARS)
                    .map_err(|err| format!("PDF 文件 {name} 解析失败: {err}"))?;
            Ok(json!({
                "type": "pdf_file",
                "name": name,
                "mimeType": mime_type,
                "size": size,
                "extractedText": extracted_text,
                "truncated": truncated,
            }))
        }
    }
}

fn normalize_attachment_part(
    policy: &super::chat_attachment_policy::AttachmentPolicy,
    attachment: &AttachmentInput,
) -> Result<Value, String> {
    let resolved = resolve_attachment_input(policy, attachment)?;
    normalize_resolved_attachment(policy, &resolved)
}

fn normalize_resolved_attachment(
    policy: &super::chat_attachment_policy::AttachmentPolicy,
    attachment: &ResolvedAttachment,
) -> Result<Value, String> {
    match attachment.kind.as_str() {
        "image" => {
            let data = attachment
                .source_payload
                .as_deref()
                .ok_or_else(|| format!("图片附件 {} 缺少 sourcePayload", attachment.name))?;
            Ok(json!({
                "type": "image",
                "name": attachment.name,
                "mimeType": attachment.resolved_mime_type,
                "size": attachment.size_bytes.unwrap_or(0),
                "data": data,
            }))
        }
        "document" => normalize_resolved_document_attachment(policy, attachment),
        "audio" | "video" => Ok(json!({
            "type": "attachment",
            "attachment": {
                "id": attachment.id,
                "kind": attachment.kind,
                "sourceType": attachment.source_type,
                "name": attachment.name,
                "mimeType": attachment.resolved_mime_type,
                "sizeBytes": attachment.size_bytes,
                "transcript": attachment.transcript,
                "summary": attachment.summary,
                "warnings": attachment.warnings,
            }
        })),
        other => Err(format!("暂不支持 {other} 类型附件 {}", attachment.name)),
    }
}

fn normalize_resolved_document_attachment(
    policy: &super::chat_attachment_policy::AttachmentPolicy,
    attachment: &ResolvedAttachment,
) -> Result<Value, String> {
    if attachment.is_pdf {
        let (extracted_text, truncated) =
            if let Some(extracted_text) = attachment.extracted_text.as_ref() {
                truncate_text_excerpt(extracted_text, policy.document.max_extracted_text_chars)
            } else {
                let data = attachment
                    .source_payload
                    .as_deref()
                    .ok_or_else(|| format!("PDF 附件 {} 缺少 sourcePayload", attachment.name))?;
                extract_pdf_text(data, policy.document.max_extracted_text_chars)
                    .map_err(|err| format!("PDF 文件 {} 解析失败: {err}", attachment.name))?
            };
        return Ok(json!({
            "type": "pdf_file",
            "name": attachment.name,
            "mimeType": attachment.resolved_mime_type,
            "size": attachment.size_bytes.unwrap_or(0),
            "extractedText": extracted_text,
            "truncated": truncated,
        }));
    }

    if !attachment_is_text_document(&AttachmentInput {
        id: attachment.id.clone(),
        kind: attachment.kind.clone(),
        source_type: attachment.source_type.clone(),
        name: attachment.name.clone(),
        declared_mime_type: Some(attachment.resolved_mime_type.clone()),
        size_bytes: attachment.size_bytes,
        source_payload: attachment.source_payload.clone(),
        source_uri: attachment.source_uri.clone(),
        extracted_text: attachment.extracted_text.clone(),
        truncated: Some(attachment.truncated),
    }) {
        if let Some(payload) = attachment.source_payload.as_deref() {
            if let Some((extracted_text, truncated)) = extract_binary_document_text(
                payload,
                &attachment.resolved_mime_type,
                &attachment.name,
                policy.document.max_extracted_text_chars,
            )? {
                return Ok(json!({
                    "type": "file_text",
                    "name": attachment.name,
                    "mimeType": attachment.resolved_mime_type,
                    "size": attachment.size_bytes.unwrap_or(0),
                    "text": extracted_text,
                    "truncated": truncated,
                }));
            }
        }
        return Ok(json!({
            "type": "attachment",
            "attachment": {
                "id": attachment.id,
                "kind": attachment.kind,
                "sourceType": attachment.source_type,
                "name": attachment.name,
                "mimeType": attachment.resolved_mime_type,
                "sizeBytes": attachment.size_bytes,
                "summary": attachment.summary,
                "warnings": attachment.warnings,
            }
        }));
    }

    let text = attachment
        .source_payload
        .as_deref()
        .or(attachment.extracted_text.as_deref())
        .ok_or_else(|| {
            format!(
                "文档附件 {} 缺少 sourcePayload 或 extractedText",
                attachment.name
            )
        })?;
    let (text, text_truncated) =
        truncate_plain_text_excerpt(text, policy.document.max_extracted_text_chars);
    let truncated = attachment.truncated || text_truncated;
    Ok(json!({
        "type": "file_text",
        "name": attachment.name,
        "mimeType": attachment.resolved_mime_type,
        "size": attachment.size_bytes.unwrap_or(0),
        "text": text,
        "truncated": truncated,
    }))
}

fn truncate_text_excerpt(content: &str, max_chars: usize) -> (String, bool) {
    let normalized = content
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    if normalized.is_empty() {
        return ("未提取到可读的 PDF 文本内容。".to_string(), false);
    }

    let mut iter = normalized.chars();
    let excerpt: String = iter.by_ref().take(max_chars).collect();
    let truncated = iter.next().is_some();
    (excerpt, truncated)
}

fn truncate_plain_text_excerpt(content: &str, max_chars: usize) -> (String, bool) {
    let normalized = content
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    if normalized.is_empty() {
        return (String::new(), false);
    }

    let mut iter = normalized.chars();
    let excerpt: String = iter.by_ref().take(max_chars).collect();
    let truncated = iter.next().is_some();
    (excerpt, truncated)
}

fn extract_pdf_text(data: &str, max_chars: usize) -> Result<(String, bool), String> {
    let payload = data
        .split_once("base64,")
        .map(|(_, payload)| payload)
        .unwrap_or(data);
    let bytes = BASE64.decode(payload).map_err(|err| err.to_string())?;
    let extracted = pdf_extract::extract_text_from_mem(&bytes).map_err(|err| err.to_string())?;
    let normalized = extracted
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    Ok(truncate_text_excerpt(&normalized, max_chars))
}

fn decode_base64_payload_bytes(payload: &str) -> Result<Vec<u8>, String> {
    let encoded = payload
        .split_once("base64,")
        .map(|(_, payload)| payload)
        .unwrap_or(payload);
    BASE64.decode(encoded).map_err(|err| err.to_string())
}

fn extract_binary_document_text(
    payload: &str,
    mime_type: &str,
    name: &str,
    max_chars: usize,
) -> Result<Option<(String, bool)>, String> {
    let normalized_mime = mime_type.trim().to_ascii_lowercase();
    let extension = name
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let bytes = decode_base64_payload_bytes(payload)?;

    let extracted = if normalized_mime
        == "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        || extension == "docx"
    {
        Some(extract_docx_text_from_bytes(&bytes)?)
    } else if normalized_mime == "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        || extension == "xlsx"
    {
        Some(extract_xlsx_text_from_bytes(&bytes)?)
    } else if normalized_mime == "application/msword" || extension == "doc" {
        extract_legacy_office_text_from_bytes(&bytes)
    } else if normalized_mime == "application/vnd.ms-excel" || extension == "xls" {
        extract_legacy_office_text_from_bytes(&bytes)
    } else {
        None
    };

    Ok(extracted.map(|text| truncate_text_excerpt(&text, max_chars)))
}

fn decode_xml_entities(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn strip_xml_tags(value: &str) -> String {
    let mut plain = String::with_capacity(value.len());
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => plain.push(ch),
            _ => {}
        }
    }
    decode_xml_entities(&plain)
}

fn extract_docx_text_from_bytes(bytes: &[u8]) -> Result<String, String> {
    let cursor = Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|err| format!("解析 DOCX 文件失败: {err}"))?;
    let mut document = archive
        .by_name("word/document.xml")
        .map_err(|err| format!("读取 DOCX 文档内容失败: {err}"))?;
    let mut xml = String::new();
    document
        .read_to_string(&mut xml)
        .map_err(|err| format!("读取 DOCX 文本失败: {err}"))?;

    let with_breaks = xml
        .replace("<w:tab/>", "\t")
        .replace("<w:tab />", "\t")
        .replace("<w:br/>", "\n")
        .replace("<w:br />", "\n")
        .replace("</w:p>", "\n");
    let decoded = strip_xml_tags(&with_breaks);
    let lines = decoded
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    Ok(lines.join("\n"))
}

fn read_zip_entry_as_string<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<String, String> {
    let mut entry = archive
        .by_name(name)
        .map_err(|err| format!("读取压缩文档内容失败 {name}: {err}"))?;
    let mut text = String::new();
    entry
        .read_to_string(&mut text)
        .map_err(|err| format!("读取压缩文档文本失败 {name}: {err}"))?;
    Ok(text)
}

fn extract_xlsx_text_from_bytes(bytes: &[u8]) -> Result<String, String> {
    let cursor = Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|err| format!("解析 XLSX 文件失败: {err}"))?;
    let workbook_xml = read_zip_entry_as_string(&mut archive, "xl/workbook.xml")?;
    let workbook_rels_xml = read_zip_entry_as_string(&mut archive, "xl/_rels/workbook.xml.rels")?;
    let shared_strings_xml = archive
        .by_name("xl/sharedStrings.xml")
        .ok()
        .and_then(|mut entry| {
            let mut text = String::new();
            entry.read_to_string(&mut text).ok().map(|_| text)
        });

    let relation_re = Regex::new(r#"<Relationship[^>]*Id="([^"]+)"[^>]*Target="([^"]+)""#)
        .map_err(|e| e.to_string())?;
    let sheet_re =
        Regex::new(r#"<sheet[^>]*name="([^"]+)"[^>]*r:id="([^"]+)""#).map_err(|e| e.to_string())?;
    let cell_re = Regex::new(r#"(?s)<c[^>]*r="([^"]+)"[^>]*?(?:t="([^"]+)")?[^>]*>(.*?)</c>"#)
        .map_err(|e| e.to_string())?;
    let value_re =
        Regex::new(r#"(?s)<v[^>]*>(.*?)</v>|<t[^>]*>(.*?)</t>"#).map_err(|e| e.to_string())?;
    let shared_string_re = Regex::new(r#"(?s)<si[^>]*>(.*?)</si>"#).map_err(|e| e.to_string())?;

    let mut relationship_targets = HashMap::new();
    for captures in relation_re.captures_iter(&workbook_rels_xml) {
        relationship_targets.insert(captures[1].to_string(), captures[2].to_string());
    }

    let shared_strings = shared_strings_xml
        .map(|xml| {
            shared_string_re
                .captures_iter(&xml)
                .map(|captures| {
                    let with_breaks = captures[1]
                        .replace("<t xml:space=\"preserve\">", "<t>")
                        .replace("</si>", "\n");
                    strip_xml_tags(&with_breaks).trim().to_string()
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut sheet_texts = Vec::new();
    for captures in sheet_re.captures_iter(&workbook_xml) {
        let sheet_name = decode_xml_entities(&captures[1]);
        let relation_id = captures[2].to_string();
        let Some(target) = relationship_targets.get(&relation_id) else {
            continue;
        };
        let normalized_target = if target.starts_with('/') {
            target.trim_start_matches('/').to_string()
        } else {
            format!("xl/{target}")
        };
        let sheet_xml = read_zip_entry_as_string(&mut archive, &normalized_target)?;
        let mut lines = vec![format!("工作表: {sheet_name}")];

        for cell_capture in cell_re.captures_iter(&sheet_xml) {
            let cell_ref = cell_capture[1].to_string();
            let cell_type = cell_capture.get(2).map(|m| m.as_str()).unwrap_or_default();
            let cell_body = cell_capture.get(3).map(|m| m.as_str()).unwrap_or_default();
            let Some(value_capture) = value_re.captures(cell_body) else {
                continue;
            };
            let raw_value = value_capture
                .get(1)
                .or_else(|| value_capture.get(2))
                .map(|m| m.as_str())
                .unwrap_or_default()
                .trim();
            if raw_value.is_empty() {
                continue;
            }
            let resolved_value = if cell_type == "s" {
                raw_value
                    .parse::<usize>()
                    .ok()
                    .and_then(|index| shared_strings.get(index).cloned())
                    .unwrap_or_else(|| raw_value.to_string())
            } else {
                decode_xml_entities(raw_value)
            };
            if resolved_value.trim().is_empty() {
                continue;
            }
            lines.push(format!("{cell_ref}: {}", resolved_value.trim()));
        }

        sheet_texts.push(lines.join("\n"));
    }

    Ok(sheet_texts.join("\n\n"))
}

fn extract_legacy_office_text_from_bytes(bytes: &[u8]) -> Option<String> {
    let mut candidates = BTreeSet::new();

    for text in extract_utf16le_strings(bytes) {
        candidates.insert(text);
    }
    for text in extract_ascii_strings(bytes) {
        candidates.insert(text);
    }

    let lines = candidates
        .into_iter()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.chars().count() >= 2
                && trimmed
                    .chars()
                    .any(|ch| ch.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(&ch))
        })
        .collect::<Vec<_>>();

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn extract_utf16le_strings(bytes: &[u8]) -> Vec<String> {
    let mut results = Vec::new();
    let mut current = String::new();
    let mut index = 0usize;

    while index + 1 < bytes.len() {
        let value = u16::from_le_bytes([bytes[index], bytes[index + 1]]);
        if let Some(ch) = char::from_u32(u32::from(value)) {
            let keep = if ch == '\n' || ch == '\r' || ch == '\t' {
                true
            } else {
                !ch.is_control()
            };
            if keep {
                current.push(ch);
            } else if current.trim().chars().count() >= 2 {
                results.push(current.trim().to_string());
                current.clear();
            } else {
                current.clear();
            }
        } else if current.trim().chars().count() >= 2 {
            results.push(current.trim().to_string());
            current.clear();
        } else {
            current.clear();
        }
        index += 2;
    }

    if current.trim().chars().count() >= 2 {
        results.push(current.trim().to_string());
    }

    results
}

fn extract_ascii_strings(bytes: &[u8]) -> Vec<String> {
    let mut results = Vec::new();
    let mut current = String::new();

    for byte in bytes {
        let ch = char::from(*byte);
        let keep = ch.is_ascii_alphanumeric()
            || matches!(
                ch,
                ' ' | '_' | '-' | ':' | '/' | '.' | ',' | '(' | ')' | '[' | ']'
            );
        if keep {
            current.push(ch);
        } else if current.trim().len() >= 4 {
            results.push(current.trim().to_string());
            current.clear();
        } else {
            current.clear();
        }
    }

    if current.trim().len() >= 4 {
        results.push(current.trim().to_string());
    }

    results
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_message_parts, normalize_message_parts_with_pool, parse_audio_transcription_body,
        resolve_ffmpeg_command_from_env_and_candidates, supports_audio_stt_provider_candidate,
    };
    use crate::commands::chat::SendMessagePart;
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    use sqlx::SqlitePool;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn build_probeable_test_command(name: &str) -> PathBuf {
        let temp = tempdir().expect("tempdir");
        let dir = temp.keep();
        #[cfg(target_os = "windows")]
        let path = dir.join(format!("{name}.cmd"));
        #[cfg(not(target_os = "windows"))]
        let path = dir.join(name);

        #[cfg(target_os = "windows")]
        fs::write(&path, "@echo off\r\nexit /b 0\r\n").expect("write command");
        #[cfg(not(target_os = "windows"))]
        {
            use std::os::unix::fs::PermissionsExt;

            fs::write(&path, "#!/bin/sh\nexit 0\n").expect("write command");
            let mut perms = fs::metadata(&path).expect("metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).expect("chmod");
        }

        path
    }

    async fn setup_audio_route_test_db_with_provider(
        provider_key: &str,
        protocol_type: &str,
        base_url: Option<&str>,
        model_name: &str,
    ) -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("connect test sqlite");
        sqlx::query(
            "CREATE TABLE routing_policies (
                capability TEXT PRIMARY KEY,
                primary_provider_id TEXT NOT NULL,
                primary_model TEXT NOT NULL DEFAULT '',
                fallback_chain_json TEXT NOT NULL DEFAULT '[]',
                timeout_ms INTEGER NOT NULL DEFAULT 60000,
                retry_count INTEGER NOT NULL DEFAULT 0,
                enabled INTEGER NOT NULL DEFAULT 1
            )",
        )
        .execute(&pool)
        .await
        .expect("create routing_policies");
        sqlx::query(
            "CREATE TABLE provider_configs (
                id TEXT PRIMARY KEY,
                provider_key TEXT NOT NULL,
                protocol_type TEXT NOT NULL,
                base_url TEXT NOT NULL,
                api_key_encrypted TEXT NOT NULL DEFAULT '',
                enabled INTEGER NOT NULL DEFAULT 1
            )",
        )
        .execute(&pool)
        .await
        .expect("create provider_configs");

        if let Some(base_url) = base_url {
            sqlx::query(
                "INSERT INTO routing_policies (capability, primary_provider_id, primary_model, fallback_chain_json, timeout_ms, retry_count, enabled)
                 VALUES ('audio_stt', 'provider-audio', ?, '[]', 60000, 0, 1)",
            )
            .bind(model_name)
            .execute(&pool)
            .await
            .expect("insert routing policy");
            sqlx::query(
                "INSERT INTO provider_configs (id, provider_key, protocol_type, base_url, api_key_encrypted, enabled)
                 VALUES ('provider-audio', ?, ?, ?, 'sk-test', 1)",
            )
            .bind(provider_key)
            .bind(protocol_type)
            .bind(base_url)
            .execute(&pool)
            .await
            .expect("insert provider");
        }

        pool
    }

    async fn setup_audio_route_test_db(base_url: Option<&str>) -> SqlitePool {
        setup_audio_route_test_db_with_provider(
            "openai",
            "openai",
            base_url,
            "gpt-4o-mini-transcribe",
        )
        .await
    }

    fn build_minimal_pdf_with_text(text: &str) -> Vec<u8> {
        let mut pdf = String::from("%PDF-1.4\n");
        let mut offsets = Vec::new();
        let objects = [
            "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_string(),
            "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_string(),
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 144] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n".to_string(),
            {
                let escaped = text.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
                let stream = format!("BT\n/F1 24 Tf\n72 72 Td\n({escaped}) Tj\nET");
                format!(
                    "4 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n",
                    stream.len(),
                    stream
                )
            },
            "5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_string(),
        ];

        for object in &objects {
            offsets.push(pdf.len());
            pdf.push_str(object);
        }

        let xref_start = pdf.len();
        pdf.push_str("xref\n0 6\n0000000000 65535 f \n");
        for offset in offsets {
            pdf.push_str(&format!("{offset:010} 00000 n \n"));
        }
        pdf.push_str("trailer\n<< /Root 1 0 R /Size 6 >>\n");
        pdf.push_str(&format!("startxref\n{xref_start}\n%%EOF"));
        pdf.into_bytes()
    }

    #[test]
    fn normalize_message_parts_extracts_pdf_payload_to_text() {
        let pdf_data = BASE64.encode(build_minimal_pdf_with_text("Hello PDF"));
        let parts = normalize_message_parts(&[SendMessagePart::PdfFile {
            name: "brief.pdf".to_string(),
            mime_type: "application/pdf".to_string(),
            size: 652,
            data: pdf_data,
        }])
        .expect("normalize");

        assert_eq!(parts[0]["type"].as_str(), Some("pdf_file"));
        assert_eq!(parts[0]["name"].as_str(), Some("brief.pdf"));
        assert!(parts[0]["extractedText"]
            .as_str()
            .expect("extracted text")
            .contains("Hello PDF"));
    }

    #[test]
    fn parse_audio_transcription_body_accepts_json_and_plain_text() {
        assert_eq!(
            parse_audio_transcription_body(r#"{"text":"会议纪要"}"#).as_deref(),
            Some("会议纪要")
        );
        assert_eq!(
            parse_audio_transcription_body("直接文本返回").as_deref(),
            Some("直接文本返回")
        );
    }

    #[tokio::test]
    async fn normalize_message_parts_with_pool_transcribes_audio_when_audio_route_exists() {
        let pool = setup_audio_route_test_db(Some("http://mock-audio-stt-success")).await;
        let parts = normalize_message_parts_with_pool(
            &[SendMessagePart::Attachment {
                attachment: crate::commands::chat::AttachmentInput {
                    id: "att-audio-1".to_string(),
                    kind: "audio".to_string(),
                    source_type: "browser_file".to_string(),
                    name: "memo.mp3".to_string(),
                    declared_mime_type: Some("audio/mpeg".to_string()),
                    size_bytes: Some(128),
                    source_payload: Some("ZmFrZQ==".to_string()),
                    source_uri: None,
                    extracted_text: None,
                    truncated: None,
                },
            }],
            &pool,
        )
        .await
        .expect("normalize audio attachment");

        assert_eq!(parts[0]["type"].as_str(), Some("attachment"));
        assert_eq!(
            parts[0]["attachment"]["transcript"].as_str(),
            Some("MOCK_TRANSCRIPT: memo.mp3")
        );
        assert_eq!(
            parts[0]["attachment"]["warnings"]
                .as_array()
                .expect("warnings")
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn normalize_message_parts_with_pool_keeps_pending_audio_when_audio_route_missing() {
        let pool = setup_audio_route_test_db(None).await;
        let parts = normalize_message_parts_with_pool(
            &[SendMessagePart::Attachment {
                attachment: crate::commands::chat::AttachmentInput {
                    id: "att-audio-2".to_string(),
                    kind: "audio".to_string(),
                    source_type: "browser_file".to_string(),
                    name: "call.mp3".to_string(),
                    declared_mime_type: Some("audio/mpeg".to_string()),
                    size_bytes: Some(128),
                    source_payload: Some("ZmFrZQ==".to_string()),
                    source_uri: None,
                    extracted_text: None,
                    truncated: None,
                },
            }],
            &pool,
        )
        .await
        .expect("normalize pending audio attachment");

        assert_eq!(
            parts[0]["attachment"]["transcript"].as_str(),
            Some("TRANSCRIPTION_REQUIRED")
        );
        assert!(parts[0]["attachment"]["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning.as_str() == Some("transcription_pending")));
    }

    #[tokio::test]
    async fn normalize_message_parts_with_pool_accepts_qwen_openai_compatible_audio_route() {
        let pool = setup_audio_route_test_db_with_provider(
            "qwen",
            "openai",
            Some("http://mock-audio-stt-success"),
            "paraformer-v2",
        )
        .await;
        let parts = normalize_message_parts_with_pool(
            &[SendMessagePart::Attachment {
                attachment: crate::commands::chat::AttachmentInput {
                    id: "att-audio-qwen-1".to_string(),
                    kind: "audio".to_string(),
                    source_type: "browser_file".to_string(),
                    name: "meeting.wav".to_string(),
                    declared_mime_type: Some("audio/wav".to_string()),
                    size_bytes: Some(128),
                    source_payload: Some("ZmFrZQ==".to_string()),
                    source_uri: None,
                    extracted_text: None,
                    truncated: None,
                },
            }],
            &pool,
        )
        .await
        .expect("normalize qwen audio attachment");

        assert_eq!(
            parts[0]["attachment"]["transcript"].as_str(),
            Some("MOCK_TRANSCRIPT: meeting.wav")
        );
    }

    #[test]
    fn supports_audio_stt_provider_candidate_rejects_anthropic_routes() {
        assert!(!supports_audio_stt_provider_candidate(
            "anthropic",
            "https://api.anthropic.com",
            "anthropic",
            "sk-test",
        ));
        assert!(supports_audio_stt_provider_candidate(
            "openai",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "qwen",
            "sk-test",
        ));
        assert!(supports_audio_stt_provider_candidate(
            "",
            "https://api.openai.com/v1",
            "custom-openai",
            "sk-test",
        ));
    }

    #[test]
    fn resolve_ffmpeg_command_prefers_env_override_when_valid() {
        let candidate = build_probeable_test_command("ffmpeg-env");
        std::env::set_var("WORKCLAW_TEST_FFMPEG_ENV", &candidate);
        let resolved = resolve_ffmpeg_command_from_env_and_candidates(
            &["WORKCLAW_TEST_FFMPEG_ENV"],
            &[PathBuf::from("ffmpeg-does-not-exist")],
        );
        std::env::remove_var("WORKCLAW_TEST_FFMPEG_ENV");

        assert_eq!(resolved.as_deref(), Some(candidate.as_path()));
    }

    #[test]
    fn resolve_ffmpeg_command_uses_candidates_when_env_missing() {
        let candidate = build_probeable_test_command("ffmpeg-candidate");
        let resolved = resolve_ffmpeg_command_from_env_and_candidates(
            &["WORKCLAW_TEST_FFMPEG_MISSING"],
            &[candidate.clone()],
        );

        assert_eq!(resolved.as_deref(), Some(candidate.as_path()));
    }
}
