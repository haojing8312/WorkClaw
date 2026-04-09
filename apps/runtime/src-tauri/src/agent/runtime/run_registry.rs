use crate::session_journal::SessionRunEvent;
use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Debug, Default)]
pub struct RunRegistry {
    active_runs: RwLock<HashMap<String, String>>,
}

impl RunRegistry {
    pub fn active_run_id(&self, session_id: &str) -> Option<String> {
        self.read_active_runs().get(session_id).cloned()
    }

    pub fn register_active_run(&self, session_id: &str, run_id: &str) {
        let session_id = session_id.trim();
        let run_id = run_id.trim();
        if session_id.is_empty() || run_id.is_empty() {
            return;
        }
        self.write_active_runs()
            .insert(session_id.to_string(), run_id.to_string());
    }

    pub fn complete_run(&self, session_id: &str, run_id: &str) {
        self.clear_if_matches(session_id, run_id);
    }

    pub fn cancel_run(&self, session_id: &str, run_id: &str) {
        self.clear_if_matches(session_id, run_id);
    }

    pub fn restore_session(
        &self,
        session_id: &str,
        current_run_id: Option<&str>,
    ) -> Option<String> {
        if let Some(run_id) = self.active_run_id(session_id) {
            return Some(run_id);
        }

        let Some(run_id) = normalize_run_id(current_run_id) else {
            return None;
        };
        self.register_active_run(session_id, run_id);
        Some(run_id.to_string())
    }

    pub fn apply_event(&self, session_id: &str, event: &SessionRunEvent) {
        match event {
            SessionRunEvent::TaskStateProjected { .. }
            | SessionRunEvent::TaskDelegated { .. }
            | SessionRunEvent::TaskRecordUpserted { .. }
            | SessionRunEvent::TaskStatusChanged { .. } => {}
            SessionRunEvent::RunStarted { run_id, .. } => {
                self.register_active_run(session_id, run_id);
            }
            SessionRunEvent::RunCompleted { run_id, .. }
            | SessionRunEvent::RunStopped { run_id, .. }
            | SessionRunEvent::RunFailed { run_id, .. } => {
                self.complete_run(session_id, run_id);
            }
            SessionRunEvent::RunCancelled { run_id, .. } => {
                self.cancel_run(session_id, run_id);
            }
            SessionRunEvent::AssistantChunkAppended { .. }
            | SessionRunEvent::SkillRouteRecorded { .. }
            | SessionRunEvent::ToolStarted { .. }
            | SessionRunEvent::ToolCompleted { .. }
            | SessionRunEvent::ApprovalRequested { .. }
            | SessionRunEvent::RunGuardWarning { .. } => {}
        }
    }

    fn clear_if_matches(&self, session_id: &str, run_id: &str) {
        let session_id = session_id.trim();
        let run_id = run_id.trim();
        if session_id.is_empty() || run_id.is_empty() {
            return;
        }

        let mut active_runs = self.write_active_runs();
        if active_runs
            .get(session_id)
            .is_some_and(|current| current == run_id)
        {
            active_runs.remove(session_id);
        }
    }

    fn read_active_runs(&self) -> RwLockReadGuard<'_, HashMap<String, String>> {
        self.active_runs
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write_active_runs(&self) -> RwLockWriteGuard<'_, HashMap<String, String>> {
        self.active_runs
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn normalize_run_id(run_id: Option<&str>) -> Option<&str> {
    run_id.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

#[derive(Debug, Clone)]
pub struct RunRegistryState(pub Arc<RunRegistry>);

#[cfg(test)]
mod tests {
    use super::RunRegistry;

    #[test]
    fn restore_session_hydrates_registry_from_snapshot() {
        let registry = RunRegistry::default();

        let restored = registry.restore_session("session-1", Some("run-1"));

        assert_eq!(restored.as_deref(), Some("run-1"));
        assert_eq!(
            registry.active_run_id("session-1").as_deref(),
            Some("run-1")
        );
    }

    #[test]
    fn complete_run_only_clears_matching_active_run() {
        let registry = RunRegistry::default();
        registry.register_active_run("session-1", "run-1");

        registry.complete_run("session-1", "run-2");
        assert_eq!(
            registry.active_run_id("session-1").as_deref(),
            Some("run-1")
        );

        registry.complete_run("session-1", "run-1");
        assert_eq!(registry.active_run_id("session-1"), None);
    }
}
