use crate::agent::runtime::task_record::{TaskLifecycleStatus, TaskRecord};
use crate::agent::runtime::task_state::{TaskKind, TaskSurfaceKind};
use crate::agent::runtime::task_transition::TaskContinuationMode;

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
) -> (TaskContinuationMode, String) {
    let terminal_reason = record
        .terminal_reason
        .as_deref()
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or("recovery_resume");
    let mode = resolve_continuation_mode_from_reason(terminal_reason);

    (mode, terminal_reason.to_string())
}

pub(crate) fn resolve_parent_rejoin_continuation_contract(
    returned_task_record: &TaskRecord,
) -> (TaskContinuationMode, String) {
    let terminal_reason = returned_task_record
        .terminal_reason
        .as_deref()
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or_else(|| returned_task_record.status.as_key());
    let mode = resolve_continuation_mode_from_reason(terminal_reason);
    (mode, format!("delegated_return:{}", terminal_reason.trim()))
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
        is_task_continuation_request, resolve_local_chat_continuation_contract,
        resolve_parent_rejoin_continuation_contract, should_resume_local_chat_task,
    };
    use crate::agent::runtime::task_record::{TaskLifecycleStatus, TaskRecord};
    use crate::agent::runtime::task_state::{
        TaskBackendKind, TaskIdentity, TaskKind, TaskSurfaceKind,
    };
    use crate::agent::runtime::task_transition::TaskContinuationMode;

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
        let (mode, reason) = resolve_local_chat_continuation_contract(&build_record(
            TaskKind::PrimaryUserTask,
            TaskSurfaceKind::LocalChatSurface,
            TaskLifecycleStatus::Failed,
            "approval_recovery",
        ));

        assert_eq!(mode, TaskContinuationMode::ApprovalResume);
        assert_eq!(reason, "approval_recovery");
    }

    #[test]
    fn resolve_local_chat_continuation_contract_recognizes_permission_resume() {
        let (mode, reason) = resolve_local_chat_continuation_contract(&build_record(
            TaskKind::PrimaryUserTask,
            TaskSurfaceKind::LocalChatSurface,
            TaskLifecycleStatus::Failed,
            "PERMISSION_DENIED: tool is blocked",
        ));

        assert_eq!(mode, TaskContinuationMode::PermissionResume);
        assert_eq!(reason, "PERMISSION_DENIED: tool is blocked");
    }

    #[test]
    fn resolve_parent_rejoin_continuation_contract_marks_approval_returns() {
        let (mode, reason) = resolve_parent_rejoin_continuation_contract(&build_record(
            TaskKind::SubAgentTask,
            TaskSurfaceKind::HiddenChildSurface,
            TaskLifecycleStatus::Failed,
            "approval_recovery",
        ));

        assert_eq!(mode, TaskContinuationMode::ApprovalResume);
        assert_eq!(reason, "delegated_return:approval_recovery");
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

        let (mode, reason) = resolve_parent_rejoin_continuation_contract(&record);

        assert_eq!(mode, TaskContinuationMode::RecoveryResume);
        assert_eq!(reason, "delegated_return:completed");
    }
}
