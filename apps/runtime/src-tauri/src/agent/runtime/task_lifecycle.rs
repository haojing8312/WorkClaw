use crate::agent::run_guard::RunStopReasonKind;
use crate::agent::runtime::kernel::execution_plan::ExecutionOutcome;
use crate::agent::runtime::session_runs::append_session_run_event_with_pool;
use crate::agent::runtime::task_record::{TaskLifecycleStatus, TaskRecord};
use crate::agent::runtime::task_repo::TaskRepo;
use crate::agent::runtime::task_state::{
    TaskBackendKind, TaskIdentity, TaskKind, TaskState, TaskSurfaceKind,
};
use crate::agent::runtime::task_transition::{
    resolve_commit_transition, resolve_initial_transition, resolve_stop_transition,
    resolve_terminal_transition, TaskTransition,
};
use crate::session_journal::{
    SessionJournalStore, SessionRunEvent, SessionRunTaskIdentitySnapshot, SessionTaskRecordSnapshot,
};
use chrono::Utc;

#[derive(Debug, Clone, Copy)]
pub(crate) struct TaskBeginParentContext<'a> {
    pub session_id: &'a str,
    pub active_task_record: &'a TaskRecord,
}

pub(crate) fn build_task_identity_snapshot_from_parts(
    task_identity: &TaskIdentity,
    task_kind: TaskKind,
    surface_kind: TaskSurfaceKind,
    backend_kind: TaskBackendKind,
) -> SessionRunTaskIdentitySnapshot {
    SessionRunTaskIdentitySnapshot {
        task_id: task_identity.task_id.clone(),
        parent_task_id: task_identity.parent_task_id.clone(),
        root_task_id: task_identity.root_task_id.clone(),
        task_kind: task_kind.journal_key().to_string(),
        surface_kind: surface_kind.journal_key().to_string(),
        backend_kind: backend_kind.journal_key().to_string(),
    }
}

pub(crate) fn build_task_identity_snapshot(
    task_state: &TaskState,
) -> SessionRunTaskIdentitySnapshot {
    build_task_identity_snapshot_from_parts(
        &task_state.task_identity,
        task_state.task_kind,
        task_state.surface_kind,
        task_state.backend_kind,
    )
}

pub(crate) fn build_task_identity_snapshot_from_record(
    record: &TaskRecord,
) -> SessionRunTaskIdentitySnapshot {
    build_task_identity_snapshot_from_parts(
        &record.task_identity,
        record.task_kind,
        record.surface_kind,
        record.backend_kind,
    )
}

pub(crate) fn build_pending_task_record(
    task_state: &TaskState,
    now: impl Into<String>,
) -> TaskRecord {
    TaskRecord::new_pending(
        task_state.task_identity.clone(),
        task_state.task_kind,
        task_state.surface_kind,
        task_state.backend_kind,
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

pub(crate) fn rebuild_task_record(snapshot: &SessionTaskRecordSnapshot) -> TaskRecord {
    TaskRecord {
        task_identity: TaskIdentity::new(
            snapshot.task_identity.task_id.clone(),
            snapshot.task_identity.parent_task_id.clone(),
            Some(snapshot.task_identity.root_task_id.clone()),
        ),
        task_kind: match snapshot.task_identity.task_kind.as_str() {
            "delegated_skill_task" => TaskKind::DelegatedSkillTask,
            "sub_agent_task" => TaskKind::SubAgentTask,
            "employee_step_task" => TaskKind::EmployeeStepTask,
            "recovery_task" => TaskKind::RecoveryTask,
            _ => TaskKind::PrimaryUserTask,
        },
        surface_kind: match snapshot.task_identity.surface_kind.as_str() {
            "hidden_child_surface" => TaskSurfaceKind::HiddenChildSurface,
            "employee_step_surface" => TaskSurfaceKind::EmployeeStepSurface,
            _ => TaskSurfaceKind::LocalChatSurface,
        },
        backend_kind: match snapshot.task_identity.backend_kind.as_str() {
            "hidden_child_backend" => TaskBackendKind::HiddenChildBackend,
            "employee_step_backend" => TaskBackendKind::EmployeeStepBackend,
            _ => TaskBackendKind::InteractiveChatBackend,
        },
        session_id: snapshot.session_id.clone(),
        user_message_id: snapshot.user_message_id.clone(),
        run_id: snapshot.run_id.clone(),
        status: snapshot.status,
        created_at: snapshot.created_at.clone(),
        updated_at: snapshot.updated_at.clone(),
        started_at: snapshot.started_at.clone(),
        completed_at: snapshot.completed_at.clone(),
        terminal_reason: snapshot.terminal_reason.clone(),
    }
}

pub(crate) async fn resolve_latest_task_record_for_session(
    journal: &SessionJournalStore,
    session_id: &str,
) -> Option<TaskRecord> {
    let state = journal.read_state(session_id).await.ok()?;
    state
        .tasks
        .iter()
        .rev()
        .find(|task| task.session_id == session_id)
        .map(rebuild_task_record)
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
            task_identity: build_task_identity_snapshot(task_state),
        },
    )
    .await
}

async fn project_task_continued(
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    record: &TaskRecord,
) -> Result<(), String> {
    append_session_run_event_with_pool(
        db,
        journal,
        session_id,
        SessionRunEvent::TaskContinued {
            run_id: record.run_id.clone(),
            task_identity: build_task_identity_snapshot_from_record(record),
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

async fn project_task_delegation(
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    active_task_record: &TaskRecord,
    delegated_task_identity: &TaskIdentity,
    delegated_task_kind: TaskKind,
    delegated_surface_kind: TaskSurfaceKind,
    delegated_backend_kind: TaskBackendKind,
) -> Result<(), String> {
    append_session_run_event_with_pool(
        db,
        journal,
        session_id,
        SessionRunEvent::TaskDelegated {
            run_id: active_task_record.run_id.clone(),
            from_task_id: active_task_record.task_identity.task_id.clone(),
            from_task_kind: active_task_record.task_kind.journal_key().to_string(),
            from_surface_kind: active_task_record.surface_kind.journal_key().to_string(),
            delegated_task: build_task_identity_snapshot_from_parts(
                delegated_task_identity,
                delegated_task_kind,
                delegated_surface_kind,
                delegated_backend_kind,
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
    let pending_record = build_pending_task_record(task_state, now);
    project_task_record_upsert(db, journal, session_id, &pending_record).await?;
    let running_record = pending_record
        .clone()
        .mark_running(Utc::now().to_rfc3339())
        .map_err(format_transition_error)?;
    project_task_status_change(
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

async fn apply_initial_transition(
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    task_state: &TaskState,
    active_task_record: &TaskRecord,
    parent_context: Option<TaskBeginParentContext<'_>>,
) -> Result<(), String> {
    let transition = resolve_initial_transition(task_state);
    match &transition {
        TaskTransition::Continue
        | TaskTransition::StopCompleted { .. }
        | TaskTransition::StopFailed { .. }
        | TaskTransition::StopCancelled { .. } => {
            apply_transition(
                db,
                journal,
                &task_state.session_id,
                active_task_record,
                &transition,
            )
            .await?;
        }
        TaskTransition::DelegateToChild { .. } | TaskTransition::DelegateToEmployee { .. } => {
            if let Some(parent_context) = parent_context {
                apply_transition(
                    db,
                    journal,
                    parent_context.session_id,
                    parent_context.active_task_record,
                    &transition,
                )
                .await?;
            }
        }
    }
    Ok(())
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
    let transitioned = transition(active_task_record.clone()).map_err(format_transition_error)?;
    project_task_status_change(
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
        TaskTransition::Continue => {
            project_task_continued(db, journal, session_id, active_task_record).await?;
            Ok(active_task_record.clone())
        }
        TaskTransition::DelegateToChild {
            delegated_task_identity,
            delegated_task_kind,
            delegated_surface_kind,
            delegated_backend_kind,
        }
        | TaskTransition::DelegateToEmployee {
            delegated_task_identity,
            delegated_task_kind,
            delegated_surface_kind,
            delegated_backend_kind,
        } => {
            project_task_delegation(
                db,
                journal,
                session_id,
                active_task_record,
                delegated_task_identity,
                *delegated_task_kind,
                *delegated_surface_kind,
                *delegated_backend_kind,
            )
            .await?;
            Ok(active_task_record.clone())
        }
        TaskTransition::StopCompleted { terminal_reason } => {
            transition_task_record(db, journal, session_id, active_task_record, {
                let terminal_reason = terminal_reason.clone();
                move |record| record.mark_completed(Utc::now().to_rfc3339(), terminal_reason)
            })
            .await
        }
        TaskTransition::StopFailed { terminal_reason } => {
            transition_task_record(db, journal, session_id, active_task_record, {
                let terminal_reason = terminal_reason.clone();
                move |record| record.mark_failed(Utc::now().to_rfc3339(), terminal_reason)
            })
            .await
        }
        TaskTransition::StopCancelled { terminal_reason } => {
            transition_task_record(db, journal, session_id, active_task_record, {
                let terminal_reason = terminal_reason.clone();
                move |record| record.mark_cancelled(Utc::now().to_rfc3339(), terminal_reason)
            })
            .await
        }
    }
}

pub(crate) async fn finalize_after_commit(
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    active_task_record: &TaskRecord,
    commit_result: Result<(), String>,
    failure_reason: Option<String>,
) -> Result<(), String> {
    let transition = resolve_commit_transition(&commit_result, failure_reason.as_deref());
    let _ = apply_transition(db, journal, session_id, active_task_record, &transition).await;
    commit_result
}

pub(crate) async fn finalize_after_terminal(
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    active_task_record: &TaskRecord,
    success: bool,
    failure_reason: Option<&str>,
) {
    let transition = resolve_terminal_transition(success, failure_reason);
    let _ = apply_transition(db, journal, session_id, active_task_record, &transition).await;
}

pub(crate) async fn finalize_after_stop(
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    active_task_record: &TaskRecord,
    stop_reason_kind: RunStopReasonKind,
    fallback_reason: Option<&str>,
) {
    let transition = resolve_stop_transition(stop_reason_kind, fallback_reason);
    let _ = apply_transition(db, journal, session_id, active_task_record, &transition).await;
}

pub(crate) async fn begin_task_run(
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    task_state: &TaskState,
    parent_context: Option<TaskBeginParentContext<'_>>,
) -> Result<TaskRecord, String> {
    let active_task_record = start_task(db, journal, &task_state.session_id, task_state).await?;
    if let Err(error) = project_task_state(db, journal, &task_state.session_id, task_state).await {
        let _ = mark_task_failed(
            db,
            journal,
            &task_state.session_id,
            &active_task_record,
            &error,
        )
        .await;
        return Err(error);
    }
    if let Err(error) =
        apply_initial_transition(db, journal, task_state, &active_task_record, parent_context).await
    {
        let _ = mark_task_failed(
            db,
            journal,
            &task_state.session_id,
            &active_task_record,
            &error,
        )
        .await;
        return Err(error);
    }
    Ok(active_task_record)
}

pub(crate) async fn mark_task_failed(
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    active_task_record: &TaskRecord,
    terminal_reason: impl Into<String>,
) -> Result<TaskRecord, String> {
    apply_transition(
        db,
        journal,
        session_id,
        active_task_record,
        &TaskTransition::failed(terminal_reason),
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

#[cfg(test)]
mod tests {
    use super::{begin_task_run, finalize_after_commit, resolve_latest_task_record_for_session};
    use crate::agent::runtime::task_state::TaskState;
    use crate::session_journal::SessionJournalStore;
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::tempdir;

    async fn setup_task_lifecycle_pool() -> sqlx::SqlitePool {
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

        pool
    }

    #[tokio::test]
    async fn begin_task_run_starts_primary_local_chat_through_unified_entry() {
        let pool = setup_task_lifecycle_pool().await;
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());
        let task_state = TaskState::new_primary_local_chat("session-1", "user-1", "run-1");

        let task_record = begin_task_run(
            &pool,
            &journal,
            &task_state,
            Option::<super::TaskBeginParentContext<'_>>::None,
        )
        .await
        .expect("begin primary task");

        assert_eq!(task_record.status.as_key(), "running");

        let state = journal.read_state("session-1").await.expect("read state");
        let run = state.runs.first().expect("run snapshot");
        assert_eq!(
            run.task_identity
                .as_ref()
                .map(|identity| identity.task_id.as_str()),
            Some(task_state.task_identity.task_id.as_str())
        );
    }

    #[tokio::test]
    async fn finalize_after_commit_marks_running_task_as_failed_when_commit_fails() {
        let pool = setup_task_lifecycle_pool().await;
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());
        let task_state = TaskState::new_primary_local_chat("session-1", "user-1", "run-1");
        let task_record = begin_task_run(
            &pool,
            &journal,
            &task_state,
            Option::<super::TaskBeginParentContext<'_>>::None,
        )
        .await
        .expect("begin primary task");

        let commit_result = finalize_after_commit(
            &pool,
            &journal,
            "session-1",
            &task_record,
            Err("commit failed".to_string()),
            Some("skill_command_dispatch".to_string()),
        )
        .await;

        assert_eq!(commit_result, Err("commit failed".to_string()));

        let task_record = resolve_latest_task_record_for_session(&journal, "session-1")
            .await
            .expect("load latest task record");
        assert_eq!(task_record.status.as_key(), "failed");
        assert_eq!(
            task_record.terminal_reason.as_deref(),
            Some("skill_command_dispatch")
        );
    }
}
