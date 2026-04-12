use crate::agent_catalog::agent_definition::{AgentCapabilityFlags, AgentRoleKind};

pub(crate) fn default_group_step_allowed_tools() -> Vec<String> {
    vec![
        "read_file".to_string(),
        "write_file".to_string(),
        "glob".to_string(),
        "grep".to_string(),
        "edit".to_string(),
        "list_dir".to_string(),
        "file_stat".to_string(),
        "file_copy".to_string(),
        "bash".to_string(),
        "web_fetch".to_string(),
    ]
}

pub(crate) fn derive_capabilities_for_role(role_kind: &AgentRoleKind) -> AgentCapabilityFlags {
    match role_kind {
        AgentRoleKind::Coordinator => AgentCapabilityFlags {
            can_delegate: true,
            can_spawn_subagents: true,
            can_review: false,
            background_capable: true,
        },
        AgentRoleKind::Planner => AgentCapabilityFlags {
            can_delegate: true,
            can_spawn_subagents: false,
            can_review: false,
            background_capable: true,
        },
        AgentRoleKind::Reviewer => AgentCapabilityFlags {
            can_delegate: false,
            can_spawn_subagents: false,
            can_review: true,
            background_capable: true,
        },
        AgentRoleKind::Executor => AgentCapabilityFlags {
            can_delegate: false,
            can_spawn_subagents: false,
            can_review: false,
            background_capable: true,
        },
        AgentRoleKind::General => AgentCapabilityFlags {
            can_delegate: false,
            can_spawn_subagents: false,
            can_review: false,
            background_capable: true,
        },
    }
}

pub(crate) fn derive_allowed_tools_for_role(_role_kind: &AgentRoleKind) -> Vec<String> {
    default_group_step_allowed_tools()
}

#[cfg(test)]
mod tests {
    use super::derive_capabilities_for_role;
    use crate::agent_catalog::agent_definition::AgentRoleKind;

    #[test]
    fn derive_capabilities_for_role_disables_executor_delegation() {
        let caps = derive_capabilities_for_role(&AgentRoleKind::Executor);
        assert!(!caps.can_delegate);
        assert!(!caps.can_spawn_subagents);
        assert!(!caps.can_review);
    }
}
