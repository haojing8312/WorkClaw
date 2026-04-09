use super::events::ToolConfirmResponder;
use crate::agent::context::build_tool_context;
use crate::agent::run_guard::{RunBudgetPolicy, RunBudgetScope};
use crate::agent::runtime::attempt_runner::RouteExecutionOutcome;
use crate::agent::runtime::kernel::execution_plan::{
    ExecutionContext, ExecutionOutcome, SessionEngineError,
};
use crate::agent::runtime::kernel::outcome_commit::{OutcomeCommitter, TerminalOutcome};
use crate::agent::runtime::kernel::turn_preparation::parse_user_skill_command;
use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
use crate::agent::runtime::task_engine::{TaskEngine, TaskExecutionOutcome};
use crate::agent::runtime::task_record::TaskRecord;
use crate::agent::runtime::task_transition::resolve_commit_transition;
use crate::agent::AgentExecutor;
use crate::session_journal::SessionJournalStore;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;
use uuid::Uuid;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SessionRuntime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkillCommandDispatchOutcome {
    pub output: String,
    pub skill_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkillCommandDispatchError {
    pub error: String,
    pub skill_id: String,
}

#[derive(Debug)]
enum SessionTurnCompletion {
    DirectDispatch {
        output: String,
        turn_state: TurnStateSnapshot,
        task_record: TaskRecord,
    },
    RouteExecution {
        route_execution: RouteExecutionOutcome,
        reconstructed_history_len: usize,
        turn_state: TurnStateSnapshot,
        task_record: TaskRecord,
    },
    SkillCommandFailed {
        error: String,
        turn_state: TurnStateSnapshot,
        task_record: TaskRecord,
    },
    SkillCommandStopped {
        turn_state: TurnStateSnapshot,
        stop_reason: crate::agent::run_guard::RunStopReason,
        error: String,
        task_record: TaskRecord,
    },
    GenericError(String),
}

impl SessionRuntime {
    pub fn new() -> Self {
        Self
    }

    pub(crate) async fn maybe_execute_user_skill_command(
        app: &AppHandle,
        agent_executor: &Arc<AgentExecutor>,
        session_id: &str,
        run_id: &str,
        user_message: &str,
        execution_context: &ExecutionContext,
        cancel_flag: Arc<AtomicBool>,
        tool_confirm_responder: ToolConfirmResponder,
    ) -> Result<Option<SkillCommandDispatchOutcome>, SkillCommandDispatchError> {
        let Some((command_name, raw_args)) = parse_user_skill_command(user_message) else {
            return Ok(None);
        };
        let Some(spec) = execution_context
            .skill_command_specs()
            .iter()
            .find(|spec| spec.name.eq_ignore_ascii_case(&command_name) && spec.dispatch.is_some())
        else {
            return Ok(None);
        };

        let tool_ctx = build_tool_context(
            Some(session_id),
            execution_context
                .executor_work_dir
                .as_ref()
                .map(PathBuf::from),
            execution_context.allowed_tools(),
        )
        .map_err(|err| SkillCommandDispatchError {
            error: err.to_string(),
            skill_id: spec.skill_id.clone(),
        })?;
        let dispatch_context = crate::agent::runtime::tool_dispatch::ToolDispatchContext {
            registry: agent_executor.registry(),
            app_handle: Some(app),
            session_id: Some(session_id),
            persisted_run_id: Some(run_id),
            allowed_tools: execution_context.allowed_tools(),
            effective_tool_plan: execution_context.effective_tool_plan(),
            permission_mode: execution_context.permission_mode,
            tool_ctx: &tool_ctx,
            tool_confirm_tx: Some(&tool_confirm_responder),
            cancel_flag: Some(cancel_flag),
            route_run_id: run_id,
            route_node_timeout_secs: execution_context.node_timeout_seconds,
            route_retry_count: 0,
            iteration: 1,
            run_budget_policy: RunBudgetPolicy::for_scope(RunBudgetScope::Skill),
        };

        crate::agent::runtime::tool_dispatch::dispatch_skill_command(
            &dispatch_context,
            spec,
            &raw_args,
        )
        .await
        .map(|output| {
            Some(SkillCommandDispatchOutcome {
                output,
                skill_id: spec.skill_id.clone(),
            })
        })
        .map_err(|err| SkillCommandDispatchError {
            error: err.to_string(),
            skill_id: spec.skill_id.clone(),
        })
    }

    pub(crate) async fn run_send_message(
        app: &AppHandle,
        agent_executor: &Arc<AgentExecutor>,
        db: &sqlx::SqlitePool,
        journal: &SessionJournalStore,
        session_id: &str,
        user_message_id: &str,
        user_message: &str,
        user_message_parts: &[Value],
        max_iterations_override: Option<usize>,
        cancel_flag: Arc<AtomicBool>,
        tool_confirm_responder: ToolConfirmResponder,
    ) -> Result<(), String> {
        let run_id = Uuid::new_v4().to_string();
        let task_engine_result = TaskEngine::run_primary_local_chat_task(
            app,
            agent_executor,
            db,
            journal,
            session_id,
            &run_id,
            user_message_id,
            user_message,
            user_message_parts,
            max_iterations_override,
            cancel_flag.clone(),
            tool_confirm_responder.clone(),
        )
        .await;

        match Self::classify_task_engine_result(task_engine_result) {
            SessionTurnCompletion::DirectDispatch {
                output,
                turn_state,
                task_record,
            } => {
                let commit_result = OutcomeCommitter::commit_terminal_outcome(
                    app,
                    db,
                    journal,
                    session_id,
                    &run_id,
                    TerminalOutcome::DirectDispatch { output, turn_state },
                )
                .await;
                Self::finalize_task_after_commit(
                    db,
                    journal,
                    session_id,
                    &task_record,
                    commit_result,
                    None,
                )
                .await
            }
            SessionTurnCompletion::RouteExecution {
                route_execution,
                reconstructed_history_len,
                turn_state,
                task_record,
            } => {
                let commit_result = OutcomeCommitter::commit_terminal_outcome(
                    app,
                    db,
                    journal,
                    session_id,
                    &run_id,
                    TerminalOutcome::RouteExecution {
                        route_execution,
                        reconstructed_history_len,
                        turn_state,
                    },
                )
                .await;
                Self::finalize_task_after_commit(
                    db,
                    journal,
                    session_id,
                    &task_record,
                    commit_result,
                    None,
                )
                .await
            }
            SessionTurnCompletion::SkillCommandFailed {
                error,
                turn_state,
                task_record,
            } => {
                let commit_result = OutcomeCommitter::commit_terminal_outcome(
                    app,
                    db,
                    journal,
                    session_id,
                    &run_id,
                    TerminalOutcome::SkillCommandFailed { error, turn_state },
                )
                .await;
                Self::finalize_task_after_commit(
                    db,
                    journal,
                    session_id,
                    &task_record,
                    commit_result,
                    Some("skill_command_dispatch".to_string()),
                )
                .await
            }
            SessionTurnCompletion::SkillCommandStopped {
                turn_state,
                stop_reason,
                error,
                task_record,
            } => {
                let task_reason = stop_reason.kind.as_key().to_string();
                let commit_result = OutcomeCommitter::commit_terminal_outcome(
                    app,
                    db,
                    journal,
                    session_id,
                    &run_id,
                    TerminalOutcome::SkillCommandStopped {
                        turn_state,
                        stop_reason,
                        error,
                    },
                )
                .await;
                Self::finalize_task_after_commit(
                    db,
                    journal,
                    session_id,
                    &task_record,
                    commit_result,
                    Some(task_reason),
                )
                .await
            }
            SessionTurnCompletion::GenericError(error) => Err(error),
        }
    }

    async fn finalize_task_after_commit(
        db: &sqlx::SqlitePool,
        journal: &SessionJournalStore,
        session_id: &str,
        task_record: &TaskRecord,
        commit_result: Result<(), String>,
        failure_reason: Option<String>,
    ) -> Result<(), String> {
        let transition = resolve_commit_transition(&commit_result, failure_reason.as_deref());
        let _ =
            TaskEngine::apply_transition(db, journal, session_id, task_record, &transition).await;
        commit_result
    }

    fn classify_task_engine_result(
        result: Result<TaskExecutionOutcome, SessionEngineError>,
    ) -> SessionTurnCompletion {
        match result {
            Ok(TaskExecutionOutcome {
                task_state,
                active_task_record,
                execution_outcome,
            }) => Self::classify_session_engine_result(
                Ok(TaskEngine::attach_task_state(
                    &task_state,
                    execution_outcome,
                )),
                Some(active_task_record),
            ),
            Err(error) => Self::classify_session_engine_result(Err(error), None),
        }
    }

    fn classify_session_engine_result(
        result: Result<ExecutionOutcome, SessionEngineError>,
        task_record: Option<TaskRecord>,
    ) -> SessionTurnCompletion {
        match result {
            Ok(ExecutionOutcome::DirectDispatch { output, turn_state }) => {
                SessionTurnCompletion::DirectDispatch {
                    output,
                    turn_state,
                    task_record: task_record.expect("task record required for terminal outcome"),
                }
            }
            Ok(ExecutionOutcome::RouteExecution {
                route_execution,
                reconstructed_history_len,
                turn_state,
            }) => SessionTurnCompletion::RouteExecution {
                route_execution,
                reconstructed_history_len,
                turn_state,
                task_record: task_record.expect("task record required for terminal outcome"),
            },
            Ok(ExecutionOutcome::SkillCommandFailed { error, turn_state }) => {
                SessionTurnCompletion::SkillCommandFailed {
                    error,
                    turn_state,
                    task_record: task_record.expect("task record required for terminal outcome"),
                }
            }
            Ok(ExecutionOutcome::SkillCommandStopped {
                turn_state,
                stop_reason,
                error,
            }) => SessionTurnCompletion::SkillCommandStopped {
                turn_state,
                stop_reason,
                error,
                task_record: task_record.expect("task record required for terminal outcome"),
            },
            Err(SessionEngineError::Generic(error)) => SessionTurnCompletion::GenericError(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SessionRuntime, SessionTurnCompletion};
    use crate::agent::runtime::kernel::execution_plan::ExecutionOutcome;
    use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
    use crate::agent::runtime::task_engine::{TaskEngine, TaskExecutionOutcome};
    use crate::agent::runtime::task_record::TaskRecord;
    use crate::agent::runtime::task_state::{TaskKind, TaskSurfaceKind};

    #[test]
    fn classify_task_engine_result_accepts_primary_user_task_outcome() {
        let task_state =
            TaskEngine::build_primary_local_chat_task_state("session-1", "user-1", "run-1");
        assert_eq!(task_state.task_kind, TaskKind::PrimaryUserTask);

        let task_record = TaskRecord::new_pending(
            task_state.task_identity.clone(),
            task_state.task_kind,
            task_state.surface_kind,
            task_state.session_id.clone(),
            task_state.user_message_id.clone(),
            task_state.run_id.clone(),
            "2026-04-09T10:00:00Z",
        );
        let result = Ok(TaskExecutionOutcome::new(
            task_state,
            task_record,
            ExecutionOutcome::DirectDispatch {
                output: "done".to_string(),
                turn_state: TurnStateSnapshot::default(),
            },
        ));

        let completion = SessionRuntime::classify_task_engine_result(result);

        assert!(matches!(
            completion,
            SessionTurnCompletion::DirectDispatch { output, .. } if output == "done"
        ));
    }

    #[test]
    fn classify_task_engine_result_preserves_task_identity_in_turn_state() {
        let task_state =
            TaskEngine::build_primary_local_chat_task_state("session-1", "user-1", "run-1");

        let task_record = TaskRecord::new_pending(
            task_state.task_identity.clone(),
            task_state.task_kind,
            task_state.surface_kind,
            task_state.session_id.clone(),
            task_state.user_message_id.clone(),
            task_state.run_id.clone(),
            "2026-04-09T10:00:00Z",
        );
        let result = Ok(TaskExecutionOutcome::new(
            task_state.clone(),
            task_record,
            ExecutionOutcome::DirectDispatch {
                output: "done".to_string(),
                turn_state: TurnStateSnapshot::default(),
            },
        ));

        let completion = SessionRuntime::classify_task_engine_result(result);

        match completion {
            SessionTurnCompletion::DirectDispatch { turn_state, .. } => {
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
