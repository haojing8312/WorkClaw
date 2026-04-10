use crate::agent::runtime::events::ToolConfirmResponder;
use crate::agent::runtime::task_active_run::{
    run_delegated_task_backend as run_active_delegated_task_backend,
    run_primary_local_chat_task as run_active_primary_local_chat_task,
    DelegatedTaskBackendRunRequest, PrimaryLocalChatTaskRunRequest, TaskExecutionOutcome,
};
use crate::agent::runtime::task_terminal::{
    finalize_delegated_task_execution_outcome, finalize_primary_task_execution_outcome,
    DelegatedTaskTerminalFinalizeRequest, DelegatedTaskTerminalOutcome,
};
use crate::agent::AgentExecutor;
use crate::session_journal::SessionJournalStore;
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;

pub(crate) struct DelegatedTaskBackendRunAndFinalizeRequest<'a, F>
where
    F: FnOnce(&mut crate::agent::runtime::task_backend::PreparedTaskBackendSurface),
{
    pub backend_request: DelegatedTaskBackendRunRequest<'a, F>,
}

pub(crate) struct DelegatedTaskTerminalFinalizeEntryRequest<'a> {
    pub db: &'a sqlx::SqlitePool,
    pub journal: &'a SessionJournalStore,
    pub task_execution_outcome: TaskExecutionOutcome,
}

pub(crate) struct PrimaryLocalChatTaskRunAndFinalizeRequest<'a> {
    pub app: &'a AppHandle,
    pub agent_executor: &'a Arc<AgentExecutor>,
    pub db: &'a sqlx::SqlitePool,
    pub journal: &'a SessionJournalStore,
    pub session_id: &'a str,
    pub run_id: &'a str,
    pub user_message_id: &'a str,
    pub user_message: &'a str,
    pub user_message_parts: &'a [Value],
    pub max_iterations_override: Option<usize>,
    pub cancel_flag: Arc<AtomicBool>,
    pub tool_confirm_responder: ToolConfirmResponder,
}

pub(crate) async fn run_and_finalize_primary_local_chat_task(
    request: PrimaryLocalChatTaskRunAndFinalizeRequest<'_>,
) -> Result<(), String> {
    let PrimaryLocalChatTaskRunAndFinalizeRequest {
        app,
        agent_executor,
        db,
        journal,
        session_id,
        run_id,
        user_message_id,
        user_message,
        user_message_parts,
        max_iterations_override,
        cancel_flag,
        tool_confirm_responder,
    } = request;

    let task_execution_outcome =
        run_active_primary_local_chat_task(PrimaryLocalChatTaskRunRequest {
            app,
            agent_executor,
            db,
            journal,
            session_id,
            run_id,
            user_message_id,
            user_message,
            user_message_parts,
            max_iterations_override,
            cancel_flag,
            tool_confirm_responder,
        })
        .await?;

    finalize_primary_task_execution_outcome(
        app,
        db,
        journal,
        session_id,
        run_id,
        task_execution_outcome,
    )
    .await
}

pub(crate) async fn finalize_delegated_task_execution_outcome_entry(
    request: DelegatedTaskTerminalFinalizeEntryRequest<'_>,
) -> Result<DelegatedTaskTerminalOutcome, String> {
    finalize_delegated_task_execution_outcome(DelegatedTaskTerminalFinalizeRequest {
        db: request.db,
        journal: request.journal,
        task_execution_outcome: request.task_execution_outcome,
    })
    .await
}

pub(crate) async fn run_and_finalize_delegated_task_backend<F>(
    request: DelegatedTaskBackendRunAndFinalizeRequest<'_, F>,
) -> Result<DelegatedTaskTerminalOutcome, String>
where
    F: FnOnce(&mut crate::agent::runtime::task_backend::PreparedTaskBackendSurface),
{
    let DelegatedTaskBackendRunAndFinalizeRequest { backend_request } = request;
    let db = backend_request.db;
    let journal = backend_request.journal;

    let task_execution_outcome = run_active_delegated_task_backend(backend_request).await?;

    finalize_delegated_task_execution_outcome(DelegatedTaskTerminalFinalizeRequest {
        db,
        journal,
        task_execution_outcome,
    })
    .await
}
