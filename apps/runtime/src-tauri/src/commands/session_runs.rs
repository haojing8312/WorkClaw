use super::skills::DbState;
use crate::agent::runtime::task_lineage::{build_task_path, effective_task_identity};
use crate::agent::runtime::trace_builder::{
    build_session_run_trace, summarize_stored_event, SessionRunEventSummary, SessionRunTrace,
    StoredSessionRunEvent,
};
use crate::session_journal::{
    SessionJournalStateHandle, SessionJournalStore, SessionRunEvent,
    SessionRunTaskIdentitySnapshot, SessionRunTurnStateSnapshot, SessionTaskRecordSnapshot,
};
use chrono::Utc;
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_identity: Option<SessionRunTaskIdentitySnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_state: Option<SessionRunTurnStateSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_record: Option<SessionRunTaskRecordProjection>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SessionRunTaskRecordProjection {
    pub task_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<String>,
    pub root_task_id: String,
    pub task_kind: String,
    pub surface_kind: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_reason: Option<String>,
}

impl From<&SessionTaskRecordSnapshot> for SessionRunTaskRecordProjection {
    fn from(value: &SessionTaskRecordSnapshot) -> Self {
        Self {
            task_id: value.task_identity.task_id.clone(),
            parent_task_id: value.task_identity.parent_task_id.clone(),
            root_task_id: value.task_identity.root_task_id.clone(),
            task_kind: value.task_identity.task_kind.clone(),
            surface_kind: value.task_identity.surface_kind.clone(),
            status: value.status.as_key().to_string(),
            created_at: value.created_at.clone(),
            updated_at: value.updated_at.clone(),
            started_at: value.started_at.clone(),
            completed_at: value.completed_at.clone(),
            terminal_reason: value.terminal_reason.clone(),
        }
    }
}

pub async fn list_session_runs_with_pool(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Vec<SessionRunProjection>, String> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            Option<String>,
            String,
            String,
            Option<String>,
            Option<String>,
            String,
            String,
        ),
    >(
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
    .map_err(|e| format!("读取会话运行记录失败: {e}"))?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                session_id,
                user_message_id,
                assistant_message_id,
                status,
                buffered_text,
                error_kind,
                error_message,
                created_at,
                updated_at,
            )| SessionRunProjection {
                id,
                session_id,
                user_message_id,
                assistant_message_id,
                status,
                buffered_text,
                error_kind,
                error_message,
                created_at,
                updated_at,
                task_identity: None,
                turn_state: None,
                task_path: None,
                task_status: None,
                task_record: None,
            },
        )
        .collect())
}

fn resolve_task_record_for_run<'a>(
    state: &'a crate::session_journal::SessionJournalState,
    snapshot: &crate::session_journal::SessionRunSnapshot,
) -> Option<&'a SessionTaskRecordSnapshot> {
    let effective_identity = effective_task_identity(
        snapshot.task_identity.as_ref(),
        snapshot.turn_state.as_ref(),
    );

    if let Some(task_identity) = effective_identity {
        return state
            .tasks
            .iter()
            .rev()
            .find(|task| task.task_identity.task_id == task_identity.task_id);
    }

    state
        .tasks
        .iter()
        .rev()
        .find(|task| task.run_id == snapshot.run_id)
}

pub async fn list_session_runs_with_runtime_state(
    pool: &SqlitePool,
    session_id: &str,
    journal: Option<&SessionJournalStore>,
) -> Result<Vec<SessionRunProjection>, String> {
    let mut runs = list_session_runs_with_pool(pool, session_id).await?;
    let Some(journal) = journal else {
        return Ok(runs);
    };
    let Ok(state) = journal.read_state(session_id).await else {
        return Ok(runs);
    };

    for run in &mut runs {
        if let Some(snapshot) = state.runs.iter().find(|snapshot| snapshot.run_id == run.id) {
            run.turn_state = snapshot.turn_state.clone();
            let effective_identity =
                effective_task_identity(snapshot.task_identity.as_ref(), run.turn_state.as_ref());
            run.task_identity = effective_identity.cloned();
            run.task_path = effective_identity.and_then(build_task_path);
            run.task_record = resolve_task_record_for_run(&state, snapshot)
                .map(SessionRunTaskRecordProjection::from);
            run.task_status = run.task_record.as_ref().map(|task| task.status.clone());
        }
    }

    Ok(runs)
}

#[tauri::command]
pub async fn list_session_runs(
    session_id: String,
    db: State<'_, DbState>,
    journal: State<'_, SessionJournalStateHandle>,
) -> Result<Vec<SessionRunProjection>, String> {
    list_session_runs_with_runtime_state(&db.0, &session_id, Some(journal.0.as_ref())).await
}

pub async fn list_session_run_events_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    run_id: Option<&str>,
    limit: Option<u32>,
) -> Result<Vec<SessionRunEventSummary>, String> {
    Ok(
        load_session_run_event_rows_with_pool(pool, session_id, run_id, limit)
            .await?
            .into_iter()
            .map(|record| summarize_stored_event(&record))
            .collect(),
    )
}

async fn load_session_run_event_rows_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    run_id: Option<&str>,
    limit: Option<u32>,
) -> Result<Vec<StoredSessionRunEvent>, String> {
    let mut sql = String::from(
        "SELECT session_id, run_id, event_type, payload_json, created_at
         FROM session_run_events
         WHERE session_id = ?",
    );
    if run_id.is_some() {
        sql.push_str(" AND run_id = ?");
    }
    sql.push_str(" ORDER BY created_at ASC, id ASC");
    if limit.is_some() {
        sql.push_str(" LIMIT ?");
    }

    let mut query =
        sqlx::query_as::<_, (String, String, String, String, String)>(&sql).bind(session_id.trim());
    if let Some(run_id) = run_id {
        query = query.bind(run_id.trim());
    }
    if let Some(limit) = limit {
        query = query.bind(i64::from(limit));
    }

    let rows = query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("读取会话运行事件失败: {e}"))?;

    Ok(rows
        .into_iter()
        .map(
            |(session_id, run_id, event_type, payload_json, created_at)| StoredSessionRunEvent {
                session_id,
                run_id,
                event_type,
                payload_json,
                created_at,
            },
        )
        .collect())
}

#[tauri::command]
pub async fn list_session_run_events(
    session_id: String,
    run_id: Option<String>,
    limit: Option<u32>,
    db: State<'_, DbState>,
) -> Result<Vec<SessionRunEventSummary>, String> {
    list_session_run_events_with_pool(&db.0, &session_id, run_id.as_deref(), limit).await
}

pub async fn export_session_run_trace_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    run_id: &str,
) -> Result<SessionRunTrace, String> {
    let events =
        load_session_run_event_rows_with_pool(pool, session_id, Some(run_id), None).await?;
    Ok(build_session_run_trace(
        session_id.trim(),
        run_id.trim(),
        &events,
    ))
}

#[tauri::command]
pub async fn export_session_run_trace(
    session_id: String,
    run_id: String,
    db: State<'_, DbState>,
) -> Result<SessionRunTrace, String> {
    export_session_run_trace_with_pool(&db.0, &session_id, &run_id).await
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
        SessionRunEvent::TaskStateProjected { .. }
        | SessionRunEvent::TaskDelegated { .. }
        | SessionRunEvent::TaskRecordUpserted { .. }
        | SessionRunEvent::TaskStatusChanged { .. } => {}
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
        SessionRunEvent::SkillRouteRecorded { .. } => {}
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
        SessionRunEvent::ApprovalRequested {
            run_id,
            approval_id,
            tool_name,
            call_id,
            input,
            summary,
            impact,
            irreversible,
        } => {
            sqlx::query(
                "INSERT INTO approvals (
                    id, session_id, run_id, call_id, tool_name, input_json, summary, impact,
                    irreversible, status, decision, notify_targets_json, resume_payload_json,
                    resolved_by_surface, resolved_by_user, resolved_at, resumed_at, expires_at,
                    created_at, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, '', '[]', '{}', '', '', NULL, NULL, NULL, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                    session_id = excluded.session_id,
                    run_id = excluded.run_id,
                    call_id = excluded.call_id,
                    tool_name = excluded.tool_name,
                    input_json = excluded.input_json,
                    summary = excluded.summary,
                    impact = excluded.impact,
                    irreversible = excluded.irreversible,
                    status = excluded.status,
                    updated_at = excluded.updated_at",
            )
            .bind(&approval_id)
            .bind(session_id)
            .bind(&run_id)
            .bind(&call_id)
            .bind(&tool_name)
            .bind(input.to_string())
            .bind(&summary)
            .bind(impact.unwrap_or_default())
            .bind(if irreversible { 1_i64 } else { 0_i64 })
            .bind("pending")
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("写入 approval 投影失败: {e}"))?;

            upsert_run_status(
                pool,
                &run_id,
                session_id,
                "waiting_approval",
                &now,
                None,
                None,
            )
            .await?;
        }
        SessionRunEvent::RunCompleted { run_id, .. } => {
            upsert_run_status(pool, &run_id, session_id, "completed", &now, None, None).await?;
        }
        SessionRunEvent::RunGuardWarning { .. } => {}
        SessionRunEvent::RunStopped {
            run_id,
            stop_reason,
            ..
        } => {
            upsert_run_status(
                pool,
                &run_id,
                session_id,
                "failed",
                &now,
                Some(stop_reason.kind.as_key().to_string()),
                Some(format_run_stop_message(&stop_reason)),
            )
            .await?;
        }
        SessionRunEvent::RunFailed {
            run_id,
            error_kind,
            error_message,
            ..
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
        SessionRunEvent::TaskStateProjected { .. } => "task_state_projected",
        SessionRunEvent::TaskDelegated { .. } => "task_delegated",
        SessionRunEvent::TaskRecordUpserted { .. } => "task_record_upserted",
        SessionRunEvent::TaskStatusChanged { .. } => "task_status_changed",
        SessionRunEvent::RunStarted { .. } => "run_started",
        SessionRunEvent::SkillRouteRecorded { .. } => "skill_route_recorded",
        SessionRunEvent::AssistantChunkAppended { .. } => "assistant_chunk_appended",
        SessionRunEvent::ToolStarted { .. } => "tool_started",
        SessionRunEvent::ToolCompleted { .. } => "tool_completed",
        SessionRunEvent::ApprovalRequested { .. } => "approval_requested",
        SessionRunEvent::RunCompleted { .. } => "run_completed",
        SessionRunEvent::RunGuardWarning { .. } => "run_guard_warning",
        SessionRunEvent::RunStopped { .. } => "run_stopped",
        SessionRunEvent::RunFailed { .. } => "run_failed",
        SessionRunEvent::RunCancelled { .. } => "run_cancelled",
    }
}

fn event_run_id(event: &SessionRunEvent) -> &str {
    match event {
        SessionRunEvent::TaskStateProjected { run_id, .. }
        | SessionRunEvent::TaskDelegated { run_id, .. }
        | SessionRunEvent::TaskRecordUpserted { run_id, .. }
        | SessionRunEvent::TaskStatusChanged { run_id, .. }
        | SessionRunEvent::RunStarted { run_id, .. }
        | SessionRunEvent::SkillRouteRecorded { run_id, .. }
        | SessionRunEvent::AssistantChunkAppended { run_id, .. }
        | SessionRunEvent::ToolStarted { run_id, .. }
        | SessionRunEvent::ToolCompleted { run_id, .. }
        | SessionRunEvent::ApprovalRequested { run_id, .. }
        | SessionRunEvent::RunCompleted { run_id, .. }
        | SessionRunEvent::RunGuardWarning { run_id, .. }
        | SessionRunEvent::RunStopped { run_id, .. }
        | SessionRunEvent::RunFailed { run_id, .. }
        | SessionRunEvent::RunCancelled { run_id, .. } => run_id.as_str(),
    }
}

fn format_run_stop_message(stop_reason: &crate::agent::run_guard::RunStopReason) -> String {
    let mut lines = vec![stop_reason.message.clone()];
    if let Some(detail) = stop_reason.detail.as_deref() {
        if !detail.trim().is_empty() && detail != stop_reason.message {
            lines.push(detail.to_string());
        }
    }
    if let Some(step) = stop_reason.last_completed_step.as_deref() {
        if !step.trim().is_empty() {
            lines.push(format!("最后完成步骤：{step}"));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{
        export_session_run_trace_with_pool, format_run_stop_message,
        list_session_run_events_with_pool,
    };
    use crate::agent::run_guard::RunStopReason;
    use crate::agent::runtime::task_repo::{TaskRecordUpsertPayload, TaskStatusChangedPayload};
    use crate::session_journal::{
        SessionJournalStore, SessionRunEvent, SessionRunTurnStateCompactionBoundary,
        SessionRunTurnStateSnapshot,
    };
    use serde_json::json;
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::tempdir;

    async fn setup_session_run_event_pool() -> sqlx::SqlitePool {
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

        sqlx::query(
            "CREATE TABLE approvals (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                call_id TEXT NOT NULL DEFAULT '',
                tool_name TEXT NOT NULL,
                input_json TEXT NOT NULL DEFAULT '{}',
                summary TEXT NOT NULL DEFAULT '',
                impact TEXT NOT NULL DEFAULT '',
                irreversible INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'pending',
                decision TEXT NOT NULL DEFAULT '',
                notify_targets_json TEXT NOT NULL DEFAULT '[]',
                resume_payload_json TEXT NOT NULL DEFAULT '{}',
                resolved_by_surface TEXT NOT NULL DEFAULT '',
                resolved_by_user TEXT NOT NULL DEFAULT '',
                resolved_at TEXT,
                resumed_at TEXT,
                expires_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create approvals table");

        pool
    }

    async fn insert_raw_session_run_event(
        pool: &sqlx::SqlitePool,
        id: &str,
        session_id: &str,
        run_id: &str,
        event_type: &str,
        payload_json: &str,
        created_at: &str,
    ) {
        sqlx::query(
            "INSERT INTO session_run_events (id, run_id, session_id, event_type, payload_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(run_id)
        .bind(session_id)
        .bind(event_type)
        .bind(payload_json)
        .bind(created_at)
        .execute(pool)
        .await
        .expect("insert session_run_event");
    }

    #[test]
    fn format_run_stop_message_includes_detail_and_step() {
        let reason = RunStopReason::policy_blocked(
            "目标路径不在当前工作目录范围内。你可以先切换当前会话的工作目录后重试。",
        )
        .with_last_completed_step("已读取当前工作区");

        let formatted = format_run_stop_message(&reason);

        assert!(formatted.contains("本次请求触发了安全或工作区限制"));
        assert!(formatted
            .contains("目标路径不在当前工作目录范围内。你可以先切换当前会话的工作目录后重试。"));
        assert!(formatted.contains("最后完成步骤：已读取当前工作区"));
    }

    #[tokio::test]
    async fn list_session_run_events_returns_chronological_session_events() {
        let pool = setup_session_run_event_pool().await;
        insert_raw_session_run_event(
            &pool,
            "evt-1",
            "session-1",
            "run-a",
            "run_started",
            &serde_json::to_string(&SessionRunEvent::RunStarted {
                run_id: "run-a".to_string(),
                user_message_id: "user-1".to_string(),
            })
            .expect("serialize run_started"),
            "2026-03-27T00:00:00Z",
        )
        .await;
        insert_raw_session_run_event(
            &pool,
            "evt-2",
            "session-1",
            "run-b",
            "run_started",
            &serde_json::to_string(&SessionRunEvent::RunStarted {
                run_id: "run-b".to_string(),
                user_message_id: "user-2".to_string(),
            })
            .expect("serialize run_started"),
            "2026-03-27T00:00:01Z",
        )
        .await;
        insert_raw_session_run_event(
            &pool,
            "evt-3",
            "session-1",
            "run-a",
            "tool_started",
            &serde_json::to_string(&SessionRunEvent::ToolStarted {
                run_id: "run-a".to_string(),
                tool_name: "shell_command".to_string(),
                call_id: "call-1".to_string(),
                input: json!({ "command": "pwd" }),
            })
            .expect("serialize tool_started"),
            "2026-03-27T00:00:02Z",
        )
        .await;
        insert_raw_session_run_event(
            &pool,
            "evt-4",
            "session-2",
            "run-x",
            "run_started",
            &serde_json::to_string(&SessionRunEvent::RunStarted {
                run_id: "run-x".to_string(),
                user_message_id: "user-x".to_string(),
            })
            .expect("serialize foreign run_started"),
            "2026-03-27T00:00:03Z",
        )
        .await;
        insert_raw_session_run_event(
            &pool,
            "evt-5",
            "session-1",
            "run-a",
            "run_completed",
            &serde_json::to_string(&SessionRunEvent::RunCompleted {
                run_id: "run-a".to_string(),
                turn_state: None,
            })
            .expect("serialize run_completed"),
            "2026-03-27T00:00:04Z",
        )
        .await;

        let events = list_session_run_events_with_pool(&pool, "session-1", None, None)
            .await
            .expect("list session run events");

        assert_eq!(events.len(), 4);
        assert_eq!(
            events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>(),
            vec![
                "run_started",
                "run_started",
                "tool_started",
                "run_completed"
            ]
        );
        assert_eq!(events[0].run_id, "run-a");
        assert_eq!(events[1].run_id, "run-b");
        assert_eq!(events[2].tool_name.as_deref(), Some("shell_command"));
    }

    #[tokio::test]
    async fn list_session_run_events_filters_by_run_id_and_limit() {
        let pool = setup_session_run_event_pool().await;
        for (id, created_at, payload_json, event_type) in [
            (
                "evt-1",
                "2026-03-27T00:10:00Z",
                serde_json::to_string(&SessionRunEvent::RunStarted {
                    run_id: "run-a".to_string(),
                    user_message_id: "user-1".to_string(),
                })
                .expect("serialize run-a started"),
                "run_started".to_string(),
            ),
            (
                "evt-2",
                "2026-03-27T00:10:01Z",
                serde_json::to_string(&SessionRunEvent::ToolStarted {
                    run_id: "run-a".to_string(),
                    tool_name: "read_file".to_string(),
                    call_id: "call-1".to_string(),
                    input: json!({ "path": "README.md" }),
                })
                .expect("serialize run-a tool"),
                "tool_started".to_string(),
            ),
            (
                "evt-3",
                "2026-03-27T00:10:02Z",
                serde_json::to_string(&SessionRunEvent::RunCompleted {
                    run_id: "run-a".to_string(),
                    turn_state: None,
                })
                .expect("serialize run-a completed"),
                "run_completed".to_string(),
            ),
            (
                "evt-4",
                "2026-03-27T00:10:03Z",
                serde_json::to_string(&SessionRunEvent::RunStarted {
                    run_id: "run-b".to_string(),
                    user_message_id: "user-2".to_string(),
                })
                .expect("serialize run-b started"),
                "run_started".to_string(),
            ),
        ] {
            insert_raw_session_run_event(
                &pool,
                id,
                "session-1",
                if id == "evt-4" { "run-b" } else { "run-a" },
                &event_type,
                &payload_json,
                created_at,
            )
            .await;
        }

        let events = list_session_run_events_with_pool(&pool, "session-1", Some("run-a"), Some(2))
            .await
            .expect("list filtered session run events");

        assert_eq!(events.len(), 2);
        assert_eq!(
            events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>(),
            vec!["run_started", "tool_started"]
        );
        assert!(events.iter().all(|event| event.run_id == "run-a"));
    }

    #[tokio::test]
    async fn export_session_run_trace_builds_structured_trace_for_run() {
        let pool = setup_session_run_event_pool().await;
        insert_raw_session_run_event(
            &pool,
            "evt-1",
            "session-1",
            "run-a",
            "run_started",
            &serde_json::to_string(&SessionRunEvent::RunStarted {
                run_id: "run-a".to_string(),
                user_message_id: "user-1".to_string(),
            })
            .expect("serialize run_started"),
            "2026-03-27T01:00:00Z",
        )
        .await;
        insert_raw_session_run_event(
            &pool,
            "evt-2",
            "session-1",
            "run-a",
            "tool_started",
            &serde_json::to_string(&SessionRunEvent::ToolStarted {
                run_id: "run-a".to_string(),
                tool_name: "read_file".to_string(),
                call_id: "call-1".to_string(),
                input: json!({ "path": "README.md" }),
            })
            .expect("serialize tool_started"),
            "2026-03-27T01:00:01Z",
        )
        .await;
        insert_raw_session_run_event(
            &pool,
            "evt-3",
            "session-1",
            "run-a",
            "tool_completed",
            &serde_json::to_string(&SessionRunEvent::ToolCompleted {
                run_id: "run-a".to_string(),
                tool_name: "read_file".to_string(),
                call_id: "call-1".to_string(),
                input: json!({ "path": "README.md" }),
                output: "README loaded".to_string(),
                is_error: false,
            })
            .expect("serialize tool_completed"),
            "2026-03-27T01:00:02Z",
        )
        .await;
        insert_raw_session_run_event(
            &pool,
            "evt-4",
            "session-1",
            "run-a",
            "run_completed",
            &serde_json::to_string(&SessionRunEvent::RunCompleted {
                run_id: "run-a".to_string(),
                turn_state: None,
            })
            .expect("serialize run_completed"),
            "2026-03-27T01:00:03Z",
        )
        .await;

        let trace = export_session_run_trace_with_pool(&pool, "session-1", "run-a")
            .await
            .expect("export session run trace");

        assert_eq!(trace.session_id, "session-1");
        assert_eq!(trace.run_id, "run-a");
        assert_eq!(trace.final_status, "completed");
        assert_eq!(trace.event_count, 4);
        assert_eq!(trace.tools.len(), 1);
        assert_eq!(trace.tools[0].status, "completed");
    }

    #[tokio::test]
    async fn export_session_run_trace_keeps_parse_warnings_for_bad_payload_rows() {
        let pool = setup_session_run_event_pool().await;
        insert_raw_session_run_event(
            &pool,
            "evt-1",
            "session-1",
            "run-b",
            "run_started",
            &serde_json::to_string(&SessionRunEvent::RunStarted {
                run_id: "run-b".to_string(),
                user_message_id: "user-2".to_string(),
            })
            .expect("serialize run_started"),
            "2026-03-27T02:00:00Z",
        )
        .await;
        insert_raw_session_run_event(
            &pool,
            "evt-2",
            "session-1",
            "run-b",
            "tool_started",
            "{\"type\":\"tool_started\",\"run_id\":",
            "2026-03-27T02:00:01Z",
        )
        .await;

        let trace = export_session_run_trace_with_pool(&pool, "session-1", "run-b")
            .await
            .expect("export trace with malformed payload");

        assert_eq!(trace.run_id, "run-b");
        assert_eq!(trace.event_count, 2);
        assert_eq!(trace.parse_warnings.len(), 1);
        assert!(trace.parse_warnings[0].contains("failed to parse payload_json"));
    }

    #[tokio::test]
    async fn list_session_runs_with_runtime_state_projects_turn_state_from_journal() {
        let pool = setup_session_run_event_pool().await;
        let journal_root = tempdir().expect("journal temp dir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());

        sqlx::query(
            "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
             VALUES (?, ?, ?, '', ?, ?, ?, ?, ?, ?)",
        )
        .bind("run-compacted")
        .bind("session-1")
        .bind("user-1")
        .bind("failed")
        .bind("已保留当前执行上下文")
        .bind("max_turns")
        .bind("已达到执行步数上限")
        .bind("2026-04-08T00:00:00Z")
        .bind("2026-04-08T00:00:01Z")
        .execute(&pool)
        .await
        .expect("insert session run");

        journal
            .append_event(
                "session-1",
                SessionRunEvent::RunStarted {
                    run_id: "run-compacted".to_string(),
                    user_message_id: "user-1".to_string(),
                },
            )
            .await
            .expect("append run started");
        journal
            .append_event(
                "session-1",
                SessionRunEvent::RunFailed {
                    run_id: "run-compacted".to_string(),
                    error_kind: "max_turns".to_string(),
                    error_message: "已达到执行步数上限".to_string(),
                    turn_state: Some(SessionRunTurnStateSnapshot {
                        task_identity: None,
                        session_surface: Some("employee_step_session".to_string()),
                        execution_lane: Some("open_task".to_string()),
                        selected_runner: Some("OpenTaskRunner".to_string()),
                        selected_skill: Some("builtin-general".to_string()),
                        fallback_reason: None,
                        allowed_tools: vec!["read".to_string(), "exec".to_string()],
                        invoked_skills: vec!["builtin-general".to_string()],
                        partial_assistant_text: "已保留当前执行上下文".to_string(),
                        tool_failure_streak: 0,
                        reconstructed_history_len: Some(7),
                        compaction_boundary: Some(SessionRunTurnStateCompactionBoundary {
                            transcript_path: "temp/transcripts/run-compacted.json".to_string(),
                            original_tokens: 4096,
                            compacted_tokens: 1024,
                            summary: "保留最近的文件修改计划和工具结果".to_string(),
                        }),
                    }),
                },
            )
            .await
            .expect("append run failed");

        let runs = super::list_session_runs_with_runtime_state(&pool, "session-1", Some(&journal))
            .await
            .expect("list session runs");

        assert_eq!(runs.len(), 1);
        assert_eq!(
            runs[0]
                .turn_state
                .as_ref()
                .and_then(|turn_state| turn_state.session_surface.as_deref()),
            Some("employee_step_session")
        );
        assert_eq!(
            runs[0]
                .turn_state
                .as_ref()
                .and_then(|turn_state| turn_state.compaction_boundary.as_ref())
                .map(|boundary| boundary.original_tokens),
            Some(4096)
        );
        assert_eq!(
            runs[0]
                .turn_state
                .as_ref()
                .and_then(|turn_state| turn_state.reconstructed_history_len),
            Some(7)
        );
    }

    #[tokio::test]
    async fn list_session_runs_with_runtime_state_projects_task_identity_from_journal() {
        let pool = setup_session_run_event_pool().await;
        let journal_root = tempdir().expect("journal temp dir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());

        sqlx::query(
            "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
             VALUES (?, ?, ?, '', ?, ?, ?, ?, ?, ?)",
        )
        .bind("run-tasked")
        .bind("session-1")
        .bind("user-1")
        .bind("thinking")
        .bind("")
        .bind("")
        .bind("")
        .bind("2026-04-09T00:00:00Z")
        .bind("2026-04-09T00:00:01Z")
        .execute(&pool)
        .await
        .expect("insert session run");

        journal
            .append_event(
                "session-1",
                SessionRunEvent::TaskStateProjected {
                    run_id: "run-tasked".to_string(),
                    task_identity: crate::session_journal::SessionRunTaskIdentitySnapshot {
                        task_id: "task-1".to_string(),
                        parent_task_id: None,
                        root_task_id: "task-1".to_string(),
                        task_kind: "primary_user_task".to_string(),
                        surface_kind: "local_chat_surface".to_string(),
                    },
                },
            )
            .await
            .expect("append task state projected");

        let runs = super::list_session_runs_with_runtime_state(&pool, "session-1", Some(&journal))
            .await
            .expect("list session runs");

        assert_eq!(runs.len(), 1);
        assert_eq!(
            runs[0]
                .task_identity
                .as_ref()
                .map(|task| task.task_id.as_str()),
            Some("task-1")
        );
        assert_eq!(
            runs[0]
                .task_identity
                .as_ref()
                .map(|task| task.task_kind.as_str()),
            Some("primary_user_task")
        );
        assert_eq!(runs[0].task_path.as_deref(), Some("task-1"));
        assert_eq!(runs[0].task_status, None);
    }

    #[tokio::test]
    async fn list_session_runs_with_runtime_state_projects_task_status_from_task_records() {
        let pool = setup_session_run_event_pool().await;
        let journal_root = tempdir().expect("journal temp dir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());

        sqlx::query(
            "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
             VALUES (?, ?, ?, '', ?, ?, ?, ?, ?, ?)",
        )
        .bind("run-task-status")
        .bind("session-1")
        .bind("user-1")
        .bind("failed")
        .bind("保留输出")
        .bind("max_turns")
        .bind("stopped")
        .bind("2026-04-09T00:00:00Z")
        .bind("2026-04-09T00:00:01Z")
        .execute(&pool)
        .await
        .expect("insert session run");

        journal
            .append_event(
                "session-1",
                SessionRunEvent::TaskRecordUpserted {
                    run_id: "run-task-status".to_string(),
                    task: TaskRecordUpsertPayload {
                        task_identity: crate::agent::runtime::task_state::TaskIdentity::new(
                            "task-1",
                            Option::<String>::None,
                            Some("task-1"),
                        ),
                        task_kind: crate::agent::runtime::task_state::TaskKind::PrimaryUserTask,
                        surface_kind:
                            crate::agent::runtime::task_state::TaskSurfaceKind::LocalChatSurface,
                        session_id: "session-1".to_string(),
                        user_message_id: "user-1".to_string(),
                        run_id: "run-task-status".to_string(),
                        status: crate::agent::runtime::task_record::TaskLifecycleStatus::Pending,
                        created_at: "2026-04-09T00:00:00Z".to_string(),
                        updated_at: "2026-04-09T00:00:00Z".to_string(),
                        started_at: None,
                        completed_at: None,
                        terminal_reason: None,
                    },
                },
            )
            .await
            .expect("append task upsert");
        journal
            .append_event(
                "session-1",
                SessionRunEvent::TaskStatusChanged {
                    run_id: "run-task-status".to_string(),
                    status_change: TaskStatusChangedPayload {
                        task_id: "task-1".to_string(),
                        parent_task_id: None,
                        root_task_id: "task-1".to_string(),
                        from_status:
                            crate::agent::runtime::task_record::TaskLifecycleStatus::Pending,
                        to_status: crate::agent::runtime::task_record::TaskLifecycleStatus::Failed,
                        terminal_reason: Some("max_turns".to_string()),
                        updated_at: "2026-04-09T00:00:02Z".to_string(),
                    },
                },
            )
            .await
            .expect("append task status changed");
        journal
            .append_event(
                "session-1",
                SessionRunEvent::TaskStateProjected {
                    run_id: "run-task-status".to_string(),
                    task_identity: crate::session_journal::SessionRunTaskIdentitySnapshot {
                        task_id: "task-1".to_string(),
                        parent_task_id: None,
                        root_task_id: "task-1".to_string(),
                        task_kind: "primary_user_task".to_string(),
                        surface_kind: "local_chat_surface".to_string(),
                    },
                },
            )
            .await
            .expect("append task state projected");

        let runs = super::list_session_runs_with_runtime_state(&pool, "session-1", Some(&journal))
            .await
            .expect("list session runs");

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].task_status.as_deref(), Some("failed"));
    }

    #[tokio::test]
    async fn list_session_runs_with_runtime_state_projects_task_record_summary() {
        let pool = setup_session_run_event_pool().await;
        let journal_root = tempdir().expect("journal temp dir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());

        sqlx::query(
            "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
             VALUES (?, ?, ?, '', ?, ?, ?, ?, ?, ?)",
        )
        .bind("run-task-record")
        .bind("session-1")
        .bind("user-1")
        .bind("failed")
        .bind("保留输出")
        .bind("max_turns")
        .bind("stopped")
        .bind("2026-04-09T00:00:00Z")
        .bind("2026-04-09T00:00:02Z")
        .execute(&pool)
        .await
        .expect("insert session run");

        journal
            .append_event(
                "session-1",
                SessionRunEvent::TaskRecordUpserted {
                    run_id: "run-task-record".to_string(),
                    task: TaskRecordUpsertPayload {
                        task_identity: crate::agent::runtime::task_state::TaskIdentity::new(
                            "task-1",
                            Option::<String>::None,
                            Some("task-1"),
                        ),
                        task_kind: crate::agent::runtime::task_state::TaskKind::PrimaryUserTask,
                        surface_kind:
                            crate::agent::runtime::task_state::TaskSurfaceKind::LocalChatSurface,
                        session_id: "session-1".to_string(),
                        user_message_id: "user-1".to_string(),
                        run_id: "run-task-record".to_string(),
                        status: crate::agent::runtime::task_record::TaskLifecycleStatus::Running,
                        created_at: "2026-04-09T00:00:00Z".to_string(),
                        updated_at: "2026-04-09T00:00:01Z".to_string(),
                        started_at: Some("2026-04-09T00:00:01Z".to_string()),
                        completed_at: None,
                        terminal_reason: None,
                    },
                },
            )
            .await
            .expect("append task upsert");
        journal
            .append_event(
                "session-1",
                SessionRunEvent::TaskStatusChanged {
                    run_id: "run-task-record".to_string(),
                    status_change: TaskStatusChangedPayload {
                        task_id: "task-1".to_string(),
                        parent_task_id: None,
                        root_task_id: "task-1".to_string(),
                        from_status:
                            crate::agent::runtime::task_record::TaskLifecycleStatus::Running,
                        to_status: crate::agent::runtime::task_record::TaskLifecycleStatus::Failed,
                        terminal_reason: Some("max_turns".to_string()),
                        updated_at: "2026-04-09T00:00:02Z".to_string(),
                    },
                },
            )
            .await
            .expect("append task status changed");
        journal
            .append_event(
                "session-1",
                SessionRunEvent::TaskStateProjected {
                    run_id: "run-task-record".to_string(),
                    task_identity: crate::session_journal::SessionRunTaskIdentitySnapshot {
                        task_id: "task-1".to_string(),
                        parent_task_id: None,
                        root_task_id: "task-1".to_string(),
                        task_kind: "primary_user_task".to_string(),
                        surface_kind: "local_chat_surface".to_string(),
                    },
                },
            )
            .await
            .expect("append task state projected");

        let runs = super::list_session_runs_with_runtime_state(&pool, "session-1", Some(&journal))
            .await
            .expect("list session runs");

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].task_status.as_deref(), Some("failed"));
        assert_eq!(
            runs[0]
                .task_record
                .as_ref()
                .map(|record| record.task_kind.as_str()),
            Some("primary_user_task")
        );
        assert_eq!(
            runs[0]
                .task_record
                .as_ref()
                .map(|record| record.surface_kind.as_str()),
            Some("local_chat_surface")
        );
        assert_eq!(
            runs[0]
                .task_record
                .as_ref()
                .map(|record| record.status.as_str()),
            Some("failed")
        );
        assert_eq!(
            runs[0]
                .task_record
                .as_ref()
                .and_then(|record| record.started_at.as_deref()),
            Some("2026-04-09T00:00:01Z")
        );
        assert_eq!(
            runs[0]
                .task_record
                .as_ref()
                .and_then(|record| record.completed_at.as_deref()),
            Some("2026-04-09T00:00:02Z")
        );
        assert_eq!(
            runs[0]
                .task_record
                .as_ref()
                .and_then(|record| record.terminal_reason.as_deref()),
            Some("max_turns")
        );
    }

    #[tokio::test]
    async fn list_session_runs_with_runtime_state_falls_back_to_turn_state_task_identity() {
        let pool = setup_session_run_event_pool().await;
        let journal_root = tempdir().expect("journal temp dir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());

        sqlx::query(
            "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
             VALUES (?, ?, ?, '', ?, ?, ?, ?, ?, ?)",
        )
        .bind("run-turn-tasked")
        .bind("session-1")
        .bind("user-1")
        .bind("failed")
        .bind("")
        .bind("max_turns")
        .bind("stopped")
        .bind("2026-04-09T00:00:00Z")
        .bind("2026-04-09T00:00:01Z")
        .execute(&pool)
        .await
        .expect("insert session run");

        journal
            .append_event(
                "session-1",
                SessionRunEvent::RunFailed {
                    run_id: "run-turn-tasked".to_string(),
                    error_kind: "max_turns".to_string(),
                    error_message: "stopped".to_string(),
                    turn_state: Some(SessionRunTurnStateSnapshot {
                        task_identity: Some(
                            crate::session_journal::SessionRunTaskIdentitySnapshot {
                                task_id: "task-child".to_string(),
                                parent_task_id: Some("task-parent".to_string()),
                                root_task_id: "task-root".to_string(),
                                task_kind: "sub_agent_task".to_string(),
                                surface_kind: "hidden_child_surface".to_string(),
                            },
                        ),
                        session_surface: Some("hidden_child_session".to_string()),
                        execution_lane: Some("open_task".to_string()),
                        selected_runner: Some("OpenTaskRunner".to_string()),
                        selected_skill: None,
                        fallback_reason: None,
                        allowed_tools: vec!["read".to_string()],
                        invoked_skills: Vec::new(),
                        partial_assistant_text: String::new(),
                        tool_failure_streak: 0,
                        reconstructed_history_len: Some(2),
                        compaction_boundary: None,
                    }),
                },
            )
            .await
            .expect("append run failed");

        let runs = super::list_session_runs_with_runtime_state(&pool, "session-1", Some(&journal))
            .await
            .expect("list session runs");

        assert_eq!(runs.len(), 1);
        assert_eq!(
            runs[0]
                .task_identity
                .as_ref()
                .map(|task| task.task_id.as_str()),
            Some("task-child")
        );
        assert_eq!(
            runs[0]
                .task_identity
                .as_ref()
                .and_then(|task| task.parent_task_id.as_deref()),
            Some("task-parent")
        );
        assert_eq!(
            runs[0].task_path.as_deref(),
            Some("task-root -> task-parent -> task-child")
        );
    }

    #[tokio::test]
    async fn export_session_run_trace_projects_task_graph_from_task_lineage() {
        let pool = setup_session_run_event_pool().await;
        insert_raw_session_run_event(
            &pool,
            "evt-1",
            "session-1",
            "run-task",
            "task_state_projected",
            &serde_json::to_string(&SessionRunEvent::TaskStateProjected {
                run_id: "run-task".to_string(),
                task_identity: crate::session_journal::SessionRunTaskIdentitySnapshot {
                    task_id: "task-child".to_string(),
                    parent_task_id: Some("task-parent".to_string()),
                    root_task_id: "task-root".to_string(),
                    task_kind: "sub_agent_task".to_string(),
                    surface_kind: "hidden_child_surface".to_string(),
                },
            })
            .expect("serialize task_state_projected"),
            "2026-04-09T00:00:00Z",
        )
        .await;
        insert_raw_session_run_event(
            &pool,
            "evt-2",
            "session-1",
            "run-task",
            "run_failed",
            &serde_json::to_string(&SessionRunEvent::RunFailed {
                run_id: "run-task".to_string(),
                error_kind: "max_turns".to_string(),
                error_message: "stopped".to_string(),
                turn_state: Some(SessionRunTurnStateSnapshot {
                    task_identity: Some(crate::session_journal::SessionRunTaskIdentitySnapshot {
                        task_id: "task-child".to_string(),
                        parent_task_id: Some("task-parent".to_string()),
                        root_task_id: "task-root".to_string(),
                        task_kind: "sub_agent_task".to_string(),
                        surface_kind: "hidden_child_surface".to_string(),
                    }),
                    session_surface: Some("hidden_child_session".to_string()),
                    execution_lane: Some("open_task".to_string()),
                    selected_runner: Some("OpenTaskRunner".to_string()),
                    selected_skill: None,
                    fallback_reason: None,
                    allowed_tools: vec!["read".to_string()],
                    invoked_skills: Vec::new(),
                    partial_assistant_text: String::new(),
                    tool_failure_streak: 0,
                    reconstructed_history_len: None,
                    compaction_boundary: None,
                }),
            })
            .expect("serialize run_failed"),
            "2026-04-09T00:00:01Z",
        )
        .await;

        let trace = export_session_run_trace_with_pool(&pool, "session-1", "run-task")
            .await
            .expect("export session run trace");

        assert_eq!(trace.task_graph.len(), 1);
        assert_eq!(trace.task_graph[0].task_id, "task-child");
        assert_eq!(
            trace.task_graph[0].task_path,
            "task-root -> task-parent -> task-child"
        );
    }
}
