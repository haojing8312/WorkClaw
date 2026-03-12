use crate::agent::types::{LLMResponse, ToolCall};
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
                message["content"]
                    .as_str()
                    .map(|content| content.trim().to_string())
                    .filter(|content| !content.is_empty())
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

fn parse_tool_call_arguments(args_str: &str) -> Result<Value> {
    let trimmed = args_str.trim();
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(trimmed)
        .map_err(|e| anyhow!("工具参数 JSON 解析失败: {}; raw={}", e, trimmed))
}

fn build_http_client() -> Result<Client> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| anyhow!("构建 OpenAI HTTP 客户端失败: {}", e))
}

/// Strip <think>…</think> spans from a streaming token chunk.
/// `in_think` carries state across chunk boundaries.
fn filter_thinking(input: &str, in_think: &mut bool) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut buf = String::new();

    while let Some(c) = chars.next() {
        buf.push(c);
        if *in_think {
            // Look for </think>
            if buf.ends_with("</think>") {
                *in_think = false;
                buf.clear();
            }
            // Keep buf bounded so it doesn't grow unbounded on large thinking blocks
            if buf.len() > 16 {
                buf = buf[buf.len() - 16..].to_string();
            }
        } else {
            // Look for <think>
            if buf.ends_with("<think>") {
                *in_think = true;
                // Remove the <think> prefix we may have already added to out
                let clean_len = out.len().saturating_sub(6); // len("<think>") - 1
                out.truncate(clean_len);
                buf.clear();
            } else {
                // Safe to emit everything except the last 6 chars (potential partial tag)
                if buf.len() > 7 {
                    let safe = buf.len() - 7;
                    out.push_str(&buf[..safe]);
                    buf = buf[safe..].to_string();
                }
            }
        }
    }
    // Flush remaining buffer if not in a thinking block
    if !*in_think {
        out.push_str(&buf);
    }
    out
}

#[derive(Default)]
struct OpenAiStreamState {
    text_content: String,
    in_think: bool,
    tool_calls_map: HashMap<u64, (String, String, String)>,
    finish_reason: Option<String>,
    stop_stream: bool,
}

fn process_openai_sse_text(
    text: &str,
    state: &mut OpenAiStreamState,
    on_token: &mut impl FnMut(String),
) -> Result<()> {
    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if data.trim() == "[DONE]" {
                state.stop_stream = true;
                break;
            }

            if let Ok(v) = serde_json::from_str::<Value>(data) {
                let choice = &v["choices"][0];
                let delta = &choice["delta"];

                if let Some(fr) = choice["finish_reason"].as_str() {
                    state.finish_reason = Some(fr.to_string());
                }

                if delta["reasoning_content"]
                    .as_str()
                    .map(|s| !s.is_empty())
                    .unwrap_or(false)
                {
                    continue;
                }

                if let Some(token) = delta["content"].as_str() {
                    let filtered = filter_thinking(token, &mut state.in_think);
                    if !filtered.is_empty() {
                        state.text_content.push_str(&filtered);
                        on_token(filtered);
                    }
                }

                if let Some(tc_array) = delta["tool_calls"].as_array() {
                    for tc_delta in tc_array {
                        let index = tc_delta["index"].as_u64().unwrap_or(0);

                        let entry = state
                            .tool_calls_map
                            .entry(index)
                            .or_insert_with(|| (String::new(), String::new(), String::new()));

                        if let Some(id) = tc_delta["id"].as_str() {
                            entry.0 = id.to_string();
                        }
                        if let Some(name) = tc_delta["function"]["name"].as_str() {
                            entry.1 = name.to_string();
                        }

                        if let Some(args) = tc_delta["function"]["arguments"].as_str() {
                            entry.2.push_str(args);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn finish_openai_stream(mut state: OpenAiStreamState) -> LLMResponse {
    if state.finish_reason.as_deref() == Some("tool_calls") || !state.tool_calls_map.is_empty() {
        let mut indices: Vec<u64> = state.tool_calls_map.keys().cloned().collect();
        indices.sort();

        let tool_calls: Vec<ToolCall> = indices
            .into_iter()
            .map(|idx| {
                let (id, name, args_str) = state.tool_calls_map.remove(&idx).unwrap();
                let input = match parse_tool_call_arguments(&args_str) {
                    Ok(value) => value,
                    Err(err) => json!({
                        "__tool_call_parse_error": err.to_string(),
                        "__raw_arguments": args_str,
                    }),
                };
                ToolCall { id, name, input }
            })
            .collect();

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
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    messages: Vec<Value>,
    tools: Vec<Value>,
    mut on_token: impl FnMut(String) + Send,
) -> Result<crate::agent::types::LLMResponse> {
    if is_mock_text_base_url(base_url) {
        let mock_text = mock_response_text(model, &messages);
        on_token(mock_text.clone());
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

    let client = build_http_client()?;

    // 构建消息数组，前置 system 消息
    let mut all_messages = vec![json!({"role": "system", "content": system_prompt})];
    all_messages.extend(messages);

    // 将 Anthropic 格式工具定义转换为 OpenAI function calling 格式
    let openai_tools: Vec<Value> = tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t["name"],
                    "description": t["description"],
                    "parameters": t["input_schema"],
                }
            })
        })
        .collect();

    let body = json!({
        "model": model,
        "messages": all_messages,
        "tools": openai_tools,
        "stream": true,
    });

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
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

pub async fn test_connection(base_url: &str, api_key: &str, model: &str) -> Result<bool> {
    if is_mock_text_base_url(base_url)
        || is_mock_tool_loop_base_url(base_url)
        || is_mock_repeat_invalid_write_file_base_url(base_url)
    {
        return Ok(true);
    }
    let client = build_http_client()?;
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let body = json!({
        "model": model,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 10
    });
    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;
    Ok(resp.status().is_success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_tool_arguments_should_not_silently_become_empty_object() {
        let parsed = parse_tool_call_arguments(r#"{"path":"brief.html""#);
        assert!(parsed.is_err(), "损坏的 tool arguments 应返回错误");
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
}
