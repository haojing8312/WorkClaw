use crate::agent::run_guard::RunStopReason;
use crate::session_journal::{SessionJournalStore, SessionRunEvent};
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

pub(crate) async fn append_session_run_event_with_pool(
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
        SessionRunEvent::TaskContinued { .. }
        | SessionRunEvent::TaskStateProjected { .. }
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

pub(crate) async fn attach_assistant_message_to_run_with_pool(
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
        SessionRunEvent::TaskContinued { .. } => "task_continued",
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
        SessionRunEvent::TaskContinued { run_id, .. }
        | SessionRunEvent::TaskStateProjected { run_id, .. }
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

fn format_run_stop_message(stop_reason: &RunStopReason) -> String {
    let mut lines = vec![stop_reason.message.clone()];
    if let Some(detail) = stop_reason.detail.as_deref() {
        if !detail.trim().is_empty() && detail != stop_reason.message {
            lines.push(detail.to_string());
        }
    }
    if let Some(last_completed_step) = stop_reason.last_completed_step.as_deref() {
        if !last_completed_step.trim().is_empty() {
            lines.push(format!("最后完成步骤：{last_completed_step}"));
        }
    }
    lines.join("\n")
}
