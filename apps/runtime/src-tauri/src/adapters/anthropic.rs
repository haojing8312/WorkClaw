use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};

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

pub async fn chat_stream_with_tools(
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    messages: Vec<Value>,
    tools: Vec<Value>,
    mut on_token: impl FnMut(String) + Send,
) -> Result<crate::agent::types::LLMResponse> {
    use crate::agent::types::{LLMResponse, ToolCall};

    if is_mock_text_base_url(base_url) {
        let mock_text = mock_response_text(model, &messages);
        on_token(mock_text.clone());
        return Ok(LLMResponse::Text(mock_text));
    }
    if is_mock_tool_loop_base_url(base_url) {
        return Err(anyhow!("达到最大迭代次数 8"));
    }

    let client = Client::new();
    let url = format!("{}/messages", base_url.trim_end_matches('/'));

    let body = json!({
        "model": model,
        "system": system_prompt,
        "messages": messages,
        "tools": tools,
        "max_tokens": 4096,
        "stream": true,
    });

    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let text = resp.text().await?;
        return Err(anyhow!("Anthropic API error: {}", text));
    }

    let mut stream = resp.bytes_stream();
    let mut tool_calls: Vec<ToolCall> = vec![];
    let mut text_content = String::new();
    let mut current_tool_call: Option<ToolCall> = None;
    let mut current_tool_input = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim().is_empty() {
                    continue;
                }

                if let Ok(v) = serde_json::from_str::<Value>(data) {
                    let event_type = v["type"].as_str().unwrap_or("");

                    match event_type {
                        "content_block_start" => {
                            if v["content_block"]["type"] == "tool_use" {
                                current_tool_call = Some(ToolCall {
                                    id: v["content_block"]["id"].as_str().unwrap_or("").to_string(),
                                    name: v["content_block"]["name"]
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string(),
                                    input: json!({}),
                                });
                                current_tool_input.clear();
                            }
                        }
                        "content_block_delta" => {
                            if v["delta"]["type"] == "text_delta" {
                                let token = v["delta"]["text"].as_str().unwrap_or("");
                                text_content.push_str(token);
                                on_token(token.to_string());
                            } else if v["delta"]["type"] == "input_json_delta" {
                                current_tool_input
                                    .push_str(v["delta"]["partial_json"].as_str().unwrap_or(""));
                            }
                        }
                        "content_block_stop" => {
                            if let Some(mut call) = current_tool_call.take() {
                                if !current_tool_input.is_empty() {
                                    call.input = serde_json::from_str(&current_tool_input)
                                        .unwrap_or(json!({}));
                                }
                                tool_calls.push(call);
                            }
                        }
                        "message_stop" => {
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    if !tool_calls.is_empty() {
        if !text_content.is_empty() {
            Ok(LLMResponse::TextWithToolCalls(text_content, tool_calls))
        } else {
            Ok(LLMResponse::ToolCalls(tool_calls))
        }
    } else {
        Ok(LLMResponse::Text(text_content))
    }
}

pub async fn test_connection(base_url: &str, api_key: &str, model: &str) -> Result<bool> {
    if is_mock_text_base_url(base_url) || is_mock_tool_loop_base_url(base_url) {
        return Ok(true);
    }
    let client = Client::new();
    let body = json!({
        "model": model,
        "max_tokens": 10,
        "messages": [{"role": "user", "content": "hi"}]
    });
    let url = format!("{}/messages", base_url.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;
    Ok(resp.status().is_success())
}
