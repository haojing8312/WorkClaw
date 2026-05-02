use serde_json::{Value, json};
use std::collections::HashSet;

const SYNTHETIC_TOOL_RESULT_TEXT: &str = "[已执行]";

pub(crate) fn repair_outbound_messages(messages: Vec<Value>, api_format: &str) -> Vec<Value> {
    match api_format {
        "openai" => repair_openai_outbound_messages(messages),
        "anthropic" => repair_anthropic_outbound_messages(messages),
        _ => messages,
    }
}

fn repair_openai_outbound_messages(messages: Vec<Value>) -> Vec<Value> {
    let mut repaired = Vec::new();
    let mut pending_tool_call_ids = Vec::new();
    let mut seen_tool_call_ids = HashSet::new();
    let mut resolved_tool_call_ids = HashSet::new();

    for message in messages {
        match message["role"].as_str().unwrap_or_default() {
            "assistant" => {
                if let Some((next_message, new_tool_call_ids)) =
                    repair_openai_assistant_message(message)
                {
                    pending_tool_call_ids.extend(new_tool_call_ids.iter().cloned());
                    seen_tool_call_ids.extend(new_tool_call_ids);
                    repaired.push(next_message);
                }
            }
            "tool" => {
                let tool_call_id = message["tool_call_id"]
                    .as_str()
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if tool_call_id.is_empty()
                    || !seen_tool_call_ids.contains(&tool_call_id)
                    || resolved_tool_call_ids.contains(&tool_call_id)
                {
                    continue;
                }

                resolved_tool_call_ids.insert(tool_call_id.clone());
                repaired.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "content": normalize_tool_result_content(message.get("content")),
                }));
            }
            _ => repaired.push(message),
        }
    }

    for tool_call_id in pending_tool_call_ids {
        if resolved_tool_call_ids.contains(&tool_call_id) {
            continue;
        }
        repaired.push(json!({
            "role": "tool",
            "tool_call_id": tool_call_id,
            "content": SYNTHETIC_TOOL_RESULT_TEXT,
        }));
    }

    repaired
}

fn repair_openai_assistant_message(message: Value) -> Option<(Value, Vec<String>)> {
    let trimmed_content = message
        .get("content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .map(str::to_string);
    let mut sanitized_tool_calls = Vec::new();
    let mut tool_call_ids = Vec::new();

    for tool_call in message["tool_calls"].as_array().into_iter().flatten() {
        let id = tool_call["id"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string();
        let name = normalize_tool_name(tool_call["function"]["name"].as_str());
        let Some(arguments) = normalize_tool_arguments(&tool_call["function"]["arguments"]) else {
            continue;
        };
        if id.is_empty() || name.is_empty() {
            continue;
        }

        tool_call_ids.push(id.clone());
        sanitized_tool_calls.push(json!({
            "id": id,
            "type": "function",
            "function": {
                "name": name,
                "arguments": arguments,
            }
        }));
    }

    if trimmed_content.is_none() && sanitized_tool_calls.is_empty() {
        return None;
    }

    let mut next = json!({
        "role": "assistant",
        "content": trimmed_content.map(Value::String).unwrap_or(Value::Null),
    });
    if !sanitized_tool_calls.is_empty() {
        next["tool_calls"] = Value::Array(sanitized_tool_calls);
    }

    Some((next, tool_call_ids))
}

fn normalize_tool_name(name: Option<&str>) -> String {
    name.unwrap_or_default().trim().to_string()
}

fn normalize_tool_arguments(arguments: &Value) -> Option<String> {
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

fn normalize_tool_result_content(content: Option<&Value>) -> Value {
    match content {
        Some(Value::String(text)) => Value::String(text.trim().to_string()),
        Some(Value::Null) | None => Value::String(SYNTHETIC_TOOL_RESULT_TEXT.to_string()),
        Some(other) => other.clone(),
    }
}

fn repair_anthropic_outbound_messages(messages: Vec<Value>) -> Vec<Value> {
    let mut repaired = Vec::new();
    let mut pending_tool_use_ids: Vec<String> = Vec::new();

    for message in messages {
        let role = message["role"].as_str().unwrap_or_default();
        match role {
            "assistant" => {
                flush_anthropic_missing_tool_results(&mut repaired, &mut pending_tool_use_ids);
                pending_tool_use_ids = collect_anthropic_tool_use_ids(&message);
                repaired.push(message);
            }
            "user" if !pending_tool_use_ids.is_empty() => {
                repaired.push(repair_anthropic_user_message_with_pending_results(
                    message,
                    &mut pending_tool_use_ids,
                ));
            }
            _ => {
                flush_anthropic_missing_tool_results(&mut repaired, &mut pending_tool_use_ids);
                repaired.push(message);
            }
        }
    }

    flush_anthropic_missing_tool_results(&mut repaired, &mut pending_tool_use_ids);
    repaired
}

fn collect_anthropic_tool_use_ids(message: &Value) -> Vec<String> {
    message["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block["type"].as_str() == Some("tool_use"))
        .filter_map(|block| block["id"].as_str())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn repair_anthropic_user_message_with_pending_results(
    message: Value,
    pending_tool_use_ids: &mut Vec<String>,
) -> Value {
    let mut content = match message["content"].as_array() {
        Some(blocks) => blocks.clone(),
        None => message["content"]
            .as_str()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(|text| vec![json!({"type": "text", "text": text})])
            .unwrap_or_default(),
    };
    let resolved_ids = content
        .iter()
        .filter(|block| block["type"].as_str() == Some("tool_result"))
        .filter_map(|block| block["tool_use_id"].as_str())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToString::to_string)
        .collect::<HashSet<_>>();

    for tool_use_id in pending_tool_use_ids.iter() {
        if !resolved_ids.contains(tool_use_id) {
            content.push(synthetic_anthropic_tool_result(tool_use_id));
        }
    }
    pending_tool_use_ids.clear();

    json!({
        "role": "user",
        "content": content,
    })
}

fn flush_anthropic_missing_tool_results(
    repaired: &mut Vec<Value>,
    pending_tool_use_ids: &mut Vec<String>,
) {
    if pending_tool_use_ids.is_empty() {
        return;
    }
    let content = pending_tool_use_ids
        .iter()
        .map(|tool_use_id| synthetic_anthropic_tool_result(tool_use_id))
        .collect::<Vec<_>>();
    pending_tool_use_ids.clear();
    repaired.push(json!({
        "role": "user",
        "content": content,
    }));
}

fn synthetic_anthropic_tool_result(tool_use_id: &str) -> Value {
    json!({
        "type": "tool_result",
        "tool_use_id": tool_use_id,
        "content": SYNTHETIC_TOOL_RESULT_TEXT,
    })
}

#[cfg(test)]
mod tests {
    use super::repair_outbound_messages;
    use serde_json::{Value, json};

    #[test]
    fn drops_openai_tool_calls_without_name() {
        let messages = vec![json!({
            "role": "assistant",
            "content": Value::Null,
            "tool_calls": [
                {
                    "id": "call-1",
                    "type": "function",
                    "function": {
                        "name": "",
                        "arguments": "{\"path\":\"README.md\"}"
                    }
                }
            ]
        })];

        let repaired = repair_outbound_messages(messages, "openai");

        assert!(
            repaired.is_empty(),
            "assistant message with only malformed tool calls should be dropped"
        );
    }

    #[test]
    fn drops_openai_tool_calls_with_invalid_arguments() {
        let messages = vec![json!({
            "role": "assistant",
            "content": Value::Null,
            "tool_calls": [
                {
                    "id": "call-1",
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "arguments": "{\"path\":\"README.md\""
                    }
                }
            ]
        })];

        let repaired = repair_outbound_messages(messages, "openai");

        assert!(
            repaired.is_empty(),
            "assistant message with malformed tool arguments should be dropped"
        );
    }

    #[test]
    fn drops_orphan_openai_tool_results() {
        let messages = vec![
            json!({"role": "user", "content": "hello"}),
            json!({"role": "tool", "tool_call_id": "call-orphan", "content": "orphan"}),
        ];

        let repaired = repair_outbound_messages(messages, "openai");

        assert_eq!(repaired.len(), 1);
        assert_eq!(repaired[0]["role"].as_str(), Some("user"));
    }

    #[test]
    fn synthesizes_missing_openai_tool_results_for_replay() {
        let messages = vec![json!({
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
        })];

        let repaired = repair_outbound_messages(messages, "openai");

        assert_eq!(repaired.len(), 2);
        assert_eq!(repaired[0]["role"].as_str(), Some("assistant"));
        assert_eq!(repaired[1]["role"].as_str(), Some("tool"));
        assert_eq!(repaired[1]["tool_call_id"].as_str(), Some("call-1"));
    }

    #[test]
    fn preserves_valid_anthropic_tool_use_and_tool_result_pairs() {
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
                    }
                ]
            }),
        ];

        let repaired = repair_outbound_messages(messages.clone(), "anthropic");

        assert_eq!(repaired, messages);
    }
}
