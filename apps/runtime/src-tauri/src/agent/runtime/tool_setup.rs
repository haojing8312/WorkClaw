use super::events::{AskUserState, SearchCacheState};
use crate::agent::runtime::kernel::context_bundle::ContextBundle;
use crate::agent::runtime::kernel::capability_snapshot::CapabilitySnapshot;
use super::runtime_io as chat_io;
use crate::agent::runtime::runtime_io::WorkspaceSkillRuntimeEntry;
use crate::agent::tools::search_providers::create_provider;
use crate::agent::tools::{
    browser_compat::register_browser_compat_tool, browser_tools::register_browser_tools,
    register_tool_alias, AskUserTool, BashKillTool, BashOutputTool, BashTool, ClawhubRecommendTool,
    ClawhubSearchTool, CompactTool, EmployeeManageTool, ExecTool, GithubRepoDownloadTool,
    MemoryTool, ProcessManager, SkillInvokeTool, TaskTool, WebSearchTool,
};
use crate::agent::{AgentExecutor, Tool, ToolContext, ToolRegistry};
use crate::session_journal::SessionJournalStateHandle;
use runtime_chat_app::{ChatExecutionGuidance, ChatExecutionPreparationService};
use serde_json::{json, Map, Value};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

fn standard_web_search_input_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "搜索关键词"
            },
            "count": {
                "type": "integer",
                "description": "返回结果数量 (1-10，默认 5)",
                "default": 5
            },
            "freshness": {
                "type": "string",
                "description": "结果新鲜度: day/week/month/year",
                "enum": ["day", "week", "month", "year"]
            }
        },
        "required": ["query"]
    })
}

#[derive(Clone)]
struct McpSearchFallbackTool {
    inner: Arc<dyn Tool>,
    source_label: String,
    query_key: String,
    limit_key: Option<String>,
    freshness_key: Option<String>,
}

impl McpSearchFallbackTool {
    fn build_input(&self, input: Value) -> Value {
        let mut mapped = Map::new();

        if let Some(query) = input.get("query").and_then(Value::as_str) {
            mapped.insert(self.query_key.clone(), Value::String(query.to_string()));
        }

        if let Some(count) = input.get("count") {
            if let Some(limit_key) = &self.limit_key {
                mapped.insert(limit_key.clone(), count.clone());
            }
        }

        if let Some(freshness) = input.get("freshness") {
            if let Some(freshness_key) = &self.freshness_key {
                mapped.insert(freshness_key.clone(), freshness.clone());
            }
        }

        Value::Object(mapped)
    }
}

impl Tool for McpSearchFallbackTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "搜索互联网获取最新信息。当前未配置搜索引擎时，自动改用可用的 MCP 搜索工具。"
    }

    fn input_schema(&self) -> Value {
        standard_web_search_input_schema()
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<String> {
        let query = input
            .get("query")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("缺少 query 参数"))?;
        if query.trim().is_empty() {
            anyhow::bail!("query 不能为空");
        }

        let result = self.inner.execute(self.build_input(input), ctx)?;
        Ok(format!(
            "未配置搜索引擎，本次改用 MCP：{}\n\n{}",
            self.source_label, result
        ))
    }
}

#[derive(Clone)]
struct McpSearchFallbackSpec {
    tool_name: String,
    source_label: String,
    query_key: String,
    limit_key: Option<String>,
    freshness_key: Option<String>,
}

fn infer_query_key(schema: &Value) -> Option<String> {
    let properties = schema.get("properties")?.as_object()?;
    for key in ["query", "q", "searchTerm", "search_term", "keywords"] {
        if properties
            .get(key)
            .and_then(|value| value.get("type"))
            .and_then(Value::as_str)
            == Some("string")
        {
            return Some(key.to_string());
        }
    }
    None
}

fn infer_limit_key(schema: &Value) -> Option<String> {
    let properties = schema.get("properties")?.as_object()?;
    for key in ["count", "limit", "numResults", "num_results", "max_results"] {
        if properties.contains_key(key) {
            return Some(key.to_string());
        }
    }
    None
}

fn infer_freshness_key(schema: &Value) -> Option<String> {
    let properties = schema.get("properties")?.as_object()?;
    for key in ["freshness", "time_range", "timeRange"] {
        if properties.contains_key(key) {
            return Some(key.to_string());
        }
    }
    None
}

fn score_mcp_search_candidate(tool_name: &str, description: &str, query_key: &str) -> Option<i32> {
    let haystack = format!(
        "{} {}",
        tool_name.to_ascii_lowercase(),
        description.to_ascii_lowercase()
    );

    if !(haystack.contains("search")
        || haystack.contains("brave")
        || haystack.contains("serp")
        || haystack.contains("tavily"))
    {
        return None;
    }

    let mut score = 10;
    if haystack.contains("search") {
        score += 8;
    }
    if haystack.contains("brave") {
        score += 4;
    }
    if haystack.contains("web") {
        score += 2;
    }
    if query_key == "query" {
        score += 3;
    }

    Some(score)
}

fn format_mcp_source_label(tool_name: &str) -> String {
    tool_name
        .strip_prefix("mcp_")
        .unwrap_or(tool_name)
        .to_string()
}

fn resolve_mcp_search_fallback(registry: &ToolRegistry) -> Option<McpSearchFallbackSpec> {
    let mut names = registry.tools_with_prefix("mcp_");
    names.sort();

    names
        .into_iter()
        .filter_map(|tool_name| {
            let tool = registry.get(&tool_name)?;
            let schema = tool.input_schema();
            let query_key = infer_query_key(&schema)?;
            let score = score_mcp_search_candidate(&tool_name, tool.description(), &query_key)?;
            Some((
                score,
                McpSearchFallbackSpec {
                    tool_name: tool_name.clone(),
                    source_label: format_mcp_source_label(&tool_name),
                    query_key,
                    limit_key: infer_limit_key(&schema),
                    freshness_key: infer_freshness_key(&schema),
                },
            ))
        })
        .max_by(|(left_score, left_spec), (right_score, right_spec)| {
            left_score
                .cmp(right_score)
                .then_with(|| right_spec.tool_name.cmp(&left_spec.tool_name))
        })
        .map(|(_, spec)| spec)
}

#[derive(Clone)]
pub(crate) struct PreparedRuntimeTools {
    pub allowed_tools: Option<Vec<String>>,
    pub system_prompt: String,
    pub capability_snapshot: CapabilitySnapshot,
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
    pub skill_system_prompt: &'a str,
    pub skill_allowed_tools: Option<Vec<String>>,
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
    params
        .agent_executor
        .registry()
        .register(Arc::new(BashOutputTool::new(Arc::clone(&process_manager))));
    params
        .agent_executor
        .registry()
        .register(Arc::new(BashKillTool::new(Arc::clone(&process_manager))));
    params.agent_executor.registry().unregister("bash");
    params
        .agent_executor
        .registry()
        .register(Arc::new(BashTool::with_process_manager(Arc::clone(
            &process_manager,
        ))));
    params
        .agent_executor
        .registry()
        .register(Arc::new(ExecTool::with_process_manager(Arc::clone(
            &process_manager,
        ))));

    register_browser_tools(params.agent_executor.registry(), "http://localhost:8765");
    register_browser_compat_tool(params.agent_executor.registry(), "http://localhost:8765");
    if let Some(tool) = params.agent_executor.registry().get("read_file") {
        register_tool_alias(params.agent_executor.registry(), "read", tool);
    }
    if let Some(tool) = params.agent_executor.registry().get("glob") {
        register_tool_alias(params.agent_executor.registry(), "find", tool);
    }
    if let Some(tool) = params.agent_executor.registry().get("list_dir") {
        register_tool_alias(params.agent_executor.registry(), "ls", tool);
    }
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
    params
        .agent_executor
        .registry()
        .register(Arc::new(task_tool));
    params
        .agent_executor
        .registry()
        .register(Arc::new(ClawhubSearchTool));
    params
        .agent_executor
        .registry()
        .register(Arc::new(ClawhubRecommendTool));
    params
        .agent_executor
        .registry()
        .register(Arc::new(GithubRepoDownloadTool::new()));
    params
        .agent_executor
        .registry()
        .register(Arc::new(EmployeeManageTool::new(params.db.clone())));

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
                params
                    .agent_executor
                    .registry()
                    .register(Arc::new(web_search));
            }
            Err(e) => {
                eprintln!("[search] 创建搜索 Provider 失败: {}", e);
            }
        }
    } else if let Some(fallback) = resolve_mcp_search_fallback(params.agent_executor.registry()) {
        if let Some(inner) = params.agent_executor.registry().get(&fallback.tool_name) {
            params
                .agent_executor
                .registry()
                .register(Arc::new(McpSearchFallbackTool {
                    inner,
                    source_label: fallback.source_label.clone(),
                    query_key: fallback.query_key,
                    limit_key: fallback.limit_key,
                    freshness_key: fallback.freshness_key,
                }));
            runtime_search_note = Some(format!(
                "当前未配置搜索引擎。若需要联网检索，`web_search` 会改用 MCP 工具 `{}`，回答时要说明本次使用了 MCP fallback。",
                fallback.source_label
            ));
            eprintln!(
                "[search] 未配置搜索引擎，已回退到 MCP 搜索工具 {}",
                fallback.source_label
            );
        }
    } else {
        runtime_search_note = Some(
            "当前没有可用联网检索源。若用户请求最新信息或联网搜索，必须明确说明只能基于已有知识回答，结果可能不是最新信息。"
                .to_string(),
        );
        eprintln!("[search] 当前没有可用搜索引擎或 MCP 搜索工具，运行时仅可离线回答");
    }

    let app_data_dir = params.app.path().app_data_dir().unwrap_or_default();
    let memory_dir = chat_io::build_memory_dir_for_session(
        &app_data_dir,
        params.skill_id,
        params.memory_bucket_employee_id,
    );
    params
        .agent_executor
        .registry()
        .register(Arc::new(MemoryTool::new(memory_dir.clone())));

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
    let skill_tool = SkillInvokeTool::new(params.session_id.to_string(), skill_roots)
        .with_max_depth(params.max_call_depth);
    params
        .agent_executor
        .registry()
        .register(Arc::new(skill_tool));

    params
        .agent_executor
        .registry()
        .register(Arc::new(CompactTool::new()));

    let ask_user_responder = params.app.state::<AskUserState>().0.clone();
    let ask_user_tool = AskUserTool::new(
        params.app.clone(),
        params.session_id.to_string(),
        ask_user_responder,
    );
    params
        .agent_executor
        .registry()
        .register(Arc::new(ask_user_tool));

    let resolved_tool_names =
        chat_io::resolve_tool_name_list(&params.skill_allowed_tools, params.agent_executor);
    let capability_snapshot = CapabilitySnapshot {
        allowed_tools: params.skill_allowed_tools.clone(),
        resolved_tool_names,
        skill_command_specs: skill_command_specs.clone(),
        runtime_notes: runtime_search_note.iter().cloned().collect(),
    };
    let memory_content = chat_io::load_memory_content(&memory_dir);
    let context_bundle = ContextBundle::build(
        params.skill_system_prompt,
        &capability_snapshot,
        params.model_name,
        params.max_iter,
        params.execution_guidance,
        workspace_skills_prompt,
        params.employee_collaboration_guidance.map(str::to_string),
        Some(memory_content),
    );

    Ok(PreparedRuntimeTools {
        allowed_tools: params.skill_allowed_tools,
        system_prompt: context_bundle.system_prompt,
        capability_snapshot,
    })
}

#[cfg(test)]
mod tests {
    use super::{resolve_mcp_search_fallback, McpSearchFallbackTool};
    use crate::agent::registry::ToolRegistry;
    use crate::agent::types::{Tool, ToolContext};
    use serde_json::{json, Value};
    use std::sync::{Arc, Mutex};

    struct FakeTool {
        name: String,
        description: String,
        schema: Value,
        recorded_input: Arc<Mutex<Option<Value>>>,
    }

    impl Tool for FakeTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        fn input_schema(&self) -> Value {
            self.schema.clone()
        }

        fn execute(&self, input: Value, _ctx: &ToolContext) -> anyhow::Result<String> {
            *self.recorded_input.lock().expect("recorded input lock") = Some(input);
            Ok("search result".to_string())
        }
    }

    #[test]
    fn resolves_search_like_mcp_fallback() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(FakeTool {
            name: "mcp_filesystem_read".to_string(),
            description: "Read files".to_string(),
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                }
            }),
            recorded_input: Arc::new(Mutex::new(None)),
        }));
        registry.register(Arc::new(FakeTool {
            name: "mcp_brave-search_web_search".to_string(),
            description: "Search the web".to_string(),
            schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "count": { "type": "integer" }
                }
            }),
            recorded_input: Arc::new(Mutex::new(None)),
        }));

        let resolved = resolve_mcp_search_fallback(&registry).expect("mcp fallback");
        assert_eq!(resolved.tool_name, "mcp_brave-search_web_search");
        assert_eq!(resolved.query_key, "query");
        assert_eq!(resolved.limit_key.as_deref(), Some("count"));
    }

    #[test]
    fn mcp_search_fallback_tool_prefixes_output_and_maps_input() {
        let recorded_input = Arc::new(Mutex::new(None));
        let tool = McpSearchFallbackTool {
            inner: Arc::new(FakeTool {
                name: "mcp_brave-search_web_search".to_string(),
                description: "Search the web".to_string(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" },
                        "limit": { "type": "integer" }
                    }
                }),
                recorded_input: Arc::clone(&recorded_input),
            }),
            source_label: "brave-search_web_search".to_string(),
            query_key: "query".to_string(),
            limit_key: Some("limit".to_string()),
            freshness_key: None,
        };

        let result = tool
            .execute(
                json!({"query": "latest ai news", "count": 3}),
                &ToolContext::default(),
            )
            .expect("fallback execute");

        assert!(result.contains("未配置搜索引擎，本次改用 MCP：brave-search_web_search"));
        let recorded = recorded_input
            .lock()
            .expect("recorded input lock")
            .clone()
            .expect("recorded input");
        assert_eq!(recorded["query"], "latest ai news");
        assert_eq!(recorded["limit"], 3);
    }
}
