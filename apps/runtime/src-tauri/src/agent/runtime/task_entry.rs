use crate::agent::run_guard::RunStopReason;
use crate::agent::runtime::events::ToolConfirmResponder;
use crate::agent::runtime::task_active_run::{
    run_task_backend_with_task_state, ActiveTaskBackendRunRequest, StartedRunContext,
};
use crate::agent::runtime::task_backend::{
    InteractiveChatTaskBackendPreparationRequest, PreparedTaskBackendSurface,
    TaskBackendExecutionContext, TaskBackendPreparationRequest, TaskBackendTokenCallback,
};
use crate::agent::runtime::task_continuation::{
    resolve_latest_task_run_continuation_contract, resolve_local_chat_continuation_contract,
    should_resume_local_chat_task,
};
use crate::agent::runtime::task_execution::TaskExecutionOutcome;
use crate::agent::runtime::task_lifecycle::{
    resolve_latest_task_record_in_state, TaskBeginParentContext,
};
use crate::agent::runtime::task_state::TaskState;
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

pub(crate) struct DelegatedTaskTerminalFinalizeEntryRequest<'a> {
    pub db: &'a sqlx::SqlitePool,
    pub journal: &'a SessionJournalStore,
    pub task_execution_outcome: TaskExecutionOutcome,
}

#[derive(Debug, Clone)]
pub(crate) enum DelegatedTaskEntryOutcome {
    Completed {
        output: String,
    },
    Stopped {
        stop_reason: RunStopReason,
        error: String,
    },
    Failed {
        error: String,
    },
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

    let task_state = resolve_primary_local_chat_task_state(
        journal,
        session_id,
        user_message_id,
        run_id,
        user_message,
    )
    .await;
    let task_execution_outcome = run_task_backend_with_task_state(ActiveTaskBackendRunRequest {
        db,
        journal,
        task_state,
        parent_context: None,
        started_run: StartedRunContext {
            run_id,
            user_message_id,
        },
        preparation_request: TaskBackendPreparationRequest::InteractiveChat(
            InteractiveChatTaskBackendPreparationRequest {
                app,
                agent_executor,
                db,
                session_id,
                user_message,
                user_message_parts,
                max_iterations_override,
            },
        ),
        prepare_surface: |_| {},
        execution_context: TaskBackendExecutionContext::InteractiveChat {
            app: app.clone(),
            agent_executor: Arc::clone(agent_executor),
            db,
            journal,
            session_id,
            run_id,
            user_message,
            cancel_flag,
            tool_confirm_responder,
        },
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
) -> Result<DelegatedTaskEntryOutcome, String> {
    finalize_delegated_task_execution_outcome(DelegatedTaskTerminalFinalizeRequest {
        db: request.db,
        journal: request.journal,
        task_execution_outcome: request.task_execution_outcome,
    })
    .await
    .map(map_delegated_terminal_outcome_to_entry_outcome)
}

pub(crate) async fn run_and_finalize_delegated_task_backend<F>(
    request: DelegatedTaskBackendRunAndFinalizeRequest<'_, F>,
) -> Result<DelegatedTaskEntryOutcome, String>
where
    F: FnOnce(&mut PreparedTaskBackendSurface),
{
    let DelegatedTaskBackendRunAndFinalizeRequest {
        db,
        journal,
        task_state,
        parent_context,
        preparation_request,
        app_handle,
        agent_executor,
        on_token,
        prepare_surface,
    } = request;

    let session_id = task_state.session_id.clone();
    let run_id = task_state.run_id.clone();
    let user_message_id = task_state.user_message_id.clone();
    let task_execution_outcome = run_task_backend_with_task_state(ActiveTaskBackendRunRequest {
        db,
        journal,
        task_state,
        parent_context,
        started_run: StartedRunContext {
            run_id: &run_id,
            user_message_id: &user_message_id,
        },
        preparation_request,
        prepare_surface,
        execution_context: TaskBackendExecutionContext::Delegated {
            app_handle,
            agent_executor: Arc::clone(&agent_executor),
            session_id: &session_id,
            on_token,
        },
    })
    .await?;

    finalize_delegated_task_execution_outcome(DelegatedTaskTerminalFinalizeRequest {
        db,
        journal,
        task_execution_outcome,
    })
    .await
    .map(map_delegated_terminal_outcome_to_entry_outcome)
}

fn map_delegated_terminal_outcome_to_entry_outcome(
    outcome: DelegatedTaskTerminalOutcome,
) -> DelegatedTaskEntryOutcome {
    match outcome {
        DelegatedTaskTerminalOutcome::Completed { output } => {
            DelegatedTaskEntryOutcome::Completed { output }
        }
        DelegatedTaskTerminalOutcome::Stopped { stop_reason, error } => {
            DelegatedTaskEntryOutcome::Stopped { stop_reason, error }
        }
        DelegatedTaskTerminalOutcome::Failed { error } => {
            DelegatedTaskEntryOutcome::Failed { error }
        }
    }
}

async fn resolve_primary_local_chat_task_state(
    journal: &SessionJournalStore,
    session_id: &str,
    user_message_id: &str,
    run_id: &str,
    user_message: &str,
) -> TaskState {
    if let Some(state) = journal.read_state(session_id).await.ok() {
        if let Some(active_task_record) = resolve_latest_task_record_in_state(&state, session_id)
            .filter(|record| should_resume_local_chat_task(record, user_message))
        {
            let (continuation_mode, continuation_source, continuation_reason) =
                resolve_latest_task_run_continuation_contract(
                    &state,
                    &active_task_record.task_identity.task_id,
                )
                .unwrap_or_else(|| resolve_local_chat_continuation_contract(&active_task_record));
            return TaskState::new_recovery_local_chat_with_contract(
                session_id,
                user_message_id,
                run_id,
                &active_task_record.task_identity,
                continuation_mode,
                continuation_source,
                continuation_reason,
            );
        }
    }

    TaskState::new_primary_local_chat(session_id, user_message_id, run_id)
}

#[cfg(test)]
mod tests {
    use super::resolve_primary_local_chat_task_state;
    use crate::agent::runtime::task_record::TaskRecord;
    use crate::agent::runtime::task_repo::TaskRepo;
    use crate::agent::runtime::task_state::{
        TaskBackendKind, TaskIdentity, TaskKind, TaskSurfaceKind,
    };
    use crate::agent::runtime::task_transition::TaskContinuationMode;
    use crate::session_journal::{SessionJournalStore, SessionRunEvent};
    use tempfile::tempdir;

    #[tokio::test]
    async fn resolve_primary_local_chat_task_state_uses_recovery_task_for_continue_requests() {
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());
        let failed_record = TaskRecord {
            task_identity: TaskIdentity::new(
                "task-1",
                Option::<String>::None,
                Some("task-1".to_string()),
            ),
            task_kind: TaskKind::PrimaryUserTask,
            surface_kind: TaskSurfaceKind::LocalChatSurface,
            backend_kind: TaskBackendKind::InteractiveChatBackend,
            session_id: "session-1".to_string(),
            user_message_id: "user-1".to_string(),
            run_id: "run-1".to_string(),
            status: crate::agent::runtime::task_record::TaskLifecycleStatus::Failed,
            created_at: "2026-04-10T10:00:00Z".to_string(),
            updated_at: "2026-04-10T10:01:00Z".to_string(),
            started_at: Some("2026-04-10T10:00:00Z".to_string()),
            completed_at: Some("2026-04-10T10:01:00Z".to_string()),
            terminal_reason: Some("max_turns".to_string()),
        };

        journal
            .append_event(
                "session-1",
                SessionRunEvent::TaskRecordUpserted {
                    run_id: "run-1".to_string(),
                    task: TaskRepo::build_task_record_upsert_payload(&failed_record),
                },
            )
            .await
            .expect("append failed task");

        let task_state =
            resolve_primary_local_chat_task_state(&journal, "session-1", "user-2", "run-2", "继续")
                .await;

        assert_eq!(task_state.task_kind, TaskKind::RecoveryTask);
        assert_eq!(
            task_state.continuation_mode,
            Some(TaskContinuationMode::RecoveryResume)
        );
        assert_eq!(task_state.continuation_reason.as_deref(), Some("max_turns"));
        assert_eq!(
            task_state.task_identity.parent_task_id.as_deref(),
            Some("task-1")
        );
        assert_eq!(task_state.task_identity.root_task_id, "task-1");
    }

    #[tokio::test]
    async fn resolve_primary_local_chat_task_state_keeps_new_tasks_for_non_recovery_messages() {
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());

        let task_state = resolve_primary_local_chat_task_state(
            &journal,
            "session-1",
            "user-2",
            "run-2",
            "帮我继续总结当前项目结构",
        )
        .await;

        assert_eq!(task_state.task_kind, TaskKind::PrimaryUserTask);
        assert_eq!(task_state.task_identity.parent_task_id, None);
    }

    #[tokio::test]
    async fn resolve_primary_local_chat_task_state_marks_approval_recovery_tasks() {
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());
        let failed_record = TaskRecord {
            task_identity: TaskIdentity::new(
                "task-1",
                Option::<String>::None,
                Some("task-1".to_string()),
            ),
            task_kind: TaskKind::PrimaryUserTask,
            surface_kind: TaskSurfaceKind::LocalChatSurface,
            backend_kind: TaskBackendKind::InteractiveChatBackend,
            session_id: "session-1".to_string(),
            user_message_id: "user-1".to_string(),
            run_id: "run-1".to_string(),
            status: crate::agent::runtime::task_record::TaskLifecycleStatus::Failed,
            created_at: "2026-04-10T10:00:00Z".to_string(),
            updated_at: "2026-04-10T10:01:00Z".to_string(),
            started_at: Some("2026-04-10T10:00:00Z".to_string()),
            completed_at: Some("2026-04-10T10:01:00Z".to_string()),
            terminal_reason: Some("approval_recovery".to_string()),
        };

        journal
            .append_event(
                "session-1",
                SessionRunEvent::TaskRecordUpserted {
                    run_id: "run-1".to_string(),
                    task: TaskRepo::build_task_record_upsert_payload(&failed_record),
                },
            )
            .await
            .expect("append failed task");

        let task_state =
            resolve_primary_local_chat_task_state(&journal, "session-1", "user-2", "run-2", "继续")
                .await;

        assert_eq!(
            task_state.continuation_mode,
            Some(TaskContinuationMode::ApprovalResume)
        );
        assert_eq!(
            task_state.continuation_reason.as_deref(),
            Some("approval_recovery")
        );
    }

    #[tokio::test]
    async fn resolve_primary_local_chat_task_state_prefers_latest_run_continuation_contract() {
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());
        let failed_record = TaskRecord {
            task_identity: TaskIdentity::new(
                "task-1",
                Option::<String>::None,
                Some("task-1".to_string()),
            ),
            task_kind: TaskKind::PrimaryUserTask,
            surface_kind: TaskSurfaceKind::LocalChatSurface,
            backend_kind: TaskBackendKind::InteractiveChatBackend,
            session_id: "session-1".to_string(),
            user_message_id: "user-1".to_string(),
            run_id: "run-1".to_string(),
            status: crate::agent::runtime::task_record::TaskLifecycleStatus::Failed,
            created_at: "2026-04-10T10:00:00Z".to_string(),
            updated_at: "2026-04-10T10:01:00Z".to_string(),
            started_at: Some("2026-04-10T10:00:00Z".to_string()),
            completed_at: Some("2026-04-10T10:01:00Z".to_string()),
            terminal_reason: Some("max_turns".to_string()),
        };

        journal
            .append_event(
                "session-1",
                SessionRunEvent::TaskRecordUpserted {
                    run_id: "run-1".to_string(),
                    task: TaskRepo::build_task_record_upsert_payload(&failed_record),
                },
            )
            .await
            .expect("append failed task");
        journal
            .append_event(
                "session-1",
                SessionRunEvent::TaskContinued {
                    run_id: "run-1".to_string(),
                    task_identity: crate::session_journal::SessionRunTaskIdentitySnapshot {
                        task_id: "task-1".to_string(),
                        parent_task_id: None,
                        root_task_id: "task-1".to_string(),
                        task_kind: "primary_user_task".to_string(),
                        surface_kind: "local_chat_surface".to_string(),
                        backend_kind: "interactive_chat_backend".to_string(),
                    },
                    continuation_mode: "permission_resume".to_string(),
                    continuation_source: "parent_rejoin".to_string(),
                    continuation_reason: "permission_denied".to_string(),
                },
            )
            .await
            .expect("append continued task");

        let task_state =
            resolve_primary_local_chat_task_state(&journal, "session-1", "user-2", "run-2", "继续")
                .await;

        assert_eq!(
            task_state.continuation_mode,
            Some(TaskContinuationMode::PermissionResume)
        );
        assert_eq!(
            task_state.continuation_source,
            Some(crate::agent::runtime::task_transition::TaskContinuationSource::ParentRejoin)
        );
        assert_eq!(
            task_state.continuation_reason.as_deref(),
            Some("permission_denied")
        );
    }
}
