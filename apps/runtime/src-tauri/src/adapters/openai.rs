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

    // 文本内容累积
    let mut text_content = String::new();
    let mut in_think = false;

    // tool_calls 按 index 累积：(id, name, arguments_buffer)
    let mut tool_calls_map: std::collections::HashMap<u64, (String, String, String)> =
        std::collections::HashMap::new();

    // 跟踪 finish_reason
    let mut finish_reason: Option<String> = None;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    break;
                }

                if let Ok(v) = serde_json::from_str::<Value>(data) {
                    let choice = &v["choices"][0];
                    let delta = &choice["delta"];

                    // 捕获 finish_reason
                    if let Some(fr) = choice["finish_reason"].as_str() {
                        finish_reason = Some(fr.to_string());
                    }

                    // 跳过 DeepSeek reasoning_content tokens
                    if delta["reasoning_content"]
                        .as_str()
                        .map(|s| !s.is_empty())
                        .unwrap_or(false)
                    {
                        continue;
                    }

                    // 处理普通文本 content delta
                    if let Some(token) = delta["content"].as_str() {
                        let filtered = filter_thinking(token, &mut in_think);
                        if !filtered.is_empty() {
                            text_content.push_str(&filtered);
                            on_token(filtered);
                        }
                    }

                    // 处理 tool_calls delta 数组
                    if let Some(tc_array) = delta["tool_calls"].as_array() {
                        for tc_delta in tc_array {
                            let index = tc_delta["index"].as_u64().unwrap_or(0);

                            let entry = tool_calls_map
                                .entry(index)
                                .or_insert_with(|| (String::new(), String::new(), String::new()));

                            // 首个 delta 包含 id 和 function.name
                            if let Some(id) = tc_delta["id"].as_str() {
                                entry.0 = id.to_string();
                            }
                            if let Some(name) = tc_delta["function"]["name"].as_str() {
                                entry.1 = name.to_string();
                            }

                            // 后续 delta 持续追加 function.arguments 片段
                            if let Some(args) = tc_delta["function"]["arguments"].as_str() {
                                entry.2.push_str(args);
                            }
                        }
                    }
                }
            }
        }
    }

    // 根据 finish_reason 和累积的 tool_calls 判断返回类型
    if finish_reason.as_deref() == Some("tool_calls") || !tool_calls_map.is_empty() {
        // 按 index 排序，组装 ToolCall 列表
        let mut indices: Vec<u64> = tool_calls_map.keys().cloned().collect();
        indices.sort();

        let tool_calls: Vec<ToolCall> = indices
            .into_iter()
            .map(|idx| {
                let (id, name, args_str) = tool_calls_map.remove(&idx).unwrap();
                let input = serde_json::from_str(&args_str).unwrap_or(json!({}));
                ToolCall { id, name, input }
            })
            .collect();

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
