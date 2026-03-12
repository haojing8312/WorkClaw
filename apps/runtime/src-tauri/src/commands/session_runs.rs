use super::skills::DbState;
use crate::session_journal::{SessionJournalStore, SessionRunEvent};
use chrono::Utc;
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow, PartialEq, Eq)]
pub struct SessionRunProjection {
    pub id: String,
    pub session_id: String,
    pub user_message_id: String,
    pub assistant_message_id: Option<String>,
    pub status: String,
    pub buffered_text: String,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn list_session_runs_with_pool(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Vec<SessionRunProjection>, String> {
    sqlx::query_as::<_, SessionRunProjection>(
        "SELECT id, session_id, user_message_id,
                NULLIF(assistant_message_id, '') AS assistant_message_id,
                status, buffered_text,
                NULLIF(error_kind, '') AS error_kind,
                NULLIF(error_message, '') AS error_message,
                created_at, updated_at
         FROM session_runs
         WHERE session_id = ?
         ORDER BY created_at ASC, id ASC",
    )
    .bind(session_id.trim())
    .fetch_all(pool)
    .await
    .map_err(|e| format!("读取会话运行记录失败: {e}"))
}

#[tauri::command]
pub async fn list_session_runs(
    session_id: String,
    db: State<'_, DbState>,
) -> Result<Vec<SessionRunProjection>, String> {
    list_session_runs_with_pool(&db.0, &session_id).await
}

pub async fn append_session_run_event_with_pool(
    pool: &SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    event: SessionRunEvent,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let run_id = event_run_id(&event).to_string();
    let event_type = event_type(&event);
    let payload_json =
        serde_json::to_string(&event).map_err(|e| format!("序列化 session run event 失败: {e}"))?;

    journal.append_event(session_id, event.clone()).await?;

    sqlx::query(
        "INSERT INTO session_run_events (id, run_id, session_id, event_type, payload_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&run_id)
    .bind(session_id)
    .bind(event_type)
    .bind(payload_json)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("写入 session run event 失败: {e}"))?;

    match event {
        SessionRunEvent::RunStarted {
            run_id,
            user_message_id,
        } => {
            sqlx::query(
                "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
                 VALUES (?, ?, ?, '', ?, '', '', '', ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                    session_id = excluded.session_id,
                    user_message_id = excluded.user_message_id,
                    status = excluded.status,
                    assistant_message_id = excluded.assistant_message_id,
                    error_kind = '',
                    error_message = '',
                    updated_at = excluded.updated_at",
            )
            .bind(run_id)
            .bind(session_id)
            .bind(user_message_id)
            .bind("thinking")
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("写入 session run 启动投影失败: {e}"))?;
        }
        SessionRunEvent::AssistantChunkAppended { run_id, chunk } => {
            sqlx::query(
                "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
                 VALUES (?, ?, '', '', ?, ?, '', '', ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                    session_id = excluded.session_id,
                    status = excluded.status,
                    buffered_text = COALESCE(session_runs.buffered_text, '') || excluded.buffered_text,
                    updated_at = excluded.updated_at",
            )
            .bind(run_id)
            .bind(session_id)
            .bind("thinking")
            .bind(chunk)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("写入 session run 内容投影失败: {e}"))?;
        }
        SessionRunEvent::ToolStarted { run_id, .. } => {
            upsert_run_status(pool, &run_id, session_id, "tool_calling", &now, None, None).await?;
        }
        SessionRunEvent::ToolCompleted { run_id, .. } => {
            upsert_run_status(pool, &run_id, session_id, "thinking", &now, None, None).await?;
        }
        SessionRunEvent::RunCompleted { run_id } => {
            upsert_run_status(pool, &run_id, session_id, "completed", &now, None, None).await?;
        }
        SessionRunEvent::RunFailed {
            run_id,
            error_kind,
            error_message,
        } => {
            upsert_run_status(
                pool,
                &run_id,
                session_id,
                "failed",
                &now,
                Some(error_kind),
                Some(error_message),
            )
            .await?;
        }
        SessionRunEvent::RunCancelled { run_id, reason } => {
            upsert_run_status(
                pool,
                &run_id,
                session_id,
                "cancelled",
                &now,
                Some("cancelled".to_string()),
                reason,
            )
            .await?;
        }
    }

    Ok(())
}

pub async fn attach_assistant_message_to_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    assistant_message_id: &str,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE session_runs
         SET assistant_message_id = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(assistant_message_id.trim())
    .bind(&now)
    .bind(run_id.trim())
    .execute(pool)
    .await
    .map_err(|e| format!("绑定 session run 助手消息失败: {e}"))?;

    if result.rows_affected() == 0 {
        return Err(format!(
            "绑定 session run 助手消息失败: 未找到运行记录 {}",
            run_id
        ));
    }

    Ok(())
}

async fn upsert_run_status(
    pool: &SqlitePool,
    run_id: &str,
    session_id: &str,
    status: &str,
    now: &str,
    error_kind: Option<String>,
    error_message: Option<String>,
) -> Result<(), String> {
    let error_kind = error_kind.unwrap_or_default();
    let error_message = error_message.unwrap_or_default();

    sqlx::query(
        "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
         VALUES (?, ?, '', '', ?, '', ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            session_id = excluded.session_id,
            status = excluded.status,
            error_kind = excluded.error_kind,
            error_message = excluded.error_message,
            updated_at = excluded.updated_at",
    )
    .bind(run_id)
    .bind(session_id)
    .bind(status)
    .bind(error_kind)
    .bind(error_message)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| format!("写入 session run 状态投影失败: {e}"))?;

    Ok(())
}

fn event_type(event: &SessionRunEvent) -> &'static str {
    match event {
        SessionRunEvent::RunStarted { .. } => "run_started",
        SessionRunEvent::AssistantChunkAppended { .. } => "assistant_chunk_appended",
        SessionRunEvent::ToolStarted { .. } => "tool_started",
        SessionRunEvent::ToolCompleted { .. } => "tool_completed",
        SessionRunEvent::RunCompleted { .. } => "run_completed",
        SessionRunEvent::RunFailed { .. } => "run_failed",
        SessionRunEvent::RunCancelled { .. } => "run_cancelled",
    }
}

fn event_run_id(event: &SessionRunEvent) -> &str {
    match event {
        SessionRunEvent::RunStarted { run_id, .. }
        | SessionRunEvent::AssistantChunkAppended { run_id, .. }
        | SessionRunEvent::ToolStarted { run_id, .. }
        | SessionRunEvent::ToolCompleted { run_id, .. }
        | SessionRunEvent::RunCompleted { run_id }
        | SessionRunEvent::RunFailed { run_id, .. }
        | SessionRunEvent::RunCancelled { run_id, .. } => run_id.as_str(),
    }
}
