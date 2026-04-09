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

#[cfg(test)]
mod tests {
    use super::{resolve_commit_transition, TaskTransition};

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
