use crate::agent::runtime::session_runs::append_session_run_event_with_pool;
use crate::agent::runtime::RunRegistryState;
use crate::approval_bus::mark_approved_tool_completion_resumed_with_pool;
use crate::commands::skills::DbState;
use crate::session_journal::{SessionJournalStateHandle, SessionRunEvent};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

pub(super) async fn resolve_current_session_run_id(
    app: &AppHandle,
    session_id: &str,
) -> Option<String> {
    if let Some(run_registry) = app.try_state::<RunRegistryState>() {
        if let Some(run_id) = run_registry.0.active_run_id(session_id) {
            return Some(run_id);
        }
    }

    let journal_state = app.try_state::<SessionJournalStateHandle>()?;
    journal_state
        .0
        .read_state(session_id)
        .await
        .ok()?
        .current_run_id
}

pub(super) async fn append_tool_run_event(
    app: &AppHandle,
    session_id: &str,
    event: SessionRunEvent,
) -> Result<()> {
    let db_state = app
        .try_state::<DbState>()
        .ok_or_else(|| anyhow!("DbState unavailable"))?;
    let journal_state = app
        .try_state::<SessionJournalStateHandle>()
        .ok_or_else(|| anyhow!("SessionJournalStateHandle unavailable"))?;

    let completed_tool = match &event {
        SessionRunEvent::ToolCompleted {
            run_id,
            tool_name,
            call_id,
            ..
        } => Some((run_id.clone(), tool_name.clone(), call_id.clone())),
        _ => None,
    };

    append_session_run_event_with_pool(&db_state.0, journal_state.0.as_ref(), session_id, event)
        .await
        .map_err(|err| anyhow!(err))?;

    if let Some((run_id, tool_name, call_id)) = completed_tool {
        mark_approved_tool_completion_resumed_with_pool(
            &db_state.0,
            session_id,
            &run_id,
            &call_id,
            &tool_name,
        )
        .await
        .map_err(|err| anyhow!(err))?;
    }

    Ok(())
}

pub(super) async fn append_run_guard_warning_event(
    app: &AppHandle,
    session_id: &str,
    warning: &crate::agent::run_guard::RunGuardWarning,
) -> Result<()> {
    let Some(run_id) = resolve_current_session_run_id(app, session_id).await else {
        return Ok(());
    };

    append_tool_run_event(
        app,
        session_id,
        SessionRunEvent::RunGuardWarning {
            run_id,
            warning_kind: warning.kind.as_key().to_string(),
            title: warning.title.clone(),
            message: warning.message.clone(),
            detail: warning.detail.clone(),
            last_completed_step: warning.last_completed_step.clone(),
        },
    )
    .await
}

pub fn build_skill_route_event(
    session_id: &str,
    route_run_id: &str,
    node_id: &str,
    parent_node_id: Option<String>,
    skill_name: &str,
    depth: usize,
    status: &str,
    duration_ms: Option<u64>,
    error_code: Option<&str>,
    error_message: Option<&str>,
) -> Value {
    json!({
        "session_id": session_id,
        "route_run_id": route_run_id,
        "node_id": node_id,
        "parent_node_id": parent_node_id,
        "skill_name": skill_name,
        "depth": depth,
        "status": status,
        "duration_ms": duration_ms,
        "error_code": error_code,
        "error_message": error_message,
    })
}
