use super::effective_tool_set::{
    resolve_effective_tool_set, session_tool_policy_input, skill_tool_policy_input,
    EffectiveToolPolicyInput, EffectiveToolSet,
};
use super::events::SearchCacheState;
use super::runtime_io as chat_io;
use super::tool_catalog::{
    build_tool_candidate_records, format_tool_candidate_hints, format_tool_discovery_index,
    ToolDiscoveryCandidateRecord,
};
use super::tool_registry_builder::{RuntimeToolRegistryBuilder, DEFAULT_BROWSER_SIDECAR_URL};
use crate::agent::permissions::PermissionMode;
use crate::agent::runtime::runtime_io::WorkspaceSkillRuntimeEntry;
use crate::agent::tool_manifest::{ToolCategory, ToolSource};
use crate::agent::tools::search_providers::create_provider;
use crate::agent::tools::{ExecTool, ProcessManager, TaskTool, WebSearchTool};
use crate::agent::AgentExecutor;
use crate::agent::ToolManifestEntry;
use crate::runtime_environment::runtime_paths_from_app;
use crate::session_journal::SessionJournalStateHandle;
use runtime_chat_app::{
    compose_system_prompt, ChatExecutionGuidance, ChatExecutionPreparationService,
};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[derive(Clone)]
pub(crate) struct PreparedRuntimeTools {
    pub allowed_tools: Option<Vec<String>>,
    pub full_allowed_tools: Vec<String>,
    pub tool_manifest: Vec<ToolManifestEntry>,
    pub effective_tool_plan: EffectiveToolSet,
    pub discovery_candidates: Vec<ToolDiscoveryCandidateRecord>,
    pub system_prompt: String,
    pub skill_command_specs: Vec<chat_io::WorkspaceSkillCommandSpec>,
}

impl PreparedRuntimeTools {
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
}

pub(crate) async fn prepare_runtime_tools(
    params: ToolSetupParams<'_>,
) -> Result<PreparedRuntimeTools, String> {
    let process_manager = Arc::new(ProcessManager::new());
    let registry_builder = RuntimeToolRegistryBuilder::new(params.agent_executor.registry());
    registry_builder.register_process_shell_tools(Arc::clone(&process_manager));
    params
        .agent_executor
        .registry()
        .register(Arc::new(ExecTool::with_process_manager(Arc::clone(
            &process_manager,
        ))));

    registry_builder.register_browser_and_alias_tools(DEFAULT_BROWSER_SIDECAR_URL);
    let task_tool = TaskTool::new(
        params.agent_executor.registry_arc(),
        params.api_format.to_string(),
        params.base_url.to_string(),
        params.api_key.to_string(),
        params.model_name.to_string(),
    )
    .with_app_handle(params.app.clone(), params.session_id.to_string());
    let task_tool = if let Some(journal) = params.app.try_state::<SessionJournalStateHandle>() {
        task_tool.with_runtime_state(params.db.clone(), journal.0.clone())
    } else {
        task_tool
    };
    let runtime_paths = runtime_paths_from_app(params.app).unwrap_or_else(|_| {
        crate::runtime_paths::RuntimePaths::new(crate::runtime_paths::resolve_runtime_root())
    });
    let memory_dir = chat_io::build_memory_dir_for_session(
        &runtime_paths.memory_dir,
        params.skill_id,
        params.memory_bucket_employee_id,
    );
    registry_builder.register_runtime_support_tools(
        task_tool,
        params.db.clone(),
        memory_dir.clone(),
    );

    let search_cache = params.app.state::<SearchCacheState>().0.clone();
    let mut runtime_search_note: Option<String> = None;
    if let Some((search_api_format, search_base_url, search_api_key, search_model_name)) =
        chat_io::load_default_search_provider_config_with_pool(params.db).await?
    {
        match create_provider(
            &search_api_format,
            &search_base_url,
            &search_api_key,
            &search_model_name,
        ) {
            Ok(provider) => {
                let web_search = WebSearchTool::with_provider(provider, search_cache);
                registry_builder.register_search_tool(Arc::new(web_search));
            }
            Err(e) => {
                eprintln!("[search] 创建搜索 Provider 失败: {}", e);
            }
        }
    } else if let Some(source_label) = registry_builder.register_mcp_search_fallback() {
        runtime_search_note = Some(format!(
            "当前未配置搜索引擎。若需要联网检索，`web_search` 会改用 MCP 工具 `{}`，回答时要说明本次使用了 MCP fallback。",
            source_label
        ));
        eprintln!(
            "[search] 未配置搜索引擎，已回退到 MCP 搜索工具 {}",
            source_label
        );
    } else {
        runtime_search_note = Some(
            "当前没有可用联网检索源。若用户请求最新信息或联网搜索，必须明确说明只能基于已有知识回答，结果可能不是最新信息。"
                .to_string(),
        );
        eprintln!("[search] 当前没有可用搜索引擎或 MCP 搜索工具，运行时仅可离线回答");
    }

    let skill_roots = chat_io::build_skill_roots(
        params
            .execution_preparation_service
            .resolve_skill_root_work_dir(params.execution_guidance),
        params.source_type,
        params.pack_path,
    );
    let (workspace_skills_prompt, skill_command_specs) = match params
        .execution_preparation_service
        .resolve_executor_work_dir(params.execution_guidance)
    {
        Some(work_dir) => {
            chat_io::sync_workspace_skills_to_directory(
                std::path::Path::new(&work_dir),
                params.workspace_skill_entries,
            )?;
            (
                if params.suppress_workspace_skills_prompt {
                    None
                } else {
                    Some(chat_io::prepare_workspace_skills_prompt(
                        std::path::Path::new(&work_dir),
                        params.workspace_skill_entries,
                    )?)
                },
                chat_io::build_workspace_skill_command_specs(params.workspace_skill_entries),
            )
        }
        None => (None, Vec::new()),
    };
    registry_builder.register_skill_and_compaction_tools(
        params.session_id,
        skill_roots,
        params.max_call_depth,
    );
    registry_builder.register_ask_user_tool(params.app, params.session_id);

    let effective_tool_set = resolve_effective_tool_set(
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
    let memory_content = chat_io::load_memory_content(&memory_dir);
    let system_prompt = compose_system_prompt(
        params.skill_system_prompt,
        &effective_tool_set.tool_names_csv(),
        params.model_name,
        params.max_iter,
        params.execution_guidance,
        workspace_skills_prompt.as_deref(),
        params.employee_collaboration_guidance,
        Some(&memory_content),
    );
    let system_prompt = if let Some(runtime_search_note) = runtime_search_note {
        format!("{system_prompt}\n\n[联网检索状态]\n{runtime_search_note}")
    } else {
        system_prompt
    };
    let system_prompt = if let Some(discovery_index) =
        format_tool_discovery_index(&effective_tool_set.tool_manifest)
    {
        format!("{system_prompt}\n\n{discovery_index}")
    } else {
        system_prompt
    };
    let discovery_candidates = if let Some(discovery_query) = params.tool_discovery_query {
        build_tool_candidate_records(&effective_tool_set.tool_manifest, discovery_query, 4)
    } else {
        Vec::new()
    };
    let mut effective_tool_set = effective_tool_set;
    effective_tool_set.apply_recommended_tools(&discovery_candidates);
    let system_prompt = if let Some(discovery_query) = params.tool_discovery_query {
        if let Some(candidate_hints) = format_tool_candidate_hints(
            &effective_tool_set.active_tool_manifest,
            discovery_query,
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
        allowed_tools: effective_tool_set.allowed_tools.clone(),
        full_allowed_tools: effective_tool_set.full_allowed_tools(),
        tool_manifest: effective_tool_set.active_tool_manifest.clone(),
        effective_tool_plan: effective_tool_set,
        discovery_candidates,
        system_prompt,
        skill_command_specs,
    })
}

#[cfg(test)]
mod tests {
    use super::PreparedRuntimeTools;
    use crate::agent::runtime::effective_tool_set::{
        EffectiveToolPolicySummary, EffectiveToolSet, EffectiveToolSetSource,
    };
    use crate::agent::runtime::tool_catalog::{
        format_tool_candidate_hints, format_tool_discovery_index, ToolDiscoveryCandidateRecord,
    };
    use crate::agent::tool_manifest::{ToolCategory, ToolMetadata, ToolSource};
    use crate::agent::ToolManifestEntry;

    #[test]
    fn prepared_runtime_tools_keeps_allowed_tool_list() {
        let prepared = PreparedRuntimeTools {
            allowed_tools: Some(vec!["read_file".to_string(), "bash".to_string()]),
            full_allowed_tools: vec!["read_file".to_string(), "bash".to_string()],
            tool_manifest: Vec::new(),
            effective_tool_plan: EffectiveToolSet {
                source: EffectiveToolSetSource::ExplicitAllowList,
                allowed_tools: Some(vec!["read_file".to_string(), "bash".to_string()]),
                tool_names: vec!["read_file".to_string(), "bash".to_string()],
                tool_manifest: Vec::new(),
                active_tools: vec!["read_file".to_string(), "bash".to_string()],
                active_tool_manifest: Vec::new(),
                recommended_tools: Vec::new(),
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
            },
            discovery_candidates: Vec::new(),
            system_prompt: "system prompt".to_string(),
            skill_command_specs: Vec::new(),
        };

        assert_eq!(
            prepared.allowed_tools,
            Some(vec!["read_file".to_string(), "bash".to_string()])
        );
        assert_eq!(prepared.full_allowed_tools.len(), 2);
        assert!(prepared.tool_manifest.is_empty());
        assert_eq!(prepared.effective_tool_plan.tool_names.len(), 2);
        assert_eq!(prepared.system_prompt, "system prompt");
        assert!(prepared.skill_command_specs.is_empty());
    }

    #[test]
    fn tool_plan_record_includes_discovery_candidates() {
        let prepared = PreparedRuntimeTools {
            allowed_tools: Some(vec!["web_search".to_string()]),
            full_allowed_tools: vec!["web_search".to_string()],
            tool_manifest: Vec::new(),
            effective_tool_plan: EffectiveToolSet {
                source: EffectiveToolSetSource::ExplicitAllowList,
                allowed_tools: Some(vec!["web_search".to_string()]),
                tool_names: vec!["web_search".to_string()],
                tool_manifest: Vec::new(),
                active_tools: vec!["web_search".to_string()],
                active_tool_manifest: Vec::new(),
                recommended_tools: Vec::new(),
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
            },
            discovery_candidates: vec![ToolDiscoveryCandidateRecord {
                name: "web_search".to_string(),
                category: ToolCategory::Search,
                source: ToolSource::Runtime,
                score: 10,
                matched_terms: vec!["search".to_string()],
                matched_fields: vec!["name".to_string()],
            }],
            system_prompt: "system prompt".to_string(),
            skill_command_specs: Vec::new(),
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
}
