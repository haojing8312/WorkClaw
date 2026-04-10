use serde::{Deserialize, Serialize};

use crate::agent::runtime::task_record::{TaskLifecycleStatus, TaskRecord};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TaskRepo;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRecordUpsertPayload {
    pub(crate) task_identity: crate::agent::runtime::task_state::TaskIdentity,
    pub(crate) task_kind: crate::agent::runtime::task_state::TaskKind,
    pub(crate) surface_kind: crate::agent::runtime::task_state::TaskSurfaceKind,
    pub(crate) backend_kind: crate::agent::runtime::task_state::TaskBackendKind,
    pub(crate) session_id: String,
    pub(crate) user_message_id: String,
    pub(crate) run_id: String,
    pub(crate) status: TaskLifecycleStatus,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) started_at: Option<String>,
    pub(crate) completed_at: Option<String>,
    pub(crate) terminal_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskStatusChangedPayload {
    pub(crate) task_id: String,
    pub(crate) parent_task_id: Option<String>,
    pub(crate) root_task_id: String,
    pub(crate) from_status: TaskLifecycleStatus,
    pub(crate) to_status: TaskLifecycleStatus,
    pub(crate) terminal_reason: Option<String>,
    pub(crate) updated_at: String,
}

impl TaskRecordUpsertPayload {
    fn from_task_record(record: &TaskRecord) -> Self {
        Self {
            task_identity: record.task_identity.clone(),
            task_kind: record.task_kind,
            surface_kind: record.surface_kind,
            backend_kind: record.backend_kind,
            session_id: record.session_id.clone(),
            user_message_id: record.user_message_id.clone(),
            run_id: record.run_id.clone(),
            status: record.status,
            created_at: record.created_at.clone(),
            updated_at: record.updated_at.clone(),
            started_at: record.started_at.clone(),
            completed_at: record.completed_at.clone(),
            terminal_reason: record.terminal_reason.clone(),
        }
    }
}

impl TaskStatusChangedPayload {
    fn from_task_record(
        record: &TaskRecord,
        from_status: TaskLifecycleStatus,
        to_status: TaskLifecycleStatus,
    ) -> Self {
        Self {
            task_id: record.task_identity.task_id.clone(),
            parent_task_id: record.task_identity.parent_task_id.clone(),
            root_task_id: record.task_identity.root_task_id.clone(),
            from_status,
            to_status,
            terminal_reason: record.terminal_reason.clone(),
            updated_at: record.updated_at.clone(),
        }
    }
}

impl TaskRepo {
    pub(crate) fn build_task_record_upsert_payload(record: &TaskRecord) -> TaskRecordUpsertPayload {
        TaskRecordUpsertPayload::from_task_record(record)
    }

    pub(crate) fn build_task_status_changed_payload(
        record: &TaskRecord,
        from_status: TaskLifecycleStatus,
        to_status: TaskLifecycleStatus,
    ) -> TaskStatusChangedPayload {
        TaskStatusChangedPayload::from_task_record(record, from_status, to_status)
    }

    pub(crate) fn apply_task_status_change(
        snapshot: &mut crate::session_journal::SessionTaskRecordSnapshot,
        status_change: &TaskStatusChangedPayload,
    ) {
        snapshot.status = status_change.to_status;
        snapshot.updated_at = status_change.updated_at.clone();
        snapshot.terminal_reason = status_change.terminal_reason.clone();

        match status_change.to_status {
            TaskLifecycleStatus::Pending => {
                snapshot.completed_at = None;
            }
            TaskLifecycleStatus::Running => {
                snapshot.completed_at = None;
                if snapshot.started_at.is_none() {
                    snapshot.started_at = Some(status_change.updated_at.clone());
                }
            }
            TaskLifecycleStatus::Completed
            | TaskLifecycleStatus::Failed
            | TaskLifecycleStatus::Cancelled => {
                snapshot.completed_at = Some(status_change.updated_at.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TaskLifecycleStatus, TaskRecord, TaskRepo, TaskStatusChangedPayload};
    use crate::agent::runtime::task_state::{
        TaskBackendKind, TaskIdentity, TaskKind, TaskSurfaceKind,
    };

    #[test]
    fn build_task_record_upsert_payload_preserves_task_identity_and_snapshot_fields() {
        let record = TaskRecord::new_pending(
            TaskIdentity::new("task-child", Some("task-parent"), Some("task-root")),
            TaskKind::EmployeeStepTask,
            TaskSurfaceKind::EmployeeStepSurface,
            TaskBackendKind::EmployeeStepBackend,
            "session-1",
            "user-1",
            "run-1",
            "2026-04-09T10:00:00Z",
        );

        let payload = TaskRepo::build_task_record_upsert_payload(&record);

        assert_eq!(payload.task_identity.task_id, "task-child");
        assert_eq!(
            payload.task_identity.parent_task_id.as_deref(),
            Some("task-parent")
        );
        assert_eq!(payload.task_identity.root_task_id, "task-root");
        assert_eq!(payload.task_kind, TaskKind::EmployeeStepTask);
        assert_eq!(payload.surface_kind, TaskSurfaceKind::EmployeeStepSurface);
        assert_eq!(payload.backend_kind, TaskBackendKind::EmployeeStepBackend);
        assert_eq!(payload.session_id, "session-1");
        assert_eq!(payload.user_message_id, "user-1");
        assert_eq!(payload.run_id, "run-1");
        assert_eq!(payload.status, TaskLifecycleStatus::Pending);
        assert_eq!(payload.created_at, "2026-04-09T10:00:00Z");
        assert_eq!(payload.updated_at, "2026-04-09T10:00:00Z");
        assert_eq!(payload.started_at, None);
        assert_eq!(payload.completed_at, None);
        assert_eq!(payload.terminal_reason, None);
    }

    #[test]
    fn build_task_status_changed_payload_preserves_lineage_and_terminal_reason() {
        let record = TaskRecord::new_pending(
            TaskIdentity::new("task-child", Some("task-parent"), Some("task-root")),
            TaskKind::SubAgentTask,
            TaskSurfaceKind::HiddenChildSurface,
            TaskBackendKind::HiddenChildBackend,
            "session-1",
            "user-1",
            "run-1",
            "2026-04-09T10:00:00Z",
        )
        .mark_running("2026-04-09T10:01:00Z")
        .unwrap()
        .mark_failed("2026-04-09T10:02:00Z", "tool_timeout")
        .unwrap();

        let payload = TaskRepo::build_task_status_changed_payload(
            &record,
            TaskLifecycleStatus::Running,
            TaskLifecycleStatus::Failed,
        );

        assert_eq!(payload.task_id, "task-child");
        assert_eq!(payload.parent_task_id.as_deref(), Some("task-parent"));
        assert_eq!(payload.root_task_id, "task-root");
        assert_eq!(payload.from_status, TaskLifecycleStatus::Running);
        assert_eq!(payload.to_status, TaskLifecycleStatus::Failed);
        assert_eq!(payload.terminal_reason.as_deref(), Some("tool_timeout"));
        assert_eq!(payload.updated_at, "2026-04-09T10:02:00Z");
    }

    #[test]
    fn status_changed_payload_is_serializable_for_future_journal_events() {
        let payload = TaskStatusChangedPayload {
            task_id: "task-child".to_string(),
            parent_task_id: Some("task-parent".to_string()),
            root_task_id: "task-root".to_string(),
            from_status: TaskLifecycleStatus::Pending,
            to_status: TaskLifecycleStatus::Cancelled,
            terminal_reason: Some("user_cancelled".to_string()),
            updated_at: "2026-04-09T10:03:00Z".to_string(),
        };

        let serialized = serde_json::to_string(&payload).unwrap();

        assert!(serialized.contains("\"task_id\":\"task-child\""));
        assert!(serialized.contains("\"parent_task_id\":\"task-parent\""));
        assert!(serialized.contains("\"terminal_reason\":\"user_cancelled\""));
    }
}
