use crate::agent::runtime::effective_tool_set::{
    EffectiveToolDecisionRecord, EffectiveToolSet,
};
use crate::agent::runtime::runtime_io::WorkspaceSkillCommandSpec;
use crate::agent::runtime::tool_catalog::ToolDiscoveryCandidateRecord;
use crate::agent::ToolManifestEntry;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CapabilitySnapshot {
    pub allowed_tools: Option<Vec<String>>,
    pub resolved_tool_names: Vec<String>,
    pub full_allowed_tools: Vec<String>,
    pub tool_manifest: Vec<ToolManifestEntry>,
    pub effective_tool_plan: Option<EffectiveToolSet>,
    pub discovery_candidates: Vec<ToolDiscoveryCandidateRecord>,
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
            resolved_tool_names: resolved_tool_names.clone(),
            full_allowed_tools: resolved_tool_names,
            tool_manifest: Vec::new(),
            effective_tool_plan: None,
            discovery_candidates: Vec::new(),
            skill_command_specs,
            runtime_notes,
        }
    }

    pub(crate) fn build_with_tool_plan(
        effective_tool_plan: EffectiveToolSet,
        discovery_candidates: Vec<ToolDiscoveryCandidateRecord>,
        skill_command_specs: Vec<WorkspaceSkillCommandSpec>,
        runtime_notes: Vec<String>,
    ) -> Self {
        Self {
            allowed_tools: effective_tool_plan.allowed_tools.clone(),
            resolved_tool_names: effective_tool_plan.active_tools.clone(),
            full_allowed_tools: effective_tool_plan.full_allowed_tools(),
            tool_manifest: effective_tool_plan.active_tool_manifest.clone(),
            effective_tool_plan: Some(effective_tool_plan),
            discovery_candidates,
            skill_command_specs,
            runtime_notes,
        }
    }

    pub(crate) fn tool_plan_record(&self) -> Option<EffectiveToolDecisionRecord> {
        self.effective_tool_plan
            .as_ref()
            .map(|plan| plan.decision_record_with_candidates(self.discovery_candidates.clone()))
    }

    pub(crate) fn has_deferred_tools(&self) -> bool {
        self.effective_tool_plan
            .as_ref()
            .is_some_and(EffectiveToolSet::has_deferred_tools)
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
        assert!(snapshot.full_allowed_tools.is_empty());
        assert!(snapshot.tool_manifest.is_empty());
        assert!(snapshot.effective_tool_plan.is_none());
        assert!(snapshot.discovery_candidates.is_empty());
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
        assert_eq!(
            snapshot.full_allowed_tools,
            vec!["read".to_string(), "exec".to_string()]
        );
        assert_eq!(snapshot.skill_command_specs.len(), 1);
        assert_eq!(snapshot.skill_command_specs[0].name, "pm_summary");
        assert_eq!(
            snapshot.runtime_notes,
            vec!["当前未配置搜索引擎".to_string()]
        );
        assert!(snapshot.tool_plan_record().is_none());
    }
}
