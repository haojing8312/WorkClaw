use crate::agent::runtime::runtime_io::{
    WorkspaceSkillRouteExecutionMode, WorkspaceSkillRouteProjection, WorkspaceSkillRuntimeEntry,
};
use runtime_skill_core::{
    OpenClawSkillMetadata, SkillCommandArgMode, SkillCommandDispatchKind,
    SkillCommandDispatchSpec, SkillConfig, SkillInvocationPolicy,
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillRouteIndex {
    entries: HashMap<String, WorkspaceSkillRouteProjection>,
}

impl SkillRouteIndex {
    pub fn build(entries: &[WorkspaceSkillRuntimeEntry]) -> Self {
        let entries = entries
            .iter()
            .map(|entry| {
                let projection = WorkspaceSkillRouteProjection {
                    skill_id: entry.skill_id.clone(),
                    display_name: entry.name.trim().to_string(),
                    aliases: collect_aliases(entry),
                    description: entry.description.trim().to_string(),
                    when_to_use: extract_when_to_use(&entry.config.system_prompt, &entry.description),
                    execution_mode: resolve_execution_mode(entry),
                    command_dispatch: entry.command_dispatch.clone(),
                };

                (entry.skill_id.clone(), projection)
            })
            .collect();

        Self { entries }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, skill_id: &str) -> Option<&WorkspaceSkillRouteProjection> {
        self.entries.get(skill_id)
    }

    pub fn entries(&self) -> impl Iterator<Item = &WorkspaceSkillRouteProjection> {
        self.entries.values()
    }
}

fn collect_aliases(entry: &WorkspaceSkillRuntimeEntry) -> Vec<String> {
    let mut aliases = Vec::new();
    let mut seen = HashSet::new();

    let push_alias = |aliases: &mut Vec<String>, seen: &mut HashSet<String>, alias: String| {
        let trimmed = alias.trim();
        if trimmed.is_empty() {
            return;
        }
        let key = trimmed.to_ascii_lowercase();
        if seen.insert(key) {
            aliases.push(trimmed.to_string());
        }
    };

    push_alias(&mut aliases, &mut seen, entry.skill_id.clone());
    push_alias(&mut aliases, &mut seen, entry.name.clone());

    if let Some(metadata) = &entry.metadata {
        if let Some(skill_key) = metadata.skill_key.as_ref() {
            push_alias(&mut aliases, &mut seen, skill_key.clone());
        }
    }

    aliases
}

fn extract_when_to_use(system_prompt: &str, description: &str) -> String {
    let mut lines = system_prompt.lines().peekable();
    while let Some(line) = lines.next() {
        let heading = line.trim().to_ascii_lowercase();
        if heading != "## when to use" && heading != "# when to use" && heading != "### when to use"
        {
            continue;
        }

        let mut collected = Vec::new();
        while let Some(next_line) = lines.peek() {
            let trimmed = next_line.trim();
            if trimmed.starts_with('#') {
                break;
            }
            let _ = lines.next();
            if trimmed.is_empty() {
                if !collected.is_empty() {
                    break;
                }
                continue;
            }

            let cleaned = trimmed
                .trim_start_matches('-')
                .trim_start_matches('*')
                .trim()
                .to_string();
            if !cleaned.is_empty() {
                collected.push(cleaned);
            }
        }

        let joined = collected.join(" ").trim().to_string();
        if !joined.is_empty() {
            return joined;
        }
    }

    description.trim().to_string()
}

fn resolve_execution_mode(entry: &WorkspaceSkillRuntimeEntry) -> WorkspaceSkillRouteExecutionMode {
    if entry.command_dispatch.is_some() {
        WorkspaceSkillRouteExecutionMode::DirectDispatch
    } else if matches!(
        entry.config.context.as_deref().map(str::trim),
        Some("fork")
    ) {
        WorkspaceSkillRouteExecutionMode::Fork
    } else {
        WorkspaceSkillRouteExecutionMode::Inline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_entry(
        skill_id: &str,
        name: &str,
        description: &str,
        system_prompt: &str,
        context: Option<&str>,
        metadata_skill_key: Option<&str>,
        command_dispatch: Option<SkillCommandDispatchSpec>,
    ) -> WorkspaceSkillRuntimeEntry {
        let command_dispatch_for_config = command_dispatch.clone();
        WorkspaceSkillRuntimeEntry {
            skill_id: skill_id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            source_type: "local".to_string(),
            projected_dir_name: skill_id.to_string(),
            config: SkillConfig {
                name: Some(name.to_string()),
                description: Some(description.to_string()),
                allowed_tools: None,
                model: None,
                max_iterations: None,
                argument_hint: None,
                disable_model_invocation: command_dispatch_for_config.is_some(),
                user_invocable: true,
                invocation: SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: command_dispatch_for_config.is_some(),
                },
                metadata: metadata_skill_key.map(|skill_key| OpenClawSkillMetadata {
                    skill_key: Some(skill_key.to_string()),
                    ..Default::default()
                }),
                command_dispatch: command_dispatch_for_config,
                context: context.map(|value| value.to_string()),
                agent: None,
                mcp_servers: vec![],
                system_prompt: system_prompt.to_string(),
            },
            invocation: SkillInvocationPolicy {
                user_invocable: true,
                disable_model_invocation: command_dispatch.is_some(),
            },
            metadata: metadata_skill_key.map(|skill_key| OpenClawSkillMetadata {
                skill_key: Some(skill_key.to_string()),
                ..Default::default()
            }),
            command_dispatch,
            content: crate::agent::runtime::runtime_io::WorkspaceSkillContent::FileTree(
                std::collections::HashMap::new(),
            ),
        }
    }

    #[test]
    fn skill_route_index_projects_route_metadata_from_workspace_entries() {
        let entries = vec![
            build_entry(
                "feishu-pm-task-dispatch",
                "PM Task Dispatch",
                "Create or dispatch PM follow-up tasks",
                "## When to Use\n- Use when a leader wants to create a correction task.\n\n## Workflow\n- Resolve assignee.\n- Dispatch task.",
                None,
                Some("task-dispatch"),
                Some(SkillCommandDispatchSpec {
                    kind: SkillCommandDispatchKind::Tool,
                    tool_name: "exec".to_string(),
                    arg_mode: SkillCommandArgMode::Raw,
                }),
            ),
            build_entry(
                "feishu-pm-fork-skill",
                "PM Fork Skill",
                "Run PM flow in a forked context",
                "## When to Use\n- Use when the task needs isolated execution.\n",
                Some("fork"),
                None,
                None,
            ),
            build_entry(
                "feishu-pm-weekly-work-summary",
                "项管周工作汇总",
                "Summarize PM work",
                "## When to Use\n- Use when you need to summarize PM updates for a week.\n",
                None,
                None,
                None,
            ),
            build_entry(
                "feishu-pm-bare-skill",
                "Bare Skill",
                "",
                "No heading here, just body text.",
                None,
                None,
                None,
            ),
        ];

        let index = SkillRouteIndex::build(&entries);

        assert_eq!(index.len(), 4);
        let corpus_skill_ids = index
            .entries()
            .map(|entry| entry.skill_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(corpus_skill_ids.len(), 4);
        assert!(corpus_skill_ids.contains(&"feishu-pm-task-dispatch"));
        assert!(corpus_skill_ids.contains(&"feishu-pm-fork-skill"));
        assert!(corpus_skill_ids.contains(&"feishu-pm-weekly-work-summary"));
        assert!(corpus_skill_ids.contains(&"feishu-pm-bare-skill"));

        let dispatch = index.get("feishu-pm-task-dispatch").expect("dispatch entry");
        assert_eq!(dispatch.skill_id, "feishu-pm-task-dispatch");
        assert_eq!(dispatch.display_name, "PM Task Dispatch");
        assert_eq!(
            dispatch.aliases,
            vec![
                "feishu-pm-task-dispatch".to_string(),
                "PM Task Dispatch".to_string(),
                "task-dispatch".to_string(),
            ]
        );
        assert_eq!(
            dispatch.description,
            "Create or dispatch PM follow-up tasks"
        );
        assert_eq!(
            dispatch.when_to_use,
            "Use when a leader wants to create a correction task."
        );
        assert_eq!(
            dispatch.execution_mode,
            WorkspaceSkillRouteExecutionMode::DirectDispatch
        );
        assert_eq!(
            dispatch
                .command_dispatch
                .as_ref()
                .map(|dispatch| dispatch.tool_name.as_str()),
            Some("exec")
        );

        let inline = index
            .get("feishu-pm-weekly-work-summary")
            .expect("inline entry");
        assert_eq!(inline.skill_id, "feishu-pm-weekly-work-summary");
        assert_eq!(inline.display_name, "项管周工作汇总");
        assert_eq!(
            inline.aliases,
            vec![
                "feishu-pm-weekly-work-summary".to_string(),
                "项管周工作汇总".to_string(),
            ]
        );
        assert_eq!(inline.description, "Summarize PM work");
        assert_eq!(
            inline.when_to_use,
            "Use when you need to summarize PM updates for a week."
        );
        assert_eq!(
            inline.execution_mode,
            WorkspaceSkillRouteExecutionMode::Inline
        );
        assert!(inline.command_dispatch.is_none());

        let fork = index.get("feishu-pm-fork-skill").expect("fork entry");
        assert_eq!(fork.skill_id, "feishu-pm-fork-skill");
        assert_eq!(fork.execution_mode, WorkspaceSkillRouteExecutionMode::Fork);
        assert_eq!(fork.when_to_use, "Use when the task needs isolated execution.");

        let bare = index.get("feishu-pm-bare-skill").expect("bare entry");
        assert_eq!(bare.when_to_use, "");
        assert_eq!(
            bare.execution_mode,
            WorkspaceSkillRouteExecutionMode::Inline
        );
    }
}
