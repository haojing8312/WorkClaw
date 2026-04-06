use crate::agent::runtime::runtime_io::WorkspaceSkillCommandSpec;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CapabilitySnapshot {
    pub allowed_tools: Option<Vec<String>>,
    pub resolved_tool_names: Vec<String>,
    pub skill_command_specs: Vec<WorkspaceSkillCommandSpec>,
    pub runtime_notes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::CapabilitySnapshot;

    #[test]
    fn capability_snapshot_keeps_prompt_visible_tools_and_dispatch_specs_together() {
        let snapshot = CapabilitySnapshot::default();

        assert!(snapshot.allowed_tools.is_none());
        assert!(snapshot.resolved_tool_names.is_empty());
        assert!(snapshot.skill_command_specs.is_empty());
        assert!(snapshot.runtime_notes.is_empty());
    }
}
