use crate::agent::runtime::events::{AskUserState, SearchCacheState};
use crate::agent::runtime::runtime_io as chat_io;
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
use std::path::PathBuf;
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
pub(crate) struct McpSearchFallbackTool {
    inner: Arc<dyn Tool>,
    source_label: String,
    query_key: String,
    limit_key: Option<String>,
    freshness_key: Option<String>,
}

impl McpSearchFallbackTool {
    pub(crate) fn new(
        inner: Arc<dyn Tool>,
        source_label: String,
        query_key: String,
        limit_key: Option<String>,
        freshness_key: Option<String>,
    ) -> Self {
        Self {
            inner,
            source_label,
            query_key,
            limit_key,
            freshness_key,
        }
    }

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
pub(crate) struct McpSearchFallbackSpec {
    pub tool_name: String,
    pub source_label: String,
    pub query_key: String,
    pub limit_key: Option<String>,
    pub freshness_key: Option<String>,
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

pub(crate) fn resolve_mcp_search_fallback(
    registry: &ToolRegistry,
) -> Option<McpSearchFallbackSpec> {
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

pub(crate) fn search_fallback_runtime_note(source_label: &str) -> String {
    format!(
        "当前未配置搜索引擎。若需要联网检索，`web_search` 会改用 MCP 工具 `{source_label}`，回答时要说明本次使用了 MCP fallback。"
    )
}

pub(crate) fn offline_only_search_note() -> String {
    "当前没有可用联网检索源。若用户请求最新信息或联网搜索，必须明确说明只能基于已有知识回答，结果可能不是最新信息。"
        .to_string()
}

pub(crate) struct ToolRegistrySetupParams<'a> {
    pub app: &'a AppHandle,
    pub db: &'a sqlx::SqlitePool,
    pub agent_executor: &'a Arc<AgentExecutor>,
    pub session_id: &'a str,
    pub api_format: &'a str,
    pub base_url: &'a str,
    pub model_name: &'a str,
    pub api_key: &'a str,
    pub skill_id: &'a str,
    pub source_type: &'a str,
    pub pack_path: &'a str,
    pub max_call_depth: usize,
    pub execution_preparation_service: &'a ChatExecutionPreparationService,
    pub execution_guidance: &'a ChatExecutionGuidance,
    pub memory_bucket_employee_id: &'a str,
}

pub(crate) struct ToolRegistrySetupResult {
    pub memory_dir: PathBuf,
    pub runtime_notes: Vec<String>,
}

pub(crate) async fn setup_runtime_tool_registry(
    params: ToolRegistrySetupParams<'_>,
) -> Result<ToolRegistrySetupResult, String> {
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
    let mut runtime_notes = Vec::new();
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
                .register(Arc::new(McpSearchFallbackTool::new(
                    inner,
                    fallback.source_label.clone(),
                    fallback.query_key,
                    fallback.limit_key,
                    fallback.freshness_key,
                )));
            runtime_notes.push(search_fallback_runtime_note(&fallback.source_label));
            eprintln!(
                "[search] 未配置搜索引擎，已回退到 MCP 搜索工具 {}",
                fallback.source_label
            );
        }
    } else {
        runtime_notes.push(offline_only_search_note());
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
    let ask_user_pending_session = params
        .app
        .state::<crate::agent::runtime::AskUserPendingSessionState>()
        .0
        .clone();
    let ask_user_tool = AskUserTool::new(
        params.app.clone(),
        params.session_id.to_string(),
        ask_user_responder,
        ask_user_pending_session,
    );
    params
        .agent_executor
        .registry()
        .register(Arc::new(ask_user_tool));

    Ok(ToolRegistrySetupResult {
        memory_dir,
        runtime_notes,
    })
}
