use super::safety::critical_action_summary;
use crate::approval_bus::{ApprovalDecision, ApprovalManager, CreateApprovalRequest};
use crate::commands::chat::{ApprovalManagerState, PendingApprovalBridgeState};
use crate::commands::feishu_gateway::notify_feishu_approval_requested_with_pool;
use crate::commands::skills::DbState;
use crate::session_journal::{
    SessionJournalStateHandle, SessionJournalStore, SessionRunTaskContinuationSnapshot,
    SessionRunTaskIdentitySnapshot,
};
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

pub(super) const TOOL_CONFIRM_TIMEOUT_SECS: u64 = 15;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ToolConfirmationDecision {
    Confirmed,
    Rejected,
    TimedOut,
}

pub(super) fn wait_for_tool_confirmation(
    rx: &std::sync::mpsc::Receiver<bool>,
    timeout: std::time::Duration,
) -> ToolConfirmationDecision {
    match rx.recv_timeout(timeout) {
        Ok(true) => ToolConfirmationDecision::Confirmed,
        Ok(false) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            ToolConfirmationDecision::Rejected
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => ToolConfirmationDecision::TimedOut,
    }
}

#[derive(Clone)]
pub(super) struct ApprovalWaitRuntime {
    pub pool: sqlx::SqlitePool,
    pub journal: Arc<SessionJournalStore>,
    pub approval_manager: Arc<ApprovalManager>,
    pub pending_bridge: Arc<std::sync::Mutex<Option<String>>>,
}

pub(super) fn resolve_approval_wait_runtime(app: &AppHandle) -> Result<ApprovalWaitRuntime> {
    let db_state = app
        .try_state::<DbState>()
        .ok_or_else(|| anyhow!("DbState unavailable"))?;
    let journal_state = app
        .try_state::<SessionJournalStateHandle>()
        .ok_or_else(|| anyhow!("SessionJournalStateHandle unavailable"))?;
    let approval_state = app
        .try_state::<ApprovalManagerState>()
        .ok_or_else(|| anyhow!("ApprovalManagerState unavailable"))?;
    let pending_bridge = app
        .try_state::<PendingApprovalBridgeState>()
        .ok_or_else(|| anyhow!("PendingApprovalBridgeState unavailable"))?;

    Ok(ApprovalWaitRuntime {
        pool: db_state.0.clone(),
        journal: journal_state.0.clone(),
        approval_manager: approval_state.0.clone(),
        pending_bridge: pending_bridge.0.clone(),
    })
}

pub(super) async fn request_tool_approval_and_wait(
    runtime: &ApprovalWaitRuntime,
    app_handle: Option<&AppHandle>,
    session_id: &str,
    run_id: Option<&str>,
    task_identity: Option<SessionRunTaskIdentitySnapshot>,
    task_continuation: Option<SessionRunTaskContinuationSnapshot>,
    tool_name: &str,
    call_id: &str,
    input: &Value,
    work_dir: Option<&std::path::Path>,
    cancel_flag: Option<Arc<AtomicBool>>,
) -> Result<ApprovalDecision> {
    let approval_id = uuid::Uuid::new_v4().to_string();
    let receiver = runtime
        .approval_manager
        .register_waiter(approval_id.clone());
    let (title, summary, impact, irreversible) =
        critical_action_summary(tool_name, input, work_dir);

    let record = runtime
        .approval_manager
        .create_pending_with_pool(
            &runtime.pool,
            Some(runtime.journal.as_ref()),
            CreateApprovalRequest {
                approval_id: approval_id.clone(),
                session_id: session_id.to_string(),
                run_id: run_id.map(str::to_string),
                task_identity,
                task_continuation: task_continuation.clone(),
                call_id: call_id.to_string(),
                tool_name: tool_name.to_string(),
                input: input.clone(),
                summary: summary.clone(),
                impact: Some(impact.clone()),
                irreversible,
                work_dir: work_dir.map(|dir| dir.to_string_lossy().to_string()),
            },
        )
        .await
        .map_err(|err| anyhow!(err))?;

    if let Ok(mut guard) = runtime.pending_bridge.lock() {
        *guard = Some(approval_id.clone());
    }

    if let Some(app) = app_handle {
        let _ = app.emit(
            "approval-created",
            serde_json::json!({
                "approval_id": record.approval_id,
                "session_id": record.session_id,
                "run_id": record.run_id,
                "call_id": record.call_id,
                "tool_name": record.tool_name,
                "tool_input": record.input,
                "task_continuation": task_continuation,
                "title": title,
                "summary": record.summary,
                "impact": record.impact,
                "irreversible": record.irreversible,
                "status": record.status,
            }),
        );
        let _ = app.emit(
            "tool-confirm-event",
            serde_json::json!({
                "approval_id": approval_id,
                "session_id": session_id,
                "tool_name": tool_name,
                "tool_input": input,
                "task_continuation": task_continuation,
                "title": title,
                "summary": summary,
                "impact": impact,
                "irreversible": irreversible,
            }),
        );
    }

    let _ =
        notify_feishu_approval_requested_with_pool(&runtime.pool, session_id, &record, None).await;

    let resolution = runtime
        .approval_manager
        .wait_for_resolution(receiver, cancel_flag)
        .await
        .map_err(|err| anyhow!(err))?;

    if let Ok(mut guard) = runtime.pending_bridge.lock() {
        if guard.as_deref() == Some(resolution.approval_id.as_str()) {
            *guard = None;
        }
    }

    Ok(resolution.decision)
}
