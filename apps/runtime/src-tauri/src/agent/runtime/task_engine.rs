use crate::agent::runtime::events::ToolConfirmResponder;
use crate::agent::runtime::kernel::execution_plan::{ExecutionOutcome, SessionEngineError};
use crate::agent::runtime::kernel::session_engine::SessionEngine;
use crate::agent::runtime::session_runs::append_session_run_event_with_pool;
use crate::agent::runtime::task_record::{TaskLifecycleStatus, TaskRecord};
use crate::agent::runtime::task_repo::TaskRepo;
use crate::agent::runtime::task_state::{TaskIdentity, TaskState};
use crate::agent::runtime::task_transition::TaskTransition;
use crate::agent::AgentExecutor;
use crate::session_journal::{
    SessionJournalStore, SessionRunEvent, SessionRunTaskIdentitySnapshot,
};
use chrono::Utc;
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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct TaskEngine;

impl TaskEngine {
    fn build_task_identity_snapshot(task_state: &TaskState) -> SessionRunTaskIdentitySnapshot {
        SessionRunTaskIdentitySnapshot {
            task_id: task_state.task_identity.task_id.clone(),
            parent_task_id: task_state.task_identity.parent_task_id.clone(),
            root_task_id: task_state.task_identity.root_task_id.clone(),
            task_kind: task_state.task_kind.journal_key().to_string(),
            surface_kind: task_state.surface_kind.journal_key().to_string(),
        }
    }

    fn build_pending_task_record(task_state: &TaskState, now: impl Into<String>) -> TaskRecord {
        TaskRecord::new_pending(
            task_state.task_identity.clone(),
            task_state.task_kind,
            task_state.surface_kind,
            task_state.session_id.clone(),
            task_state.user_message_id.clone(),
            task_state.run_id.clone(),
            now,
        )
    }

    fn format_transition_error(
        error: crate::agent::runtime::task_record::TaskLifecycleTransitionError,
    ) -> String {
        format!(
            "任务状态转换失败: {} -> {}",
            error.from.as_key(),
            error.to.as_key()
        )
    }

    pub(crate) fn build_primary_local_chat_task_state(
        session_id: impl Into<String>,
        user_message_id: impl Into<String>,
        run_id: impl Into<String>,
    ) -> TaskState {
        TaskState::new_primary_local_chat(session_id, user_message_id, run_id)
    }

    pub(crate) fn build_hidden_child_task_state(
        session_id: impl Into<String>,
        user_message_id: impl Into<String>,
        run_id: impl Into<String>,
        parent_task_identity: Option<&TaskIdentity>,
    ) -> TaskState {
        TaskState::new_sub_agent(session_id, user_message_id, run_id, parent_task_identity)
    }

    pub(crate) fn build_employee_step_task_state(
        session_id: impl Into<String>,
        user_message_id: impl Into<String>,
        run_id: impl Into<String>,
        parent_task_identity: Option<&TaskIdentity>,
    ) -> TaskState {
        TaskState::new_employee_step(session_id, user_message_id, run_id, parent_task_identity)
    }

    pub(crate) async fn resolve_latest_task_identity_for_session(
        journal: &SessionJournalStore,
        session_id: &str,
    ) -> Option<TaskIdentity> {
        let state = journal.read_state(session_id).await.ok()?;
        state.runs.iter().rev().find_map(|run| {
            run.task_identity.as_ref().map(|snapshot| {
                TaskIdentity::new(
                    snapshot.task_id.clone(),
                    snapshot.parent_task_id.clone(),
                    Some(snapshot.root_task_id.clone()),
                )
            })
        })
    }

    pub(crate) async fn project_task_state(
        db: &sqlx::SqlitePool,
        journal: &SessionJournalStore,
        session_id: &str,
        task_state: &TaskState,
    ) -> Result<(), String> {
        append_session_run_event_with_pool(
            db,
            journal,
            session_id,
            SessionRunEvent::TaskStateProjected {
                run_id: task_state.run_id.clone(),
                task_identity: Self::build_task_identity_snapshot(task_state),
            },
        )
        .await
    }

    async fn project_task_record_upsert(
        db: &sqlx::SqlitePool,
        journal: &SessionJournalStore,
        session_id: &str,
        record: &TaskRecord,
    ) -> Result<(), String> {
        append_session_run_event_with_pool(
            db,
            journal,
            session_id,
            SessionRunEvent::TaskRecordUpserted {
                run_id: record.run_id.clone(),
                task: TaskRepo::build_task_record_upsert_payload(record),
            },
        )
        .await
    }

    async fn project_task_status_change(
        db: &sqlx::SqlitePool,
        journal: &SessionJournalStore,
        session_id: &str,
        record: &TaskRecord,
        from_status: TaskLifecycleStatus,
        to_status: TaskLifecycleStatus,
    ) -> Result<(), String> {
        append_session_run_event_with_pool(
            db,
            journal,
            session_id,
            SessionRunEvent::TaskStatusChanged {
                run_id: record.run_id.clone(),
                status_change: TaskRepo::build_task_status_changed_payload(
                    record,
                    from_status,
                    to_status,
                ),
            },
        )
        .await
    }

    pub(crate) async fn start_task(
        db: &sqlx::SqlitePool,
        journal: &SessionJournalStore,
        session_id: &str,
        task_state: &TaskState,
    ) -> Result<TaskRecord, String> {
        let now = Utc::now().to_rfc3339();
        let pending_record = Self::build_pending_task_record(task_state, now);
        Self::project_task_record_upsert(db, journal, session_id, &pending_record).await?;
        let running_record = pending_record
            .clone()
            .mark_running(Utc::now().to_rfc3339())
            .map_err(Self::format_transition_error)?;
        Self::project_task_status_change(
            db,
            journal,
            session_id,
            &running_record,
            pending_record.status,
            running_record.status,
        )
        .await?;
        Ok(running_record)
    }

    async fn transition_task_record(
        db: &sqlx::SqlitePool,
        journal: &SessionJournalStore,
        session_id: &str,
        active_task_record: &TaskRecord,
        transition: impl FnOnce(
            TaskRecord,
        ) -> Result<
            TaskRecord,
            crate::agent::runtime::task_record::TaskLifecycleTransitionError,
        >,
    ) -> Result<TaskRecord, String> {
        let transitioned =
            transition(active_task_record.clone()).map_err(Self::format_transition_error)?;
        Self::project_task_status_change(
            db,
            journal,
            session_id,
            &transitioned,
            active_task_record.status,
            transitioned.status,
        )
        .await?;
        Ok(transitioned)
    }

    pub(crate) async fn apply_transition(
        db: &sqlx::SqlitePool,
        journal: &SessionJournalStore,
        session_id: &str,
        active_task_record: &TaskRecord,
        transition: &TaskTransition,
    ) -> Result<TaskRecord, String> {
        match transition {
            TaskTransition::Continue => Ok(active_task_record.clone()),
            TaskTransition::StopCompleted { terminal_reason } => {
                Self::transition_task_record(db, journal, session_id, active_task_record, {
                    let terminal_reason = terminal_reason.clone();
                    move |record| record.mark_completed(Utc::now().to_rfc3339(), terminal_reason)
                })
                .await
            }
            TaskTransition::StopFailed { terminal_reason } => {
                Self::transition_task_record(db, journal, session_id, active_task_record, {
                    let terminal_reason = terminal_reason.clone();
                    move |record| record.mark_failed(Utc::now().to_rfc3339(), terminal_reason)
                })
                .await
            }
            TaskTransition::StopCancelled { terminal_reason } => {
                Self::transition_task_record(db, journal, session_id, active_task_record, {
                    let terminal_reason = terminal_reason.clone();
                    move |record| record.mark_cancelled(Utc::now().to_rfc3339(), terminal_reason)
                })
                .await
            }
        }
    }

    pub(crate) async fn mark_task_completed(
        db: &sqlx::SqlitePool,
        journal: &SessionJournalStore,
        session_id: &str,
        active_task_record: &TaskRecord,
        terminal_reason: impl Into<String>,
    ) -> Result<TaskRecord, String> {
        Self::apply_transition(
            db,
            journal,
            session_id,
            active_task_record,
            &TaskTransition::completed(terminal_reason),
        )
        .await
    }

    pub(crate) async fn mark_task_failed(
        db: &sqlx::SqlitePool,
        journal: &SessionJournalStore,
        session_id: &str,
        active_task_record: &TaskRecord,
        terminal_reason: impl Into<String>,
    ) -> Result<TaskRecord, String> {
        Self::apply_transition(
            db,
            journal,
            session_id,
            active_task_record,
            &TaskTransition::failed(terminal_reason),
        )
        .await
    }

    pub(crate) async fn mark_task_cancelled(
        db: &sqlx::SqlitePool,
        journal: &SessionJournalStore,
        session_id: &str,
        active_task_record: &TaskRecord,
        terminal_reason: impl Into<String>,
    ) -> Result<TaskRecord, String> {
        Self::apply_transition(
            db,
            journal,
            session_id,
            active_task_record,
            &TaskTransition::cancelled(terminal_reason),
        )
        .await
    }

    pub(crate) fn attach_task_state(
        task_state: &TaskState,
        execution_outcome: ExecutionOutcome,
    ) -> ExecutionOutcome {
        match execution_outcome {
            ExecutionOutcome::DirectDispatch { output, turn_state } => {
                ExecutionOutcome::DirectDispatch {
                    output,
                    turn_state: turn_state.with_task_state(task_state),
                }
            }
            ExecutionOutcome::RouteExecution {
                route_execution,
                reconstructed_history_len,
                turn_state,
            } => ExecutionOutcome::RouteExecution {
                route_execution,
                reconstructed_history_len,
                turn_state: turn_state.with_task_state(task_state),
            },
            ExecutionOutcome::SkillCommandFailed { error, turn_state } => {
                ExecutionOutcome::SkillCommandFailed {
                    error,
                    turn_state: turn_state.with_task_state(task_state),
                }
            }
            ExecutionOutcome::SkillCommandStopped {
                turn_state,
                stop_reason,
                error,
            } => ExecutionOutcome::SkillCommandStopped {
                turn_state: turn_state.with_task_state(task_state),
                stop_reason,
                error,
            },
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn run_primary_local_chat_task(
        app: &AppHandle,
        agent_executor: &Arc<AgentExecutor>,
        db: &sqlx::SqlitePool,
        journal: &crate::session_journal::SessionJournalStore,
        session_id: &str,
        run_id: &str,
        user_message_id: &str,
        user_message: &str,
        user_message_parts: &[Value],
        max_iterations_override: Option<usize>,
        cancel_flag: Arc<AtomicBool>,
        tool_confirm_responder: ToolConfirmResponder,
    ) -> Result<TaskExecutionOutcome, SessionEngineError> {
        let task_state =
            Self::build_primary_local_chat_task_state(session_id, user_message_id, run_id);
        let active_task_record = Self::start_task(db, journal, session_id, &task_state)
            .await
            .map_err(SessionEngineError::Generic)?;
        if let Err(error) = Self::project_task_state(db, journal, session_id, &task_state).await {
            let _ =
                Self::mark_task_failed(db, journal, session_id, &active_task_record, &error).await;
            return Err(SessionEngineError::Generic(error));
        }
        let execution_outcome = match SessionEngine::run_local_turn(
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
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(SessionEngineError::Generic(error)) => {
                let _ =
                    Self::mark_task_failed(db, journal, session_id, &active_task_record, &error)
                        .await;
                return Err(SessionEngineError::Generic(error));
            }
        };

        Ok(TaskExecutionOutcome::new(
            task_state,
            active_task_record,
            execution_outcome,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{TaskEngine, TaskExecutionOutcome};
    use crate::agent::runtime::kernel::execution_plan::ExecutionOutcome;
    use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
    use crate::agent::runtime::task_record::TaskRecord;
    use crate::agent::runtime::task_state::{TaskIdentity, TaskKind, TaskSurfaceKind};
    use crate::agent::runtime::task_transition::{resolve_commit_transition, TaskTransition};

    #[test]
    fn task_engine_builds_primary_local_chat_task_state() {
        let task_state =
            TaskEngine::build_primary_local_chat_task_state("session-1", "user-1", "run-1");

        assert_eq!(task_state.task_kind, TaskKind::PrimaryUserTask);
        assert_eq!(task_state.surface_kind, TaskSurfaceKind::LocalChatSurface);
        assert_eq!(task_state.session_id, "session-1");
        assert_eq!(task_state.user_message_id, "user-1");
        assert_eq!(task_state.run_id, "run-1");
    }

    #[test]
    fn task_execution_outcome_keeps_task_state_and_execution_outcome_together() {
        let task_state =
            TaskEngine::build_primary_local_chat_task_state("session-1", "user-1", "run-1");
        let outcome = ExecutionOutcome::SkillCommandFailed {
            error: "failed".to_string(),
            turn_state: TurnStateSnapshot::default(),
        };

        let task_record = TaskRecord::new_pending(
            task_state.task_identity.clone(),
            task_state.task_kind,
            task_state.surface_kind,
            task_state.session_id.clone(),
            task_state.user_message_id.clone(),
            task_state.run_id.clone(),
            "2026-04-09T10:00:00Z",
        );
        let wrapped = TaskExecutionOutcome::new(task_state.clone(), task_record.clone(), outcome);

        assert_eq!(wrapped.task_state, task_state);
        assert_eq!(wrapped.active_task_record, task_record);
        assert!(matches!(
            wrapped.execution_outcome,
            ExecutionOutcome::SkillCommandFailed { .. }
        ));
    }

    #[test]
    fn hidden_child_task_state_inherits_parent_lineage() {
        let parent = TaskIdentity::new("task-parent", Option::<String>::None, Some("task-root"));

        let task_state = TaskEngine::build_hidden_child_task_state(
            "child-session",
            "user-1",
            "run-1",
            Some(&parent),
        );

        assert_eq!(task_state.task_kind, TaskKind::SubAgentTask);
        assert_eq!(task_state.surface_kind, TaskSurfaceKind::HiddenChildSurface);
        assert_eq!(
            task_state.task_identity.parent_task_id.as_deref(),
            Some("task-parent")
        );
        assert_eq!(task_state.task_identity.root_task_id, "task-root");
    }

    #[test]
    fn resolve_commit_transition_marks_success_as_completed() {
        let transition = resolve_commit_transition(&Ok(()), None);

        assert_eq!(
            transition,
            TaskTransition::StopCompleted {
                terminal_reason: "completed".to_string(),
            }
        );
    }

    #[test]
    fn resolve_commit_transition_prefers_explicit_failure_reason() {
        let transition = resolve_commit_transition(
            &Err("commit failed".to_string()),
            Some("skill_command_dispatch"),
        );

        assert_eq!(
            transition,
            TaskTransition::StopFailed {
                terminal_reason: "skill_command_dispatch".to_string(),
            }
        );
    }

    #[test]
    fn resolve_commit_transition_falls_back_to_commit_error() {
        let transition = resolve_commit_transition(&Err("commit failed".to_string()), None);

        assert_eq!(
            transition,
            TaskTransition::StopFailed {
                terminal_reason: "commit failed".to_string(),
            }
        );
    }
}
