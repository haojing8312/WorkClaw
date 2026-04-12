#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentProfileContext {
    pub workspace_dir: String,
    pub persona_text: String,
}

pub(crate) fn resolve_agent_workspace_dir(default_work_dir: &str) -> String {
    default_work_dir.trim().to_string()
}

pub(crate) fn resolve_agent_persona_text(persona: &str) -> String {
    persona.trim().to_string()
}

pub(crate) fn build_agent_profile_context(
    default_work_dir: &str,
    persona: &str,
) -> AgentProfileContext {
    AgentProfileContext {
        workspace_dir: resolve_agent_workspace_dir(default_work_dir),
        persona_text: resolve_agent_persona_text(persona),
    }
}
