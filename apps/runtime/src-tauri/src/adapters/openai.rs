use crate::agent::types::{LLMResponse, StreamDelta, ToolCall};
use crate::model_transport::{ModelTransportKind, ResolvedModelTransport};
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;

fn mock_response_text(model: &str, messages: &[Value]) -> String {
    let last_user = messages
        .iter()
        .rev()
        .find_map(|message| {
            if message["role"].as_str() == Some("user") {
                if let Some(content) = message["content"].as_str() {
                    return Some(content.trim().to_string()).filter(|content| !content.is_empty());
                }
                message["content"].as_array().and_then(|parts| {
                    parts
                        .iter()
                        .filter_map(|part| part.get("text").and_then(Value::as_str))
                        .map(str::trim)
                        .find(|text| !text.is_empty())
                        .map(str::to_string)
                })
            } else {
                None
            }
        })
        .unwrap_or_else(|| "未提供任务".to_string());
    format!("MOCK_RESPONSE [{}] {}", model, last_user)
}

fn is_mock_text_base_url(base_url: &str) -> bool {
    base_url.trim().eq_ignore_ascii_case("http://mock")
}

fn is_mock_tool_loop_base_url(base_url: &str) -> bool {
    base_url
        .trim()
        .eq_ignore_ascii_case("http://mock-tool-loop")
}

fn is_mock_repeat_invalid_write_file_base_url(base_url: &str) -> bool {
    base_url
        .trim()
        .eq_ignore_ascii_case("http://mock-repeat-invalid-write-file")
}

fn is_mock_write_file_from_user_path_base_url(base_url: &str) -> bool {
    base_url
        .trim()
        .eq_ignore_ascii_case("http://mock-write-file-from-user-path")
}

fn is_mock_responses_read_file_from_user_path_base_url(base_url: &str) -> bool {
    base_url
        .trim()
        .eq_ignore_ascii_case("http://mock-responses-read-file-from-user-path")
}

fn is_mock_responses_malformed_tool_call_start_task_base_url(base_url: &str) -> bool {
    base_url
        .trim()
        .eq_ignore_ascii_case("http://mock-responses-malformed-tool-call-start-task")
}

fn is_mock_repeat_read_file_loop_base_url(base_url: &str) -> bool {
    base_url
        .trim()
        .eq_ignore_ascii_case("http://mock-repeat-read-file-loop")
}

fn is_mock_list_dir_with_interleaved_move_failures_base_url(base_url: &str) -> bool {
    base_url
        .trim()
        .eq_ignore_ascii_case("http://mock-list-dir-interleaved-move-failures")
}

fn count_tool_messages(messages: &[Value]) -> usize {
    messages
        .iter()
        .filter(|message| message["role"].as_str() == Some("tool"))
        .count()
}

fn normalize_message_text(content: &Value) -> Option<String> {
    match content {
        Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Array(parts) => {
            let text = parts
                .iter()
                .filter_map(|part| match part.get("type").and_then(Value::as_str) {
                    Some("text") | Some("input_text") | Some("output_text") => {
                        part.get("text").and_then(Value::as_str)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("");
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        _ => None,
    }
}

fn content_to_responses_parts(content: &Value) -> Vec<Value> {
    match content {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                Vec::new()
            } else {
                vec![json!({ "type": "input_text", "text": trimmed })]
            }
        }
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| match part.get("type").and_then(Value::as_str) {
                Some("text") | Some("input_text") | Some("output_text") => {
                    let text = part.get("text").and_then(Value::as_str)?.trim();
                    (!text.is_empty()).then(|| json!({ "type": "input_text", "text": text }))
                }
                Some("image_url") => {
                    let image_url = part
                        .get("image_url")
                        .and_then(|value| value.get("url"))
                        .and_then(Value::as_str)?;
                    Some(json!({
                        "type": "input_image",
                        "image_url": image_url,
                    }))
                }
                Some("input_image") => Some(part.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn convert_messages_to_responses_input(messages: &[Value]) -> Vec<Value> {
    let mut items = Vec::new();

    for message in messages {
        match message["role"].as_str().unwrap_or_default() {
            "user" => {
                let parts = content_to_responses_parts(&message["content"]);
                if parts.is_empty() {
                    continue;
                }
                items.push(json!({
                    "type": "message",
                    "role": "user",
                    "content": parts,
                }));
            }
            "assistant" => {
                if let Some(text) = normalize_message_text(&message["content"]) {
                    items.push(json!({
                        "type": "message",
                        "role": "assistant",
                        "content": text,
                    }));
                }

                for tool_call in message["tool_calls"].as_array().into_iter().flatten() {
                    let call_id = tool_call["id"].as_str().unwrap_or_default().trim();
                    let name = tool_call["function"]["name"]
                        .as_str()
                        .unwrap_or_default()
                        .trim();
                    let arguments = tool_call["function"]["arguments"]
                        .as_str()
                        .unwrap_or_default()
                        .trim();
                    if call_id.is_empty() || name.is_empty() || arguments.is_empty() {
                        continue;
                    }
                    items.push(json!({
                        "type": "function_call",
                        "call_id": call_id,
                        "name": name,
                        "arguments": arguments,
                    }));
                }
            }
            "tool" => {
                let call_id = message["tool_call_id"].as_str().unwrap_or_default().trim();
                if call_id.is_empty() {
                    continue;
                }
                let output = match &message["content"] {
                    Value::String(text) => text.clone(),
                    Value::Null => String::new(),
                    other => serde_json::to_string(other).unwrap_or_default(),
                };
                items.push(json!({
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": output,
                }));
            }
            _ => {}
        }
    }

    items
}

fn parse_last_user_tool_input(messages: &[Value]) -> Result<Value> {
    let last_user_content = messages
        .iter()
        .rev()
        .find_map(|message| {
            if message["role"].as_str() == Some("user") {
                message["content"]
                    .as_str()
                    .map(str::trim)
                    .map(str::to_string)
            } else {
                None
            }
        })
        .filter(|content| !content.is_empty())
        .ok_or_else(|| anyhow!("mock write_file 缺少用户输入"))?;

    serde_json::from_str(&last_user_content)
        .map_err(|e| anyhow!("mock write_file 用户输入 JSON 解析失败: {}", e))
}

fn decode_nested_tool_call_arguments(value: Value) -> Value {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                || (trimmed.starts_with('[') && trimmed.ends_with(']'))
            {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    return parsed;
                }
            }
            Value::String(text)
        }
        other => other,
    }
}

fn strip_trailing_json_commas(input: &str) -> Option<String> {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape = false;
    let mut changed = false;

    'outer: while let Some(ch) = chars.next() {
        if in_string {
            out.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            out.push(ch);
            continue;
        }

        if ch == ',' {
            let mut lookahead = chars.clone();
            while let Some(next) = lookahead.next() {
                if next.is_whitespace() {
                    continue;
                }
                if next == '}' || next == ']' {
                    changed = true;
                    continue 'outer;
                }
                break;
            }
        }

        out.push(ch);
    }

    changed.then_some(out)
}

fn parse_tool_call_arguments(args_str: &str) -> Result<Value> {
    let trimmed = args_str.trim();
    if trimmed.is_empty() {
        return Ok(json!({}));
    }

    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
        return Ok(decode_nested_tool_call_arguments(parsed));
    }

    if let Some(repaired) = strip_trailing_json_commas(trimmed) {
        if let Ok(parsed) = serde_json::from_str::<Value>(&repaired) {
            return Ok(decode_nested_tool_call_arguments(parsed));
        }
    }

    serde_json::from_str::<Value>(trimmed)
        .map(decode_nested_tool_call_arguments)
        .map_err(|e| anyhow!("工具参数 JSON 解析失败: {}; raw={}", e, trimmed))
}

fn build_http_client() -> Result<Client> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| anyhow!("构建 OpenAI HTTP 客户端失败: {}", e))
}

fn validate_test_connection_response(body: &str) -> Result<bool> {
    let parsed: Value = serde_json::from_str(body).map_err(|_| {
        let preview: String = body.chars().take(160).collect();
        anyhow!("OpenAI 连接测试返回了非 JSON 内容: {}", preview)
    })?;

    if parsed.get("choices").and_then(Value::as_array).is_some() {
        return Ok(true);
    }

    if parsed.get("output").and_then(Value::as_array).is_some() {
        return Ok(true);
    }

    if let Some(error_message) = parsed
        .get("error")
        .and_then(|error| error.get("message").or(Some(error)))
        .and_then(Value::as_str)
    {
        return Err(anyhow!("OpenAI 连接测试返回错误: {}", error_message));
    }

    Err(anyhow!(
        "OpenAI 连接测试返回了非标准响应，缺少 choices/output 字段"
    ))
}

fn openai_responses_url(base_url: &str) -> String {
    format!("{}/responses", base_url.trim().trim_end_matches('/'))
}

fn openai_chat_completions_url(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim().trim_end_matches('/'))
}

fn normalize_openai_tool_schema(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => {
            let mut normalized = serde_json::Map::with_capacity(map.len());
            for (key, value) in map {
                if key == "const" {
                    continue;
                }
                normalized.insert(key.clone(), normalize_openai_tool_schema(value));
            }
            if let Some(const_value) = map.get("const") {
                normalized.entry("enum".to_string()).or_insert_with(|| {
                    Value::Array(vec![normalize_openai_tool_schema(const_value)])
                });
            }
            Value::Object(normalized)
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(normalize_openai_tool_schema).collect())
        }
        _ => schema.clone(),
    }
}

fn openai_tools_from_anthropic_defs(tools: &[Value]) -> Vec<Value> {
    tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t["name"],
                    "description": t["description"],
                    "parameters": normalize_openai_tool_schema(&t["input_schema"]),
                }
            })
        })
        .collect()
}

fn openai_responses_tools_from_anthropic_defs(tools: &[Value]) -> Vec<Value> {
    tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "name": t["name"],
                "description": t["description"],
                "parameters": normalize_openai_tool_schema(&t["input_schema"]),
            })
        })
        .collect()
}

fn build_openai_chat_completion_messages(system_prompt: &str, messages: Vec<Value>) -> Vec<Value> {
    let mut request_messages = Vec::with_capacity(messages.len() + 1);
    if !system_prompt.trim().is_empty() {
        request_messages.push(json!({
            "role": "system",
            "content": system_prompt.trim(),
        }));
    }
    request_messages.extend(messages);
    request_messages
}

fn build_openai_responses_request_body(
    model: &str,
    system_prompt: &str,
    messages: &[Value],
    tools: &[Value],
) -> Value {
    json!({
        "model": model,
        "instructions": system_prompt,
        "input": convert_messages_to_responses_input(messages),
        "tools": openai_responses_tools_from_anthropic_defs(tools),
        "stream": true,
    })
}

fn build_openai_chat_completions_request_body(
    transport: &ResolvedModelTransport,
    model: &str,
    system_prompt: &str,
    messages: Vec<Value>,
    tools: &[Value],
) -> Value {
    let mut body = json!({
        "model": model,
        "messages": build_openai_chat_completion_messages(system_prompt, messages),
        "tools": openai_tools_from_anthropic_defs(tools),
        "stream": true,
    });

    if transport
        .openai_compat
        .map(|features| features.supports_usage_in_streaming)
        .unwrap_or(false)
    {
        if let Some(object) = body.as_object_mut() {
            object.insert(
                "stream_options".to_string(),
                json!({ "include_usage": true }),
            );
        }
    }

    body
}

fn last_tool_message_content(messages: &[Value]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find(|message| message["role"].as_str() == Some("tool"))
        .and_then(|message| message["content"].as_str())
        .map(str::to_string)
}

fn parse_mock_response_chunks(
    chunks: &[&str],
    on_token: &mut impl FnMut(StreamDelta),
) -> Result<LLMResponse> {
    let mut state = OpenAiStreamState::default();
    for chunk in chunks {
        process_openai_sse_text(chunk, &mut state, on_token)?;
        if state.stop_stream {
            break;
        }
    }
    Ok(finish_openai_stream(state))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HiddenTag {
    Think,
    Thinking,
}

impl HiddenTag {
    fn open_tag(self) -> &'static [char] {
        match self {
            Self::Think => &['<', 't', 'h', 'i', 'n', 'k', '>'],
            Self::Thinking => &['<', 't', 'h', 'i', 'n', 'k', 'i', 'n', 'g', '>'],
        }
    }

    fn close_tag(self) -> &'static [char] {
        match self {
            Self::Think => &['<', '/', 't', 'h', 'i', 'n', 'k', '>'],
            Self::Thinking => &['<', '/', 't', 'h', 'i', 'n', 'k', 'i', 'n', 'g', '>'],
        }
    }
}

const FINAL_OPEN_TAG: [char; 7] = ['<', 'f', 'i', 'n', 'a', 'l', '>'];
const FINAL_CLOSE_TAG: [char; 8] = ['<', '/', 'f', 'i', 'n', 'a', 'l', '>'];
const VISIBLE_TAGS: [&[char]; 2] = [&FINAL_OPEN_TAG, &FINAL_CLOSE_TAG];
const HIDDEN_TAGS: [HiddenTag; 2] = [HiddenTag::Think, HiddenTag::Thinking];

/// Strip openclaw-compatible hidden/visible scaffold tags from a streaming token chunk.
/// `hidden_tag` carries state across chunk boundaries for tags whose content should be suppressed.
fn tag_matches(chars: &[char], start: usize, tag: &[char]) -> bool {
    start + tag.len() <= chars.len() && chars[start..start + tag.len()] == *tag
}

fn tag_prefix_at_end(chars: &[char], start: usize, tag: &[char]) -> bool {
    let remaining = chars.len().saturating_sub(start);
    remaining > 0 && remaining < tag.len() && chars[start..] == tag[..remaining]
}

fn filter_thinking(
    input: &str,
    hidden_tag: &mut Option<HiddenTag>,
    pending_tag: &mut String,
) -> String {
    let mut combined = String::with_capacity(pending_tag.len() + input.len());
    combined.push_str(pending_tag);
    combined.push_str(input);
    pending_tag.clear();

    let chars: Vec<char> = combined.chars().collect();
    let mut out = String::with_capacity(combined.len());
    let mut index = 0;

    while index < chars.len() {
        if let Some(current_hidden_tag) = *hidden_tag {
            let close_tag = current_hidden_tag.close_tag();
            if tag_matches(&chars, index, close_tag) {
                *hidden_tag = None;
                index += close_tag.len();
                continue;
            }

            if tag_prefix_at_end(&chars, index, close_tag) {
                pending_tag.extend(chars[index..].iter());
                break;
            }

            index += 1;
            continue;
        }

        let mut consumed = false;
        for hidden in HIDDEN_TAGS {
            let open_tag = hidden.open_tag();
            if tag_matches(&chars, index, open_tag) {
                *hidden_tag = Some(hidden);
                index += open_tag.len();
                consumed = true;
                break;
            }
            if tag_prefix_at_end(&chars, index, open_tag) {
                pending_tag.extend(chars[index..].iter());
                consumed = true;
                break;
            }
        }
        if consumed {
            if !pending_tag.is_empty() {
                break;
            }
            continue;
        }

        for visible_tag in VISIBLE_TAGS {
            if tag_matches(&chars, index, visible_tag) {
                index += visible_tag.len();
                consumed = true;
                break;
            }
            if tag_prefix_at_end(&chars, index, visible_tag) {
                pending_tag.extend(chars[index..].iter());
                consumed = true;
                break;
            }
        }
        if consumed {
            if !pending_tag.is_empty() {
                break;
            }
            continue;
        }

        out.push(chars[index]);
        index += 1;
    }

    out
}

#[derive(Default)]
struct OpenAiStreamState {
    text_content: String,
    fallback_text_content: String,
    saw_text_delta: bool,
    hidden_tag: Option<HiddenTag>,
    pending_think_tag: String,
    tool_calls_map: HashMap<u64, OpenAiResponseToolCall>,
    stop_stream: bool,
    pending_line: String,
}

#[derive(Default)]
struct OpenAiResponseToolCall {
    item_id: String,
    call_id: String,
    name: String,
    arguments: String,
}

fn process_openai_sse_text(
    text: &str,
    state: &mut OpenAiStreamState,
    on_token: &mut impl FnMut(StreamDelta),
) -> Result<()> {
    state.pending_line.push_str(text);
    let ends_with_newline = state.pending_line.ends_with('\n');
    let owned = std::mem::take(&mut state.pending_line);
    let mut lines: Vec<&str> = owned.lines().collect();

    if !ends_with_newline {
        state.pending_line = lines.pop().unwrap_or_default().to_string();
    }

    for line in lines {
        if line.starts_with("event: ") {
            continue;
        }
        if let Some(data) = line.strip_prefix("data: ") {
            if data.trim() == "[DONE]" {
                state.stop_stream = true;
                break;
            }

            if let Ok(v) = serde_json::from_str::<Value>(data) {
                match v["type"].as_str().unwrap_or_default() {
                    "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                        if let Some(delta) = v["delta"].as_str().map(str::trim) {
                            if !delta.is_empty() {
                                on_token(StreamDelta::Reasoning(delta.to_string()));
                            }
                        }
                    }
                    "response.output_text.delta" => {
                        if let Some(delta) = v["delta"].as_str() {
                            let filtered = filter_thinking(
                                delta,
                                &mut state.hidden_tag,
                                &mut state.pending_think_tag,
                            );
                            if !filtered.is_empty() {
                                state.saw_text_delta = true;
                                state.text_content.push_str(&filtered);
                                on_token(StreamDelta::Text(filtered));
                            }
                        }
                    }
                    "response.output_item.added" => {
                        if v["item"]["type"].as_str() == Some("function_call") {
                            let index = v["output_index"].as_u64().unwrap_or(0);
                            let entry = state.tool_calls_map.entry(index).or_default();
                            if let Some(item_id) = v["item"]["id"].as_str() {
                                entry.item_id = item_id.to_string();
                            }
                            if let Some(call_id) = v["item"]["call_id"].as_str() {
                                entry.call_id = call_id.to_string();
                            }
                            if let Some(name) = v["item"]["name"].as_str() {
                                entry.name = name.to_string();
                            }
                            if let Some(arguments) = v["item"]["arguments"].as_str() {
                                entry.arguments = arguments.to_string();
                            }
                        }
                    }
                    "response.function_call_arguments.delta" => {
                        let index = v["output_index"].as_u64().unwrap_or(0);
                        let entry = state.tool_calls_map.entry(index).or_default();
                        if let Some(item_id) = v["item_id"].as_str() {
                            entry.item_id = item_id.to_string();
                        }
                        if let Some(delta) = v["delta"].as_str() {
                            entry.arguments.push_str(delta);
                        }
                    }
                    "response.function_call_arguments.done" | "response.output_item.done" => {
                        let item = &v["item"];
                        match item["type"].as_str().unwrap_or_default() {
                            "function_call" => {
                                let index = v["output_index"].as_u64().unwrap_or(0);
                                let entry = state.tool_calls_map.entry(index).or_default();
                                if let Some(item_id) = item["id"].as_str() {
                                    entry.item_id = item_id.to_string();
                                }
                                if let Some(call_id) = item["call_id"].as_str() {
                                    entry.call_id = call_id.to_string();
                                }
                                if let Some(name) = item["name"].as_str() {
                                    entry.name = name.to_string();
                                }
                                if let Some(arguments) = item["arguments"].as_str() {
                                    entry.arguments = arguments.to_string();
                                }
                            }
                            "message" => {
                                if !state.saw_text_delta {
                                    let text = item["content"]
                                        .as_array()
                                        .into_iter()
                                        .flatten()
                                        .filter_map(|part| {
                                            (part["type"].as_str() == Some("output_text"))
                                                .then(|| part["text"].as_str())
                                                .flatten()
                                        })
                                        .collect::<Vec<_>>()
                                        .join("");
                                    if !text.is_empty() {
                                        let filtered = filter_thinking(
                                            &text,
                                            &mut state.hidden_tag,
                                            &mut state.pending_think_tag,
                                        );
                                        if !filtered.is_empty() {
                                            state.fallback_text_content.push_str(&filtered);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {
                        if let Some(choice) = v["choices"].get(0) {
                            let delta = &choice["delta"];

                            if delta["reasoning_content"]
                                .as_str()
                                .map(|s| !s.is_empty())
                                .unwrap_or(false)
                            {
                                on_token(StreamDelta::Reasoning(
                                    delta["reasoning_content"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string(),
                                ));
                                continue;
                            }

                            if let Some(token) = delta["content"].as_str() {
                                let filtered = filter_thinking(
                                    token,
                                    &mut state.hidden_tag,
                                    &mut state.pending_think_tag,
                                );
                                if !filtered.is_empty() {
                                    state.saw_text_delta = true;
                                    state.text_content.push_str(&filtered);
                                    on_token(StreamDelta::Text(filtered));
                                }
                            }

                            if let Some(tc_array) = delta["tool_calls"].as_array() {
                                for tc_delta in tc_array {
                                    let index = tc_delta["index"].as_u64().unwrap_or(0);

                                    let entry = state.tool_calls_map.entry(index).or_default();

                                    if let Some(id) = tc_delta["id"].as_str() {
                                        entry.call_id = id.to_string();
                                    }
                                    if let Some(name) = tc_delta["function"]["name"].as_str() {
                                        entry.name = name.to_string();
                                    }

                                    if let Some(args) = tc_delta["function"]["arguments"].as_str() {
                                        entry.arguments.push_str(args);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn finish_openai_stream(mut state: OpenAiStreamState) -> LLMResponse {
    if state.hidden_tag.is_none() && !state.pending_think_tag.is_empty() {
        state.text_content.push_str(&state.pending_think_tag);
    }

    if state.text_content.is_empty() && !state.fallback_text_content.is_empty() {
        state.text_content = state.fallback_text_content;
    }

    if !state.tool_calls_map.is_empty() {
        let mut indices: Vec<u64> = state.tool_calls_map.keys().cloned().collect();
        indices.sort();

        let tool_calls: Vec<ToolCall> = indices
            .into_iter()
            .filter_map(|idx| {
                let entry = state.tool_calls_map.remove(&idx).unwrap();
                let trimmed_id = entry.call_id.trim().to_string();
                let trimmed_name = entry.name.trim().to_string();
                if trimmed_id.is_empty() || trimmed_name.is_empty() {
                    return None;
                }
                let args_str = entry.arguments;
                let input = match parse_tool_call_arguments(&args_str) {
                    Ok(value) => value,
                    Err(err) => json!({
                        "__tool_call_parse_error": err.to_string(),
                        "__raw_arguments": args_str,
                    }),
                };
                Some(ToolCall {
                    id: trimmed_id,
                    name: trimmed_name,
                    input,
                })
            })
            .collect();

        if tool_calls.is_empty() {
            return LLMResponse::Text(state.text_content);
        }

        if !state.text_content.is_empty() {
            LLMResponse::TextWithToolCalls(state.text_content, tool_calls)
        } else {
            LLMResponse::ToolCalls(tool_calls)
        }
    } else {
        LLMResponse::Text(state.text_content)
    }
}

/// OpenAI 兼容的流式 tool calling
///
/// 将 Anthropic 格式的工具定义转换为 OpenAI function calling 格式，
/// 发送带 `tools` 和 `stream: true` 的请求，并解析增量 SSE delta 中的 tool_calls。
///
/// 当 `finish_reason == "tool_calls"` 时返回 `LLMResponse::ToolCalls`，
/// 否则返回 `LLMResponse::Text`。
pub async fn chat_stream_with_tools(
    transport: &ResolvedModelTransport,
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    messages: Vec<Value>,
    tools: Vec<Value>,
    mut on_token: impl FnMut(StreamDelta) + Send,
) -> Result<crate::agent::types::LLMResponse> {
    if is_mock_text_base_url(base_url) {
        let mock_text = mock_response_text(model, &messages);
        on_token(StreamDelta::Text(mock_text.clone()));
        return Ok(LLMResponse::Text(mock_text));
    }
    if is_mock_tool_loop_base_url(base_url) {
        return Err(anyhow!("达到最大迭代次数 8"));
    }
    if is_mock_repeat_invalid_write_file_base_url(base_url) {
        return Ok(LLMResponse::ToolCalls(vec![ToolCall {
            id: "mock-write-file-empty".to_string(),
            name: "write_file".to_string(),
            input: json!({}),
        }]));
    }
    if is_mock_write_file_from_user_path_base_url(base_url) {
        if messages
            .iter()
            .any(|message| message["role"].as_str() == Some("tool"))
        {
            let final_text = "已完成文件写入";
            on_token(StreamDelta::Text(final_text.to_string()));
            return Ok(LLMResponse::Text(final_text.to_string()));
        }

        return Ok(LLMResponse::ToolCalls(vec![ToolCall {
            id: "mock-write-file-from-user-path".to_string(),
            name: "write_file".to_string(),
            input: parse_last_user_tool_input(&messages)?,
        }]));
    }
    if is_mock_responses_read_file_from_user_path_base_url(base_url) {
        if let Some(tool_output) = last_tool_message_content(&messages) {
            return parse_mock_response_chunks(
                &[
                    "event: response.output_item.added\n",
                    "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[]}}\n",
                    "event: response.output_text.delta\n",
                    &format!(
                        "data: {{\"type\":\"response.output_text.delta\",\"output_index\":0,\"content_index\":0,\"delta\":{}}}\n",
                        serde_json::to_string(&format!("已读取文件内容：{}", tool_output)).unwrap_or_else(|_| "\"已读取文件内容\"".to_string())
                    ),
                    "data: [DONE]\n",
                ],
                &mut on_token,
            );
        }

        let input = parse_last_user_tool_input(&messages)?;
        let args = serde_json::to_string(&input).unwrap_or_else(|_| "{}".to_string());
        let done_event = format!(
            "data: {{\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{{\"id\":\"fc_1\",\"type\":\"function_call\",\"call_id\":\"call-read-1\",\"name\":\"read_file\",\"arguments\":{}}}}}\n",
            serde_json::to_string(&args).unwrap_or_else(|_| "\"{}\"".to_string())
        );
        return parse_mock_response_chunks(
            &[
                "event: response.output_item.added\n",
                "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"id\":\"fc_1\",\"type\":\"function_call\",\"call_id\":\"call-read-1\",\"name\":\"read_file\",\"arguments\":\"\"}}\n",
                "event: response.function_call_arguments.delta\n",
                &format!(
                    "data: {{\"type\":\"response.function_call_arguments.delta\",\"output_index\":0,\"item_id\":\"fc_1\",\"delta\":{}}}\n",
                    serde_json::to_string(&args).unwrap_or_else(|_| "\"{}\"".to_string())
                ),
                "event: response.output_item.done\n",
                &done_event,
                "data: [DONE]\n",
            ],
            &mut on_token,
        );
    }
    if is_mock_responses_malformed_tool_call_start_task_base_url(base_url) {
        return parse_mock_response_chunks(
            &[
                "event: response.output_item.added\n",
                "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"id\":\"fc_bad_1\",\"type\":\"function_call\",\"call_id\":\"call-bad-1\",\"arguments\":\"\"}}\n",
                "event: response.function_call_arguments.delta\n",
                "data: {\"type\":\"response.function_call_arguments.delta\",\"output_index\":0,\"item_id\":\"fc_bad_1\",\"delta\":\"{\\\"path\\\":\\\"README.md\\\"}\"}\n",
                "event: response.output_text.delta\n",
                "data: {\"type\":\"response.output_text.delta\",\"output_index\":1,\"content_index\":0,\"delta\":\"我先忽略这个损坏的工具调用，继续处理请求。\"}\n",
                "data: [DONE]\n",
            ],
            &mut on_token,
        );
    }
    if is_mock_repeat_read_file_loop_base_url(base_url) {
        return Ok(LLMResponse::ToolCalls(vec![ToolCall {
            id: "mock-repeat-read-file-loop".to_string(),
            name: "read_file".to_string(),
            input: json!({ "path": "loop.txt" }),
        }]));
    }
    if is_mock_list_dir_with_interleaved_move_failures_base_url(base_url) {
        let tool_count = count_tool_messages(&messages);
        if tool_count >= 12 {
            let final_text = "已识别到连续失败，需要人工确认文件名后再继续。";
            on_token(StreamDelta::Text(final_text.to_string()));
            return Ok(LLMResponse::Text(final_text.to_string()));
        }

        if tool_count % 2 == 0 {
            return Ok(LLMResponse::ToolCalls(vec![ToolCall {
                id: format!("mock-list-dir-{tool_count}"),
                name: "list_dir".to_string(),
                input: json!({ "path": "." }),
            }]));
        }

        return Ok(LLMResponse::ToolCalls(vec![ToolCall {
            id: format!("mock-file-move-{tool_count}"),
            name: "file_move".to_string(),
            input: json!({
                "source": format!("missing-{tool_count}.txt"),
                "destination": format!("dest-{tool_count}.txt"),
            }),
        }]));
    }

    let client = build_http_client()?;
    let (url, body) = match transport.kind {
        ModelTransportKind::OpenAiResponses => (
            openai_responses_url(base_url),
            build_openai_responses_request_body(model, system_prompt, &messages, &tools),
        ),
        ModelTransportKind::OpenAiCompletions => (
            openai_chat_completions_url(base_url),
            build_openai_chat_completions_request_body(
                transport,
                model,
                system_prompt,
                messages,
                &tools,
            ),
        ),
        ModelTransportKind::AnthropicMessages => {
            return Err(anyhow!("OpenAI adapter 不支持 Anthropic transport"));
        }
    };
    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let text = resp.text().await?;
        return Err(anyhow!("OpenAI API error: {}", text));
    }

    let mut stream = resp.bytes_stream();
    let mut state = OpenAiStreamState::default();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);
        process_openai_sse_text(&text, &mut state, &mut on_token)?;
        if state.stop_stream {
            break;
        }
    }

    Ok(finish_openai_stream(state))
}

pub async fn test_connection(
    transport: &ResolvedModelTransport,
    base_url: &str,
    api_key: &str,
    model: &str,
) -> Result<bool> {
    if is_mock_text_base_url(base_url)
        || is_mock_tool_loop_base_url(base_url)
        || is_mock_repeat_invalid_write_file_base_url(base_url)
        || is_mock_write_file_from_user_path_base_url(base_url)
        || is_mock_repeat_read_file_loop_base_url(base_url)
        || is_mock_list_dir_with_interleaved_move_failures_base_url(base_url)
    {
        return Ok(true);
    }
    let client = build_http_client()?;
    let (url, body) = match transport.kind {
        ModelTransportKind::OpenAiResponses => (
            openai_responses_url(base_url),
            json!({
                "model": model,
                "input": "hi",
                "max_output_tokens": 10
            }),
        ),
        ModelTransportKind::OpenAiCompletions => (
            openai_chat_completions_url(base_url),
            json!({
                "model": model,
                "messages": [{ "role": "user", "content": "hi" }],
                "max_tokens": 10
            }),
        ),
        ModelTransportKind::AnthropicMessages => {
            return Err(anyhow!("OpenAI adapter 不支持 Anthropic transport"));
        }
    };
    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow!("OpenAI API error: {}", text));
    }
    validate_test_connection_response(&text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[test]
    fn invalid_tool_arguments_should_not_silently_become_empty_object() {
        let parsed = parse_tool_call_arguments(r#"{"path":"brief.html""#);
        assert!(parsed.is_err(), "损坏的 tool arguments 应返回错误");
    }

    #[test]
    fn parse_tool_arguments_repairs_trailing_commas() {
        let parsed =
            parse_tool_call_arguments(r#"{"path":"README.md","recursive":true,}"#).expect("parsed");

        assert_eq!(parsed["path"].as_str(), Some("README.md"));
        assert_eq!(parsed["recursive"].as_bool(), Some(true));
    }

    #[test]
    fn parse_tool_arguments_decodes_double_encoded_json() {
        let parsed = parse_tool_call_arguments(r#""{\"path\":\"README.md\"}""#).expect("parsed");

        assert_eq!(parsed["path"].as_str(), Some("README.md"));
    }

    fn parse_openai_chunks_for_test(chunks: &[&str]) -> Result<LLMResponse> {
        let mut state = OpenAiStreamState::default();
        let mut sink = Vec::new();
        for chunk in chunks {
            process_openai_sse_text(chunk, &mut state, &mut |token| sink.push(token))?;
            if state.stop_stream {
                break;
            }
        }
        Ok(finish_openai_stream(state))
    }

    #[test]
    fn done_marker_stops_processing_later_chunks() {
        let response = parse_openai_chunks_for_test(&[
            "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"},\"finish_reason\":null}]}\n",
            "data: [DONE]\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"ignored\"},\"finish_reason\":null}]}\n",
        ])
        .expect("parse chunks");

        match response {
            LLMResponse::Text(text) => assert_eq!(text, "hello"),
            other => panic!("expected text response, got {other:?}"),
        }
    }

    #[test]
    fn done_marker_split_across_chunks_still_stops_stream() {
        let mut state = OpenAiStreamState::default();
        let mut sink = Vec::new();

        for chunk in [
            "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"},\"finish_reason\":null}]}\n",
            "data: [DO",
            "NE]\n",
        ] {
            process_openai_sse_text(chunk, &mut state, &mut |token| sink.push(token))
                .expect("parse chunk");
            if state.stop_stream {
                break;
            }
        }

        assert!(state.stop_stream, "split [DONE] marker should stop stream");
        match finish_openai_stream(state) {
            LLMResponse::Text(text) => assert_eq!(text, "hello"),
            other => panic!("expected text response, got {other:?}"),
        }
    }

    #[test]
    fn test_connection_rejects_html_success_pages() {
        let result = validate_test_connection_response(
            "<!doctype html><html><head><title>Gateway</title></head><body>ok</body></html>",
        );

        assert!(result.is_err(), "HTML 落地页不能被视为模型接口成功响应");
    }

    #[test]
    fn test_connection_accepts_openai_chat_completion_json() {
        let result = validate_test_connection_response(
            r#"{"id":"chatcmpl-1","object":"chat.completion","choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}]}"#,
        )
        .expect("valid openai response");

        assert!(result);
    }

    #[test]
    fn test_connection_accepts_openai_responses_json() {
        let result = validate_test_connection_response(
            r#"{"id":"resp_1","object":"response","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"ok"}]}]}"#,
        )
        .expect("valid openai responses payload");

        assert!(result);
    }

    #[test]
    fn qwen_style_backends_use_chat_completions_url() {
        assert_eq!(
            openai_chat_completions_url("https://dashscope.aliyuncs.com/compatible-mode/v1"),
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
        );
    }

    #[test]
    fn native_openai_uses_responses_url() {
        assert_eq!(
            openai_responses_url("https://api.openai.com/v1"),
            "https://api.openai.com/v1/responses"
        );
    }

    #[test]
    fn completions_request_body_includes_stream_options_for_native_streaming_compat() {
        let body = build_openai_chat_completions_request_body(
            &ResolvedModelTransport {
                kind: ModelTransportKind::OpenAiCompletions,
                openai_compat: Some(crate::model_transport::OpenAiCompatFeatures {
                    supports_developer_role: false,
                    supports_usage_in_streaming: true,
                    supports_strict_mode: false,
                }),
            },
            "qwen3.6-plus",
            "system prompt",
            vec![json!({ "role": "user", "content": "hello" })],
            &[json!({
                "name": "read_file",
                "description": "Read a file",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    }
                }
            })],
        );

        assert_eq!(
            body["stream_options"]["include_usage"].as_bool(),
            Some(true)
        );
    }

    #[test]
    fn completions_request_body_omits_stream_options_for_generic_proxy_compat() {
        let body = build_openai_chat_completions_request_body(
            &ResolvedModelTransport {
                kind: ModelTransportKind::OpenAiCompletions,
                openai_compat: Some(crate::model_transport::OpenAiCompatFeatures {
                    supports_developer_role: false,
                    supports_usage_in_streaming: false,
                    supports_strict_mode: false,
                }),
            },
            "qwen3.6-plus",
            "system prompt",
            vec![json!({ "role": "user", "content": "hello" })],
            &[],
        );

        assert!(body.get("stream_options").is_none());
    }

    #[test]
    fn convert_messages_to_responses_input_replays_tool_calls_and_outputs() {
        let messages = vec![
            json!({"role":"user","content":"rename this file"}),
            json!({
                "role":"assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id":"call_1",
                    "type":"function",
                    "function":{"name":"read_file","arguments":"{\"path\":\"a.txt\"}"}
                }]
            }),
            json!({"role":"tool","tool_call_id":"call_1","content":"contents"}),
        ];

        let input = convert_messages_to_responses_input(&messages);

        assert_eq!(input.len(), 3);
        assert_eq!(input[0]["type"].as_str(), Some("message"));
        assert_eq!(input[1]["type"].as_str(), Some("function_call"));
        assert_eq!(input[1]["call_id"].as_str(), Some("call_1"));
        assert_eq!(input[2]["type"].as_str(), Some("function_call_output"));
        assert_eq!(input[2]["call_id"].as_str(), Some("call_1"));
    }

    #[test]
    fn filter_thinking_keeps_multibyte_text_without_panicking() {
        let mut hidden_tag = None;
        let mut pending_tag = String::new();
        let result = catch_unwind(AssertUnwindSafe(|| {
            filter_thinking("有什么", &mut hidden_tag, &mut pending_tag)
        }));

        assert!(result.is_ok(), "多字节文本不应触发 panic");
        assert_eq!(result.unwrap(), "有什么");
        assert!(hidden_tag.is_none());
        assert!(pending_tag.is_empty());
    }

    #[test]
    fn filter_thinking_hides_cross_chunk_think_blocks() {
        let mut hidden_tag = None;
        let mut pending_tag = String::new();

        let first = filter_thinking("<think>推理中", &mut hidden_tag, &mut pending_tag);
        let second = filter_thinking("</think>你好", &mut hidden_tag, &mut pending_tag);

        assert_eq!(first, "");
        assert_eq!(second, "你好");
        assert!(hidden_tag.is_none());
        assert!(pending_tag.is_empty());
    }

    #[test]
    fn filter_thinking_handles_split_open_tag_across_chunks() {
        let mut hidden_tag = None;
        let mut pending_tag = String::new();

        let first = filter_thinking("<thi", &mut hidden_tag, &mut pending_tag);
        let second = filter_thinking("nk>内部</think>结果", &mut hidden_tag, &mut pending_tag);

        assert_eq!(first, "");
        assert_eq!(second, "结果");
        assert!(hidden_tag.is_none());
        assert!(pending_tag.is_empty());
    }

    #[test]
    fn filter_thinking_preserves_multibyte_text_after_think_block() {
        let response = parse_openai_chunks_for_test(&[
            "data: {\"choices\":[{\"delta\":{\"content\":\"<think>内部推理\"},\"finish_reason\":null}]}\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"</think>有什么文件夹\"},\"finish_reason\":null}]}\n",
            "data: [DONE]\n",
        ])
        .expect("parse chunks");

        match response {
            LLMResponse::Text(text) => assert_eq!(text, "有什么文件夹"),
            other => panic!("expected text response, got {other:?}"),
        }
    }

    #[test]
    fn openai_tag_compatibility_strips_thinking_tags() {
        let response = parse_openai_chunks_for_test(&[
            "data: {\"choices\":[{\"delta\":{\"content\":\"<thinking>内部推理</thinking>你好\"},\"finish_reason\":null}]}\n",
            "data: [DONE]\n",
        ])
        .expect("parse chunks");

        match response {
            LLMResponse::Text(text) => assert_eq!(text, "你好"),
            other => panic!("expected text response, got {other:?}"),
        }
    }

    #[test]
    fn openai_tag_compatibility_keeps_final_block_text() {
        let response = parse_openai_chunks_for_test(&[
            "data: {\"choices\":[{\"delta\":{\"content\":\"<think>内部</think><final>可见结果</final>\"},\"finish_reason\":null}]}\n",
            "data: [DONE]\n",
        ])
        .expect("parse chunks");

        match response {
            LLMResponse::Text(text) => assert_eq!(text, "可见结果"),
            other => panic!("expected text response, got {other:?}"),
        }
    }

    #[test]
    fn openai_tag_compatibility_handles_split_final_tags() {
        let response = parse_openai_chunks_for_test(&[
            "data: {\"choices\":[{\"delta\":{\"content\":\"<fi\"},\"finish_reason\":null}]}\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"nal>分片\"},\"finish_reason\":null}]}\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"结果</fi\"},\"finish_reason\":null}]}\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"nal>\"},\"finish_reason\":null}]}\n",
            "data: [DONE]\n",
        ])
        .expect("parse chunks");

        match response {
            LLMResponse::Text(text) => assert_eq!(text, "分片结果"),
            other => panic!("expected text response, got {other:?}"),
        }
    }

    #[test]
    fn reasoning_content_is_emitted_separately_from_answer_text() {
        let mut state = OpenAiStreamState::default();
        let mut deltas = Vec::new();

        process_openai_sse_text(
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"先分析问题\"},\"finish_reason\":null}]}\n\
             data: {\"choices\":[{\"delta\":{\"content\":\"最终答案\"},\"finish_reason\":null}]}\n\
             data: [DONE]\n",
            &mut state,
            &mut |delta| deltas.push(delta),
        )
        .expect("parse chunk");

        assert_eq!(
            deltas,
            vec![
                StreamDelta::Reasoning("先分析问题".to_string()),
                StreamDelta::Text("最终答案".to_string()),
            ]
        );

        match finish_openai_stream(state) {
            LLMResponse::Text(text) => assert_eq!(text, "最终答案"),
            other => panic!("expected text response, got {other:?}"),
        }
    }

    #[test]
    fn responses_reasoning_summary_text_is_emitted_to_reasoning_stream() {
        let mut state = OpenAiStreamState::default();
        let mut deltas = Vec::new();

        process_openai_sse_text(
            "event: response.reasoning_summary_text.delta\n\
             data: {\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\"先分析上下文\"}\n\
             event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"delta\":\"最终回答\"}\n\
             data: [DONE]\n",
            &mut state,
            &mut |delta| deltas.push(delta),
        )
        .expect("parse chunk");

        assert_eq!(
            deltas,
            vec![
                StreamDelta::Reasoning("先分析上下文".to_string()),
                StreamDelta::Text("最终回答".to_string()),
            ]
        );

        match finish_openai_stream(state) {
            LLMResponse::Text(text) => assert_eq!(text, "最终回答"),
            other => panic!("expected text response, got {other:?}"),
        }
    }

    #[test]
    fn responses_reasoning_text_delta_is_emitted_to_reasoning_stream() {
        let mut state = OpenAiStreamState::default();
        let mut deltas = Vec::new();

        process_openai_sse_text(
            "event: response.reasoning_text.delta\n\
             data: {\"type\":\"response.reasoning_text.delta\",\"delta\":\"继续推理\"}\n\
             data: [DONE]\n",
            &mut state,
            &mut |delta| deltas.push(delta),
        )
        .expect("parse chunk");

        assert_eq!(deltas, vec![StreamDelta::Reasoning("继续推理".to_string())]);
    }

    #[test]
    fn responses_stream_emits_reasoning_before_visible_output() {
        let mut state = OpenAiStreamState::default();
        let mut deltas = Vec::new();

        process_openai_sse_text(
            "event: response.created\n\
             data: {\"type\":\"response.created\",\"response\":{\"output\":[],\"status\":\"queued\"}}\n\
             event: response.in_progress\n\
             data: {\"type\":\"response.in_progress\",\"response\":{\"output\":[],\"status\":\"in_progress\"}}\n\
             event: response.output_item.added\n\
             data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"reasoning\",\"id\":\"msg_reasoning_1\",\"summary\":[]}}\n\
             event: response.reasoning_summary_text.delta\n\
             data: {\"type\":\"response.reasoning_summary_text.delta\",\"output_index\":0,\"item_id\":\"msg_reasoning_1\",\"summary_index\":0,\"delta\":\"先分析上下文\"}\n\
             event: response.reasoning_summary_text.delta\n\
             data: {\"type\":\"response.reasoning_summary_text.delta\",\"output_index\":0,\"item_id\":\"msg_reasoning_1\",\"summary_index\":0,\"delta\":\"，再组织答案\"}\n\
             event: response.reasoning_summary_text.done\n\
             data: {\"type\":\"response.reasoning_summary_text.done\",\"output_index\":0,\"item_id\":\"msg_reasoning_1\",\"summary_index\":0,\"text\":\"先分析上下文，再组织答案\"}\n\
             event: response.output_item.done\n\
             data: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"reasoning\",\"id\":\"msg_reasoning_1\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"先分析上下文，再组织答案\"}]}}\n\
             event: response.content_part.added\n\
             data: {\"type\":\"response.content_part.added\",\"output_index\":1,\"content_index\":0,\"item_id\":\"msg_output_1\",\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\
             event: response.output_text.delta\n\
             data: {\"type\":\"response.output_text.delta\",\"output_index\":1,\"content_index\":0,\"item_id\":\"msg_output_1\",\"delta\":\"最终回答\"}\n\
             event: response.output_text.done\n\
             data: {\"type\":\"response.output_text.done\",\"output_index\":1,\"content_index\":0,\"item_id\":\"msg_output_1\",\"text\":\"最终回答\"}\n\
             event: response.completed\n\
             data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\
             data: [DONE]\n",
            &mut state,
            &mut |delta| deltas.push(delta),
        )
        .expect("parse chunk");

        assert_eq!(
            deltas,
            vec![
                StreamDelta::Reasoning("先分析上下文".to_string()),
                StreamDelta::Reasoning("，再组织答案".to_string()),
                StreamDelta::Text("最终回答".to_string()),
            ]
        );

        match finish_openai_stream(state) {
            LLMResponse::Text(text) => assert_eq!(text, "最终回答"),
            other => panic!("expected text response, got {other:?}"),
        }
    }

    #[test]
    fn openai_stream_parser_drops_tool_call_without_name() {
        let response = parse_openai_chunks_for_test(&[
            "event: response.output_item.added\n",
            "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"function_call\",\"call_id\":\"call-1\",\"arguments\":\"\"}}\n",
            "event: response.function_call_arguments.delta\n",
            "data: {\"type\":\"response.function_call_arguments.delta\",\"output_index\":0,\"item_id\":\"fc_1\",\"delta\":\"{\\\"path\\\":\\\"README.md\\\"}\"}\n",
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"output_index\":1,\"content_index\":0,\"delta\":\"继续处理\"}\n",
            "data: [DONE]\n",
        ])
        .expect("parse chunks");

        match response {
            LLMResponse::Text(text) => assert_eq!(text, "继续处理"),
            other => panic!("expected text response, got {other:?}"),
        }
    }

    #[test]
    fn responses_stream_parser_returns_tool_calls() {
        let response = parse_openai_chunks_for_test(&[
            "event: response.output_item.added\n",
            "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"id\":\"fc_1\",\"type\":\"function_call\",\"call_id\":\"call-1\",\"name\":\"read_file\",\"arguments\":\"\"}}\n",
            "event: response.function_call_arguments.delta\n",
            "data: {\"type\":\"response.function_call_arguments.delta\",\"output_index\":0,\"item_id\":\"fc_1\",\"delta\":\"{\\\"path\\\":\\\"README.md\\\"}\"}\n",
            "event: response.output_item.done\n",
            "data: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"id\":\"fc_1\",\"type\":\"function_call\",\"call_id\":\"call-1\",\"name\":\"read_file\",\"arguments\":\"{\\\"path\\\":\\\"README.md\\\"}\"}}\n",
            "data: [DONE]\n",
        ])
        .expect("parse chunks");

        match response {
            LLMResponse::ToolCalls(tool_calls) => {
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(tool_calls[0].id, "call-1");
                assert_eq!(tool_calls[0].name, "read_file");
                assert_eq!(tool_calls[0].input["path"].as_str(), Some("README.md"));
            }
            other => panic!("expected tool calls response, got {other:?}"),
        }
    }

    #[test]
    fn openai_tools_from_anthropic_defs_normalizes_const_to_enum_recursively() {
        let tools = openai_tools_from_anthropic_defs(&[json!({
            "name": "write_file",
            "description": "write a file",
            "input_schema": {
                "type": "object",
                "properties": {
                    "mode": { "type": "string", "const": "overwrite" },
                    "nested": {
                        "type": "object",
                        "properties": {
                            "kind": { "const": "text" }
                        }
                    },
                    "items": {
                        "type": "array",
                        "items": { "const": "line" }
                    }
                }
            }
        })]);

        let parameters = &tools[0]["function"]["parameters"];
        assert!(parameters.get("const").is_none());
        assert_eq!(
            parameters["properties"]["mode"]["enum"],
            json!(["overwrite"])
        );
        assert!(parameters["properties"]["mode"].get("const").is_none());
        assert_eq!(
            parameters["properties"]["nested"]["properties"]["kind"]["enum"],
            json!(["text"])
        );
        assert!(parameters["properties"]["nested"]["properties"]["kind"]
            .get("const")
            .is_none());
        assert_eq!(
            parameters["properties"]["items"]["items"]["enum"],
            json!(["line"])
        );
        assert!(parameters["properties"]["items"]["items"]
            .get("const")
            .is_none());
    }
}
