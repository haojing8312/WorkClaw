use serde::{Deserialize, Serialize};

use crate::agent::runtime::task_state::{TaskBackendKind, TaskIdentity, TaskKind, TaskSurfaceKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TaskLifecycleTransitionError {
    pub from: TaskLifecycleStatus,
    pub to: TaskLifecycleStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskLifecycleStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl TaskLifecycleStatus {
    pub(crate) fn as_key(self) -> &'static str {
        match self {
            TaskLifecycleStatus::Pending => "pending",
            TaskLifecycleStatus::Running => "running",
            TaskLifecycleStatus::Completed => "completed",
            TaskLifecycleStatus::Failed => "failed",
            TaskLifecycleStatus::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TaskRecord {
    pub task_identity: TaskIdentity,
    pub task_kind: TaskKind,
    pub surface_kind: TaskSurfaceKind,
    pub backend_kind: TaskBackendKind,
    pub session_id: String,
    pub user_message_id: String,
    pub run_id: String,
    pub status: TaskLifecycleStatus,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub terminal_reason: Option<String>,
}

impl TaskRecord {
    pub(crate) fn new_pending(
        task_identity: TaskIdentity,
        task_kind: TaskKind,
        surface_kind: TaskSurfaceKind,
        backend_kind: TaskBackendKind,
        session_id: impl Into<String>,
        user_message_id: impl Into<String>,
        run_id: impl Into<String>,
        now: impl Into<String>,
    ) -> Self {
        let now = now.into();
        Self {
            task_identity,
            task_kind,
            surface_kind,
            backend_kind,
            session_id: session_id.into(),
            user_message_id: user_message_id.into(),
            run_id: run_id.into(),
            status: TaskLifecycleStatus::Pending,
            created_at: now.clone(),
            updated_at: now,
            started_at: None,
            completed_at: None,
            terminal_reason: None,
        }
    }

    pub(crate) fn mark_running(
        mut self,
        now: impl Into<String>,
    ) -> Result<Self, TaskLifecycleTransitionError> {
        let now = now.into();
        match self.status {
            TaskLifecycleStatus::Pending => {
                self.started_at = Some(now.clone());
                self.status = TaskLifecycleStatus::Running;
                self.updated_at = now;
                Ok(self)
            }
            TaskLifecycleStatus::Running => {
                self.updated_at = now;
                Ok(self)
            }
            status => Err(TaskLifecycleTransitionError {
                from: status,
                to: TaskLifecycleStatus::Running,
            }),
        }
    }

    pub(crate) fn mark_completed(
        self,
        now: impl Into<String>,
        terminal_reason: impl Into<String>,
    ) -> Result<Self, TaskLifecycleTransitionError> {
        self.mark_terminal(
            TaskLifecycleStatus::Completed,
            now.into(),
            terminal_reason.into(),
        )
    }

    pub(crate) fn mark_failed(
        self,
        now: impl Into<String>,
        terminal_reason: impl Into<String>,
    ) -> Result<Self, TaskLifecycleTransitionError> {
        self.mark_terminal(
            TaskLifecycleStatus::Failed,
            now.into(),
            terminal_reason.into(),
        )
    }

    pub(crate) fn mark_cancelled(
        self,
        now: impl Into<String>,
        terminal_reason: impl Into<String>,
    ) -> Result<Self, TaskLifecycleTransitionError> {
        self.mark_terminal(
            TaskLifecycleStatus::Cancelled,
            now.into(),
            terminal_reason.into(),
        )
    }

    fn mark_terminal(
        mut self,
        terminal_status: TaskLifecycleStatus,
        now: String,
        terminal_reason: String,
    ) -> Result<Self, TaskLifecycleTransitionError> {
        match self.status {
            TaskLifecycleStatus::Pending | TaskLifecycleStatus::Running => {
                self.status = terminal_status;
                self.completed_at = Some(now.clone());
                self.updated_at = now;
                self.terminal_reason = Some(terminal_reason);
                Ok(self)
            }
            status => Err(TaskLifecycleTransitionError {
                from: status,
                to: terminal_status,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TaskLifecycleStatus, TaskLifecycleTransitionError, TaskRecord};
    use crate::agent::runtime::task_state::{
        TaskBackendKind, TaskIdentity, TaskKind, TaskSurfaceKind,
    };

    #[test]
    fn task_lifecycle_status_serializes_with_stable_snake_case_keys() {
        assert_eq!(
            serde_json::to_string(&TaskLifecycleStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&TaskLifecycleStatus::Running).unwrap(),
            "\"running\""
        );
        assert_eq!(
            serde_json::from_str::<TaskLifecycleStatus>("\"cancelled\"").unwrap(),
            TaskLifecycleStatus::Cancelled
        );
    }

    #[test]
    fn new_pending_preserves_task_identity_lineage_and_initial_timestamps() {
        let identity = TaskIdentity::new("task-child", Some("task-parent"), Some("task-root"));

        let record = TaskRecord::new_pending(
            identity.clone(),
            TaskKind::EmployeeStepTask,
            TaskSurfaceKind::EmployeeStepSurface,
            TaskBackendKind::EmployeeStepBackend,
            "session-1",
            "user-1",
            "run-1",
            "2026-04-09T10:00:00Z",
        );

        assert_eq!(record.task_identity, identity);
        assert_eq!(record.task_kind, TaskKind::EmployeeStepTask);
        assert_eq!(record.surface_kind, TaskSurfaceKind::EmployeeStepSurface);
        assert_eq!(record.backend_kind, TaskBackendKind::EmployeeStepBackend);
        assert_eq!(record.session_id, "session-1");
        assert_eq!(record.user_message_id, "user-1");
        assert_eq!(record.run_id, "run-1");
        assert_eq!(record.status, TaskLifecycleStatus::Pending);
        assert_eq!(record.created_at, "2026-04-09T10:00:00Z");
        assert_eq!(record.updated_at, "2026-04-09T10:00:00Z");
        assert_eq!(record.started_at, None);
        assert_eq!(record.completed_at, None);
        assert_eq!(record.terminal_reason, None);
    }

    #[test]
    fn terminal_helpers_preserve_start_time_and_set_terminal_reason() {
        let identity = TaskIdentity::new("task-child", Some("task-parent"), Some("task-root"));
        let pending = TaskRecord::new_pending(
            identity,
            TaskKind::SubAgentTask,
            TaskSurfaceKind::HiddenChildSurface,
            TaskBackendKind::HiddenChildBackend,
            "session-1",
            "user-1",
            "run-1",
            "2026-04-09T10:00:00Z",
        );

        let running = pending.mark_running("2026-04-09T10:01:00Z").unwrap();
        let completed = running
            .mark_completed("2026-04-09T10:02:00Z", "finished")
            .unwrap();

        assert_eq!(completed.status, TaskLifecycleStatus::Completed);
        assert_eq!(completed.created_at, "2026-04-09T10:00:00Z");
        assert_eq!(completed.updated_at, "2026-04-09T10:02:00Z");
        assert_eq!(
            completed.started_at.as_deref(),
            Some("2026-04-09T10:01:00Z")
        );
        assert_eq!(
            completed.completed_at.as_deref(),
            Some("2026-04-09T10:02:00Z")
        );
        assert_eq!(completed.terminal_reason.as_deref(), Some("finished"));
    }

    #[test]
    fn terminal_helpers_keep_previous_identity_and_override_reason_for_cancelled() {
        let identity = TaskIdentity::new("task-child", Some("task-parent"), Some("task-root"));
        let pending = TaskRecord::new_pending(
            identity,
            TaskKind::PrimaryUserTask,
            TaskSurfaceKind::LocalChatSurface,
            TaskBackendKind::InteractiveChatBackend,
            "session-1",
            "user-1",
            "run-1",
            "2026-04-09T10:00:00Z",
        );

        let running = pending.mark_running("2026-04-09T10:01:00Z").unwrap();
        let cancelled = running
            .mark_cancelled("2026-04-09T10:03:00Z", "user_cancelled")
            .unwrap();

        assert_eq!(cancelled.status, TaskLifecycleStatus::Cancelled);
        assert_eq!(cancelled.task_identity.root_task_id, "task-root");
        assert_eq!(
            cancelled.started_at.as_deref(),
            Some("2026-04-09T10:01:00Z")
        );
        assert_eq!(
            cancelled.completed_at.as_deref(),
            Some("2026-04-09T10:03:00Z")
        );
        assert_eq!(cancelled.terminal_reason.as_deref(), Some("user_cancelled"));
    }

    #[test]
    fn mark_failed_preserves_start_time_and_sets_failure_reason() {
        let identity = TaskIdentity::new("task-child", Some("task-parent"), Some("task-root"));
        let pending = TaskRecord::new_pending(
            identity,
            TaskKind::SubAgentTask,
            TaskSurfaceKind::HiddenChildSurface,
            TaskBackendKind::HiddenChildBackend,
            "session-1",
            "user-1",
            "run-1",
            "2026-04-09T10:00:00Z",
        );

        let running = pending.mark_running("2026-04-09T10:01:00Z").unwrap();
        let failed = running
            .mark_failed("2026-04-09T10:02:00Z", "tool_timeout")
            .unwrap();

        assert_eq!(failed.status, TaskLifecycleStatus::Failed);
        assert_eq!(failed.started_at.as_deref(), Some("2026-04-09T10:01:00Z"));
        assert_eq!(failed.completed_at.as_deref(), Some("2026-04-09T10:02:00Z"));
        assert_eq!(failed.terminal_reason.as_deref(), Some("tool_timeout"));
    }

    #[test]
    fn cannot_return_terminal_task_to_running() {
        let identity = TaskIdentity::new("task-child", Some("task-parent"), Some("task-root"));
        let pending = TaskRecord::new_pending(
            identity,
            TaskKind::PrimaryUserTask,
            TaskSurfaceKind::LocalChatSurface,
            TaskBackendKind::InteractiveChatBackend,
            "session-1",
            "user-1",
            "run-1",
            "2026-04-09T10:00:00Z",
        );

        let completed = pending
            .mark_completed("2026-04-09T10:02:00Z", "finished")
            .unwrap();
        let error = completed.mark_running("2026-04-09T10:03:00Z").unwrap_err();

        assert_eq!(
            error,
            TaskLifecycleTransitionError {
                from: TaskLifecycleStatus::Completed,
                to: TaskLifecycleStatus::Running,
            }
        );
    }

    #[test]
    fn cannot_apply_second_terminal_transition() {
        let identity = TaskIdentity::new("task-child", Some("task-parent"), Some("task-root"));
        let pending = TaskRecord::new_pending(
            identity,
            TaskKind::SubAgentTask,
            TaskSurfaceKind::HiddenChildSurface,
            TaskBackendKind::HiddenChildBackend,
            "session-1",
            "user-1",
            "run-1",
            "2026-04-09T10:00:00Z",
        );

        let failed = pending
            .mark_failed("2026-04-09T10:02:00Z", "tool_timeout")
            .unwrap();
        let error = failed
            .mark_cancelled("2026-04-09T10:03:00Z", "user_cancelled")
            .unwrap_err();

        assert_eq!(
            error,
            TaskLifecycleTransitionError {
                from: TaskLifecycleStatus::Failed,
                to: TaskLifecycleStatus::Cancelled,
            }
        );
    }
}
