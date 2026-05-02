use crate::adapters;
use crate::model_transport::{resolve_model_transport, ModelTransportKind};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;

/// 自动压缩触发的 token 阈值
const AUTO_COMPACT_THRESHOLD: usize = 50_000;

/// 摘要生成的系统提示词
const COMPACT_SYSTEM_PROMPT: &str = "你是一个对话总结助手。请准确、结构化地总结对话内容。";

/// 摘要生成的用户提示词模板
const COMPACT_USER_PROMPT: &str = r#"请总结以下对话，确保连续性。输出以下章节（每章节用 ## 标题）：

## 用户请求与意图
所有明确的用户请求

## 关键技术上下文
涉及的技术栈、框架、架构

## 已修改文件
文件路径和修改内容（含代码片段）

## 错误与修复
遇到的错误及解决方式

## 待办任务
已请求但未完成的任务

## 当前工作状态
压缩前正在进行的工作

## 下一步
建议的下一个操作

---

对话内容：
"#;

/// 检查是否需要自动压缩
pub fn needs_auto_compact(estimated_tokens: usize) -> bool {
    estimated_tokens > AUTO_COMPACT_THRESHOLD
}

/// 从压缩后的消息中提取适合 UI / 运行状态展示的摘要正文。
///
/// `auto_compact` 会在模型摘要前附加本地 transcript 路径，便于后续恢复完整记录；
/// 展示摘要只保留模型生成的结构化内容，避免把本地路径混入状态快照。
pub fn extract_compaction_display_summary(compacted_messages: &[Value]) -> String {
    let content = compacted_messages
        .iter()
        .find(|message| message["role"].as_str() == Some("user"))
        .and_then(|message| message["content"].as_str())
        .unwrap_or("")
        .trim();

    strip_compaction_transcript_header(content)
        .trim_start()
        .to_string()
}

/// 从压缩前消息中提取少量路径级上下文，帮助压缩后续跑保持方向。
///
/// 这里只恢复文件路径和工具动作，不恢复正文内容，避免把大文件重新塞回上下文。
pub fn build_compaction_rehydration_context(messages: &[Value]) -> String {
    let mut entries = Vec::new();
    for message in messages {
        collect_openai_file_tool_entries(message, &mut entries);
        collect_anthropic_file_tool_entries(message, &mut entries);
    }

    const MAX_REHYDRATED_FILE_CONTEXT_ENTRIES: usize = 8;
    let mut unique_recent = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for entry in entries.into_iter().rev() {
        let key = format!("{}:{}", entry.tool_name, entry.path);
        if seen.insert(key) {
            unique_recent.push(entry);
        }
        if unique_recent.len() >= MAX_REHYDRATED_FILE_CONTEXT_ENTRIES {
            break;
        }
    }
    unique_recent.reverse();

    if unique_recent.is_empty() {
        return String::new();
    }

    let mut lines = vec![
        "## 已恢复的近期文件上下文".to_string(),
        "以下是压缩前最近的文件级工具线索，仅用于续跑定位；完整内容以当前磁盘文件为准。"
            .to_string(),
    ];
    for entry in unique_recent {
        lines.push(format!("- {}: {}", entry.tool_name, entry.path));
    }
    lines.join("\n")
}

fn strip_compaction_transcript_header(content: &str) -> &str {
    let Some(rest) = content.strip_prefix("[对话已压缩。完整记录:") else {
        return content;
    };

    let Some((_, summary)) = rest.split_once(']') else {
        return content;
    };

    summary
}

#[derive(Debug)]
struct FileToolEntry {
    tool_name: String,
    path: String,
}

fn collect_openai_file_tool_entries(message: &Value, entries: &mut Vec<FileToolEntry>) {
    for tool_call in message["tool_calls"].as_array().into_iter().flatten() {
        let tool_name = tool_call["function"]["name"].as_str().unwrap_or_default();
        let Some(path) = extract_file_path_from_tool_input(&tool_call["function"]["arguments"])
        else {
            continue;
        };
        if is_rehydration_file_tool(tool_name) {
            entries.push(FileToolEntry {
                tool_name: tool_name.to_string(),
                path,
            });
        }
    }
}

fn collect_anthropic_file_tool_entries(message: &Value, entries: &mut Vec<FileToolEntry>) {
    for block in message["content"].as_array().into_iter().flatten() {
        if block["type"].as_str() != Some("tool_use") {
            continue;
        }
        let tool_name = block["name"].as_str().unwrap_or_default();
        let Some(path) = extract_file_path_from_tool_input(&block["input"]) else {
            continue;
        };
        if is_rehydration_file_tool(tool_name) {
            entries.push(FileToolEntry {
                tool_name: tool_name.to_string(),
                path,
            });
        }
    }
}

fn is_rehydration_file_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "read_file"
            | "write_file"
            | "edit"
            | "file_stat"
            | "file_copy"
            | "file_move"
            | "file_delete"
            | "document_analyze"
            | "open_in_folder"
    )
}

fn extract_file_path_from_tool_input(input: &Value) -> Option<String> {
    let value = if let Some(text) = input.as_str() {
        serde_json::from_str::<Value>(text).ok()?
    } else {
        input.clone()
    };

    for key in [
        "path",
        "file_path",
        "target_path",
        "source_path",
        "absolute_path",
    ] {
        if let Some(path) = value[key]
            .as_str()
            .map(str::trim)
            .filter(|path| !path.is_empty())
        {
            return Some(path.to_string());
        }
    }
    None
}

/// 将完整对话记录以 JSONL 格式保存到磁盘
///
/// 文件名格式：`{session_id}_{timestamp}.jsonl`
/// 每行是一条消息的 JSON 序列化。
pub fn save_transcript(
    transcript_dir: &PathBuf,
    session_id: &str,
    messages: &[Value],
) -> Result<PathBuf> {
    std::fs::create_dir_all(transcript_dir)?;
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!(
        "{}_{}.jsonl",
        transcript_filename_slug(session_id),
        timestamp
    );
    let path = transcript_dir.join(&filename);

    let content: String = messages
        .iter()
        .map(|m| serde_json::to_string(m).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");

    std::fs::write(&path, content)?;
    Ok(path)
}

fn transcript_filename_slug(session_id: &str) -> String {
    let slug: String = session_id
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '_',
        })
        .collect();
    let slug = slug.trim_matches('_');
    if slug.is_empty() {
        "session".to_string()
    } else {
        slug.chars().take(96).collect()
    }
}

/// 调用 LLM 生成结构化摘要，将 messages 替换为压缩版本
///
/// 摘要以用户消息形式注入，附加完整记录文件路径，
/// 后跟一条 assistant 消息表示已接收上下文。
pub async fn auto_compact(
    api_format: &str,
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: &[Value],
    transcript_path: &str,
) -> Result<Vec<Value>> {
    // 将所有消息序列化为可读文本
    let conversation_text: String = messages
        .iter()
        .map(|m| {
            let role = m["role"].as_str().unwrap_or("unknown");
            // content 可能是字符串也可能是数组（tool_use / tool_result blocks）
            let content = if let Some(s) = m["content"].as_str() {
                s.to_string()
            } else {
                serde_json::to_string(&m["content"]).unwrap_or_default()
            };
            format!("[{}]: {}", role, content)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let user_prompt = format!("{}\n{}", COMPACT_USER_PROMPT, conversation_text);
    let summary_messages = vec![json!({"role": "user", "content": user_prompt})];

    // 调用 LLM 生成摘要（空工具列表，空回调）
    let transport = resolve_model_transport(api_format, base_url, None);
    let response = if transport.kind == ModelTransportKind::AnthropicMessages {
        adapters::anthropic::chat_stream_with_tools(
            base_url,
            api_key,
            model,
            COMPACT_SYSTEM_PROMPT,
            summary_messages,
            vec![],
            |_| {},
        )
        .await?
    } else {
        adapters::openai::chat_stream_with_tools(
            &transport,
            base_url,
            api_key,
            model,
            COMPACT_SYSTEM_PROMPT,
            summary_messages,
            vec![],
            |_| {},
        )
        .await?
    };

    let summary = match response {
        super::types::LLMResponse::Text(text) => text,
        super::types::LLMResponse::TextWithToolCalls(text, _) => text,
        // 摘要请求不应返回工具调用；如果出现则退化为错误提示
        super::types::LLMResponse::ToolCalls(_) => "摘要生成失败：LLM 返回了工具调用".to_string(),
    };

    let rehydration_context = build_compaction_rehydration_context(messages);
    let summary_with_context = if rehydration_context.trim().is_empty() {
        summary
    } else {
        format!("{}\n\n{}", summary.trim(), rehydration_context)
    };

    // 用摘要替换整个消息列表，保留完整记录路径的引用
    Ok(vec![
        json!({
            "role": "user",
            "content": format!(
                "[对话已压缩。完整记录: {}]\n\n{}",
                transcript_path, summary_with_context
            )
        }),
        json!({
            "role": "assistant",
            "content": "已了解之前的对话上下文，准备继续工作。"
        }),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_needs_auto_compact_below_threshold() {
        assert!(!needs_auto_compact(0));
        assert!(!needs_auto_compact(49_999));
        assert!(!needs_auto_compact(50_000));
    }

    #[test]
    fn test_needs_auto_compact_above_threshold() {
        assert!(needs_auto_compact(50_001));
        assert!(needs_auto_compact(100_000));
    }

    #[test]
    fn test_save_transcript_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let messages = vec![
            json!({"role": "user", "content": "你好"}),
            json!({"role": "assistant", "content": "你好！有什么可以帮助你的？"}),
        ];

        let path = save_transcript(&dir.path().to_path_buf(), "session-123", &messages).unwrap();

        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        // 验证每行是合法的 JSON
        for line in &lines {
            assert!(serde_json::from_str::<serde_json::Value>(line).is_ok());
        }
    }

    #[test]
    fn test_save_transcript_filename_contains_session_id() {
        let dir = tempfile::tempdir().unwrap();
        let messages = vec![json!({"role": "user", "content": "test"})];

        let path = save_transcript(&dir.path().to_path_buf(), "my-session-456", &messages).unwrap();
        let filename = path.file_name().unwrap().to_string_lossy();

        assert!(filename.starts_with("my-session-456_"));
        assert!(filename.ends_with(".jsonl"));
    }

    #[test]
    fn test_save_transcript_creates_directory() {
        let dir = tempfile::tempdir().unwrap();
        let nested_dir = dir.path().join("transcripts").join("nested");
        let messages = vec![json!({"role": "user", "content": "test"})];

        // nested_dir 尚不存在，save_transcript 应自动创建
        let path = save_transcript(&nested_dir, "sess", &messages).unwrap();
        assert!(path.exists());
    }
}
