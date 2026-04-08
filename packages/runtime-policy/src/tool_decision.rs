use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPermissionAction {
    Allow,
    Ask,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPermissionDecision {
    pub action: ToolPermissionAction,
    pub reason: Option<String>,
    pub fingerprint: Option<String>,
}

impl ToolPermissionDecision {
    pub fn allow() -> Self {
        Self {
            action: ToolPermissionAction::Allow,
            reason: None,
            fingerprint: None,
        }
    }

    pub fn ask(reason: impl Into<String>, fingerprint: Option<String>) -> Self {
        Self {
            action: ToolPermissionAction::Ask,
            reason: Some(reason.into()),
            fingerprint,
        }
    }

    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            action: ToolPermissionAction::Deny,
            reason: Some(reason.into()),
            fingerprint: None,
        }
    }

    pub fn is_allow(&self) -> bool {
        matches!(self.action, ToolPermissionAction::Allow)
    }

    pub fn is_ask(&self) -> bool {
        matches!(self.action, ToolPermissionAction::Ask)
    }

    pub fn is_deny(&self) -> bool {
        matches!(self.action, ToolPermissionAction::Deny)
    }
}

