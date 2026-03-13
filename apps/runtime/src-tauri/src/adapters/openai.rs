use crate::agent::types::{LLMResponse, StreamDelta, ToolCall};
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

fn validate_test_connection_response(body: &str) -> Result<bool> {
    let parsed: Value = serde_json::from_str(body).map_err(|_| {
        let preview: String = body.chars().take(160).collect();
        anyhow!("OpenAI 连接测试返回了非 JSON 内容: {}", preview)
    })?;

    if parsed.get("choices").and_then(Value::as_array).is_some() {
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
        "OpenAI 连接测试返回了非标准响应，缺少 choices 字段"
    ))
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
    hidden_tag: Option<HiddenTag>,
    pending_think_tag: String,
    tool_calls_map: HashMap<u64, (String, String, String)>,
    finish_reason: Option<String>,
    stop_stream: bool,
    pending_line: String,
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
                    on_token(StreamDelta::Reasoning(
                        delta["reasoning_content"].as_str().unwrap_or_default().to_string(),
                    ));
                    continue;
                }

                if let Some(token) = delta["content"].as_str() {
                    let filtered =
                        filter_thinking(token, &mut state.hidden_tag, &mut state.pending_think_tag);
                    if !filtered.is_empty() {
                        state.text_content.push_str(&filtered);
                        on_token(StreamDelta::Text(filtered));
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
    if state.hidden_tag.is_none() && !state.pending_think_tag.is_empty() {
        state.text_content.push_str(&state.pending_think_tag);
    }

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
}
