use serde_json::{json, Value};
use std::collections::HashSet;

pub(crate) fn sanitize_reconstructed_messages(
    messages: Vec<Value>,
    api_format: &str,
) -> Vec<Value> {
    let mut seen_tool_call_ids = HashSet::new();
    let mut sanitized = Vec::new();

    for message in messages {
        let role = message["role"].as_str().unwrap_or_default();
        match role {
            "assistant" => {
                let next = if api_format == "anthropic" {
                    sanitize_anthropic_assistant_message(message)
                } else {
                    sanitize_openai_assistant_message(message)
                };
                if let Some(next_message) = next {
                    remember_assistant_tool_call_ids(&next_message, api_format, &mut seen_tool_call_ids);
                    sanitized.push(next_message);
                }
            }
            "tool" if api_format != "anthropic" => {
                let tool_call_id = message["tool_call_id"].as_str().unwrap_or_default();
                if !tool_call_id.is_empty() && seen_tool_call_ids.contains(tool_call_id) {
                    sanitized.push(message);
                }
            }
            "user" if api_format == "anthropic" => {
                if let Some(next_message) =
                    sanitize_anthropic_user_message(message, &seen_tool_call_ids)
                {
                    sanitized.push(next_message);
                }
            }
            _ => sanitized.push(message),
        }
    }

    sanitized
}

fn sanitize_openai_assistant_message(message: Value) -> Option<Value> {
    let mut content = message.get("content").cloned().unwrap_or(Value::Null);
    let trimmed_text = content.as_str().map(str::trim).unwrap_or_default().to_string();
    let mut sanitized_tool_calls = Vec::new();

    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        for tool_call in tool_calls {
            let id = tool_call["id"].as_str().unwrap_or_default().trim();
            let function_name = tool_call["function"]["name"]
                .as_str()
                .unwrap_or_default()
                .trim();
            if id.is_empty() || function_name.is_empty() {
                continue;
            }

            let arguments = match normalize_openai_tool_arguments(&tool_call["function"]["arguments"]) {
                Some(arguments) => arguments,
                None => continue,
            };

            sanitized_tool_calls.push(json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": function_name,
                    "arguments": arguments,
                }
            }));
        }
    }

    if !trimmed_text.is_empty() {
        content = Value::String(trimmed_text);
    } else if !sanitized_tool_calls.is_empty() {
        content = Value::Null;
    }

    if content.is_null() && sanitized_tool_calls.is_empty() {
        return None;
    }

    let mut next = json!({
        "role": "assistant",
        "content": content,
    });
    if !sanitized_tool_calls.is_empty() {
        next["tool_calls"] = Value::Array(sanitized_tool_calls);
    }
    Some(next)
}

fn normalize_openai_tool_arguments(arguments: &Value) -> Option<String> {
    match arguments {
        Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return None;
            }
            serde_json::from_str::<Value>(trimmed).ok()?;
            Some(trimmed.to_string())
        }
        Value::Object(_) | Value::Array(_) => serde_json::to_string(arguments).ok(),
        _ => None,
    }
}

fn sanitize_anthropic_assistant_message(message: Value) -> Option<Value> {
    let mut sanitized_blocks = Vec::new();
    for block in message["content"].as_array().into_iter().flatten() {
        match block["type"].as_str() {
            Some("text") => {
                let text = block["text"].as_str().unwrap_or_default().trim();
                if !text.is_empty() {
                    sanitized_blocks.push(json!({
                        "type": "text",
                        "text": text,
                    }));
                }
            }
            Some("tool_use") => {
                let id = block["id"].as_str().unwrap_or_default().trim();
                let name = block["name"].as_str().unwrap_or_default().trim();
                let input = block.get("input").cloned().unwrap_or(Value::Null);
                if id.is_empty() || name.is_empty() || input.is_null() {
                    continue;
                }
                sanitized_blocks.push(json!({
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": input,
                }));
            }
            _ => sanitized_blocks.push(block.clone()),
        }
    }

    if sanitized_blocks.is_empty() {
        return None;
    }

    Some(json!({
        "role": "assistant",
        "content": sanitized_blocks,
    }))
}

fn sanitize_anthropic_user_message(
    message: Value,
    seen_tool_call_ids: &HashSet<String>,
) -> Option<Value> {
    let Some(content) = message["content"].as_array() else {
        return Some(message);
    };

    let mut sanitized_blocks = Vec::new();
    for block in content {
        if block["type"].as_str() != Some("tool_result") {
            sanitized_blocks.push(block.clone());
            continue;
        }

        let tool_use_id = block["tool_use_id"].as_str().unwrap_or_default().trim();
        if !tool_use_id.is_empty() && seen_tool_call_ids.contains(tool_use_id) {
            sanitized_blocks.push(block.clone());
        }
    }

    if sanitized_blocks.is_empty() {
        return None;
    }

    Some(json!({
        "role": "user",
        "content": sanitized_blocks,
    }))
}

fn remember_assistant_tool_call_ids(
    message: &Value,
    api_format: &str,
    seen_tool_call_ids: &mut HashSet<String>,
) {
    if api_format == "anthropic" {
        for block in message["content"].as_array().into_iter().flatten() {
            if block["type"].as_str() == Some("tool_use") {
                if let Some(id) = block["id"].as_str() {
                    seen_tool_call_ids.insert(id.to_string());
                }
            }
        }
        return;
    }

    for tool_call in message["tool_calls"].as_array().into_iter().flatten() {
        if let Some(id) = tool_call["id"].as_str() {
            seen_tool_call_ids.insert(id.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_reconstructed_messages;
    use serde_json::json;

    #[test]
    fn sanitize_reconstructed_messages_drops_openai_orphan_tool_results() {
        let messages = vec![
            json!({"role": "user", "content": "hello"}),
            json!({"role": "tool", "tool_call_id": "call-orphan", "content": "orphan"}),
            json!({"role": "assistant", "content": "done"}),
        ];

        let sanitized = sanitize_reconstructed_messages(messages, "openai");

        assert_eq!(sanitized.len(), 2);
        assert_eq!(sanitized[0]["role"].as_str(), Some("user"));
        assert_eq!(sanitized[1]["role"].as_str(), Some("assistant"));
    }

    #[test]
    fn sanitize_reconstructed_messages_drops_anthropic_orphan_tool_results() {
        let messages = vec![
            json!({
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "call-1",
                        "name": "read_file",
                        "input": {"path": "README.md"}
                    }
                ]
            }),
            json!({
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "call-1",
                        "content": "ok"
                    },
                    {
                        "type": "tool_result",
                        "tool_use_id": "call-orphan",
                        "content": "orphan"
                    }
                ]
            }),
        ];

        let sanitized = sanitize_reconstructed_messages(messages, "anthropic");

        assert_eq!(sanitized.len(), 2);
        let tool_results = sanitized[1]["content"].as_array().expect("tool results");
        assert_eq!(tool_results.len(), 1);
        assert_eq!(tool_results[0]["tool_use_id"].as_str(), Some("call-1"));
    }

    #[test]
    fn sanitize_reconstructed_messages_normalizes_openai_tool_calls_and_drops_invalid_entries() {
        let messages = vec![
            json!({
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    {
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": {"path": "README.md"}
                        }
                    },
                    {
                        "id": "",
                        "type": "function",
                        "function": {
                            "name": "broken",
                            "arguments": {}
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

        let sanitized = sanitize_reconstructed_messages(messages, "openai");

        assert_eq!(sanitized.len(), 2);
        let tool_calls = sanitized[0]["tool_calls"].as_array().expect("tool calls");
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0]["id"].as_str(), Some("call-1"));
        assert_eq!(
            tool_calls[0]["function"]["arguments"].as_str(),
            Some("{\"path\":\"README.md\"}")
        );
    }
}
