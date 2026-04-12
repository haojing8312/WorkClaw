use super::effective_tool_set::{
    resolve_effective_tool_set, session_tool_policy_input, skill_tool_policy_input,
    EffectiveToolPolicyInput, EffectiveToolSet,
};
use super::runtime_io as chat_io;
use super::tool_catalog::{
    build_tool_candidate_records, format_tool_candidate_record_hints, format_tool_discovery_index,
    ToolDiscoveryCandidateRecord,
};
use crate::agent::permissions::PermissionMode;
use crate::agent::runtime::kernel::capability_snapshot::CapabilitySnapshot;
use crate::agent::runtime::kernel::context_bundle::ContextBundle;
use crate::agent::runtime::kernel::tool_registry_setup::{
    setup_runtime_tool_registry, ToolRegistrySetupParams,
};
use crate::agent::runtime::kernel::workspace_skill_context::build_workspace_skill_context;
use crate::agent::runtime::runtime_io::WorkspaceSkillRuntimeEntry;
use crate::agent::tool_manifest::{ToolCategory, ToolSource};
use crate::agent::AgentExecutor;
use reqwest::Url;
use runtime_chat_app::{ChatExecutionGuidance, ChatExecutionPreparationService};
use std::sync::Arc;
use tauri::AppHandle;

#[derive(Clone)]
pub(crate) struct PreparedRuntimeTools {
    pub allowed_tools: Option<Vec<String>>,
    pub full_allowed_tools: Vec<String>,
    pub effective_tool_plan: EffectiveToolSet,
    pub discovery_candidates: Vec<ToolDiscoveryCandidateRecord>,
    pub system_prompt: String,
    pub capability_snapshot: CapabilitySnapshot,
}

impl PreparedRuntimeTools {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn tool_plan_record(
        &self,
    ) -> crate::agent::runtime::effective_tool_set::EffectiveToolDecisionRecord {
        self.effective_tool_plan
            .decision_record_with_candidates(self.discovery_candidates.clone())
    }
}

#[derive(Clone)]
pub(crate) struct ToolSetupParams<'a> {
    pub app: &'a AppHandle,
    pub db: &'a sqlx::SqlitePool,
    pub agent_executor: &'a Arc<AgentExecutor>,
    pub workspace_skill_entries: &'a [WorkspaceSkillRuntimeEntry],
    pub session_id: &'a str,
    pub api_format: &'a str,
    pub base_url: &'a str,
    pub model_name: &'a str,
    pub api_key: &'a str,
    pub skill_id: &'a str,
    pub source_type: &'a str,
    pub pack_path: &'a str,
    pub permission_mode: PermissionMode,
    pub runtime_default_tool_policy: EffectiveToolPolicyInput,
    pub skill_system_prompt: &'a str,
    pub skill_allowed_tools: Option<Vec<String>>,
    pub skill_denied_tools: Option<Vec<String>>,
    pub skill_allowed_tool_sources: Option<Vec<ToolSource>>,
    pub skill_denied_tool_sources: Option<Vec<ToolSource>>,
    pub skill_allowed_tool_categories: Option<Vec<ToolCategory>>,
    pub skill_denied_tool_categories: Option<Vec<ToolCategory>>,
    pub skill_allowed_mcp_servers: Option<Vec<String>>,
    pub tool_discovery_query: Option<&'a str>,
    pub max_iter: usize,
    pub max_call_depth: usize,
    pub suppress_workspace_skills_prompt: bool,
    pub execution_preparation_service: &'a ChatExecutionPreparationService,
    pub execution_guidance: &'a ChatExecutionGuidance,
    pub memory_bucket_employee_id: &'a str,
    pub employee_collaboration_guidance: Option<&'a str>,
    pub supplemental_runtime_notes: &'a [String],
}

pub(crate) async fn prepare_runtime_tools(
    params: ToolSetupParams<'_>,
) -> Result<PreparedRuntimeTools, String> {
    let registry_setup = setup_runtime_tool_registry(ToolRegistrySetupParams {
        app: params.app,
        db: params.db,
        agent_executor: params.agent_executor,
        session_id: params.session_id,
        api_format: params.api_format,
        base_url: params.base_url,
        model_name: params.model_name,
        api_key: params.api_key,
        skill_id: params.skill_id,
        source_type: params.source_type,
        pack_path: params.pack_path,
        max_call_depth: params.max_call_depth,
        execution_preparation_service: params.execution_preparation_service,
        execution_guidance: params.execution_guidance,
        memory_bucket_employee_id: params.memory_bucket_employee_id,
    })
    .await?;

    let executor_work_dir = params
        .execution_preparation_service
        .resolve_executor_work_dir(params.execution_guidance);
    let workspace_skill_context = build_workspace_skill_context(
        executor_work_dir.as_deref().map(std::path::Path::new),
        params.workspace_skill_entries,
        params.suppress_workspace_skills_prompt,
    )?;

    let mut effective_tool_set = resolve_effective_tool_set(
        params.agent_executor.registry(),
        params.skill_allowed_tools.clone(),
        None,
        &[
            params.runtime_default_tool_policy.clone(),
            session_tool_policy_input(params.permission_mode),
            skill_tool_policy_input(
                params.skill_denied_tools.clone().unwrap_or_default(),
                params
                    .skill_denied_tool_categories
                    .clone()
                    .unwrap_or_default(),
                params.skill_allowed_tool_categories.clone(),
                params.skill_allowed_tool_sources.clone(),
                params.skill_denied_tool_sources.clone().unwrap_or_default(),
                params.skill_allowed_mcp_servers.clone(),
            ),
        ],
    );
    if !effective_tool_set.missing_tools.is_empty() {
        eprintln!(
            "[tooling] 忽略不存在的工具白名单项: {}",
            effective_tool_set.missing_tools.join(", ")
        );
    }
    if !effective_tool_set.filtered_out_tools.is_empty() {
        eprintln!(
            "[tooling] 因 MCP server 过滤规则跳过工具: {}",
            effective_tool_set.filtered_out_tools.join(", ")
        );
    }
    if !effective_tool_set.source_counts.is_empty() {
        let source_summary = effective_tool_set
            .source_counts
            .iter()
            .map(|entry| format!("{:?}={}", entry.source, entry.count))
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("[tooling] 生效工具来源统计: {source_summary}");
    }

    let discovery_candidates = if let Some(discovery_query) = params.tool_discovery_query {
        build_tool_candidate_records(&effective_tool_set.tool_manifest, discovery_query, 4)
    } else {
        Vec::new()
    };
    effective_tool_set.apply_recommended_tools(&discovery_candidates);

    let capability_snapshot = CapabilitySnapshot::build_with_tool_plan(
        effective_tool_set.clone(),
        discovery_candidates.clone(),
        workspace_skill_context.skill_command_specs.clone(),
        merge_runtime_notes(
            registry_setup.runtime_notes,
            params.supplemental_runtime_notes,
        ),
    );
    let memory_content = chat_io::load_memory_content(&registry_setup.memory_dir);
    let context_bundle = ContextBundle::build(
        params.skill_system_prompt,
        &capability_snapshot,
        params.model_name,
        params.max_iter,
        params.execution_guidance,
        workspace_skill_context.workspace_skills_prompt,
        params.employee_collaboration_guidance.map(str::to_string),
        Some(memory_content),
    );

    let should_embed_discovery_scaffold =
        should_embed_tool_discovery_scaffold(params.api_format, params.base_url, params.model_name);

    let system_prompt = if should_embed_discovery_scaffold {
        if let Some(discovery_index) =
            format_tool_discovery_index(&effective_tool_set.tool_manifest)
        {
            format!("{}\n\n{discovery_index}", context_bundle.system_prompt)
        } else {
            context_bundle.system_prompt
        }
    } else {
        context_bundle.system_prompt
    };
    let system_prompt = if should_embed_discovery_scaffold && params.tool_discovery_query.is_some()
    {
        if let Some(candidate_hints) = format_tool_candidate_record_hints(
            &discovery_candidates,
            &effective_tool_set.active_tools,
            4,
        ) {
            format!("{system_prompt}\n\n{candidate_hints}")
        } else {
            system_prompt
        }
    } else {
        system_prompt
    };

    Ok(PreparedRuntimeTools {
        allowed_tools: capability_snapshot.allowed_tools.clone(),
        full_allowed_tools: capability_snapshot.full_allowed_tools.clone(),
        effective_tool_plan: capability_snapshot
            .effective_tool_plan
            .clone()
            .expect("capability snapshot should preserve effective tool plan"),
        discovery_candidates: capability_snapshot.discovery_candidates.clone(),
        system_prompt,
        capability_snapshot,
    })
}

fn merge_runtime_notes(
    mut runtime_notes: Vec<String>,
    supplemental_runtime_notes: &[String],
) -> Vec<String> {
    for note in supplemental_runtime_notes {
        let trimmed = note.trim();
        if trimmed.is_empty() {
            continue;
        }
        if runtime_notes
            .iter()
            .any(|existing| existing.trim() == trimmed)
        {
            continue;
        }
        runtime_notes.push(trimmed.to_string());
    }
    runtime_notes
}

fn should_embed_tool_discovery_scaffold(
    api_format: &str,
    base_url: &str,
    model_name: &str,
) -> bool {
    if !api_format.trim().eq_ignore_ascii_case("openai") {
        return true;
    }

    let normalized_model = model_name.trim().to_ascii_lowercase();
    if !normalized_model.contains("qwen") {
        return true;
    }

    let Ok(url) = Url::parse(base_url.trim()) else {
        return true;
    };
    let host = url.host_str().unwrap_or_default();
    !(host.eq_ignore_ascii_case("dashscope.aliyuncs.com")
        || host.eq_ignore_ascii_case("dashscope-intl.aliyuncs.com")
        || host.eq_ignore_ascii_case("openrouter.ai"))
}

#[cfg(test)]
mod tests {
    use super::{merge_runtime_notes, should_embed_tool_discovery_scaffold, PreparedRuntimeTools};
    use crate::agent::runtime::effective_tool_set::{
        EffectiveToolPolicySummary, EffectiveToolSet, EffectiveToolSetSource,
    };
    use crate::agent::runtime::kernel::capability_snapshot::CapabilitySnapshot;
    use crate::agent::runtime::tool_catalog::{
        format_tool_candidate_hints, format_tool_discovery_index, ToolDiscoveryCandidateRecord,
        ToolRecommendationStage,
    };
    use crate::agent::tool_manifest::{ToolCategory, ToolMetadata, ToolSource};
    use crate::agent::ToolManifestEntry;

    fn build_effective_tool_plan(allowed_tools: &[&str]) -> EffectiveToolSet {
        EffectiveToolSet {
            source: EffectiveToolSetSource::ExplicitAllowList,
            allowed_tools: Some(
                allowed_tools
                    .iter()
                    .map(|name| (*name).to_string())
                    .collect(),
            ),
            tool_names: allowed_tools
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
            tool_manifest: Vec::new(),
            active_tools: allowed_tools
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
            active_tool_manifest: Vec::new(),
            recommended_tools: Vec::new(),
            supporting_tools: Vec::new(),
            deferred_tools: Vec::new(),
            loading_policy: crate::agent::runtime::effective_tool_set::ToolLoadingPolicy::Full,
            expanded_to_full: false,
            expansion_reason: None,
            missing_tools: Vec::new(),
            filtered_out_tools: Vec::new(),
            excluded_tools: Vec::new(),
            source_counts: Vec::new(),
            policy: EffectiveToolPolicySummary {
                denied_tool_names: Vec::new(),
                denied_categories: Vec::new(),
                allowed_categories: None,
                allowed_sources: None,
                denied_sources: Vec::new(),
                allowed_mcp_servers: None,
                inputs: Vec::new(),
            },
        }
    }

    #[test]
    fn prepared_runtime_tools_keeps_allowed_tool_list() {
        let capability_snapshot = CapabilitySnapshot::build_with_tool_plan(
            build_effective_tool_plan(&["read_file", "bash"]),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let prepared = PreparedRuntimeTools {
            allowed_tools: capability_snapshot.allowed_tools.clone(),
            full_allowed_tools: capability_snapshot.full_allowed_tools.clone(),
            tool_manifest: capability_snapshot.tool_manifest.clone(),
            effective_tool_plan: capability_snapshot
                .effective_tool_plan
                .clone()
                .expect("tool plan"),
            discovery_candidates: capability_snapshot.discovery_candidates.clone(),
            system_prompt: "system prompt".to_string(),
            capability_snapshot,
        };

        assert_eq!(
            prepared.allowed_tools,
            Some(vec!["read_file".to_string(), "bash".to_string()])
        );
        assert_eq!(prepared.full_allowed_tools.len(), 2);
        assert!(prepared.tool_manifest.is_empty());
        assert_eq!(prepared.effective_tool_plan.tool_names.len(), 2);
        assert_eq!(prepared.system_prompt, "system prompt");
        assert!(prepared.capability_snapshot.skill_command_specs.is_empty());
    }

    #[test]
    fn tool_plan_record_includes_discovery_candidates() {
        let discovery_candidates = vec![ToolDiscoveryCandidateRecord {
            name: "web_search".to_string(),
            category: ToolCategory::Search,
            source: ToolSource::Runtime,
            score: 10,
            stage: ToolRecommendationStage::Primary,
            matched_terms: vec!["search".to_string()],
            matched_fields: vec!["name".to_string()],
        }];
        let capability_snapshot = CapabilitySnapshot::build_with_tool_plan(
            build_effective_tool_plan(&["web_search"]),
            discovery_candidates.clone(),
            Vec::new(),
            Vec::new(),
        );
        let prepared = PreparedRuntimeTools {
            allowed_tools: capability_snapshot.allowed_tools.clone(),
            full_allowed_tools: capability_snapshot.full_allowed_tools.clone(),
            tool_manifest: capability_snapshot.tool_manifest.clone(),
            effective_tool_plan: capability_snapshot
                .effective_tool_plan
                .clone()
                .expect("tool plan"),
            discovery_candidates: capability_snapshot.discovery_candidates.clone(),
            system_prompt: "system prompt".to_string(),
            capability_snapshot,
        };

        let record = prepared.tool_plan_record();
        assert_eq!(record.discovery_candidates.len(), 1);
        assert_eq!(record.discovery_candidates[0].name, "web_search");
    }

    #[test]
    fn tool_discovery_index_is_emitted_for_large_tool_sets() {
        let entries = vec![
            ToolManifestEntry::from_parts(
                "read_file",
                "Read file",
                ToolMetadata {
                    category: ToolCategory::File,
                    source: ToolSource::Native,
                    ..ToolMetadata::default()
                },
            ),
            ToolManifestEntry::from_parts(
                "write_file",
                "Write file",
                ToolMetadata {
                    category: ToolCategory::File,
                    source: ToolSource::Native,
                    ..ToolMetadata::default()
                },
            ),
            ToolManifestEntry::from_parts(
                "edit",
                "Edit file",
                ToolMetadata {
                    category: ToolCategory::File,
                    source: ToolSource::Native,
                    ..ToolMetadata::default()
                },
            ),
            ToolManifestEntry::from_parts(
                "list_dir",
                "List dir",
                ToolMetadata {
                    category: ToolCategory::File,
                    source: ToolSource::Native,
                    ..ToolMetadata::default()
                },
            ),
            ToolManifestEntry::from_parts(
                "browser_launch",
                "Launch browser",
                ToolMetadata {
                    category: ToolCategory::Browser,
                    source: ToolSource::Sidecar,
                    ..ToolMetadata::default()
                },
            ),
            ToolManifestEntry::from_parts(
                "browser_snapshot",
                "Snapshot browser",
                ToolMetadata {
                    category: ToolCategory::Browser,
                    source: ToolSource::Sidecar,
                    ..ToolMetadata::default()
                },
            ),
            ToolManifestEntry::from_parts(
                "web_search",
                "Search web",
                ToolMetadata {
                    category: ToolCategory::Search,
                    source: ToolSource::Runtime,
                    ..ToolMetadata::default()
                },
            ),
            ToolManifestEntry::from_parts(
                "web_fetch",
                "Fetch web",
                ToolMetadata {
                    category: ToolCategory::Web,
                    source: ToolSource::Native,
                    ..ToolMetadata::default()
                },
            ),
        ];

        let index = format_tool_discovery_index(&entries).expect("discovery index");
        assert!(index.contains("[工具发现索引]"));
        assert!(index.contains("文件类"));
    }

    #[test]
    fn tool_candidate_hints_are_emitted_for_matching_queries() {
        let entries = vec![
            ToolManifestEntry::from_parts(
                "read_file",
                "Read file",
                ToolMetadata {
                    category: ToolCategory::File,
                    source: ToolSource::Native,
                    ..ToolMetadata::default()
                },
            ),
            ToolManifestEntry::from_parts(
                "write_file",
                "Write file",
                ToolMetadata {
                    category: ToolCategory::File,
                    source: ToolSource::Native,
                    ..ToolMetadata::default()
                },
            ),
            ToolManifestEntry::from_parts(
                "edit",
                "Edit file",
                ToolMetadata {
                    category: ToolCategory::File,
                    source: ToolSource::Native,
                    ..ToolMetadata::default()
                },
            ),
            ToolManifestEntry::from_parts(
                "list_dir",
                "List dir",
                ToolMetadata {
                    category: ToolCategory::File,
                    source: ToolSource::Native,
                    ..ToolMetadata::default()
                },
            ),
            ToolManifestEntry::from_parts(
                "browser_launch",
                "Launch browser",
                ToolMetadata {
                    category: ToolCategory::Browser,
                    source: ToolSource::Sidecar,
                    ..ToolMetadata::default()
                },
            ),
            ToolManifestEntry::from_parts(
                "browser_snapshot",
                "Snapshot browser",
                ToolMetadata {
                    category: ToolCategory::Browser,
                    source: ToolSource::Sidecar,
                    ..ToolMetadata::default()
                },
            ),
            ToolManifestEntry::from_parts(
                "web_search",
                "Search web",
                ToolMetadata {
                    category: ToolCategory::Search,
                    source: ToolSource::Runtime,
                    ..ToolMetadata::default()
                },
            ),
            ToolManifestEntry::from_parts(
                "web_fetch",
                "Fetch web",
                ToolMetadata {
                    category: ToolCategory::Web,
                    source: ToolSource::Native,
                    ..ToolMetadata::default()
                },
            ),
        ];

        let hints = format_tool_candidate_hints(&entries, "search latest web news", 4)
            .expect("candidate hints");
        assert!(hints.contains("[当前任务候选工具]"));
        assert!(hints.contains("web_search"));
    }

    #[test]
    fn merge_runtime_notes_appends_continuation_notes_after_registry_notes() {
        let merged = merge_runtime_notes(
            vec!["当前未配置搜索引擎".to_string()],
            &[
                "当前会话最近一次上下文压缩已生效：4096 -> 1024 tokens".to_string(),
                "若用户要求继续，应基于当前压缩后上下文直接继续执行".to_string(),
            ],
        );

        assert_eq!(
            merged,
            vec![
                "当前未配置搜索引擎".to_string(),
                "当前会话最近一次上下文压缩已生效：4096 -> 1024 tokens".to_string(),
                "若用户要求继续，应基于当前压缩后上下文直接继续执行".to_string(),
            ]
        );
    }

    #[test]
    fn qwen_dashscope_skips_tool_discovery_scaffold() {
        assert!(!should_embed_tool_discovery_scaffold(
            "openai",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "qwen3.6-plus",
        ));
    }

    #[test]
    fn qwen_openrouter_skips_tool_discovery_scaffold() {
        assert!(!should_embed_tool_discovery_scaffold(
            "openai",
            "https://openrouter.ai/api/v1",
            "qwen/qwen3-coder:free",
        ));
    }

    #[test]
    fn non_qwen_models_keep_tool_discovery_scaffold() {
        assert!(should_embed_tool_discovery_scaffold(
            "openai",
            "https://api.openai.com/v1",
            "gpt-5.4",
        ));
    }
}
