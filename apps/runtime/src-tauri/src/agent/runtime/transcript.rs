use serde_json::{json, Value};

use super::transcript_policy;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RuntimeTranscript;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImagePayloadMode {
    Preserve,
    Placeholder,
}

impl RuntimeTranscript {
    pub fn new() -> Self {
        Self
    }

    pub(crate) fn build_current_turn_message(api_format: &str, parts: &[Value]) -> Option<Value> {
        Self::build_user_message_from_parts(api_format, parts, ImagePayloadMode::Preserve)
    }

    fn build_user_message_from_parts(
        api_format: &str,
        parts: &[Value],
        image_payload_mode: ImagePayloadMode,
    ) -> Option<Value> {
        if parts.is_empty() {
            return None;
        }
        let mut combined_text_parts = Vec::new();
        let mut content_blocks = Vec::new();
        let mut attachment_blocks = Vec::new();

        for part in parts {
            match part.get("type").and_then(Value::as_str).unwrap_or_default() {
                "text" => {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        if !text.trim().is_empty() {
                            combined_text_parts.push(text.trim().to_string());
                        }
                    }
                }
                "image" => {
                    if image_payload_mode == ImagePayloadMode::Placeholder {
                        combined_text_parts.push(Self::historical_image_placeholder(part));
                        continue;
                    }

                    let mime_type = part
                        .get("mimeType")
                        .and_then(Value::as_str)
                        .unwrap_or("image/png");
                    let data = part.get("data").and_then(Value::as_str).unwrap_or_default();
                    if data.is_empty() {
                        continue;
                    }
                    if api_format == "anthropic" {
                        let base64_data = data
                            .split_once("base64,")
                            .map(|(_, payload)| payload)
                            .unwrap_or(data);
                        content_blocks.push(json!({
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": mime_type,
                                "data": base64_data,
                            }
                        }));
                    } else {
                        let data_url = if data.starts_with("data:") {
                            data.to_string()
                        } else {
                            format!("data:{mime_type};base64,{data}")
                        };
                        content_blocks.push(json!({
                            "type": "image_url",
                            "image_url": { "url": data_url }
                        }));
                    }
                }
                "attachment" => {
                    attachment_blocks.push(part.clone());
                    content_blocks.push(part.clone());
                }
                _ => {}
            }
        }

        if let Some(file_context) = Self::build_attachment_context_text(parts) {
            combined_text_parts.push(file_context);
        }
        if let Some(attachment_context) =
            Self::build_attachment_context_text_from_attachment_blocks(&attachment_blocks)
        {
            combined_text_parts.push(attachment_context);
        }
        let combined_text = combined_text_parts.join("\n\n").trim().to_string();
        if !combined_text.is_empty() {
            content_blocks.insert(0, json!({ "type": "text", "text": combined_text }));
        }

        if content_blocks.is_empty() {
            None
        } else {
            Some(json!({
                "role": "user",
                "content": content_blocks,
            }))
        }
    }

    pub(crate) fn reconstruct_llm_messages(parsed: &Value, api_format: &str) -> Vec<Value> {
        let final_text = parsed["text"].as_str().unwrap_or("");
        let items = match parsed["items"].as_array() {
            Some(arr) => arr,
            None => return vec![json!({"role": "assistant", "content": final_text})],
        };

        let mut result = Vec::new();
        let mut tool_calls: Vec<(&Value, Option<&str>)> = Vec::new();
        let mut companion_texts: Vec<String> = Vec::new();

        for item in items {
            match item["type"].as_str() {
                Some("text") => {
                    let text = item["content"].as_str().unwrap_or("");
                    if !text.is_empty() {
                        companion_texts.push(text.to_string());
                    }
                }
                Some("tool_call") => {
                    let tc = if item.get("toolCall").is_some() {
                        &item["toolCall"]
                    } else {
                        item
                    };
                    let output = tc["output"].as_str();
                    tool_calls.push((tc, output));
                }
                _ => {}
            }
        }

        if !tool_calls.is_empty() {
            if api_format == "anthropic" {
                let mut content_blocks: Vec<Value> = Vec::new();
                for text in &companion_texts {
                    content_blocks.push(json!({"type": "text", "text": text}));
                }
                for (tc, _) in &tool_calls {
                    content_blocks.push(json!({
                        "type": "tool_use",
                        "id": tc["id"],
                        "name": tc["name"],
                        "input": tc["input"],
                    }));
                }
                result.push(json!({"role": "assistant", "content": content_blocks}));

                let tool_results: Vec<Value> = tool_calls
                    .iter()
                    .map(|(tc, output)| {
                        json!({
                            "type": "tool_result",
                            "tool_use_id": tc["id"],
                            "content": output.unwrap_or("[已执行]"),
                        })
                    })
                    .collect();
                result.push(json!({"role": "user", "content": tool_results}));
            } else {
                let companion = companion_texts.join("\n");
                let content_val = if companion.is_empty() {
                    Value::Null
                } else {
                    Value::String(companion)
                };
                let tc_arr: Vec<Value> = tool_calls
                    .iter()
                    .map(|(tc, _)| {
                        json!({
                            "id": tc["id"],
                            "type": "function",
                            "function": {
                                "name": tc["name"],
                                "arguments": serde_json::to_string(&tc["input"]).unwrap_or_default(),
                            }
                        })
                    })
                    .collect();
                result.push(
                    json!({"role": "assistant", "content": content_val, "tool_calls": tc_arr}),
                );

                for (tc, output) in &tool_calls {
                    result.push(json!({
                        "role": "tool",
                        "tool_call_id": tc["id"],
                        "content": output.unwrap_or("[已执行]"),
                    }));
                }
            }
        }

        if !final_text.is_empty() {
            result.push(json!({"role": "assistant", "content": final_text}));
        }

        if result.is_empty() {
            result.push(json!({"role": "assistant", "content": ""}));
        }

        result
    }

    pub(crate) fn reconstruct_history_messages(
        history: &[(String, String, Option<String>)],
        api_format: &str,
    ) -> Vec<Value> {
        history
            .iter()
            .flat_map(|(role, content, content_json)| {
                if role == "assistant" {
                    if let Ok(parsed) = serde_json::from_str::<Value>(content) {
                        if parsed.get("text").is_some() && parsed.get("items").is_some() {
                            return Self::reconstruct_llm_messages(&parsed, api_format);
                        }
                        if let Some(text) = parsed.get("text").and_then(Value::as_str) {
                            return vec![json!({"role": "assistant", "content": text})];
                        }
                    }
                }
                if role == "user" {
                    if let Some(content_json) = content_json {
                        if let Ok(parts) = serde_json::from_str::<Value>(content_json) {
                            if let Some(parts_array) = parts.as_array() {
                                if let Some(message) = Self::build_user_message_from_parts(
                                    api_format,
                                    parts_array,
                                    ImagePayloadMode::Placeholder,
                                ) {
                                    return vec![message];
                                }
                            }
                        }
                    }
                }
                vec![json!({"role": role, "content": content})]
            })
            .collect()
    }

    pub(crate) fn sanitize_reconstructed_messages(
        messages: Vec<Value>,
        api_format: &str,
    ) -> Vec<Value> {
        transcript_policy::sanitize_outbound_messages(messages, api_format)
    }

    pub(crate) fn build_assistant_content_from_final_messages(
        final_messages: &[Value],
        reconstructed_history_len: usize,
    ) -> (String, bool, String) {
        let new_messages = Self::extract_new_messages_after_reconstructed_history(
            final_messages,
            reconstructed_history_len,
        );
        let mut ordered_items: Vec<Value> = Vec::new();
        let mut final_text = String::new();

        for msg in &new_messages {
            let role = msg["role"].as_str().unwrap_or("");

            if role == "assistant" {
                if let Some(content_arr) = msg["content"].as_array() {
                    for block in content_arr {
                        match block["type"].as_str() {
                            Some("text") => {
                                let text = block["text"].as_str().unwrap_or("");
                                if !text.is_empty() {
                                    ordered_items.push(json!({"type": "text", "content": text}));
                                }
                            }
                            Some("tool_use") => {
                                ordered_items.push(json!({
                                    "type": "tool_call",
                                    "toolCall": {
                                        "id": block["id"],
                                        "name": block["name"],
                                        "input": block["input"],
                                        "status": "completed"
                                    }
                                }));
                            }
                            _ => {}
                        }
                    }
                } else if let Some(text) = msg["content"].as_str() {
                    if !text.is_empty() {
                        final_text = text.to_string();
                        ordered_items.push(json!({
                            "type": "text",
                            "content": text
                        }));
                    }
                }
                if let Some(tool_calls_arr) = msg["tool_calls"].as_array() {
                    for tc in tool_calls_arr {
                        let func = &tc["function"];
                        let input_val = serde_json::from_str::<Value>(
                            func["arguments"].as_str().unwrap_or("{}"),
                        )
                        .unwrap_or(json!({}));
                        ordered_items.push(json!({
                            "type": "tool_call",
                            "toolCall": {
                                "id": tc["id"],
                                "name": func["name"],
                                "input": input_val,
                                "status": "completed"
                            }
                        }));
                    }
                }
            }

            if role == "user" {
                if let Some(content_arr) = msg["content"].as_array() {
                    for block in content_arr {
                        if block["type"].as_str() == Some("tool_result") {
                            let tool_use_id = block["tool_use_id"].as_str().unwrap_or("");
                            let output = block["content"].as_str().unwrap_or("");
                            for item in ordered_items.iter_mut().rev() {
                                if item["type"].as_str() == Some("tool_call") {
                                    let tc = &item["toolCall"];
                                    if tc["id"].as_str() == Some(tool_use_id)
                                        && tc.get("output").map_or(true, |v| v.is_null())
                                    {
                                        item["toolCall"]["output"] =
                                            Value::String(output.to_string());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if role == "tool" {
                let tool_call_id = msg["tool_call_id"].as_str().unwrap_or("");
                let output = msg["content"].as_str().unwrap_or("");
                for item in ordered_items.iter_mut().rev() {
                    if item["type"].as_str() == Some("tool_call") {
                        let tc = &item["toolCall"];
                        if tc["id"].as_str() == Some(tool_call_id)
                            && tc.get("output").map_or(true, |v| v.is_null())
                        {
                            item["toolCall"]["output"] = Value::String(output.to_string());
                            break;
                        }
                    }
                }
            }
        }

        let has_tool_calls = ordered_items
            .iter()
            .any(|item| item["type"].as_str() == Some("tool_call"));
        let content = if has_tool_calls {
            serde_json::to_string(&json!({
                "text": final_text,
                "items": ordered_items,
            }))
            .unwrap_or(final_text.clone())
        } else {
            final_text.clone()
        };

        (final_text, has_tool_calls, content)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn build_assistant_content_with_stream_fallback(
        final_messages: &[Value],
        reconstructed_history_len: usize,
        streamed_text: &str,
    ) -> (String, bool, String) {
        let (mut final_text, has_tool_calls, mut content) =
            Self::build_assistant_content_from_final_messages(
                final_messages,
                reconstructed_history_len,
            );
        let fallback_text = streamed_text.trim();

        if final_text.trim().is_empty() && !fallback_text.is_empty() {
            final_text = streamed_text.to_string();
            content = if has_tool_calls {
                let parsed = serde_json::from_str::<Value>(&content).unwrap_or_else(|_| {
                    json!({
                        "text": "",
                        "items": [],
                    })
                });
                let items = parsed
                    .get("items")
                    .and_then(|value| value.as_array())
                    .cloned()
                    .unwrap_or_default();
                serde_json::to_string(&json!({
                    "text": final_text,
                    "items": items,
                }))
                .unwrap_or_else(|_| final_text.clone())
            } else {
                final_text.clone()
            };
        }

        (final_text, has_tool_calls, content)
    }

    fn build_attachment_context_text(parts: &[Value]) -> Option<String> {
        let mut file_blocks = Vec::new();
        for part in parts {
            match part.get("type").and_then(Value::as_str) {
                Some("file_text") => {
                    let name = part
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("attachment.txt");
                    let mime_type = part
                        .get("mimeType")
                        .and_then(Value::as_str)
                        .unwrap_or("text/plain");
                    let text = part.get("text").and_then(Value::as_str).unwrap_or_default();
                    let truncated = part
                        .get("truncated")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let ext = name.split('.').last().unwrap_or("txt");
                    let truncated_note = if truncated { "\n[内容已截断]" } else { "" };
                    file_blocks.push(format!(
                        "## {name} ({mime_type})\n```{ext}\n{text}\n```{truncated_note}"
                    ));
                }
                Some("pdf_file") => {
                    let name = part
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("attachment.pdf");
                    let mime_type = part
                        .get("mimeType")
                        .and_then(Value::as_str)
                        .unwrap_or("application/pdf");
                    let text = part
                        .get("extractedText")
                        .and_then(Value::as_str)
                        .unwrap_or("未提取到可读的 PDF 文本内容。");
                    let truncated = part
                        .get("truncated")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let truncated_note = if truncated { "\n[内容已截断]" } else { "" };
                    file_blocks.push(format!(
                        "## PDF 附件 {name} ({mime_type})\n```text\n{text}\n```{truncated_note}"
                    ));
                }
                _ => {}
            }
        }
        if file_blocks.is_empty() {
            None
        } else {
            Some(format!("附件文本文件：\n{}", file_blocks.join("\n\n")))
        }
    }

    fn build_attachment_context_text_from_attachment_blocks(parts: &[Value]) -> Option<String> {
        let mut blocks = Vec::new();
        for part in parts {
            let attachment = match part.get("attachment") {
                Some(value) => value,
                None => continue,
            };
            let name = attachment
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("attachment");
            let kind = attachment
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("attachment");
            let extracted_text = attachment
                .get("extractedText")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            let transcript = attachment
                .get("transcript")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            let summary = attachment
                .get("summary")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            let warnings = attachment
                .get("warnings")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .filter(|value| !value.is_empty());

            let mut body_parts = Vec::new();
            if let Some(text) = extracted_text {
                body_parts.push(format!("提取文本：\n{text}"));
            }
            if let Some(text) = transcript {
                let label = if text == "TRANSCRIPTION_REQUIRED" {
                    "转写状态"
                } else {
                    "转写内容"
                };
                body_parts.push(format!("{label}：{text}"));
            }
            if let Some(text) = summary {
                let pending_value = if kind == "document" {
                    "EXTRACTION_REQUIRED"
                } else {
                    "SUMMARY_REQUIRED"
                };
                let label = if text == pending_value || Self::is_video_summary_status(text) {
                    if kind == "document" {
                        "提取状态"
                    } else {
                        "摘要状态"
                    }
                } else if kind == "document" {
                    "提取内容"
                } else {
                    "摘要内容"
                };
                body_parts.push(format!("{label}：{text}"));
            }
            if let Some(text) = warnings {
                body_parts.push(format!("警告：{text}"));
            }

            if body_parts.is_empty() {
                continue;
            }

            blocks.push(format!(
                "## 附件 {name} ({kind})\n{}",
                body_parts.join("\n\n")
            ));
        }

        if blocks.is_empty() {
            None
        } else {
            Some(format!("附件上下文：\n{}", blocks.join("\n\n")))
        }
    }

    fn historical_image_placeholder(part: &Value) -> String {
        let name = part
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("图片");
        format!("[历史图片 {name} 已从模型上下文移除]")
    }

    fn is_video_summary_status(text: &str) -> bool {
        matches!(
            text,
            "VIDEO_NO_AUDIO_TRACK"
                | "VIDEO_AUDIO_EXTRACTION_UNAVAILABLE"
                | "VIDEO_AUDIO_EXTRACTION_FAILED"
        )
    }

    fn extract_new_messages_after_reconstructed_history<'a>(
        final_messages: &'a [Value],
        reconstructed_history_len: usize,
    ) -> Vec<&'a Value> {
        final_messages
            .iter()
            .skip(reconstructed_history_len)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeTranscript;
    use serde_json::{json, Value};

    #[test]
    fn build_current_turn_message_includes_pdf_attachment_context() {
        let message = RuntimeTranscript::build_current_turn_message(
            "openai",
            &[json!({
                "type": "pdf_file",
                "name": "brief.pdf",
                "mimeType": "application/pdf",
                "extractedText": "这是 PDF 正文",
                "truncated": true
            })],
        )
        .expect("message");

        let text = message["content"][0]["text"].as_str().expect("text block");
        assert!(text.contains("PDF 附件"));
        assert!(text.contains("brief.pdf"));
        assert!(text.contains("这是 PDF 正文"));
        assert!(text.contains("[内容已截断]"));
    }

    #[test]
    fn build_current_turn_message_preserves_attachment_blocks_until_adapter_fallback() {
        let message = RuntimeTranscript::build_current_turn_message(
            "openai",
            &[json!({
                "type": "attachment",
                "attachment": {
                    "id": "att-1",
                    "kind": "document",
                    "name": "brief.pdf",
                    "mimeType": "application/pdf",
                    "sizeBytes": 120,
                    "extractedText": "这是附件正文",
                    "warnings": ["document_truncated"]
                }
            })],
        )
        .expect("message");

        let content = message["content"].as_array().expect("content array");
        assert_eq!(content[0]["type"].as_str(), Some("text"));
        assert!(content[0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("附件上下文"));
        assert_eq!(content[1]["type"].as_str(), Some("attachment"));
        assert_eq!(content[1]["attachment"]["kind"].as_str(), Some("document"));
        assert_eq!(
            content[1]["attachment"]["extractedText"].as_str(),
            Some("这是附件正文")
        );
    }

    #[test]
    fn build_current_turn_message_uses_content_labels_for_completed_audio_and_document_attachments()
    {
        let message = RuntimeTranscript::build_current_turn_message(
            "openai",
            &[
                json!({
                    "type": "attachment",
                    "attachment": {
                        "id": "att-audio-1",
                        "kind": "audio",
                        "name": "memo.mp3",
                        "transcript": "会议结论"
                    }
                }),
                json!({
                    "type": "attachment",
                    "attachment": {
                        "id": "att-doc-1",
                        "kind": "document",
                        "name": "brief.docx",
                        "summary": "已提取文档内容"
                    }
                }),
            ],
        )
        .expect("message");

        let text = message["content"][0]["text"].as_str().expect("text block");
        assert!(text.contains("转写内容：会议结论"));
        assert!(text.contains("提取内容：已提取文档内容"));
    }

    #[test]
    fn build_current_turn_message_keeps_video_status_labels_for_explicit_video_fallback_states() {
        let message = RuntimeTranscript::build_current_turn_message(
            "openai",
            &[json!({
                "type": "attachment",
                "attachment": {
                    "id": "att-video-1",
                    "kind": "video",
                    "name": "silent.mp4",
                    "summary": "VIDEO_NO_AUDIO_TRACK",
                    "warnings": ["video_no_audio_track"]
                }
            })],
        )
        .expect("message");

        let text = message["content"][0]["text"].as_str().expect("text block");
        assert!(text.contains("摘要状态：VIDEO_NO_AUDIO_TRACK"));
        assert!(text.contains("警告：video_no_audio_track"));
    }

    #[test]
    fn build_current_turn_message_preserves_openai_image_url() {
        let message = RuntimeTranscript::build_current_turn_message(
            "openai",
            &[json!({
                "type": "image",
                "name": "screen.png",
                "mimeType": "image/png",
                "data": "aGVsbG8="
            })],
        )
        .expect("message");

        let content = message["content"].as_array().expect("content array");
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"].as_str(), Some("image_url"));
        assert_eq!(
            content[0]["image_url"]["url"].as_str(),
            Some("data:image/png;base64,aGVsbG8=")
        );
    }

    #[test]
    fn stream_fallback_restores_empty_text_response() {
        let final_messages = vec![json!({
            "role": "assistant",
            "content": ""
        })];

        let (final_text, has_tool_calls, content) =
            RuntimeTranscript::build_assistant_content_with_stream_fallback(
                &final_messages,
                0,
                "你好，我在。",
            );

        assert_eq!(final_text, "你好，我在。");
        assert!(!has_tool_calls);
        assert_eq!(content, "你好，我在。");
    }

    #[test]
    fn stream_fallback_preserves_tool_calls_when_text_missing() {
        let final_messages = vec![
            json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [
                    {
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "search",
                            "arguments": "{\"q\":\"minimax\"}"
                        }
                    }
                ]
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call-1",
                "content": "ok"
            }),
        ];

        let (_, has_tool_calls_before, content_before) =
            RuntimeTranscript::build_assistant_content_from_final_messages(&final_messages, 0);
        assert!(has_tool_calls_before);

        let (final_text, has_tool_calls, content) =
            RuntimeTranscript::build_assistant_content_with_stream_fallback(
                &final_messages,
                0,
                "我查到了结果",
            );

        assert_eq!(final_text, "我查到了结果");
        assert!(has_tool_calls);

        let parsed: Value = serde_json::from_str(&content).expect("structured content");
        assert_eq!(parsed["text"].as_str(), Some("我查到了结果"));
        assert_eq!(parsed["items"].as_array().map(|items| items.len()), Some(1));
        assert_eq!(
            parsed["items"][0]["toolCall"]["name"].as_str(),
            Some("search")
        );

        let parsed_before: Value =
            serde_json::from_str(&content_before).expect("structured content before fallback");
        assert_eq!(parsed_before["text"].as_str(), Some(""));
    }

    #[test]
    fn build_assistant_content_from_final_messages_does_not_duplicate_text_when_tool_calls_exist() {
        let final_messages = vec![json!({
            "role": "assistant",
            "content": "让我先检查正确的目录路径。",
            "tool_calls": [
                {
                    "id": "call-1",
                    "type": "function",
                    "function": {
                        "name": "list_dir",
                        "arguments": "{\"path\":\".\"}"
                    }
                }
            ]
        })];

        let (final_text, has_tool_calls, content) =
            RuntimeTranscript::build_assistant_content_from_final_messages(&final_messages, 0);

        assert_eq!(final_text, "让我先检查正确的目录路径。");
        assert!(has_tool_calls);

        let parsed: Value = serde_json::from_str(&content).expect("structured content");
        let items = parsed["items"].as_array().expect("items array");

        assert_eq!(parsed["text"].as_str(), Some("让我先检查正确的目录路径。"));
        assert_eq!(
            items
                .iter()
                .filter(|item| item["type"].as_str() == Some("text"))
                .count(),
            1
        );
        assert_eq!(
            items[0]["content"].as_str(),
            Some("让我先检查正确的目录路径。")
        );
        assert_eq!(items[1]["toolCall"]["name"].as_str(), Some("list_dir"));
    }

    #[test]
    fn reconstruct_history_messages_restores_user_multimodal_parts() {
        let history = vec![(
            "user".to_string(),
            "[图片 1 张] [文本文件 1 个]".to_string(),
            Some(
                serde_json::to_string(&vec![
                    json!({
                        "type": "text",
                        "text": "请分析这些附件"
                    }),
                    json!({
                        "type": "image",
                        "name": "screen.png",
                        "mimeType": "image/png",
                        "data": "data:image/png;base64,aGVsbG8="
                    }),
                    json!({
                        "type": "file_text",
                        "name": "debug.ts",
                        "mimeType": "text/plain",
                        "text": "console.log('hi')"
                    }),
                ])
                .expect("serialize parts"),
            ),
        )];

        let messages = RuntimeTranscript::reconstruct_history_messages(&history, "openai");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"].as_str(), Some("user"));
        let content = messages[0]["content"].as_array().expect("content array");
        assert_eq!(content[0]["type"].as_str(), Some("text"));
        assert!(content[0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("请分析这些附件"));
        assert!(content[0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("debug.ts"));
        assert!(content[0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("[历史图片 screen.png 已从模型上下文移除]"));
        assert!(!content
            .iter()
            .any(|block| block["type"].as_str() == Some("image_url")));
    }

    #[test]
    fn reconstruct_history_messages_replaces_user_images_with_placeholders() {
        let history = vec![(
            "user".to_string(),
            "[图片 2 张]".to_string(),
            Some(
                serde_json::to_string(&vec![
                    json!({
                        "type": "image",
                        "name": " screen.png ",
                        "mimeType": "image/png",
                        "data": "data:image/png;base64,aGVsbG8="
                    }),
                    json!({
                        "type": "image",
                        "name": "   ",
                        "mimeType": "image/jpeg",
                        "data": "data:image/jpeg;base64,d29ybGQ="
                    }),
                ])
                .expect("serialize parts"),
            ),
        )];

        let messages = RuntimeTranscript::reconstruct_history_messages(&history, "openai");

        assert_eq!(messages.len(), 1);
        let content = messages[0]["content"].as_array().expect("content array");
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"].as_str(), Some("text"));
        let text = content[0]["text"].as_str().expect("text");
        assert!(text.contains("[历史图片 screen.png 已从模型上下文移除]"));
        assert!(text.contains("[历史图片 图片 已从模型上下文移除]"));
        assert!(!content
            .iter()
            .any(|block| block["type"].as_str() == Some("image_url")));
        assert!(!content
            .iter()
            .any(|block| block["type"].as_str() == Some("image")));
    }

    #[test]
    fn reconstruct_history_messages_keeps_text_only_user_content_unchanged() {
        let history = vec![(
            "user".to_string(),
            "fallback text".to_string(),
            Some(
                serde_json::to_string(&vec![json!({
                    "type": "text",
                    "text": "请继续分析"
                })])
                .expect("serialize parts"),
            ),
        )];

        let messages = RuntimeTranscript::reconstruct_history_messages(&history, "openai");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"].as_str(), Some("user"));
        let content = messages[0]["content"].as_array().expect("content array");
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"].as_str(), Some("text"));
        assert_eq!(content[0]["text"].as_str(), Some("请继续分析"));
    }

    #[test]
    fn reconstruct_history_messages_restores_structured_assistant_text_without_replaying_json() {
        let history = vec![(
            "assistant".to_string(),
            serde_json::json!({
                "text": "我是 WorkClaw 助手。",
                "reasoning": {
                    "status": "completed",
                    "content": "先自我介绍"
                }
            })
            .to_string(),
            None,
        )];

        let messages = RuntimeTranscript::reconstruct_history_messages(&history, "openai");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"].as_str(), Some("assistant"));
        assert_eq!(
            messages[0]["content"].as_str(),
            Some("我是 WorkClaw 助手。")
        );
    }

    #[test]
    fn runtime_transcript_round_trip_preserves_tool_call_output_pairs() {
        let final_messages = vec![
            json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [
                    {
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"path\":\"README.md\"}"
                        }
                    }
                ]
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call-1",
                "content": "{\"summary\":\"done\"}"
            }),
        ];

        let (_, has_tool_calls, content) =
            RuntimeTranscript::build_assistant_content_from_final_messages(&final_messages, 0);
        assert!(has_tool_calls);

        let parsed: Value = serde_json::from_str(&content).expect("structured transcript");
        let reconstructed = RuntimeTranscript::reconstruct_llm_messages(&parsed, "openai");

        assert!(!reconstructed.is_empty());
        assert_eq!(reconstructed[0]["role"].as_str(), Some("assistant"));
        assert_eq!(reconstructed[1]["role"].as_str(), Some("tool"));
        assert_eq!(
            reconstructed[1]["content"].as_str(),
            Some("{\"summary\":\"done\"}")
        );
    }

    #[test]
    fn runtime_transcript_round_trip_preserves_tool_call_output_pairs_for_anthropic() {
        let final_messages = vec![
            json!({
                "role": "assistant",
                "content": [
                    {
                        "type": "text",
                        "text": "先检查目录。"
                    },
                    {
                        "type": "tool_use",
                        "id": "call-1",
                        "name": "list_dir",
                        "input": {"path": "."}
                    }
                ]
            }),
            json!({
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "call-1",
                        "content": "{\"summary\":\"ok\"}"
                    }
                ]
            }),
        ];

        let (_, has_tool_calls, content) =
            RuntimeTranscript::build_assistant_content_from_final_messages(&final_messages, 0);
        assert!(has_tool_calls);

        let parsed: Value = serde_json::from_str(&content).expect("structured transcript");
        let reconstructed = RuntimeTranscript::reconstruct_llm_messages(&parsed, "anthropic");

        assert_eq!(reconstructed.len(), 2);
        assert_eq!(reconstructed[0]["role"].as_str(), Some("assistant"));
        assert_eq!(
            reconstructed[0]["content"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(
            reconstructed[0]["content"][1]["type"].as_str(),
            Some("tool_use")
        );
        assert_eq!(reconstructed[1]["role"].as_str(), Some("user"));
        assert_eq!(
            reconstructed[1]["content"][0]["type"].as_str(),
            Some("tool_result")
        );
        assert_eq!(
            reconstructed[1]["content"][0]["tool_use_id"].as_str(),
            Some("call-1")
        );
        assert_eq!(
            reconstructed[1]["content"][0]["content"].as_str(),
            Some("{\"summary\":\"ok\"}")
        );
    }
}
