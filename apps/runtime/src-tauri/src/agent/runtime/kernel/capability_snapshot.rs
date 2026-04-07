use crate::agent::runtime::runtime_io::WorkspaceSkillCommandSpec;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CapabilitySnapshot {
    pub allowed_tools: Option<Vec<String>>,
    pub resolved_tool_names: Vec<String>,
    pub skill_command_specs: Vec<WorkspaceSkillCommandSpec>,
    pub runtime_notes: Vec<String>,
}

impl CapabilitySnapshot {
    pub(crate) fn build(
        allowed_tools: Option<Vec<String>>,
        resolved_tool_names: Vec<String>,
        skill_command_specs: Vec<WorkspaceSkillCommandSpec>,
        runtime_notes: Vec<String>,
    ) -> Self {
        Self {
            allowed_tools,
            resolved_tool_names,
            skill_command_specs,
            runtime_notes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CapabilitySnapshot;
    use crate::agent::runtime::runtime_io::WorkspaceSkillCommandSpec;

    #[test]
    fn capability_snapshot_keeps_prompt_visible_tools_and_dispatch_specs_together() {
        let snapshot = CapabilitySnapshot::default();

        assert!(snapshot.allowed_tools.is_none());
        assert!(snapshot.resolved_tool_names.is_empty());
        assert!(snapshot.skill_command_specs.is_empty());
        assert!(snapshot.runtime_notes.is_empty());
    }

    #[test]
    fn capability_snapshot_build_keeps_runtime_notes_and_specs_together() {
        let snapshot = CapabilitySnapshot::build(
            Some(vec!["read".to_string(), "exec".to_string()]),
            vec!["read".to_string(), "exec".to_string()],
            vec![WorkspaceSkillCommandSpec {
                name: "pm_summary".to_string(),
                skill_id: "skill-1".to_string(),
                skill_name: "PM Summary".to_string(),
                description: "Summarize PM updates".to_string(),
                dispatch: None,
            }],
            vec!["当前未配置搜索引擎".to_string()],
        );

        assert_eq!(
            snapshot.allowed_tools.as_deref(),
            Some(&["read".to_string(), "exec".to_string()][..])
        );
        assert_eq!(
            snapshot.resolved_tool_names,
            vec!["read".to_string(), "exec".to_string()]
        );
        assert_eq!(snapshot.skill_command_specs.len(), 1);
        assert_eq!(snapshot.skill_command_specs[0].name, "pm_summary");
        assert_eq!(
            snapshot.runtime_notes,
            vec!["当前未配置搜索引擎".to_string()]
        );
    }
}
