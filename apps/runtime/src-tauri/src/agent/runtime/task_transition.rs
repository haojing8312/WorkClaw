use crate::agent::run_guard::RunStopReasonKind;
use crate::agent::runtime::task_continuation::resolve_parent_rejoin_continuation_contract;
use crate::agent::runtime::task_record::{TaskLifecycleStatus, TaskRecord};
use crate::agent::runtime::task_state::{
    TaskBackendKind, TaskIdentity, TaskKind, TaskState, TaskSurfaceKind,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum TaskContinuationMode {
    InitialStart,
    RecoveryResume,
    ApprovalResume,
    PermissionResume,
}

impl TaskContinuationMode {
    pub(crate) fn as_key(self) -> &'static str {
        match self {
            TaskContinuationMode::InitialStart => "initial_start",
            TaskContinuationMode::RecoveryResume => "recovery_resume",
            TaskContinuationMode::ApprovalResume => "approval_resume",
            TaskContinuationMode::PermissionResume => "permission_resume",
        }
    }

    pub(crate) fn from_key(value: &str) -> Option<Self> {
        match value.trim() {
            "initial_start" => Some(TaskContinuationMode::InitialStart),
            "recovery_resume" => Some(TaskContinuationMode::RecoveryResume),
            "approval_resume" => Some(TaskContinuationMode::ApprovalResume),
            "permission_resume" => Some(TaskContinuationMode::PermissionResume),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum TaskContinuationSource {
    TaskEntry,
    ParentRejoin,
}

impl TaskContinuationSource {
    pub(crate) fn as_key(self) -> &'static str {
        match self {
            TaskContinuationSource::TaskEntry => "task_entry",
            TaskContinuationSource::ParentRejoin => "parent_rejoin",
        }
    }

    pub(crate) fn from_key(value: &str) -> Option<Self> {
        match value.trim() {
            "task_entry" => Some(TaskContinuationSource::TaskEntry),
            "parent_rejoin" => Some(TaskContinuationSource::ParentRejoin),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskTransition {
    Continue {
        mode: TaskContinuationMode,
        source: TaskContinuationSource,
        reason: String,
    },
    DelegateToChild {
        delegated_task_identity: TaskIdentity,
        delegated_task_kind: TaskKind,
        delegated_surface_kind: TaskSurfaceKind,
        delegated_backend_kind: TaskBackendKind,
    },
    DelegateToEmployee {
        delegated_task_identity: TaskIdentity,
        delegated_task_kind: TaskKind,
        delegated_surface_kind: TaskSurfaceKind,
        delegated_backend_kind: TaskBackendKind,
    },
    ReturnFromDelegatedTask {
        returned_task_identity: TaskIdentity,
        returned_task_kind: TaskKind,
        returned_surface_kind: TaskSurfaceKind,
        returned_backend_kind: TaskBackendKind,
        returned_status: TaskLifecycleStatus,
        terminal_reason: Option<String>,
    },
    StopCompleted {
        terminal_reason: String,
    },
    StopFailed {
        terminal_reason: String,
    },
    StopCancelled {
        terminal_reason: String,
    },
}

impl TaskTransition {
    pub(crate) fn continued(mode: TaskContinuationMode, reason: impl Into<String>) -> Self {
        Self::continued_with_source(mode, TaskContinuationSource::TaskEntry, reason)
    }

    pub(crate) fn continued_with_source(
        mode: TaskContinuationMode,
        source: TaskContinuationSource,
        reason: impl Into<String>,
    ) -> Self {
        Self::Continue {
            mode,
            source,
            reason: reason.into(),
        }
    }

    pub(crate) fn completed(terminal_reason: impl Into<String>) -> Self {
        Self::StopCompleted {
            terminal_reason: terminal_reason.into(),
        }
    }

    pub(crate) fn failed(terminal_reason: impl Into<String>) -> Self {
        Self::StopFailed {
            terminal_reason: terminal_reason.into(),
        }
    }

    pub(crate) fn cancelled(terminal_reason: impl Into<String>) -> Self {
        Self::StopCancelled {
            terminal_reason: terminal_reason.into(),
        }
    }
}

pub(crate) fn resolve_commit_transition(
    commit_result: &Result<(), String>,
    failure_reason: Option<&str>,
) -> TaskTransition {
    match commit_result {
        Ok(()) => TaskTransition::completed("completed"),
        Err(error) => TaskTransition::failed(
            failure_reason
                .filter(|reason| !reason.trim().is_empty())
                .unwrap_or(error.as_str()),
        ),
    }
}

pub(crate) fn resolve_terminal_transition(
    success: bool,
    failure_reason: Option<&str>,
) -> TaskTransition {
    if success {
        TaskTransition::completed("completed")
    } else {
        TaskTransition::failed(
            failure_reason
                .filter(|reason| !reason.trim().is_empty())
                .unwrap_or("failed"),
        )
    }
}

pub(crate) fn resolve_stop_transition(
    stop_reason_kind: RunStopReasonKind,
    fallback_reason: Option<&str>,
) -> TaskTransition {
    match stop_reason_kind {
        RunStopReasonKind::Cancelled => TaskTransition::cancelled(
            fallback_reason
                .filter(|reason| !reason.trim().is_empty())
                .unwrap_or(stop_reason_kind.as_key()),
        ),
        _ => TaskTransition::failed(
            fallback_reason
                .filter(|reason| !reason.trim().is_empty())
                .unwrap_or(stop_reason_kind.as_key()),
        ),
    }
}

pub(crate) fn resolve_delegation_transition(task_state: &TaskState) -> Option<TaskTransition> {
    match (task_state.task_kind, task_state.surface_kind) {
        (TaskKind::SubAgentTask, TaskSurfaceKind::HiddenChildSurface) => {
            Some(TaskTransition::DelegateToChild {
                delegated_task_identity: task_state.task_identity.clone(),
                delegated_task_kind: task_state.task_kind,
                delegated_surface_kind: task_state.surface_kind,
                delegated_backend_kind: task_state.backend_kind,
            })
        }
        (TaskKind::EmployeeStepTask, TaskSurfaceKind::EmployeeStepSurface) => {
            Some(TaskTransition::DelegateToEmployee {
                delegated_task_identity: task_state.task_identity.clone(),
                delegated_task_kind: task_state.task_kind,
                delegated_surface_kind: task_state.surface_kind,
                delegated_backend_kind: task_state.backend_kind,
            })
        }
        _ => None,
    }
}

pub(crate) fn resolve_initial_transition(task_state: &TaskState) -> TaskTransition {
    resolve_delegation_transition(task_state).unwrap_or_else(|| {
        if let Some(mode) = task_state.continuation_mode {
            return TaskTransition::continued_with_source(
                mode,
                task_state
                    .continuation_source
                    .unwrap_or(TaskContinuationSource::TaskEntry),
                task_state
                    .continuation_reason
                    .clone()
                    .unwrap_or_else(|| mode.as_key().to_string()),
            );
        }

        TaskTransition::continued(TaskContinuationMode::InitialStart, "initial_start")
    })
}

pub(crate) fn resolve_delegated_return_transition(
    returned_task_record: &TaskRecord,
) -> TaskTransition {
    TaskTransition::ReturnFromDelegatedTask {
        returned_task_identity: returned_task_record.task_identity.clone(),
        returned_task_kind: returned_task_record.task_kind,
        returned_surface_kind: returned_task_record.surface_kind,
        returned_backend_kind: returned_task_record.backend_kind,
        returned_status: returned_task_record.status,
        terminal_reason: returned_task_record.terminal_reason.clone(),
    }
}

pub(crate) fn resolve_parent_rejoin_transition(
    returned_task_record: &TaskRecord,
) -> TaskTransition {
    let (mode, source, reason) = resolve_parent_rejoin_continuation_contract(returned_task_record);
    TaskTransition::continued_with_source(mode, source, reason)
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_commit_transition, resolve_delegated_return_transition,
        resolve_delegation_transition, resolve_initial_transition,
        resolve_parent_rejoin_transition, resolve_stop_transition, resolve_terminal_transition,
        TaskContinuationMode, TaskContinuationSource, TaskTransition,
    };
    use crate::agent::run_guard::RunStopReasonKind;
    use crate::agent::runtime::task_record::{TaskLifecycleStatus, TaskRecord};
    use crate::agent::runtime::task_state::{
        TaskBackendKind, TaskIdentity, TaskKind, TaskState, TaskSurfaceKind,
    };

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

    #[test]
    fn resolve_terminal_transition_marks_success_as_completed() {
        let transition = resolve_terminal_transition(true, None);

        assert_eq!(
            transition,
            TaskTransition::StopCompleted {
                terminal_reason: "completed".to_string(),
            }
        );
    }

    #[test]
    fn resolve_terminal_transition_uses_failure_reason_when_present() {
        let transition = resolve_terminal_transition(false, Some("skill_command_dispatch"));

        assert_eq!(
            transition,
            TaskTransition::StopFailed {
                terminal_reason: "skill_command_dispatch".to_string(),
            }
        );
    }

    #[test]
    fn resolve_stop_transition_marks_cancelled_runs_as_cancelled() {
        let transition = resolve_stop_transition(RunStopReasonKind::Cancelled, None);

        assert_eq!(
            transition,
            TaskTransition::StopCancelled {
                terminal_reason: "cancelled".to_string(),
            }
        );
    }

    #[test]
    fn resolve_stop_transition_marks_other_stop_reasons_as_failed() {
        let transition =
            resolve_stop_transition(RunStopReasonKind::ToolFailureCircuitBreaker, None);

        assert_eq!(
            transition,
            TaskTransition::StopFailed {
                terminal_reason: "tool_failure_circuit_breaker".to_string(),
            }
        );
    }

    #[test]
    fn resolve_delegation_transition_marks_hidden_child_tasks_as_child_delegation() {
        let parent = TaskIdentity::new("task-parent", Option::<String>::None, Some("task-root"));
        let task_state = TaskState::new_sub_agent("session-1", "user-1", "run-1", Some(&parent));

        assert!(matches!(
            resolve_delegation_transition(&task_state),
            Some(TaskTransition::DelegateToChild {
                delegated_task_identity,
                delegated_task_kind: TaskKind::SubAgentTask,
                delegated_surface_kind: TaskSurfaceKind::HiddenChildSurface,
                delegated_backend_kind: TaskBackendKind::HiddenChildBackend,
            }) if delegated_task_identity.parent_task_id.as_deref() == Some("task-parent")
        ));
    }

    #[test]
    fn resolve_delegation_transition_marks_employee_tasks_as_employee_delegation() {
        let parent = TaskIdentity::new("task-parent", Option::<String>::None, Some("task-root"));
        let task_state =
            TaskState::new_employee_step("session-1", "user-1", "run-1", Some(&parent));

        assert!(matches!(
            resolve_delegation_transition(&task_state),
            Some(TaskTransition::DelegateToEmployee {
                delegated_task_identity,
                delegated_task_kind: TaskKind::EmployeeStepTask,
                delegated_surface_kind: TaskSurfaceKind::EmployeeStepSurface,
                delegated_backend_kind: TaskBackendKind::EmployeeStepBackend,
            }) if delegated_task_identity.parent_task_id.as_deref() == Some("task-parent")
        ));
    }

    #[test]
    fn resolve_initial_transition_keeps_primary_local_chat_on_continue() {
        let task_state = TaskState::new_primary_local_chat("session-1", "user-1", "run-1");

        assert_eq!(
            resolve_initial_transition(&task_state),
            TaskTransition::Continue {
                mode: TaskContinuationMode::InitialStart,
                source: TaskContinuationSource::TaskEntry,
                reason: "initial_start".to_string(),
            }
        );
    }

    #[test]
    fn resolve_initial_transition_marks_recovery_tasks_as_recovery_resume() {
        let parent = TaskIdentity::new("task-parent", Option::<String>::None, Some("task-root"));
        let task_state =
            TaskState::new_recovery_local_chat("session-1", "user-2", "run-2", &parent);

        assert_eq!(
            resolve_initial_transition(&task_state),
            TaskTransition::Continue {
                mode: TaskContinuationMode::RecoveryResume,
                source: TaskContinuationSource::TaskEntry,
                reason: "recovery_resume".to_string(),
            }
        );
    }

    #[test]
    fn resolve_initial_transition_preserves_parent_rejoin_source_for_recovery_tasks() {
        let parent = TaskIdentity::new("task-parent", Option::<String>::None, Some("task-root"));
        let task_state = TaskState::new_recovery_local_chat_with_contract(
            "session-1",
            "user-2",
            "run-2",
            &parent,
            TaskContinuationMode::PermissionResume,
            TaskContinuationSource::ParentRejoin,
            "permission_denied",
        );

        assert_eq!(
            resolve_initial_transition(&task_state),
            TaskTransition::Continue {
                mode: TaskContinuationMode::PermissionResume,
                source: TaskContinuationSource::ParentRejoin,
                reason: "permission_denied".to_string(),
            }
        );
    }

    #[test]
    fn resolve_delegated_return_transition_preserves_terminal_task_metadata() {
        let record = TaskRecord {
            task_identity: TaskIdentity::new("task-child", Some("task-parent"), Some("task-root")),
            task_kind: TaskKind::SubAgentTask,
            surface_kind: TaskSurfaceKind::HiddenChildSurface,
            backend_kind: TaskBackendKind::HiddenChildBackend,
            session_id: "session-child".to_string(),
            user_message_id: "user-1".to_string(),
            run_id: "run-child".to_string(),
            status: TaskLifecycleStatus::Failed,
            created_at: "2026-04-10T10:00:00Z".to_string(),
            updated_at: "2026-04-10T10:02:00Z".to_string(),
            started_at: Some("2026-04-10T10:00:30Z".to_string()),
            completed_at: Some("2026-04-10T10:02:00Z".to_string()),
            terminal_reason: Some("tool_failure_circuit_breaker".to_string()),
        };

        assert_eq!(
            resolve_delegated_return_transition(&record),
            TaskTransition::ReturnFromDelegatedTask {
                returned_task_identity: record.task_identity.clone(),
                returned_task_kind: TaskKind::SubAgentTask,
                returned_surface_kind: TaskSurfaceKind::HiddenChildSurface,
                returned_backend_kind: TaskBackendKind::HiddenChildBackend,
                returned_status: TaskLifecycleStatus::Failed,
                terminal_reason: Some("tool_failure_circuit_breaker".to_string()),
            }
        );
    }

    #[test]
    fn continuation_mode_round_trips_through_stable_keys() {
        assert_eq!(
            TaskContinuationMode::from_key(TaskContinuationMode::InitialStart.as_key()),
            Some(TaskContinuationMode::InitialStart)
        );
        assert_eq!(
            TaskContinuationMode::from_key(TaskContinuationMode::RecoveryResume.as_key()),
            Some(TaskContinuationMode::RecoveryResume)
        );
        assert_eq!(
            TaskContinuationMode::from_key(TaskContinuationMode::ApprovalResume.as_key()),
            Some(TaskContinuationMode::ApprovalResume)
        );
        assert_eq!(
            TaskContinuationMode::from_key(TaskContinuationMode::PermissionResume.as_key()),
            Some(TaskContinuationMode::PermissionResume)
        );
        assert_eq!(
            TaskContinuationSource::from_key(TaskContinuationSource::TaskEntry.as_key()),
            Some(TaskContinuationSource::TaskEntry)
        );
        assert_eq!(
            TaskContinuationSource::from_key(TaskContinuationSource::ParentRejoin.as_key()),
            Some(TaskContinuationSource::ParentRejoin)
        );
        assert_eq!(TaskContinuationMode::from_key("unknown"), None);
        assert_eq!(TaskContinuationSource::from_key("unknown"), None);
    }

    #[test]
    fn resolve_parent_rejoin_transition_preserves_permission_resume_contract() {
        let record = TaskRecord {
            task_identity: TaskIdentity::new("task-child", Some("task-parent"), Some("task-root")),
            task_kind: TaskKind::EmployeeStepTask,
            surface_kind: TaskSurfaceKind::EmployeeStepSurface,
            backend_kind: TaskBackendKind::EmployeeStepBackend,
            session_id: "session-parent".to_string(),
            user_message_id: "user-1".to_string(),
            run_id: "run-child".to_string(),
            status: TaskLifecycleStatus::Failed,
            created_at: "2026-04-10T10:00:00Z".to_string(),
            updated_at: "2026-04-10T10:02:00Z".to_string(),
            started_at: Some("2026-04-10T10:00:30Z".to_string()),
            completed_at: Some("2026-04-10T10:02:00Z".to_string()),
            terminal_reason: Some("permission_denied".to_string()),
        };

        assert_eq!(
            resolve_parent_rejoin_transition(&record),
            TaskTransition::Continue {
                mode: TaskContinuationMode::PermissionResume,
                source: TaskContinuationSource::ParentRejoin,
                reason: "permission_denied".to_string(),
            }
        );
    }
}
