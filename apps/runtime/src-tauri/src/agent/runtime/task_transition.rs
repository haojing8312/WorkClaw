use crate::agent::run_guard::RunStopReasonKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskTransition {
    Continue,
    StopCompleted { terminal_reason: String },
    StopFailed { terminal_reason: String },
    StopCancelled { terminal_reason: String },
}

impl TaskTransition {
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

#[cfg(test)]
mod tests {
    use super::{
        resolve_commit_transition, resolve_stop_transition, resolve_terminal_transition,
        TaskTransition,
    };
    use crate::agent::run_guard::RunStopReasonKind;

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
}
