use serde_json::{json, Value};

use super::transcript_policy;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RuntimeTranscript;

impl RuntimeTranscript {
    pub fn new() -> Self {
        Self
    }

    pub(crate) fn build_current_turn_message(api_format: &str, parts: &[Value]) -> Option<Value> {
        if parts.is_empty() {
            return None;
        }
        let mut combined_text_parts = Vec::new();
        let mut content_blocks = Vec::new();

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
                _ => {}
            }
        }

        if let Some(file_context) = Self::build_attachment_context_text(parts) {
            combined_text_parts.push(file_context);
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
                                if let Some(message) =
                                    Self::build_current_turn_message(api_format, parts_array)
                                {
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
    use serde_json::json;

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
}
