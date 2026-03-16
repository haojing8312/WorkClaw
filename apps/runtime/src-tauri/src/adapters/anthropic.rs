use crate::agent::types::{LLMResponse, StreamDelta, ToolCall};
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE},
    Client, StatusCode,
};
use serde_json::{json, Value};

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
        .map_err(|e| anyhow!("构建 Anthropic HTTP 客户端失败: {}", e))
}

fn anthropic_messages_url(base_url: &str) -> String {
    let normalized = base_url.trim().trim_end_matches('/');
    let lower = normalized.to_ascii_lowercase();

    if lower.ends_with("/anthropic") {
        format!("{normalized}/v1/messages")
    } else {
        format!("{normalized}/messages")
    }
}

fn build_anthropic_headers(api_key: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        "x-api-key",
        HeaderValue::from_str(api_key)
            .map_err(|e| anyhow!("Anthropic API Key 无效，无法设置 x-api-key: {e}"))?,
    );
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|e| anyhow!("Anthropic API Key 无效，无法设置 Authorization: {e}"))?,
    );
    Ok(headers)
}

fn anthropic_error_message(status: StatusCode, body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        format!("Anthropic 接口返回空错误响应，HTTP {}", status.as_u16())
    } else {
        trimmed.to_string()
    }
}

fn validate_anthropic_test_connection_response(status: StatusCode, body: &str) -> Result<bool> {
    if status.is_success() {
        return Ok(true);
    }

    Err(anyhow!("{}", anthropic_error_message(status, body)))
}

fn supports_extended_thinking(base_url: &str, model: &str) -> bool {
    let normalized_base_url = base_url.trim().trim_end_matches('/').to_ascii_lowercase();
    let normalized_model = model.trim().to_ascii_lowercase();

    let is_direct_anthropic = normalized_base_url.contains("api.anthropic.com/v1");
    let is_supported_model = normalized_model.starts_with("claude-sonnet-4")
        || normalized_model.starts_with("claude-opus-4");

    is_direct_anthropic && is_supported_model
}

fn build_anthropic_request_body(
    base_url: &str,
    model: &str,
    system_prompt: &str,
    messages: Vec<Value>,
    tools: Vec<Value>,
) -> Value {
    let mut body = json!({
        "model": model,
        "system": system_prompt,
        "messages": messages,
        "tools": tools,
        "max_tokens": 4096,
        "stream": true,
    });

    if supports_extended_thinking(base_url, model) {
        body["thinking"] = json!({
            "type": "enabled",
            "budget_tokens": 2048,
        });
    }

    body
}

#[derive(Default)]
struct AnthropicStreamState {
    tool_calls: Vec<ToolCall>,
    text_content: String,
    current_tool_call: Option<ToolCall>,
    current_tool_input: String,
    stop_stream: bool,
    pending_line: String,
}

fn process_anthropic_sse_text(
    text: &str,
    state: &mut AnthropicStreamState,
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
        if let Some(data) = line.strip_prefix("data: ") {
            if data.trim().is_empty() {
                continue;
            }

            if let Ok(v) = serde_json::from_str::<Value>(data) {
                let event_type = v["type"].as_str().unwrap_or("");

                match event_type {
                    "content_block_start" => {
                        if v["content_block"]["type"] == "tool_use" {
                            state.current_tool_call = Some(ToolCall {
                                id: v["content_block"]["id"].as_str().unwrap_or("").to_string(),
                                name: v["content_block"]["name"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string(),
                                input: json!({}),
                            });
                            state.current_tool_input.clear();
                        }
                    }
                    "content_block_delta" => {
                        if v["delta"]["type"] == "text_delta" {
                            let token = v["delta"]["text"].as_str().unwrap_or("");
                            state.text_content.push_str(token);
                            on_token(StreamDelta::Text(token.to_string()));
                        } else if v["delta"]["type"] == "thinking_delta" {
                            let reasoning = v["delta"]["thinking"].as_str().unwrap_or("");
                            if !reasoning.is_empty() {
                                on_token(StreamDelta::Reasoning(reasoning.to_string()));
                            }
                        } else if v["delta"]["type"] == "input_json_delta" {
                            state
                                .current_tool_input
                                .push_str(v["delta"]["partial_json"].as_str().unwrap_or(""));
                        }
                    }
                    "content_block_stop" => {
                        if let Some(mut call) = state.current_tool_call.take() {
                            if !state.current_tool_input.is_empty() {
                                call.input =
                                    match parse_tool_call_arguments(&state.current_tool_input) {
                                        Ok(value) => value,
                                        Err(err) => json!({
                                            "__tool_call_parse_error": err.to_string(),
                                            "__raw_arguments": state.current_tool_input.clone(),
                                        }),
                                    };
                            }
                            state.tool_calls.push(call);
                        }
                    }
                    "message_stop" => {
                        state.stop_stream = true;
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn finish_anthropic_stream(state: AnthropicStreamState) -> LLMResponse {
    if !state.tool_calls.is_empty() {
        if !state.text_content.is_empty() {
            LLMResponse::TextWithToolCalls(state.text_content, state.tool_calls)
        } else {
            LLMResponse::ToolCalls(state.tool_calls)
        }
    } else {
        LLMResponse::Text(state.text_content)
    }
}

pub async fn chat_stream_with_tools(
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

    let client = build_http_client()?;
    let url = anthropic_messages_url(base_url);
    let headers = build_anthropic_headers(api_key)?;

    let body = build_anthropic_request_body(base_url, model, system_prompt, messages, tools);

    let resp = client
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await?;
        return Err(anyhow!("{}", anthropic_error_message(status, &text)));
    }

    let mut stream = resp.bytes_stream();
    let mut state = AnthropicStreamState::default();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);
        process_anthropic_sse_text(&text, &mut state, &mut on_token)?;
        if state.stop_stream {
            break;
        }
    }

    Ok(finish_anthropic_stream(state))
}

pub async fn test_connection(base_url: &str, api_key: &str, model: &str) -> Result<bool> {
    if is_mock_text_base_url(base_url) || is_mock_tool_loop_base_url(base_url) {
        return Ok(true);
    }
    let client = build_http_client()?;
    let headers = build_anthropic_headers(api_key)?;
    let body = json!({
        "model": model,
        "max_tokens": 10,
        "messages": [{"role": "user", "content": "hi"}]
    });
    let url = anthropic_messages_url(base_url);
    let resp = client
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    validate_anthropic_test_connection_response(status, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_anthropic_chunks_for_test(chunks: &[&str]) -> Result<(LLMResponse, Vec<StreamDelta>)> {
        let mut state = AnthropicStreamState::default();
        let mut sink = Vec::new();
        for chunk in chunks {
            process_anthropic_sse_text(chunk, &mut state, &mut |token| sink.push(token))?;
            if state.stop_stream {
                break;
            }
        }
        Ok((finish_anthropic_stream(state), sink))
    }

    #[test]
    fn anthropic_message_stop_stops_processing_later_chunks() {
        let (response, sink) = parse_anthropic_chunks_for_test(&[
            "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}\n",
            "data: {\"type\":\"message_stop\"}\n",
            "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"ignored\"}}\n",
        ])
        .expect("parse chunks");

        assert_eq!(sink, vec![StreamDelta::Text("hello".to_string())]);
        match response {
            LLMResponse::Text(text) => assert_eq!(text, "hello"),
            other => panic!("expected text response, got {other:?}"),
        }
    }

    #[test]
    fn anthropic_message_stop_split_across_chunks_still_stops_stream() {
        let mut state = AnthropicStreamState::default();
        let mut sink = Vec::new();

        for chunk in [
            "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}\n",
            "data: {\"type\":\"message_",
            "stop\"}\n",
        ] {
            process_anthropic_sse_text(chunk, &mut state, &mut |token| sink.push(token))
                .expect("parse chunk");
            if state.stop_stream {
                break;
            }
        }

        assert!(
            state.stop_stream,
            "split message_stop event should stop stream"
        );
        match finish_anthropic_stream(state) {
            LLMResponse::Text(text) => assert_eq!(text, "hello"),
            other => panic!("expected text response, got {other:?}"),
        }
    }

    #[test]
    fn anthropic_thinking_deltas_stream_as_reasoning_without_polluting_text() {
        let (response, sink) = parse_anthropic_chunks_for_test(&[
            "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"先分析问题\"}}\n",
            "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"signature_delta\",\"signature\":\"abc123\"}}\n",
            "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"最终答案\"}}\n",
            "data: {\"type\":\"message_stop\"}\n",
        ])
        .expect("parse chunks");

        assert_eq!(
            sink,
            vec![
                StreamDelta::Reasoning("先分析问题".to_string()),
                StreamDelta::Text("最终答案".to_string()),
            ]
        );
        match response {
            LLMResponse::Text(text) => assert_eq!(text, "最终答案"),
            other => panic!("expected text response, got {other:?}"),
        }
    }

    #[test]
    fn anthropic_request_body_enables_thinking_for_direct_claude_4_models_only() {
        let direct_body = build_anthropic_request_body(
            "https://api.anthropic.com/v1",
            "claude-sonnet-4-5-20250929",
            "system",
            vec![],
            vec![],
        );
        assert_eq!(
            direct_body["thinking"],
            json!({
                "type": "enabled",
                "budget_tokens": 2048,
            })
        );

        let third_party_body = build_anthropic_request_body(
            "https://api.minimaxi.com/anthropic",
            "MiniMax-M2.5",
            "system",
            vec![],
            vec![],
        );
        assert!(third_party_body.get("thinking").is_none());

        let older_claude_body = build_anthropic_request_body(
            "https://api.anthropic.com/v1",
            "claude-3-5-sonnet-20241022",
            "system",
            vec![],
            vec![],
        );
        assert!(older_claude_body.get("thinking").is_none());
    }

    #[test]
    fn anthropic_messages_url_supports_minimax_cn_root_path() {
        assert_eq!(
            anthropic_messages_url("https://api.minimaxi.com/anthropic"),
            "https://api.minimaxi.com/anthropic/v1/messages"
        );
        assert_eq!(
            anthropic_messages_url("https://api.minimaxi.com/anthropic/v1"),
            "https://api.minimaxi.com/anthropic/v1/messages"
        );
    }

    #[test]
    fn anthropic_headers_include_x_api_key_for_sdk_compatible_providers() {
        let headers = build_anthropic_headers("sk-ant-test").expect("build headers");

        assert_eq!(
            headers
                .get("x-api-key")
                .and_then(|value| value.to_str().ok()),
            Some("sk-ant-test")
        );
        assert_eq!(
            headers
                .get("anthropic-version")
                .and_then(|value| value.to_str().ok()),
            Some("2023-06-01")
        );
    }

    #[test]
    fn anthropic_test_connection_surfaces_upstream_error_body() {
        let error = validate_anthropic_test_connection_response(
            reqwest::StatusCode::PAYMENT_REQUIRED,
            r#"{"error":{"message":"insufficient_balance"}}"#,
        )
        .expect_err("should fail")
        .to_string();

        assert!(error.contains("insufficient_balance"));
    }
}
