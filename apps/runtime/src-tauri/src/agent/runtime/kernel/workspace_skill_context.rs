use crate::agent::runtime::runtime_io::{
    build_workspace_skill_command_specs, prepare_workspace_skills_prompt,
    sync_workspace_skills_to_directory, WorkspaceSkillCommandSpec, WorkspaceSkillRuntimeEntry,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct WorkspaceSkillContext {
    pub workspace_skills_prompt: Option<String>,
    pub skill_command_specs: Vec<WorkspaceSkillCommandSpec>,
}

pub(crate) fn build_workspace_skill_context(
    work_dir: Option<&std::path::Path>,
    entries: &[WorkspaceSkillRuntimeEntry],
    suppress_workspace_skills_prompt: bool,
) -> Result<WorkspaceSkillContext, String> {
    let Some(work_dir) = work_dir else {
        return Ok(WorkspaceSkillContext::default());
    };

    let skill_command_specs = build_workspace_skill_command_specs(entries);
    let workspace_skills_prompt = if suppress_workspace_skills_prompt {
        sync_workspace_skills_to_directory(work_dir, entries)?;
        None
    } else {
        Some(prepare_workspace_skills_prompt(work_dir, entries)?)
    };

    Ok(WorkspaceSkillContext {
        workspace_skills_prompt,
        skill_command_specs,
    })
}

#[cfg(test)]
mod tests {
    use super::build_workspace_skill_context;
    use crate::agent::runtime::runtime_io::{WorkspaceSkillContent, WorkspaceSkillRuntimeEntry};
    use runtime_skill_core::{SkillConfig, SkillInvocationPolicy};
    use tempfile::tempdir;

    fn build_entry() -> WorkspaceSkillRuntimeEntry {
        WorkspaceSkillRuntimeEntry {
            skill_id: "pm-summary".to_string(),
            name: "PM Summary".to_string(),
            description: "Summarize PM updates".to_string(),
            source_type: "builtin".to_string(),
            projected_dir_name: "pm-summary".to_string(),
            config: SkillConfig::default(),
            invocation: SkillInvocationPolicy {
                user_invocable: true,
                disable_model_invocation: false,
            },
            metadata: None,
            command_dispatch: None,
            content: WorkspaceSkillContent::FileTree(
                [("SKILL.md".to_string(), b"# PM Summary".to_vec())]
                    .into_iter()
                    .collect(),
            ),
        }
    }

    #[test]
    fn workspace_skill_context_keeps_prompt_and_command_specs_together() {
        let tmp = tempdir().expect("tempdir");
        let work_dir = tmp.path().join("workspace");

        let context =
            build_workspace_skill_context(Some(work_dir.as_path()), &[build_entry()], false)
                .expect("workspace skill context");

        assert!(context
            .workspace_skills_prompt
            .as_deref()
            .expect("workspace skills prompt")
            .contains("<available_skills>"));
        assert_eq!(context.skill_command_specs.len(), 1);
        assert_eq!(context.skill_command_specs[0].name, "pm_summary");
    }

    #[test]
    fn workspace_skill_context_still_syncs_skills_when_prompt_is_suppressed() {
        let tmp = tempdir().expect("tempdir");
        let work_dir = tmp.path().join("workspace");

        let context =
            build_workspace_skill_context(Some(work_dir.as_path()), &[build_entry()], true)
                .expect("workspace skill context");

        assert!(context.workspace_skills_prompt.is_none());
        assert_eq!(context.skill_command_specs.len(), 1);
        assert!(work_dir
            .join("skills")
            .join("pm-summary")
            .join("SKILL.md")
            .exists());
    }
}
