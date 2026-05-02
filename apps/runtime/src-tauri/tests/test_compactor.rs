use runtime_lib::agent::compactor::build_compaction_rehydration_context;
use runtime_lib::agent::compactor::extract_compaction_display_summary;
use runtime_lib::agent::compactor::save_transcript;
use serde_json::json;

#[test]
fn compaction_display_summary_omits_transcript_header() {
    let messages = vec![
        json!({
            "role": "user",
            "content": "[对话已压缩。完整记录: D:\\code\\WorkClaw\\temp\\transcripts\\session_20260501_120000.jsonl]\n\n## 用户请求与意图\n继续打磨 agent 核心能力\n\n## 下一步\n验证恢复执行"
        }),
        json!({
            "role": "assistant",
            "content": "已了解之前的对话上下文，准备继续工作。"
        }),
    ];

    let summary = extract_compaction_display_summary(&messages);

    assert!(!summary.contains("完整记录"));
    assert!(!summary.contains("session_20260501_120000.jsonl"));
    assert!(summary.starts_with("## 用户请求与意图"));
    assert!(summary.contains("## 下一步"));
}

#[test]
fn compaction_display_summary_preserves_plain_user_summary() {
    let messages = vec![json!({
        "role": "user",
        "content": "## 当前工作状态\n没有压缩头的摘要"
    })];

    let summary = extract_compaction_display_summary(&messages);

    assert_eq!(summary, "## 当前工作状态\n没有压缩头的摘要");
}

#[test]
fn save_transcript_keeps_untrusted_session_ids_inside_transcript_dir() {
    let dir = tempfile::tempdir().expect("temp dir");
    let messages = vec![json!({"role": "user", "content": "hello"})];

    let path = save_transcript(
        &dir.path().to_path_buf(),
        "../evil\\session:with*bad?chars",
        &messages,
    )
    .expect("save transcript");

    assert_eq!(path.parent(), Some(dir.path()));
    let filename = path
        .file_name()
        .expect("file name")
        .to_string_lossy()
        .to_string();
    assert!(!filename.contains(".."));
    assert!(!filename.contains('/'));
    assert!(!filename.contains('\\'));
    assert!(!filename.contains(':'));
    assert!(filename.ends_with(".jsonl"));
}

#[test]
fn compaction_rehydration_context_preserves_recent_file_tool_paths() {
    let messages = vec![
        json!({
            "role": "assistant",
            "tool_calls": [
                {
                    "id": "call-read",
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "arguments": "{\"path\":\"apps/runtime/src-tauri/src/agent/compactor.rs\"}"
                    }
                }
            ]
        }),
        json!({
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    "id": "call-edit",
                    "name": "edit",
                    "input": {
                        "path": "apps/runtime/src-tauri/src/agent/runtime/compaction_pipeline.rs",
                        "old_string": "old",
                        "new_string": "new"
                    }
                }
            ]
        }),
        json!({
            "role": "assistant",
            "tool_calls": [
                {
                    "id": "call-write",
                    "type": "function",
                    "function": {
                        "name": "write_file",
                        "arguments": {
                            "path": "temp/notes.md",
                            "content": "large content that should not be rehydrated"
                        }
                    }
                }
            ]
        }),
    ];

    let context = build_compaction_rehydration_context(&messages);

    assert!(context.contains("## 已恢复的近期文件上下文"));
    assert!(context.contains("- read_file: apps/runtime/src-tauri/src/agent/compactor.rs"));
    assert!(
        context.contains("- edit: apps/runtime/src-tauri/src/agent/runtime/compaction_pipeline.rs")
    );
    assert!(context.contains("- write_file: temp/notes.md"));
    assert!(!context.contains("large content that should not be rehydrated"));
}
