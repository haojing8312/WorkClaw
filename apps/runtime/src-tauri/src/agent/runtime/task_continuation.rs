use crate::agent::runtime::task_lineage::effective_task_identity;
use crate::agent::runtime::task_record::{TaskLifecycleStatus, TaskRecord};
use crate::agent::runtime::task_state::{TaskKind, TaskSurfaceKind};
use crate::agent::runtime::task_transition::{TaskContinuationMode, TaskContinuationSource};
use crate::session_journal::SessionJournalState;

pub(crate) fn is_task_continuation_request(user_message: &str) -> bool {
    let normalized = canonicalize_continuation_match(user_message);
    if normalized.is_empty() {
        return false;
    }

    const EXACT_CONTINUATION_REQUESTS: &[&str] = &[
        "continue",
        "continueplease",
        "pleasecontinue",
        "继续",
        "继续执行",
        "继续上次",
        "继续刚才",
        "继续处理",
        "接着做",
        "接着来",
    ];

    EXACT_CONTINUATION_REQUESTS
        .iter()
        .any(|candidate| normalized == canonicalize_continuation_match(candidate))
        || (normalized.starts_with("继续") && normalized.chars().count() <= 12)
        || (normalized.starts_with("continue") && normalized.chars().count() <= 24)
}

pub(crate) fn should_resume_local_chat_task(record: &TaskRecord, user_message: &str) -> bool {
    is_task_continuation_request(user_message)
        && matches!(
            record.status,
            TaskLifecycleStatus::Failed | TaskLifecycleStatus::Cancelled
        )
        && matches!(record.surface_kind, TaskSurfaceKind::LocalChatSurface)
        && matches!(
            record.task_kind,
            TaskKind::PrimaryUserTask | TaskKind::RecoveryTask
        )
}

pub(crate) fn resolve_local_chat_continuation_contract(
    record: &TaskRecord,
) -> (TaskContinuationMode, TaskContinuationSource, String) {
    let terminal_reason = record
        .terminal_reason
        .as_deref()
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or("recovery_resume");
    let mode = resolve_continuation_mode_from_reason(terminal_reason);

    (
        mode,
        TaskContinuationSource::TaskEntry,
        terminal_reason.to_string(),
    )
}

pub(crate) fn resolve_parent_rejoin_continuation_contract(
    returned_task_record: &TaskRecord,
) -> (TaskContinuationMode, TaskContinuationSource, String) {
    let terminal_reason = returned_task_record
        .terminal_reason
        .as_deref()
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or_else(|| returned_task_record.status.as_key());
    let mode = resolve_continuation_mode_from_reason(terminal_reason);
    (
        mode,
        TaskContinuationSource::ParentRejoin,
        terminal_reason.trim().to_string(),
    )
}

fn resolve_continuation_mode_from_reason(reason: &str) -> TaskContinuationMode {
    let normalized_reason = reason.trim().to_ascii_lowercase();

    if normalized_reason.contains("approval") {
        TaskContinuationMode::ApprovalResume
    } else if normalized_reason.contains("permission denied")
        || normalized_reason.contains("permission_denied")
        || normalized_reason.contains("denied_tools")
    {
        TaskContinuationMode::PermissionResume
    } else {
        TaskContinuationMode::RecoveryResume
    }
}

pub(crate) fn resolve_latest_task_run_continuation_contract(
    state: &SessionJournalState,
    task_id: &str,
) -> Option<(TaskContinuationMode, TaskContinuationSource, String)> {
    let task_id = task_id.trim();
    if task_id.is_empty() {
        return None;
    }

    state.runs.iter().rev().find_map(|run| {
        let task_identity =
            effective_task_identity(run.task_identity.as_ref(), run.turn_state.as_ref())?;
        if task_identity.task_id.trim() != task_id {
            return None;
        }

        let mode = TaskContinuationMode::from_key(run.task_continuation_mode.as_deref()?)?;
        let source = run
            .task_continuation_source
            .as_deref()
            .and_then(TaskContinuationSource::from_key)
            .unwrap_or_else(|| {
                infer_continuation_source_from_reason(run.task_continuation_reason.as_deref())
            });
        let reason = run
            .task_continuation_reason
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| mode.as_key());

        Some((mode, source, reason.to_string()))
    })
}

fn infer_continuation_source_from_reason(reason: Option<&str>) -> TaskContinuationSource {
    let Some(reason) = reason.map(str::trim).filter(|value| !value.is_empty()) else {
        return TaskContinuationSource::TaskEntry;
    };

    if reason.starts_with("delegated_return:") {
        TaskContinuationSource::ParentRejoin
    } else {
        TaskContinuationSource::TaskEntry
    }
}

fn canonicalize_continuation_match(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|ch| ch.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(ch))
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        is_task_continuation_request, resolve_latest_task_run_continuation_contract,
        resolve_local_chat_continuation_contract, resolve_parent_rejoin_continuation_contract,
        should_resume_local_chat_task,
    };
    use crate::agent::runtime::task_record::{TaskLifecycleStatus, TaskRecord};
    use crate::agent::runtime::task_state::{
        TaskBackendKind, TaskIdentity, TaskKind, TaskSurfaceKind,
    };
    use crate::agent::runtime::task_transition::{TaskContinuationMode, TaskContinuationSource};
    use crate::session_journal::{
        SessionJournalState, SessionRunSnapshot, SessionRunStatus, SessionRunTaskIdentitySnapshot,
    };

    fn build_record(
        task_kind: TaskKind,
        surface_kind: TaskSurfaceKind,
        status: TaskLifecycleStatus,
        terminal_reason: &str,
    ) -> TaskRecord {
        TaskRecord {
            task_identity: TaskIdentity::new(
                "task-1",
                Option::<String>::None,
                Some("task-root".to_string()),
            ),
            task_kind,
            surface_kind,
            backend_kind: TaskBackendKind::InteractiveChatBackend,
            session_id: "session-1".to_string(),
            user_message_id: "user-1".to_string(),
            run_id: "run-1".to_string(),
            status,
            created_at: "2026-04-10T10:00:00Z".to_string(),
            updated_at: "2026-04-10T10:00:00Z".to_string(),
            started_at: Some("2026-04-10T10:00:00Z".to_string()),
            completed_at: Some("2026-04-10T10:01:00Z".to_string()),
            terminal_reason: Some(terminal_reason.to_string()),
        }
    }

    #[test]
    fn continuation_request_matches_short_continue_commands() {
        assert!(is_task_continuation_request("继续"));
        assert!(is_task_continuation_request("继续执行"));
        assert!(is_task_continuation_request("continue"));
        assert!(is_task_continuation_request("Please continue"));
        assert!(!is_task_continuation_request("帮我总结一下当前状态"));
    }

    #[test]
    fn should_resume_local_chat_task_only_resumes_failed_or_cancelled_local_tasks() {
        assert!(should_resume_local_chat_task(
            &build_record(
                TaskKind::PrimaryUserTask,
                TaskSurfaceKind::LocalChatSurface,
                TaskLifecycleStatus::Failed,
                "max_turns",
            ),
            "继续",
        ));
        assert!(should_resume_local_chat_task(
            &build_record(
                TaskKind::RecoveryTask,
                TaskSurfaceKind::LocalChatSurface,
                TaskLifecycleStatus::Cancelled,
                "cancelled",
            ),
            "continue",
        ));
        assert!(!should_resume_local_chat_task(
            &build_record(
                TaskKind::PrimaryUserTask,
                TaskSurfaceKind::LocalChatSurface,
                TaskLifecycleStatus::Completed,
                "completed",
            ),
            "继续",
        ));
        assert!(!should_resume_local_chat_task(
            &build_record(
                TaskKind::EmployeeStepTask,
                TaskSurfaceKind::EmployeeStepSurface,
                TaskLifecycleStatus::Failed,
                "tool_failure_circuit_breaker",
            ),
            "继续",
        ));
    }

    #[test]
    fn resolve_local_chat_continuation_contract_recognizes_approval_resume() {
        let (mode, source, reason) = resolve_local_chat_continuation_contract(&build_record(
            TaskKind::PrimaryUserTask,
            TaskSurfaceKind::LocalChatSurface,
            TaskLifecycleStatus::Failed,
            "approval_recovery",
        ));

        assert_eq!(mode, TaskContinuationMode::ApprovalResume);
        assert_eq!(source, TaskContinuationSource::TaskEntry);
        assert_eq!(reason, "approval_recovery");
    }

    #[test]
    fn resolve_local_chat_continuation_contract_recognizes_permission_resume() {
        let (mode, source, reason) = resolve_local_chat_continuation_contract(&build_record(
            TaskKind::PrimaryUserTask,
            TaskSurfaceKind::LocalChatSurface,
            TaskLifecycleStatus::Failed,
            "PERMISSION_DENIED: tool is blocked",
        ));

        assert_eq!(mode, TaskContinuationMode::PermissionResume);
        assert_eq!(source, TaskContinuationSource::TaskEntry);
        assert_eq!(reason, "PERMISSION_DENIED: tool is blocked");
    }

    #[test]
    fn resolve_parent_rejoin_continuation_contract_marks_approval_returns() {
        let (mode, source, reason) = resolve_parent_rejoin_continuation_contract(&build_record(
            TaskKind::SubAgentTask,
            TaskSurfaceKind::HiddenChildSurface,
            TaskLifecycleStatus::Failed,
            "approval_recovery",
        ));

        assert_eq!(mode, TaskContinuationMode::ApprovalResume);
        assert_eq!(source, TaskContinuationSource::ParentRejoin);
        assert_eq!(reason, "approval_recovery");
    }

    #[test]
    fn resolve_parent_rejoin_continuation_contract_uses_status_when_terminal_reason_missing() {
        let mut record = build_record(
            TaskKind::EmployeeStepTask,
            TaskSurfaceKind::EmployeeStepSurface,
            TaskLifecycleStatus::Completed,
            "completed",
        );
        record.terminal_reason = None;

        let (mode, source, reason) = resolve_parent_rejoin_continuation_contract(&record);

        assert_eq!(mode, TaskContinuationMode::RecoveryResume);
        assert_eq!(source, TaskContinuationSource::ParentRejoin);
        assert_eq!(reason, "completed");
    }

    #[test]
    fn resolve_latest_task_run_continuation_contract_prefers_latest_matching_task_run() {
        let state = SessionJournalState {
            session_id: "session-1".to_string(),
            current_run_id: None,
            runs: vec![
                SessionRunSnapshot {
                    run_id: "run-older".to_string(),
                    user_message_id: "user-1".to_string(),
                    status: SessionRunStatus::Failed,
                    buffered_text: String::new(),
                    last_error_kind: None,
                    last_error_message: None,
                    task_identity: Some(SessionRunTaskIdentitySnapshot {
                        task_id: "task-1".to_string(),
                        parent_task_id: None,
                        root_task_id: "task-1".to_string(),
                        task_kind: "primary_user_task".to_string(),
                        surface_kind: "local_chat_surface".to_string(),
                        backend_kind: "interactive_chat_backend".to_string(),
                    }),
                    task_continuation_mode: Some("recovery_resume".to_string()),
                    task_continuation_source: Some("task_entry".to_string()),
                    task_continuation_reason: Some("max_turns".to_string()),
                    turn_state: None,
                },
                SessionRunSnapshot {
                    run_id: "run-latest".to_string(),
                    user_message_id: "user-2".to_string(),
                    status: SessionRunStatus::Failed,
                    buffered_text: String::new(),
                    last_error_kind: None,
                    last_error_message: None,
                    task_identity: Some(SessionRunTaskIdentitySnapshot {
                        task_id: "task-1".to_string(),
                        parent_task_id: None,
                        root_task_id: "task-1".to_string(),
                        task_kind: "primary_user_task".to_string(),
                        surface_kind: "local_chat_surface".to_string(),
                        backend_kind: "interactive_chat_backend".to_string(),
                    }),
                    task_continuation_mode: Some("approval_resume".to_string()),
                    task_continuation_source: Some("parent_rejoin".to_string()),
                    task_continuation_reason: Some("approval_recovery".to_string()),
                    turn_state: None,
                },
            ],
            tasks: Vec::new(),
        };

        let contract = resolve_latest_task_run_continuation_contract(&state, "task-1");

        assert_eq!(
            contract,
            Some((
                TaskContinuationMode::ApprovalResume,
                TaskContinuationSource::ParentRejoin,
                "approval_recovery".to_string()
            ))
        );
    }
}
