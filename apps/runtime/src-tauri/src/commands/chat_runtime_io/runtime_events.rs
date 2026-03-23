use crate::agent::run_guard::RunStopReason;
use crate::session_journal::{SessionJournalStore, SessionRunEvent};
use chrono::Utc;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

pub(crate) async fn insert_session_message_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    role: &str,
    content: &str,
    content_json: Option<&str>,
) -> Result<String, String> {
    let msg_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, content_json, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&msg_id)
    .bind(session_id)
    .bind(role)
    .bind(content)
    .bind(content_json)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(msg_id)
}

pub(crate) async fn record_route_attempt_log_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    capability: &str,
    api_format: &str,
    model_name: &str,
    attempt_index: usize,
    retry_index: usize,
    error_kind: &str,
    success: bool,
    error_message: &str,
) {
    let _ = sqlx::query(
        "INSERT INTO route_attempt_logs (id, session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, success, error_message, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(capability)
    .bind(api_format)
    .bind(model_name)
    .bind(attempt_index as i64)
    .bind(retry_index as i64)
    .bind(error_kind)
    .bind(success)
    .bind(error_message)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await;
}

pub(crate) async fn append_run_started_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    user_message_id: &str,
) -> Result<(), String> {
    crate::commands::session_runs::append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunStarted {
            run_id: run_id.to_string(),
            user_message_id: user_message_id.to_string(),
        },
    )
    .await
}

pub(crate) async fn append_run_failed_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    error_kind: &str,
    error_message: &str,
) {
    let _ = crate::commands::session_runs::append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunFailed {
            run_id: run_id.to_string(),
            error_kind: error_kind.to_string(),
            error_message: error_message.to_string(),
        },
    )
    .await;
}

#[allow(dead_code)]
pub(crate) async fn append_run_guard_warning_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    warning_kind: &str,
    title: &str,
    message: &str,
    detail: Option<&str>,
    last_completed_step: Option<&str>,
) -> Result<(), String> {
    crate::commands::session_runs::append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunGuardWarning {
            run_id: run_id.to_string(),
            warning_kind: warning_kind.to_string(),
            title: title.to_string(),
            message: message.to_string(),
            detail: detail.map(str::to_string),
            last_completed_step: last_completed_step.map(str::to_string),
        },
    )
    .await
}

pub(crate) async fn append_run_stopped_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    stop_reason: &RunStopReason,
) -> Result<(), String> {
    crate::commands::session_runs::append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunStopped {
            run_id: run_id.to_string(),
            stop_reason: stop_reason.clone(),
        },
    )
    .await
}

pub(crate) async fn append_partial_assistant_chunk_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    chunk: &str,
) {
    let _ = crate::commands::session_runs::append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::AssistantChunkAppended {
            run_id: run_id.to_string(),
            chunk: chunk.to_string(),
        },
    )
    .await;
}

fn attach_reasoning_to_content(
    content: &str,
    final_text: &str,
    has_tool_calls: bool,
    reasoning_text: &str,
    reasoning_duration_ms: Option<u64>,
) -> String {
    if reasoning_text.trim().is_empty() {
        return content.to_string();
    }

    let base = if has_tool_calls {
        serde_json::from_str::<Value>(content).unwrap_or_else(|_| {
            json!({
                "text": final_text,
                "items": [],
            })
        })
    } else {
        json!({
            "text": final_text,
        })
    };

    let mut obj = base.as_object().cloned().unwrap_or_default();
    obj.insert(
        "reasoning".to_string(),
        json!({
            "status": "completed",
            "duration_ms": reasoning_duration_ms,
            "content": reasoning_text,
        }),
    );
    serde_json::to_string(&Value::Object(obj)).unwrap_or_else(|_| content.to_string())
}

pub(crate) async fn finalize_run_success_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    final_text: &str,
    has_tool_calls: bool,
    content: &str,
    reasoning_text: &str,
    reasoning_duration_ms: Option<u64>,
) -> Result<(), String> {
    if !final_text.is_empty() {
        crate::commands::session_runs::append_session_run_event_with_pool(
            pool,
            journal,
            session_id,
            SessionRunEvent::AssistantChunkAppended {
                run_id: run_id.to_string(),
                chunk: final_text.to_string(),
            },
        )
        .await?;
    }

    if !final_text.is_empty() || has_tool_calls {
        let persisted_content = attach_reasoning_to_content(
            content,
            final_text,
            has_tool_calls,
            reasoning_text,
            reasoning_duration_ms,
        );
        let msg_id = insert_session_message_with_pool(
            pool,
            session_id,
            "assistant",
            &persisted_content,
            None,
        )
        .await?;
        crate::commands::session_runs::attach_assistant_message_to_run_with_pool(
            pool, run_id, &msg_id,
        )
        .await?;
    }

    crate::commands::session_runs::append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunCompleted {
            run_id: run_id.to_string(),
        },
    )
    .await?;

    Ok(())
}

pub(crate) async fn maybe_handle_team_entry_pre_execution_with_pool(
    app: &AppHandle,
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    user_message_id: &str,
    user_message: &str,
) -> Result<bool, String> {
    let Some(group_run) =
        crate::commands::employee_agents::maybe_handle_team_entry_session_message_with_pool(
            pool,
            session_id,
            user_message,
        )
        .await?
    else {
        return Ok(false);
    };

    let run_id = Uuid::new_v4().to_string();
    append_run_started_with_pool(pool, journal, session_id, &run_id, user_message_id).await?;

    if !group_run.final_report.is_empty() {
        crate::commands::session_runs::append_session_run_event_with_pool(
            pool,
            journal,
            session_id,
            SessionRunEvent::AssistantChunkAppended {
                run_id: run_id.clone(),
                chunk: group_run.final_report.clone(),
            },
        )
        .await?;

        let assistant_msg_id = insert_session_message_with_pool(
            pool,
            session_id,
            "assistant",
            &group_run.final_report,
            None,
        )
        .await?;
        crate::commands::session_runs::attach_assistant_message_to_run_with_pool(
            pool,
            &run_id,
            &assistant_msg_id,
        )
        .await?;
    }

    crate::commands::session_runs::append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunCompleted { run_id },
    )
    .await?;

    let _ = app.emit(
        "stream-token",
        crate::commands::chat::StreamToken {
            session_id: session_id.to_string(),
            token: group_run.final_report.clone(),
            done: false,
            sub_agent: false,
        },
    );
    let _ = app.emit(
        "stream-token",
        crate::commands::chat::StreamToken {
            session_id: session_id.to_string(),
            token: String::new(),
            done: true,
            sub_agent: false,
        },
    );

    Ok(true)
}

#[cfg(test)]
mod run_guard_persistence_tests {
    use super::{
        append_run_guard_warning_with_pool, append_run_started_with_pool,
        append_run_stopped_with_pool,
    };
    use crate::agent::run_guard::RunStopReason;
    use crate::session_journal::SessionJournalStore;
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::tempdir;

    async fn setup_run_event_pool() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        sqlx::query(
            "CREATE TABLE session_runs (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                user_message_id TEXT NOT NULL DEFAULT '',
                assistant_message_id TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'queued',
                buffered_text TEXT NOT NULL DEFAULT '',
                error_kind TEXT NOT NULL DEFAULT '',
                error_message TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create session_runs table");

        sqlx::query(
            "CREATE TABLE session_run_events (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create session_run_events table");

        pool
    }

    #[tokio::test]
    async fn append_run_stopped_event_persists_loop_detected_reason() {
        let pool = setup_run_event_pool().await;
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());
        let stop_reason =
            RunStopReason::loop_detected("工具 browser_snapshot 已连续 6 次返回相同结果。")
                .with_last_completed_step("已填写封面标题");

        append_run_started_with_pool(&pool, &journal, "session-1", "run-1", "user-1")
            .await
            .expect("append run started");
        append_run_stopped_with_pool(&pool, &journal, "session-1", "run-1", &stop_reason)
            .await
            .expect("append run stopped");

        let (event_type, payload_json): (String, String) = sqlx::query_as(
            "SELECT event_type, payload_json
             FROM session_run_events
             WHERE run_id = 'run-1' AND event_type = 'run_stopped'
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("query run_stopped event");
        assert_eq!(event_type, "run_stopped");
        assert!(payload_json.contains("\"kind\":\"loop_detected\""));
        assert!(payload_json.contains("\"last_completed_step\":\"已填写封面标题\""));

        let (status, error_kind, error_message): (String, String, String) = sqlx::query_as(
            "SELECT status, error_kind, error_message
             FROM session_runs
             WHERE id = 'run-1'",
        )
        .fetch_one(&pool)
        .await
        .expect("query session run projection");
        assert_eq!(status, "failed");
        assert_eq!(error_kind, "loop_detected");
        assert!(error_message.contains("最后完成步骤：已填写封面标题"));
    }

    #[tokio::test]
    async fn append_run_guard_warning_event_persists_warning_payload() {
        let pool = setup_run_event_pool().await;
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());

        append_run_started_with_pool(&pool, &journal, "session-2", "run-2", "user-2")
            .await
            .expect("append run started");
        append_run_guard_warning_with_pool(
            &pool,
            &journal,
            "session-2",
            "run-2",
            "loop_detected",
            "任务可能即将卡住",
            "系统检测到连续重复步骤，若继续无变化将自动停止。",
            Some("工具 browser_snapshot 已连续 5 次使用相同输入执行。"),
            Some("已填写封面标题"),
        )
        .await
        .expect("append run guard warning");

        let (event_type, payload_json): (String, String) = sqlx::query_as(
            "SELECT event_type, payload_json
             FROM session_run_events
             WHERE run_id = 'run-2' AND event_type = 'run_guard_warning'
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("query run_guard_warning event");

        assert_eq!(event_type, "run_guard_warning");
        assert!(payload_json.contains("\"warning_kind\":\"loop_detected\""));
        assert!(payload_json.contains("\"last_completed_step\":\"已填写封面标题\""));
    }
}
