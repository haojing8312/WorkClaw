#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentRoleKind {
    Coordinator,
    Planner,
    Reviewer,
    Executor,
    General,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentMemoryScope {
    Session,
    Employee,
    Shared,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct AgentCapabilityFlags {
    pub can_delegate: bool,
    pub can_spawn_subagents: bool,
    pub can_review: bool,
    pub background_capable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentDefinition {
    pub agent_id: String,
    pub display_name: String,
    pub role_kind: AgentRoleKind,
    pub workspace_dir: String,
    pub persona_text: String,
    pub allowed_tools: Vec<String>,
    pub permission_mode: String,
    pub model_id: Option<String>,
    pub memory_scope: AgentMemoryScope,
    pub capabilities: AgentCapabilityFlags,
}

pub(crate) fn default_memory_scope_for_role(role_kind: &AgentRoleKind) -> AgentMemoryScope {
    match role_kind {
        AgentRoleKind::Coordinator => AgentMemoryScope::Shared,
        AgentRoleKind::Planner | AgentRoleKind::Reviewer => AgentMemoryScope::Session,
        AgentRoleKind::Executor | AgentRoleKind::General => AgentMemoryScope::Employee,
    }
}

pub(crate) fn normalize_agent_id(raw: &str) -> String {
    raw.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{
        default_memory_scope_for_role, normalize_agent_id, AgentMemoryScope, AgentRoleKind,
    };

    #[test]
    fn normalize_agent_id_trims_and_lowercases() {
        assert_eq!(normalize_agent_id(" Agent-A "), "agent-a");
    }

    #[test]
    fn default_memory_scope_matches_role_kind() {
        assert_eq!(
            default_memory_scope_for_role(&AgentRoleKind::Coordinator),
            AgentMemoryScope::Shared
        );
        assert_eq!(
            default_memory_scope_for_role(&AgentRoleKind::Planner),
            AgentMemoryScope::Session
        );
        assert_eq!(
            default_memory_scope_for_role(&AgentRoleKind::Executor),
            AgentMemoryScope::Employee
        );
    }
}
