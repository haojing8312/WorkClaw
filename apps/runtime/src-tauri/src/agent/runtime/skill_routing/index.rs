use crate::agent::runtime::runtime_io::{
    WorkspaceSkillRouteExecutionMode, WorkspaceSkillRouteProjection, WorkspaceSkillRuntimeEntry,
};
use runtime_skill_core::{
    OpenClawSkillMetadata, SkillCommandArgMode, SkillCommandDispatchKind, SkillCommandDispatchSpec,
    SkillConfig, SkillInvocationPolicy,
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillRouteIndex {
    entries: Vec<WorkspaceSkillRouteProjection>,
    entries_by_skill_id: HashMap<String, usize>,
}

impl SkillRouteIndex {
    pub fn build(entries: &[WorkspaceSkillRuntimeEntry]) -> Self {
        let entries = entries
            .iter()
            .filter(|entry| {
                entry.invocation.user_invocable
                    && (!entry.invocation.disable_model_invocation
                        || entry.command_dispatch.is_some())
            })
            .map(|entry| {
                let family_key = derive_family_key(&entry.skill_id);
                let projection = WorkspaceSkillRouteProjection {
                    skill_id: entry.skill_id.clone(),
                    display_name: entry.name.trim().to_string(),
                    aliases: collect_aliases(entry),
                    description: entry.description.trim().to_string(),
                    when_to_use: extract_when_to_use(
                        &entry.config.system_prompt,
                        &entry.description,
                    ),
                    family_key: family_key.clone(),
                    domain_tags: family_domain_tags(family_key.as_deref()),
                    allowed_tools: entry.config.allowed_tools.clone().unwrap_or_default(),
                    max_iterations: entry.config.max_iterations,
                    invocation: entry.invocation.clone(),
                    execution_mode: resolve_execution_mode(entry),
                    command_dispatch: entry.command_dispatch.clone(),
                };
                projection
            })
            .collect::<Vec<_>>();

        let entries_by_skill_id = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.skill_id.clone(), index))
            .collect();

        Self {
            entries,
            entries_by_skill_id,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, skill_id: &str) -> Option<&WorkspaceSkillRouteProjection> {
        self.entries_by_skill_id
            .get(skill_id)
            .and_then(|index| self.entries.get(*index))
    }

    pub fn entries(&self) -> impl Iterator<Item = &WorkspaceSkillRouteProjection> {
        self.entries.iter()
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

fn derive_family_key(skill_id: &str) -> Option<String> {
    let segments = skill_id
        .split('-')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    match segments.as_slice() {
        [first, second, ..] => Some(format!("{first}-{second}")),
        [single] => Some((*single).to_string()),
        [] => None,
    }
}

fn family_domain_tags(family_key: Option<&str>) -> Vec<String> {
    let tags = match family_key {
        Some("feishu-pm") => vec!["项管", "日报", "任务", "汇总", "同步"],
        Some("feishu-bitable") => vec!["多维表格", "表格", "字段", "视图", "关系"],
        _ => Vec::new(),
    };

    tags.into_iter().map(String::from).collect()
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
    } else if entry
        .config
        .context
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| value.eq_ignore_ascii_case("fork"))
    {
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
        allowed_tools: Option<Vec<&str>>,
        max_iterations: Option<usize>,
        invocation: SkillInvocationPolicy,
        metadata_skill_key: Option<&str>,
        command_dispatch: Option<SkillCommandDispatchSpec>,
    ) -> WorkspaceSkillRuntimeEntry {
        let command_dispatch_for_config = command_dispatch.clone();
        let allowed_tools_for_config = allowed_tools
            .clone()
            .map(|values| values.into_iter().map(|value| value.to_string()).collect());
        WorkspaceSkillRuntimeEntry {
            skill_id: skill_id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            source_type: "local".to_string(),
            projected_dir_name: skill_id.to_string(),
            config: SkillConfig {
                name: Some(name.to_string()),
                description: Some(description.to_string()),
                allowed_tools: allowed_tools_for_config,
                denied_tools: None,
                allowed_tool_sources: None,
                denied_tool_sources: None,
                allowed_tool_categories: None,
                denied_tool_categories: None,
                model: None,
                max_iterations,
                argument_hint: None,
                disable_model_invocation: invocation.disable_model_invocation,
                user_invocable: invocation.user_invocable,
                invocation: invocation.clone(),
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
            invocation,
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
                Some("FoRk"),
                Some(vec!["exec", "read_file"]),
                Some(11),
                SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: true,
                },
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
                Some("FoRk"),
                Some(vec!["read_file", "edit"]),
                Some(7),
                SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: false,
                },
                None,
                None,
            ),
            build_entry(
                "feishu-pm-weekly-work-summary",
                "项管周工作汇总",
                "Summarize PM work",
                "## When to Use\n- Use when you need to summarize PM updates for a week.\n",
                None,
                Some(vec!["read_file"]),
                Some(3),
                SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: false,
                },
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
                SkillInvocationPolicy {
                    user_invocable: false,
                    disable_model_invocation: true,
                },
                None,
                None,
            ),
        ];

        let index = SkillRouteIndex::build(&entries);

        assert_eq!(index.len(), 3);
        let corpus_skill_ids = index
            .entries()
            .map(|entry| entry.skill_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(corpus_skill_ids.len(), 3);
        assert!(corpus_skill_ids.contains(&"feishu-pm-task-dispatch"));
        assert!(corpus_skill_ids.contains(&"feishu-pm-fork-skill"));
        assert!(corpus_skill_ids.contains(&"feishu-pm-weekly-work-summary"));
        assert_eq!(
            corpus_skill_ids,
            vec![
                "feishu-pm-task-dispatch",
                "feishu-pm-fork-skill",
                "feishu-pm-weekly-work-summary",
            ]
        );
        assert!(!corpus_skill_ids.contains(&"feishu-pm-bare-skill"));

        let dispatch = index
            .get("feishu-pm-task-dispatch")
            .expect("dispatch entry");
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
            dispatch.allowed_tools,
            vec!["exec".to_string(), "read_file".to_string()]
        );
        assert_eq!(dispatch.max_iterations, Some(11));
        assert_eq!(
            dispatch.invocation,
            SkillInvocationPolicy {
                user_invocable: true,
                disable_model_invocation: true,
            }
        );
        assert_eq!(
            dispatch.when_to_use,
            "Use when a leader wants to create a correction task."
        );
        assert_eq!(dispatch.family_key.as_deref(), Some("feishu-pm"));
        assert_eq!(
            dispatch.domain_tags,
            vec![
                "项管".to_string(),
                "日报".to_string(),
                "任务".to_string(),
                "汇总".to_string(),
                "同步".to_string(),
            ]
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
        assert_eq!(inline.allowed_tools, vec!["read_file".to_string()]);
        assert_eq!(inline.max_iterations, Some(3));
        assert_eq!(
            inline.invocation,
            SkillInvocationPolicy {
                user_invocable: true,
                disable_model_invocation: false,
            }
        );
        assert_eq!(
            inline.when_to_use,
            "Use when you need to summarize PM updates for a week."
        );
        assert_eq!(inline.family_key.as_deref(), Some("feishu-pm"));
        assert_eq!(
            inline.domain_tags,
            vec![
                "项管".to_string(),
                "日报".to_string(),
                "任务".to_string(),
                "汇总".to_string(),
                "同步".to_string(),
            ]
        );
        assert_eq!(
            inline.execution_mode,
            WorkspaceSkillRouteExecutionMode::Inline
        );
        assert!(inline.command_dispatch.is_none());

        let fork = index.get("feishu-pm-fork-skill").expect("fork entry");
        assert_eq!(fork.skill_id, "feishu-pm-fork-skill");
        assert_eq!(fork.family_key.as_deref(), Some("feishu-pm"));
        assert_eq!(
            fork.domain_tags,
            vec![
                "项管".to_string(),
                "日报".to_string(),
                "任务".to_string(),
                "汇总".to_string(),
                "同步".to_string(),
            ]
        );
        assert_eq!(
            fork.allowed_tools,
            vec!["read_file".to_string(), "edit".to_string()]
        );
        assert_eq!(fork.max_iterations, Some(7));
        assert_eq!(
            fork.invocation,
            SkillInvocationPolicy {
                user_invocable: true,
                disable_model_invocation: false,
            }
        );
        assert_eq!(fork.execution_mode, WorkspaceSkillRouteExecutionMode::Fork);
        assert_eq!(
            fork.when_to_use,
            "Use when the task needs isolated execution."
        );

        assert!(
            index.get("feishu-pm-bare-skill").is_none(),
            "internal-only skills must be excluded from the route corpus"
        );
    }
}
