mod helpers;

use runtime_lib::commands::chat::export_session_markdown_with_pool;
use runtime_lib::session_journal::{SessionJournalStore, SessionRunEvent};

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
