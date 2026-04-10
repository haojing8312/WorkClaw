use crate::agent::run_guard::RunStopReason;
use crate::agent::runtime::attempt_runner::RouteExecutionOutcome;
use crate::agent::runtime::kernel::execution_plan::ExecutionOutcome;
use crate::agent::runtime::kernel::outcome_commit::{OutcomeCommitter, TerminalOutcome};
use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
use crate::agent::runtime::runtime_io::{
    append_partial_assistant_chunk_with_pool, append_run_failed_with_pool,
    append_run_stopped_with_pool, finalize_run_success_with_pool,
};
use crate::agent::runtime::task_active_run::TaskExecutionOutcome;
use crate::agent::runtime::task_lifecycle;
use crate::agent::runtime::task_record::TaskRecord;
use crate::agent::runtime::task_state::{TaskBackendKind, TaskState};
use crate::agent::runtime::RuntimeTranscript;
use crate::session_journal::SessionJournalStore;
use tauri::AppHandle;

#[derive(Debug)]
enum PrimaryTaskTerminalCompletion {
    Terminal {
        terminal_outcome: TerminalOutcome,
        task_record: TaskRecord,
        failure_reason: Option<String>,
    },
}

#[derive(Debug, Clone, Copy)]
struct TaskTerminalPolicy {
    route_failure_kind: &'static str,
    empty_success_error: Option<&'static str>,
    skill_command_failure_kind: &'static str,
}

impl TaskTerminalPolicy {
    fn for_backend_kind(backend_kind: TaskBackendKind) -> Self {
        Self {
            route_failure_kind: backend_kind.generic_error_kind(),
            empty_success_error: backend_kind.empty_success_error(),
            skill_command_failure_kind: "skill_command_dispatch",
        }
    }
}

#[derive(Debug)]
enum AttachedTaskTerminalOutcome {
    DirectDispatch {
        output: String,
        turn_state: TurnStateSnapshot,
    },
    RouteExecution {
        route_execution: RouteExecutionOutcome,
        reconstructed_history_len: usize,
        turn_state: TurnStateSnapshot,
    },
    SkillCommandFailed {
        error: String,
        turn_state: TurnStateSnapshot,
        failure_reason: String,
    },
    SkillCommandStopped {
        turn_state: TurnStateSnapshot,
        stop_reason: RunStopReason,
        error: String,
        failure_reason: String,
    },
}

fn classify_attached_task_execution_outcome(
    task_state: &TaskState,
    execution_outcome: ExecutionOutcome,
) -> AttachedTaskTerminalOutcome {
    let terminal_policy = TaskTerminalPolicy::for_backend_kind(task_state.backend_kind);
    match task_lifecycle::attach_task_state(task_state, execution_outcome) {
        ExecutionOutcome::DirectDispatch { output, turn_state } => {
            AttachedTaskTerminalOutcome::DirectDispatch { output, turn_state }
        }
        ExecutionOutcome::RouteExecution {
            route_execution,
            reconstructed_history_len,
            turn_state,
        } => AttachedTaskTerminalOutcome::RouteExecution {
            route_execution,
            reconstructed_history_len,
            turn_state,
        },
        ExecutionOutcome::SkillCommandFailed { error, turn_state } => {
            AttachedTaskTerminalOutcome::SkillCommandFailed {
                error,
                turn_state,
                failure_reason: terminal_policy.skill_command_failure_kind.to_string(),
            }
        }
        ExecutionOutcome::SkillCommandStopped {
            turn_state,
            stop_reason,
            error,
        } => AttachedTaskTerminalOutcome::SkillCommandStopped {
            turn_state,
            stop_reason: stop_reason.clone(),
            error,
            failure_reason: stop_reason.kind.as_key().to_string(),
        },
    }
}

#[derive(Debug)]
pub(crate) struct DelegatedTaskTerminalFinalizeRequest<'a> {
    pub db: &'a sqlx::SqlitePool,
    pub journal: &'a SessionJournalStore,
    pub task_execution_outcome: TaskExecutionOutcome,
}

#[derive(Debug, Clone)]
pub(crate) enum DelegatedTaskTerminalOutcome {
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

#[derive(Clone, Copy)]
struct DelegatedTaskTerminalContext<'a> {
    db: &'a sqlx::SqlitePool,
    journal: &'a SessionJournalStore,
    session_id: &'a str,
    run_id: &'a str,
    active_task_record: &'a TaskRecord,
}

async fn finalize_delegated_success(
    context: DelegatedTaskTerminalContext<'_>,
    output: &str,
    has_tool_calls: bool,
    content: &str,
    reasoning_text: &str,
    reasoning_duration_ms: Option<u64>,
    turn_state: &TurnStateSnapshot,
) -> Result<DelegatedTaskTerminalOutcome, String> {
    finalize_run_success_with_pool(
        context.db,
        context.journal,
        context.session_id,
        context.run_id,
        output,
        has_tool_calls,
        content,
        reasoning_text,
        reasoning_duration_ms,
        Some(turn_state),
    )
    .await?;
    task_lifecycle::finalize_after_terminal(
        context.db,
        context.journal,
        context.session_id,
        context.active_task_record,
        true,
        None,
    )
    .await;

    Ok(DelegatedTaskTerminalOutcome::Completed {
        output: output.to_string(),
    })
}

async fn finalize_delegated_failure(
    context: DelegatedTaskTerminalContext<'_>,
    failure_reason: &str,
    error: &str,
    turn_state: &TurnStateSnapshot,
) -> DelegatedTaskTerminalOutcome {
    append_run_failed_with_pool(
        context.db,
        context.journal,
        context.session_id,
        context.run_id,
        failure_reason,
        error,
        Some(turn_state),
    )
    .await;
    task_lifecycle::finalize_after_terminal(
        context.db,
        context.journal,
        context.session_id,
        context.active_task_record,
        false,
        Some(failure_reason),
    )
    .await;

    DelegatedTaskTerminalOutcome::Failed {
        error: error.to_string(),
    }
}

async fn finalize_delegated_stopped(
    context: DelegatedTaskTerminalContext<'_>,
    stop_reason: &RunStopReason,
    error: &str,
    turn_state: &TurnStateSnapshot,
) -> Result<DelegatedTaskTerminalOutcome, String> {
    append_run_stopped_with_pool(
        context.db,
        context.journal,
        context.session_id,
        context.run_id,
        stop_reason,
        Some(turn_state),
    )
    .await?;
    task_lifecycle::finalize_after_stop(
        context.db,
        context.journal,
        context.session_id,
        context.active_task_record,
        stop_reason.kind,
        Some(stop_reason.kind.as_key()),
    )
    .await;

    Ok(DelegatedTaskTerminalOutcome::Stopped {
        stop_reason: stop_reason.clone(),
        error: error.to_string(),
    })
}

pub(crate) async fn finalize_primary_task_execution_outcome(
    app: &AppHandle,
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    task_execution_outcome: TaskExecutionOutcome,
) -> Result<(), String> {
    let completion = classify_primary_task_execution_outcome(task_execution_outcome);
    let (terminal_outcome, task_record, failure_reason) = match completion {
        PrimaryTaskTerminalCompletion::Terminal {
            terminal_outcome,
            task_record,
            failure_reason,
        } => (terminal_outcome, task_record, failure_reason),
    };

    let commit_result = OutcomeCommitter::commit_terminal_outcome(
        app,
        db,
        journal,
        session_id,
        run_id,
        terminal_outcome,
    )
    .await;
    task_lifecycle::finalize_after_commit(
        db,
        journal,
        session_id,
        &task_record,
        commit_result,
        failure_reason,
    )
    .await
}

fn classify_primary_task_execution_outcome(
    task_execution_outcome: TaskExecutionOutcome,
) -> PrimaryTaskTerminalCompletion {
    let TaskExecutionOutcome {
        task_state,
        active_task_record,
        execution_outcome,
    } = task_execution_outcome;
    match classify_attached_task_execution_outcome(&task_state, execution_outcome) {
        AttachedTaskTerminalOutcome::DirectDispatch { output, turn_state } => {
            PrimaryTaskTerminalCompletion::Terminal {
                terminal_outcome: TerminalOutcome::DirectDispatch { output, turn_state },
                task_record: active_task_record,
                failure_reason: None,
            }
        }
        AttachedTaskTerminalOutcome::RouteExecution {
            route_execution,
            reconstructed_history_len,
            turn_state,
        } => PrimaryTaskTerminalCompletion::Terminal {
            terminal_outcome: TerminalOutcome::RouteExecution {
                route_execution,
                reconstructed_history_len,
                turn_state,
            },
            task_record: active_task_record,
            failure_reason: None,
        },
        AttachedTaskTerminalOutcome::SkillCommandFailed {
            error,
            turn_state,
            failure_reason,
        } => PrimaryTaskTerminalCompletion::Terminal {
            terminal_outcome: TerminalOutcome::SkillCommandFailed { error, turn_state },
            task_record: active_task_record,
            failure_reason: Some(failure_reason),
        },
        AttachedTaskTerminalOutcome::SkillCommandStopped {
            turn_state,
            stop_reason,
            error,
            failure_reason,
        } => PrimaryTaskTerminalCompletion::Terminal {
            terminal_outcome: TerminalOutcome::SkillCommandStopped {
                turn_state,
                stop_reason,
                error,
            },
            task_record: active_task_record,
            failure_reason: Some(failure_reason),
        },
    }
}

pub(crate) async fn finalize_delegated_task_execution_outcome(
    request: DelegatedTaskTerminalFinalizeRequest<'_>,
) -> Result<DelegatedTaskTerminalOutcome, String> {
    let TaskExecutionOutcome {
        task_state,
        active_task_record,
        execution_outcome,
    } = request.task_execution_outcome;
    let session_id = task_state.session_id.clone();
    let run_id = task_state.run_id.clone();
    let terminal_policy = TaskTerminalPolicy::for_backend_kind(task_state.backend_kind);
    let context = DelegatedTaskTerminalContext {
        db: request.db,
        journal: request.journal,
        session_id: &session_id,
        run_id: &run_id,
        active_task_record: &active_task_record,
    };
    match classify_attached_task_execution_outcome(&task_state, execution_outcome) {
        AttachedTaskTerminalOutcome::RouteExecution {
            route_execution,
            reconstructed_history_len,
            turn_state,
        } => {
            if let Some(final_messages) = route_execution.final_messages {
                let (final_text, has_tool_calls, content): (String, bool, String) =
                    RuntimeTranscript::build_assistant_content_from_final_messages(
                        &final_messages,
                        reconstructed_history_len,
                    );
                if let Some(error_text) = terminal_policy.empty_success_error {
                    if final_text.trim().is_empty() {
                        return Err(error_text.to_string());
                    }
                }
                finalize_delegated_success(
                    context,
                    &final_text,
                    has_tool_calls,
                    &content,
                    &route_execution.reasoning_text,
                    route_execution.reasoning_duration_ms,
                    &turn_state,
                )
                .await
            } else {
                let partial_text = if route_execution.partial_text.is_empty() {
                    turn_state.partial_assistant_text.clone()
                } else {
                    route_execution.partial_text.clone()
                };
                if !partial_text.is_empty() {
                    append_partial_assistant_chunk_with_pool(
                        request.db,
                        request.journal,
                        &session_id,
                        &run_id,
                        &partial_text,
                    )
                    .await;
                }

                let error_text = route_execution
                    .last_error
                    .clone()
                    .unwrap_or_else(|| terminal_policy.route_failure_kind.to_string());
                if let Some(stop_reason) = route_execution
                    .last_stop_reason
                    .as_ref()
                    .or(turn_state.stop_reason.as_ref())
                {
                    finalize_delegated_stopped(context, stop_reason, &error_text, &turn_state).await
                } else {
                    Ok(finalize_delegated_failure(
                        context,
                        route_execution
                            .last_error_kind
                            .as_deref()
                            .unwrap_or(terminal_policy.route_failure_kind),
                        &error_text,
                        &turn_state,
                    )
                    .await)
                }
            }
        }
        AttachedTaskTerminalOutcome::DirectDispatch { output, turn_state } => {
            finalize_delegated_success(context, &output, false, &output, "", None, &turn_state)
                .await
        }
        AttachedTaskTerminalOutcome::SkillCommandFailed {
            error,
            turn_state,
            failure_reason,
        } => Ok(finalize_delegated_failure(context, &failure_reason, &error, &turn_state).await),
        AttachedTaskTerminalOutcome::SkillCommandStopped {
            turn_state,
            stop_reason,
            error,
            ..
        } => finalize_delegated_stopped(context, &stop_reason, &error, &turn_state).await,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_primary_task_execution_outcome, PrimaryTaskTerminalCompletion, TerminalOutcome,
    };
    use crate::agent::runtime::kernel::execution_plan::ExecutionOutcome;
    use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
    use crate::agent::runtime::task_active_run::TaskExecutionOutcome;
    use crate::agent::runtime::task_record::TaskRecord;
    use crate::agent::runtime::task_state::{TaskKind, TaskState, TaskSurfaceKind};

    #[test]
    fn classify_primary_task_execution_outcome_accepts_primary_user_task_outcome() {
        let task_state = TaskState::new_primary_local_chat("session-1", "user-1", "run-1");
        assert_eq!(task_state.task_kind, TaskKind::PrimaryUserTask);

        let task_record = TaskRecord::new_pending(
            task_state.task_identity.clone(),
            task_state.task_kind,
            task_state.surface_kind,
            task_state.backend_kind,
            task_state.session_id.clone(),
            task_state.user_message_id.clone(),
            task_state.run_id.clone(),
            "2026-04-09T10:00:00Z",
        );
        let result = TaskExecutionOutcome::new(
            task_state,
            task_record,
            ExecutionOutcome::DirectDispatch {
                output: "done".to_string(),
                turn_state: TurnStateSnapshot::default(),
            },
        );

        let completion = classify_primary_task_execution_outcome(result);

        assert!(matches!(
            completion,
            PrimaryTaskTerminalCompletion::Terminal {
                terminal_outcome: TerminalOutcome::DirectDispatch { output, .. },
                ..
            } if output == "done"
        ));
    }

    #[test]
    fn classify_primary_task_execution_outcome_preserves_task_identity_in_turn_state() {
        let task_state = TaskState::new_primary_local_chat("session-1", "user-1", "run-1");

        let task_record = TaskRecord::new_pending(
            task_state.task_identity.clone(),
            task_state.task_kind,
            task_state.surface_kind,
            task_state.backend_kind,
            task_state.session_id.clone(),
            task_state.user_message_id.clone(),
            task_state.run_id.clone(),
            "2026-04-09T10:00:00Z",
        );
        let result = TaskExecutionOutcome::new(
            task_state.clone(),
            task_record,
            ExecutionOutcome::DirectDispatch {
                output: "done".to_string(),
                turn_state: TurnStateSnapshot::default(),
            },
        );

        match classify_primary_task_execution_outcome(result) {
            PrimaryTaskTerminalCompletion::Terminal {
                terminal_outcome: TerminalOutcome::DirectDispatch { turn_state, .. },
                ..
            } => {
                assert_eq!(
                    turn_state
                        .task_identity
                        .as_ref()
                        .map(|identity| identity.task_id.as_str()),
                    Some(task_state.task_identity.task_id.as_str())
                );
                assert_eq!(turn_state.task_kind, Some(TaskKind::PrimaryUserTask));
                assert_eq!(
                    turn_state.task_surface,
                    Some(TaskSurfaceKind::LocalChatSurface)
                );
            }
            other => panic!("unexpected completion: {other:?}"),
        }
    }
}
