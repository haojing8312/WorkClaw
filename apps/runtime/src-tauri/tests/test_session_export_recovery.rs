mod helpers;

use runtime_lib::commands::chat::export_session_markdown_with_pool;
use runtime_lib::commands::session_runs::{
    append_session_run_event_with_pool, attach_assistant_message_to_run_with_pool,
};
use runtime_lib::session_journal::{SessionJournalStore, SessionRunEvent};
use serde_json::json;

#[tokio::test]
async fn export_session_uses_journal_when_sqlite_projection_is_incomplete() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let journal_dir = tempfile::tempdir().expect("create journal dir");
    let journal = SessionJournalStore::new(journal_dir.path().to_path_buf());

    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("sess-export")
    .bind("builtin-general")
    .bind("恢复导出测试")
    .bind("2026-03-11T00:00:00Z")
    .bind("model-1")
    .bind("standard")
    .bind("")
    .bind("")
    .bind("general")
    .bind("")
    .execute(&pool)
    .await
    .expect("insert session");

    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("msg-user-1")
    .bind("sess-export")
    .bind("user")
    .bind("继续执行")
    .bind("2026-03-11T00:00:01Z")
    .execute(&pool)
    .await
    .expect("insert user message");

    journal
        .append_event(
            "sess-export",
            SessionRunEvent::RunStarted {
                run_id: "run-export-1".into(),
                user_message_id: "msg-user-1".into(),
            },
        )
        .await
        .expect("append run started");
    journal
        .append_event(
            "sess-export",
            SessionRunEvent::AssistantChunkAppended {
                run_id: "run-export-1".into(),
                chunk: "已经生成 2 个文件".into(),
            },
        )
        .await
        .expect("append partial output");
    journal
        .append_event(
            "sess-export",
            SessionRunEvent::RunFailed {
                run_id: "run-export-1".into(),
                error_kind: "billing".into(),
                error_message: "模型余额不足".into(),
            },
        )
        .await
        .expect("append run failed");

    let markdown = export_session_markdown_with_pool(&pool, "sess-export", Some(&journal))
        .await
        .expect("export markdown");

    assert!(markdown.contains("# 恢复导出测试"));
    assert!(markdown.contains("## 用户 (2026-03-11T00:00:01Z)"));
    assert!(markdown.contains("继续执行"));
    assert!(markdown.contains("恢复的运行记录"));
    assert!(markdown.contains("run-export-1"));
    assert!(markdown.contains("已经生成 2 个文件"));
    assert!(markdown.contains("模型余额不足"));
}

#[tokio::test]
async fn export_session_formats_structured_assistant_content_as_readable_markdown() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("sess-structured")
    .bind("builtin-general")
    .bind("结构化导出测试")
    .bind("2026-03-11T00:10:00Z")
    .bind("model-1")
    .bind("standard")
    .bind("")
    .bind("")
    .bind("general")
    .bind("")
    .execute(&pool)
    .await
    .expect("insert session");

    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("msg-user-structured")
    .bind("sess-structured")
    .bind("user")
    .bind("请继续执行")
    .bind("2026-03-11T00:10:01Z")
    .execute(&pool)
    .await
    .expect("insert user message");

    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("msg-assistant-structured")
    .bind("sess-structured")
    .bind("assistant")
    .bind(r#"{"text":"已经生成 2 个文件","items":[{"type":"text","content":"已经生成 2 个文件"}]}"#)
    .bind("2026-03-11T00:10:02Z")
    .execute(&pool)
    .await
    .expect("insert assistant message");

    let markdown = export_session_markdown_with_pool(&pool, "sess-structured", None)
        .await
        .expect("export markdown");

    assert!(markdown.contains("# 结构化导出测试"));
    assert!(markdown.contains("## 助手 (2026-03-11T00:10:02Z)"));
    assert!(markdown.contains("已经生成 2 个文件"));
    assert!(!markdown.contains(r#""text":"已经生成 2 个文件""#));
    assert!(!markdown.contains(r#""items""#));
}

#[tokio::test]
async fn export_session_includes_tool_call_outputs_from_structured_assistant_content() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("sess-tool-export")
    .bind("builtin-general")
    .bind("工具导出测试")
    .bind("2026-03-11T00:20:00Z")
    .bind("model-1")
    .bind("standard")
    .bind("")
    .bind("")
    .bind("general")
    .bind("")
    .execute(&pool)
    .await
    .expect("insert session");

    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("msg-user-tool-export")
    .bind("sess-tool-export")
    .bind("user")
    .bind("请继续执行")
    .bind("2026-03-11T00:20:01Z")
    .execute(&pool)
    .await
    .expect("insert user message");

    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("msg-assistant-tool-export")
    .bind("sess-tool-export")
    .bind("assistant")
    .bind(
        r##"{"text":"现在让我创建需求文档（brief），并为你提供话题角度选项：","items":[{"type":"text","content":"现在让我创建需求文档（brief），并为你提供话题角度选项："},{"type":"tool_call","toolCall":{"id":"call-1","name":"write_file","input":{"path":"C:\\Users\\36443\\WorkClaw\\workspace\\brief.md","content":"# brief"},"status":"completed","output":"工具执行错误：路径 C:\\Users\\36443\\WorkClaw\\workspace\\brief.md 的父目录不存在"}}]}"##,
    )
    .bind("2026-03-11T00:20:02Z")
    .execute(&pool)
    .await
    .expect("insert assistant message");

    let markdown = export_session_markdown_with_pool(&pool, "sess-tool-export", None)
        .await
        .expect("export markdown");

    assert!(markdown.contains("# 工具导出测试"));
    assert!(markdown.contains("## 助手 (2026-03-11T00:20:02Z)"));
    assert!(markdown.contains("write_file"));
    assert!(markdown.contains("brief.md"));
    assert!(markdown.contains("工具执行错误"));
}

#[tokio::test]
async fn export_session_renders_structured_tool_outputs_readably() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("sess-structured-tool-output")
    .bind("builtin-general")
    .bind("结构化工具输出导出测试")
    .bind("2026-03-11T00:21:00Z")
    .bind("model-1")
    .bind("standard")
    .bind("")
    .bind("")
    .bind("general")
    .bind("")
    .execute(&pool)
    .await
    .expect("insert session");

    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("msg-user-structured-tool-output")
    .bind("sess-structured-tool-output")
    .bind("user")
    .bind("请继续执行")
    .bind("2026-03-11T00:21:01Z")
    .execute(&pool)
    .await
    .expect("insert user message");

    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("msg-assistant-structured-tool-output")
    .bind("sess-structured-tool-output")
    .bind("assistant")
    .bind(
        r##"{"text":"我来创建 brief 文件","items":[{"type":"tool_call","toolCall":{"id":"call-1","name":"write_file","input":{},"status":"completed","output":"{\"ok\":true,\"tool\":\"write_file\",\"summary\":\"成功写入 7 字节到 brief.md\",\"details\":{\"path\":\"C:\\Users\\36443\\WorkClaw\\workspace\\brief.md\",\"bytes_written\":7}}"}}]}"##,
    )
    .bind("2026-03-11T00:21:02Z")
    .execute(&pool)
    .await
    .expect("insert assistant message");

    let markdown = export_session_markdown_with_pool(&pool, "sess-structured-tool-output", None)
        .await
        .expect("export markdown");

    assert!(markdown.contains("成功写入 7 字节到 brief.md"));
    assert!(markdown.contains("brief.md"));
}

#[tokio::test]
async fn export_session_includes_tool_events_linked_to_assistant_run() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let journal_dir = tempfile::tempdir().expect("create journal dir");
    let journal = SessionJournalStore::new(journal_dir.path().to_path_buf());

    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("sess-tool-events")
    .bind("builtin-general")
    .bind("工具事件导出测试")
    .bind("2026-03-11T00:30:00Z")
    .bind("model-1")
    .bind("standard")
    .bind("")
    .bind("")
    .bind("general")
    .bind("")
    .execute(&pool)
    .await
    .expect("insert session");

    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("msg-user-tool-events")
    .bind("sess-tool-events")
    .bind("user")
    .bind("请继续执行")
    .bind("2026-03-11T00:30:01Z")
    .execute(&pool)
    .await
    .expect("insert user message");

    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("msg-assistant-tool-events")
    .bind("sess-tool-events")
    .bind("assistant")
    .bind("让我检查正确的目录路径：")
    .bind("2026-03-11T00:30:02Z")
    .execute(&pool)
    .await
    .expect("insert assistant message");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-tool-events",
        SessionRunEvent::RunStarted {
            run_id: "run-tool-events".into(),
            user_message_id: "msg-user-tool-events".into(),
        },
    )
    .await
    .expect("append run started");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-tool-events",
        SessionRunEvent::ToolStarted {
            run_id: "run-tool-events".into(),
            tool_name: "write_file".into(),
            call_id: "call-1".into(),
            input: json!({
                "path": "C:\\Users\\36443\\WorkClaw\\workspace\\brief.md",
                "content": "# brief"
            }),
        },
    )
    .await
    .expect("append tool started");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-tool-events",
        SessionRunEvent::ToolCompleted {
            run_id: "run-tool-events".into(),
            tool_name: "write_file".into(),
            call_id: "call-1".into(),
            input: json!({
                "path": "C:\\Users\\36443\\WorkClaw\\workspace\\brief.md",
                "content": "# brief"
            }),
            output:
                "工具执行错误：路径 C:\\Users\\36443\\WorkClaw\\workspace\\brief.md 的父目录不存在"
                    .into(),
            is_error: true,
        },
    )
    .await
    .expect("append tool completed");

    attach_assistant_message_to_run_with_pool(
        &pool,
        "run-tool-events",
        "msg-assistant-tool-events",
    )
    .await
    .expect("attach assistant message");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-tool-events",
        SessionRunEvent::RunCompleted {
            run_id: "run-tool-events".into(),
        },
    )
    .await
    .expect("append run completed");

    let markdown = export_session_markdown_with_pool(&pool, "sess-tool-events", Some(&journal))
        .await
        .expect("export markdown");

    assert!(markdown.contains("让我检查正确的目录路径"));
    assert!(markdown.contains("write_file"));
    assert!(markdown.contains("brief.md"));
    assert!(markdown.contains("工具执行错误"));
}

#[tokio::test]
async fn export_session_does_not_duplicate_failed_run_recovery_when_assistant_message_exists() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let journal_dir = tempfile::tempdir().expect("create journal dir");
    let journal = SessionJournalStore::new(journal_dir.path().to_path_buf());

    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("sess-max-turns-with-assistant")
    .bind("builtin-general")
    .bind("最大步数导出保持执行记录")
    .bind("2026-03-29T00:30:00Z")
    .bind("model-1")
    .bind("standard")
    .bind("")
    .bind("")
    .bind("general")
    .bind("")
    .execute(&pool)
    .await
    .expect("insert session");

    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("msg-user-max-turns")
    .bind("sess-max-turns-with-assistant")
    .bind("user")
    .bind("继续执行")
    .bind("2026-03-29T00:30:01Z")
    .execute(&pool)
    .await
    .expect("insert user message");

    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("msg-assistant-max-turns")
    .bind("sess-max-turns-with-assistant")
    .bind("assistant")
    .bind(
        r##"{"text":"我先检查一下环境。","items":[{"type":"text","content":"我先检查一下环境。"},{"type":"tool_call","toolCall":{"id":"call-1","name":"bash","input":{"command":"echo hi"},"status":"completed","output":"hi"}}]}"##,
    )
    .bind("2026-03-29T00:30:02Z")
    .execute(&pool)
    .await
    .expect("insert assistant message");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-max-turns-with-assistant",
        SessionRunEvent::RunStarted {
            run_id: "run-max-turns".into(),
            user_message_id: "msg-user-max-turns".into(),
        },
    )
    .await
    .expect("append run started");

    attach_assistant_message_to_run_with_pool(
        &pool,
        "run-max-turns",
        "msg-assistant-max-turns",
    )
    .await
    .expect("attach assistant message");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-max-turns-with-assistant",
        SessionRunEvent::RunStopped {
            run_id: "run-max-turns".into(),
            stop_reason: runtime_lib::agent::run_guard::RunStopReason::max_turns(100),
        },
    )
    .await
    .expect("append run stopped");

    let markdown =
        export_session_markdown_with_pool(&pool, "sess-max-turns-with-assistant", Some(&journal))
            .await
            .expect("export markdown");

    assert!(markdown.contains("我先检查一下环境。"));
    assert!(markdown.contains("bash"));
    assert!(markdown.contains("hi"));
    assert!(!markdown.contains("## 恢复的运行记录"));
    assert!(!markdown.contains("### Run run-max-turns (failed)"));
}

#[tokio::test]
async fn export_session_recovery_includes_tool_events_for_failed_run_without_assistant_message() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let journal_dir = tempfile::tempdir().expect("create journal dir");
    let journal = SessionJournalStore::new(journal_dir.path().to_path_buf());

    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("sess-failed-tool-events")
    .bind("builtin-general")
    .bind("失败工具事件导出测试")
    .bind("2026-03-11T00:40:00Z")
    .bind("model-1")
    .bind("standard")
    .bind("")
    .bind("")
    .bind("general")
    .bind("")
    .execute(&pool)
    .await
    .expect("insert session");

    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("msg-user-failed-tool-events")
    .bind("sess-failed-tool-events")
    .bind("user")
    .bind("继续执行")
    .bind("2026-03-11T00:40:01Z")
    .execute(&pool)
    .await
    .expect("insert user message");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-failed-tool-events",
        SessionRunEvent::RunStarted {
            run_id: "run-failed-tool-events".into(),
            user_message_id: "msg-user-failed-tool-events".into(),
        },
    )
    .await
    .expect("append run started");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-failed-tool-events",
        SessionRunEvent::ToolStarted {
            run_id: "run-failed-tool-events".into(),
            tool_name: "write_file".into(),
            call_id: "call-1".into(),
            input: json!({
                "path": "C:\\Users\\36443\\WorkClaw\\workspace\\brief.md"
            }),
        },
    )
    .await
    .expect("append tool started");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-failed-tool-events",
        SessionRunEvent::ToolCompleted {
            run_id: "run-failed-tool-events".into(),
            tool_name: "write_file".into(),
            call_id: "call-1".into(),
            input: json!({
                "path": "C:\\Users\\36443\\WorkClaw\\workspace\\brief.md"
            }),
            output:
                "工具执行错误：路径 C:\\Users\\36443\\WorkClaw\\workspace\\brief.md 的父目录不存在"
                    .into(),
            is_error: true,
        },
    )
    .await
    .expect("append tool completed");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-failed-tool-events",
        SessionRunEvent::RunFailed {
            run_id: "run-failed-tool-events".into(),
            error_kind: "tool_error".into(),
            error_message: "write_file 执行失败".into(),
        },
    )
    .await
    .expect("append run failed");

    let markdown =
        export_session_markdown_with_pool(&pool, "sess-failed-tool-events", Some(&journal))
            .await
            .expect("export markdown");

    assert!(markdown.contains("恢复的运行记录"));
    assert!(markdown.contains("run-failed-tool-events"));
    assert!(markdown.contains("write_file"));
    assert!(markdown.contains("brief.md"));
    assert!(markdown.contains("工具执行错误"));
}

#[tokio::test]
async fn export_session_recovery_renders_structured_tool_event_outputs_readably() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let journal_dir = tempfile::tempdir().expect("create journal dir");
    let journal = SessionJournalStore::new(journal_dir.path().to_path_buf());

    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("sess-structured-run-events")
    .bind("builtin-general")
    .bind("结构化运行事件导出测试")
    .bind("2026-03-11T00:41:00Z")
    .bind("model-1")
    .bind("standard")
    .bind("")
    .bind("")
    .bind("general")
    .bind("")
    .execute(&pool)
    .await
    .expect("insert session");

    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("msg-user-structured-run-events")
    .bind("sess-structured-run-events")
    .bind("user")
    .bind("继续执行")
    .bind("2026-03-11T00:41:01Z")
    .execute(&pool)
    .await
    .expect("insert user message");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-structured-run-events",
        SessionRunEvent::RunStarted {
            run_id: "run-structured-tool-events".into(),
            user_message_id: "msg-user-structured-run-events".into(),
        },
    )
    .await
    .expect("append run started");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-structured-run-events",
        SessionRunEvent::ToolStarted {
            run_id: "run-structured-tool-events".into(),
            tool_name: "write_file".into(),
            call_id: "call-1".into(),
            input: json!({}),
        },
    )
    .await
    .expect("append tool started");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-structured-run-events",
        SessionRunEvent::ToolCompleted {
            run_id: "run-structured-tool-events".into(),
            tool_name: "write_file".into(),
            call_id: "call-1".into(),
            input: json!({}),
            output: json!({
                "ok": false,
                "tool": "write_file",
                "summary": "写入失败",
                "error_code": "MISSING_PATH",
                "error_message": "缺少 path 参数",
                "details": {
                    "path": "C:\\Users\\36443\\WorkClaw\\workspace\\brief.md"
                }
            })
            .to_string(),
            is_error: true,
        },
    )
    .await
    .expect("append tool completed");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-structured-run-events",
        SessionRunEvent::RunFailed {
            run_id: "run-structured-tool-events".into(),
            error_kind: "tool_error".into(),
            error_message: "write_file 执行失败".into(),
        },
    )
    .await
    .expect("append run failed");

    let markdown =
        export_session_markdown_with_pool(&pool, "sess-structured-run-events", Some(&journal))
            .await
            .expect("export markdown");

    assert!(markdown.contains("写入失败"));
    assert!(markdown.contains("缺少 path 参数"));
    assert!(markdown.contains("brief.md"));
}
