use serde_json::{json, Value};

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
            result.push(json!({"role": "assistant", "content": content_val, "tool_calls": tc_arr}));

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

pub(crate) fn extract_new_messages_after_reconstructed_history<'a>(
    final_messages: &'a [Value],
    reconstructed_history_len: usize,
) -> Vec<&'a Value> {
    final_messages
        .iter()
        .skip(reconstructed_history_len)
        .collect()
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
                        return reconstruct_llm_messages(&parsed, api_format);
                    }
                }
            }
            if role == "user" {
                if let Some(content_json) = content_json {
                    if let Ok(parts) = serde_json::from_str::<Value>(content_json) {
                        if let Some(parts_array) = parts.as_array() {
                            if let Some(message) =
                                crate::commands::chat_send_message_flow::build_current_turn_message(
                                    api_format,
                                    parts_array,
                                )
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

pub(crate) fn build_assistant_content_from_final_messages(
    final_messages: &[Value],
    reconstructed_history_len: usize,
) -> (String, bool, String) {
    let new_messages =
        extract_new_messages_after_reconstructed_history(final_messages, reconstructed_history_len);
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
                    let input_val =
                        serde_json::from_str::<Value>(func["arguments"].as_str().unwrap_or("{}"))
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
                                    item["toolCall"]["output"] = Value::String(output.to_string());
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
        build_assistant_content_from_final_messages(final_messages, reconstructed_history_len);
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

#[cfg(test)]
mod tests {
    use super::{
        build_assistant_content_from_final_messages, build_assistant_content_with_stream_fallback,
        reconstruct_history_messages,
    };
    use serde_json::{json, Value};

    #[test]
    fn stream_fallback_restores_empty_text_response() {
        let final_messages = vec![json!({
            "role": "assistant",
            "content": ""
        })];

        let (final_text, has_tool_calls, content) =
            build_assistant_content_with_stream_fallback(&final_messages, 0, "你好，我在。");

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
            build_assistant_content_from_final_messages(&final_messages, 0);
        assert!(has_tool_calls_before);

        let (final_text, has_tool_calls, content) =
            build_assistant_content_with_stream_fallback(&final_messages, 0, "我查到了结果");

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
            build_assistant_content_from_final_messages(&final_messages, 0);

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
                serde_json::to_string(&json!([
                    { "type": "text", "text": "请分析这些附件" },
                    {
                        "type": "image",
                        "name": "screen.png",
                        "mimeType": "image/png",
                        "data": "data:image/png;base64,aGVsbG8="
                    },
                    {
                        "type": "file_text",
                        "name": "debug.ts",
                        "mimeType": "text/plain",
                        "text": "console.log('hi')"
                    }
                ]))
                .expect("serialize parts"),
            ),
        )];

        let messages = reconstruct_history_messages(&history, "openai");

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
        assert_eq!(content[1]["type"].as_str(), Some("image_url"));
    }
}
