use crate::agent::runtime::events::ToolConfirmResponder;
use crate::agent::runtime::kernel::execution_plan::ExecutionOutcome;
use crate::agent::runtime::runtime_io::{
    append_run_failed_with_pool, append_run_started_with_pool,
};
use crate::agent::runtime::task_backend::{
    execute_prepared_task_backend_with_context, prepare_task_backend,
    InteractiveChatTaskBackendPreparationRequest, PreparedTaskBackendSurface,
    TaskBackendExecutionContext, TaskBackendPreparationRequest, TaskBackendTokenCallback,
};
use crate::agent::runtime::task_lifecycle::{self, TaskBeginParentContext};
use crate::agent::runtime::task_record::TaskRecord;
use crate::agent::runtime::task_state::TaskState;
use crate::agent::AgentExecutor;
use crate::session_journal::SessionJournalStore;
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;

#[derive(Debug, Clone)]
pub(crate) struct TaskExecutionOutcome {
    pub task_state: TaskState,
    pub active_task_record: TaskRecord,
    pub execution_outcome: ExecutionOutcome,
}

pub(crate) struct DelegatedTaskBackendRunRequest<'a, F>
where
    F: FnOnce(&mut PreparedTaskBackendSurface),
{
    pub db: &'a sqlx::SqlitePool,
    pub journal: &'a SessionJournalStore,
    pub task_state: TaskState,
    pub parent_context: Option<TaskBeginParentContext<'a>>,
    pub preparation_request: TaskBackendPreparationRequest<'a>,
    pub app_handle: Option<AppHandle>,
    pub agent_executor: Arc<AgentExecutor>,
    pub on_token: TaskBackendTokenCallback,
    pub prepare_surface: F,
}

impl TaskExecutionOutcome {
    pub(crate) fn new(
        task_state: TaskState,
        active_task_record: TaskRecord,
        execution_outcome: ExecutionOutcome,
    ) -> Self {
        Self {
            task_state,
            active_task_record,
            execution_outcome,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct StartedRunContext<'a> {
    pub run_id: &'a str,
    pub user_message_id: &'a str,
}

#[derive(Clone, Copy)]
struct TaskBackendFailureContext<'a> {
    db: &'a sqlx::SqlitePool,
    journal: &'a SessionJournalStore,
    session_id: &'a str,
    run_id: &'a str,
    run_failure_kind: &'a str,
    active_task_record: &'a TaskRecord,
}

pub(crate) struct ActiveTaskBackendRunRequest<'a, F>
where
    F: FnOnce(&mut PreparedTaskBackendSurface),
{
    pub db: &'a sqlx::SqlitePool,
    pub journal: &'a SessionJournalStore,
    pub task_state: TaskState,
    pub parent_context: Option<TaskBeginParentContext<'a>>,
    pub started_run: StartedRunContext<'a>,
    pub preparation_request: TaskBackendPreparationRequest<'a>,
    pub prepare_surface: F,
    pub execution_context: TaskBackendExecutionContext<'a>,
}

pub(crate) struct PrimaryLocalChatTaskRunRequest<'a> {
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

async fn handle_task_backend_failure(context: TaskBackendFailureContext<'_>, error: &str) {
    append_run_failed_with_pool(
        context.db,
        context.journal,
        context.session_id,
        context.run_id,
        context.run_failure_kind,
        error,
        None,
    )
    .await;
    let _ = task_lifecycle::mark_task_failed(
        context.db,
        context.journal,
        context.session_id,
        context.active_task_record,
        error,
    )
    .await;
}

async fn prepare_task_backend_for_active_task<'a>(
    request: TaskBackendPreparationRequest<'a>,
    failure_context: TaskBackendFailureContext<'_>,
) -> Result<PreparedTaskBackendSurface, String> {
    match prepare_task_backend(request).await {
        Ok(prepared_surface) => Ok(prepared_surface),
        Err(error) => {
            handle_task_backend_failure(failure_context, &error).await;
            Err(error)
        }
    }
}

async fn execute_prepared_task_backend_for_active_task<'a>(
    prepared_surface: &'a PreparedTaskBackendSurface,
    execution_context: TaskBackendExecutionContext<'a>,
    failure_context: TaskBackendFailureContext<'_>,
) -> Result<ExecutionOutcome, String> {
    match execute_prepared_task_backend_with_context(prepared_surface, execution_context).await {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            handle_task_backend_failure(failure_context, &error).await;
            Err(error)
        }
    }
}

async fn prepare_and_execute_backend_for_active_task<'a, F>(
    preparation_request: TaskBackendPreparationRequest<'a>,
    prepare_surface: F,
    execution_context: TaskBackendExecutionContext<'a>,
    failure_context: TaskBackendFailureContext<'_>,
) -> Result<ExecutionOutcome, String>
where
    F: FnOnce(&mut PreparedTaskBackendSurface),
{
    let mut prepared_surface =
        prepare_task_backend_for_active_task(preparation_request, failure_context).await?;
    prepare_surface(&mut prepared_surface);
    execute_prepared_task_backend_for_active_task(
        &prepared_surface,
        execution_context,
        failure_context,
    )
    .await
}

pub(crate) async fn run_task_backend_with_task_state<'a, F>(
    request: ActiveTaskBackendRunRequest<'a, F>,
) -> Result<TaskExecutionOutcome, String>
where
    F: FnOnce(&mut PreparedTaskBackendSurface),
{
    let ActiveTaskBackendRunRequest {
        db,
        journal,
        task_state,
        parent_context,
        started_run,
        preparation_request,
        prepare_surface,
        execution_context,
    } = request;
    let session_id = task_state.session_id.clone();
    let generic_failure_kind = preparation_request.generic_error_kind();
    let active_task_record =
        task_lifecycle::begin_task_run(db, journal, &task_state, parent_context).await?;

    if let Err(error) = append_run_started_with_pool(
        db,
        journal,
        &session_id,
        started_run.run_id,
        started_run.user_message_id,
    )
    .await
    {
        let _ =
            task_lifecycle::mark_task_failed(db, journal, &session_id, &active_task_record, &error)
                .await;
        return Err(error);
    }

    let execution_outcome = prepare_and_execute_backend_for_active_task(
        preparation_request,
        prepare_surface,
        execution_context,
        TaskBackendFailureContext {
            db,
            journal,
            session_id: &session_id,
            run_id: started_run.run_id,
            run_failure_kind: generic_failure_kind,
            active_task_record: &active_task_record,
        },
    )
    .await?;

    Ok(TaskExecutionOutcome::new(
        task_state,
        active_task_record,
        execution_outcome,
    ))
}

pub(crate) async fn run_primary_local_chat_task(
    request: PrimaryLocalChatTaskRunRequest<'_>,
) -> Result<TaskExecutionOutcome, String> {
    let task_state = TaskState::new_primary_local_chat(
        request.session_id,
        request.user_message_id,
        request.run_id,
    );
    run_task_backend_with_task_state(ActiveTaskBackendRunRequest {
        db: request.db,
        journal: request.journal,
        task_state,
        parent_context: None,
        started_run: StartedRunContext {
            run_id: request.run_id,
            user_message_id: request.user_message_id,
        },
        preparation_request: TaskBackendPreparationRequest::InteractiveChat(
            InteractiveChatTaskBackendPreparationRequest {
                app: request.app,
                agent_executor: request.agent_executor,
                db: request.db,
                session_id: request.session_id,
                user_message: request.user_message,
                user_message_parts: request.user_message_parts,
                max_iterations_override: request.max_iterations_override,
            },
        ),
        prepare_surface: |_| {},
        execution_context: TaskBackendExecutionContext::InteractiveChat {
            app: request.app.clone(),
            agent_executor: Arc::clone(request.agent_executor),
            db: request.db,
            journal: request.journal,
            session_id: request.session_id,
            run_id: request.run_id,
            user_message: request.user_message,
            cancel_flag: request.cancel_flag,
            tool_confirm_responder: request.tool_confirm_responder,
        },
    })
    .await
}

pub(crate) async fn run_delegated_task_backend<F>(
    request: DelegatedTaskBackendRunRequest<'_, F>,
) -> Result<TaskExecutionOutcome, String>
where
    F: FnOnce(&mut PreparedTaskBackendSurface),
{
    let session_id = request.task_state.session_id.clone();
    let run_id = request.task_state.run_id.clone();
    let user_message_id = request.task_state.user_message_id.clone();
    run_task_backend_with_task_state(ActiveTaskBackendRunRequest {
        db: request.db,
        journal: request.journal,
        task_state: request.task_state,
        parent_context: request.parent_context,
        started_run: StartedRunContext {
            run_id: &run_id,
            user_message_id: &user_message_id,
        },
        preparation_request: request.preparation_request,
        prepare_surface: request.prepare_surface,
        execution_context: TaskBackendExecutionContext::Delegated {
            app_handle: request.app_handle,
            agent_executor: Arc::clone(&request.agent_executor),
            session_id: &session_id,
            on_token: request.on_token,
        },
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::TaskExecutionOutcome;
    use crate::agent::runtime::kernel::execution_plan::ExecutionOutcome;
    use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
    use crate::agent::runtime::task_record::TaskRecord;
    use crate::agent::runtime::task_state::TaskState;

    #[test]
    fn task_execution_outcome_keeps_task_state_and_execution_outcome_together() {
        let task_state = TaskState::new_primary_local_chat("session-1", "user-1", "run-1");
        let task_record = TaskRecord::new_pending(
            task_state.task_identity.clone(),
            task_state.task_kind,
            task_state.surface_kind,
            task_state.backend_kind,
            task_state.session_id.clone(),
            task_state.user_message_id.clone(),
            task_state.run_id.clone(),
            "2026-04-10T10:00:00Z",
        );

        let wrapped = TaskExecutionOutcome::new(
            task_state.clone(),
            task_record.clone(),
            ExecutionOutcome::DirectDispatch {
                output: "done".to_string(),
                turn_state: TurnStateSnapshot::default(),
            },
        );

        assert_eq!(wrapped.task_state, task_state);
        assert_eq!(wrapped.active_task_record, task_record);
        assert!(matches!(
            wrapped.execution_outcome,
            ExecutionOutcome::DirectDispatch { .. }
        ));
    }
}
