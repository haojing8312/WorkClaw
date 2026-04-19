use crate::agent::runtime::events::AskUserState;
use crate::agent::tools::{
    browser_compat::register_browser_compat_tool, browser_tools::register_browser_tools,
    register_tool_alias, AskUserTool, BashKillTool, BashOutputTool, BashTool, ClawhubRecommendTool,
    ClawhubSearchTool, CompactTool, EmployeeManageTool, GithubRepoDownloadTool, MemoryTool,
    ProcessManager, SkillInvokeTool, TaskTool,
};
use crate::agent::{Tool, ToolContext, ToolRegistry};
use serde_json::{json, Map, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const DEFAULT_BROWSER_SIDECAR_URL: &str = "http://localhost:8765";

#[cfg_attr(not(test), allow(dead_code))]
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

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone)]
struct McpSearchFallbackTool {
    inner: Arc<dyn Tool>,
    source_label: String,
    query_key: String,
    limit_key: Option<String>,
    freshness_key: Option<String>,
}

impl McpSearchFallbackTool {
    #[cfg_attr(not(test), allow(dead_code))]
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

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone)]
struct McpSearchFallbackSpec {
    tool_name: String,
    source_label: String,
    query_key: String,
    limit_key: Option<String>,
    freshness_key: Option<String>,
}

#[cfg_attr(not(test), allow(dead_code))]
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

#[cfg_attr(not(test), allow(dead_code))]
fn infer_limit_key(schema: &Value) -> Option<String> {
    let properties = schema.get("properties")?.as_object()?;
    for key in ["count", "limit", "numResults", "num_results", "max_results"] {
        if properties.contains_key(key) {
            return Some(key.to_string());
        }
    }
    None
}

#[cfg_attr(not(test), allow(dead_code))]
fn infer_freshness_key(schema: &Value) -> Option<String> {
    let properties = schema.get("properties")?.as_object()?;
    for key in ["freshness", "time_range", "timeRange"] {
        if properties.contains_key(key) {
            return Some(key.to_string());
        }
    }
    None
}

#[cfg_attr(not(test), allow(dead_code))]
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

#[cfg_attr(not(test), allow(dead_code))]
fn format_mcp_source_label(tool_name: &str) -> String {
    tool_name
        .strip_prefix("mcp_")
        .unwrap_or(tool_name)
        .to_string()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct RuntimeToolRegistryBuilder<'a> {
    registry: &'a ToolRegistry,
}

impl<'a> RuntimeToolRegistryBuilder<'a> {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new(registry: &'a ToolRegistry) -> Self {
        Self { registry }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn register_process_shell_tools(&self, process_manager: Arc<ProcessManager>) {
        self.registry
            .register(Arc::new(BashOutputTool::new(Arc::clone(&process_manager))));
        self.registry
            .register(Arc::new(BashKillTool::new(Arc::clone(&process_manager))));
        self.registry.unregister("bash");
        self.registry
            .register(Arc::new(BashTool::with_process_manager(process_manager)));
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn register_browser_and_alias_tools(&self, sidecar_url: &str) {
        register_browser_tools(self.registry, sidecar_url);
        register_browser_compat_tool(self.registry, sidecar_url);
        if let Some(tool) = self.registry.get("read_file") {
            register_tool_alias(self.registry, "read", tool);
        }
        if let Some(tool) = self.registry.get("glob") {
            register_tool_alias(self.registry, "find", tool);
        }
        if let Some(tool) = self.registry.get("list_dir") {
            register_tool_alias(self.registry, "ls", tool);
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn register_skill_and_compaction_tools(
        &self,
        session_id: &str,
        skill_roots: Vec<PathBuf>,
        max_call_depth: usize,
    ) {
        let skill_tool = SkillInvokeTool::new(session_id.to_string(), skill_roots)
            .with_max_depth(max_call_depth);
        self.registry.register(Arc::new(skill_tool));
        self.registry.register(Arc::new(CompactTool::new()));
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn register_ask_user_tool(&self, app: &AppHandle, session_id: &str) {
        let ask_user_responder = app.state::<AskUserState>().0.clone();
        let ask_user_pending_session = app
            .state::<crate::agent::runtime::AskUserPendingSessionState>()
            .0
            .clone();
        let ask_user_tool = AskUserTool::new(
            app.clone(),
            session_id.to_string(),
            ask_user_responder,
            ask_user_pending_session,
        );
        self.registry.register(Arc::new(ask_user_tool));
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn register_runtime_support_tools(
        &self,
        task_tool: TaskTool,
        db: sqlx::SqlitePool,
        memory_dir: PathBuf,
    ) {
        self.registry.register(Arc::new(task_tool));
        self.registry.register(Arc::new(ClawhubSearchTool));
        self.registry.register(Arc::new(ClawhubRecommendTool));
        self.registry
            .register(Arc::new(GithubRepoDownloadTool::new()));
        self.registry
            .register(Arc::new(EmployeeManageTool::new(db)));
        self.registry
            .register(Arc::new(MemoryTool::new(memory_dir)));
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn register_search_tool(&self, tool: Arc<dyn Tool>) {
        self.registry.register(tool);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn register_mcp_search_fallback(&self) -> Option<String> {
        let mut names = self.registry.tools_with_prefix("mcp_");
        names.sort();

        let fallback = names
            .into_iter()
            .filter_map(|tool_name| {
                let tool = self.registry.get(&tool_name)?;
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
            .map(|(_, spec)| spec)?;

        let inner = self.registry.get(&fallback.tool_name)?;
        self.registry.register(Arc::new(McpSearchFallbackTool {
            inner,
            source_label: fallback.source_label.clone(),
            query_key: fallback.query_key,
            limit_key: fallback.limit_key,
            freshness_key: fallback.freshness_key,
        }));
        Some(fallback.source_label)
    }
}

#[cfg(test)]
mod tests {
    use super::{RuntimeToolRegistryBuilder, DEFAULT_BROWSER_SIDECAR_URL};
    use crate::agent::tools::{ProcessManager, TaskTool};
    use crate::agent::{Tool, ToolContext, ToolRegistry};
    use anyhow::Result;
    use serde_json::{json, Value};
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn builder_registers_process_browser_and_alias_tools() {
        let registry = ToolRegistry::with_standard_tools();
        let builder = RuntimeToolRegistryBuilder::new(&registry);

        builder.register_process_shell_tools(Arc::new(ProcessManager::new()));
        builder.register_browser_and_alias_tools(DEFAULT_BROWSER_SIDECAR_URL);

        for tool_name in [
            "bash",
            "bash_output",
            "bash_kill",
            "browser_launch",
            "browser_snapshot",
            "browser_act",
            "read",
            "find",
            "ls",
        ] {
            assert!(
                registry.get(tool_name).is_some(),
                "expected runtime registry to contain {tool_name}"
            );
        }
    }

    #[tokio::test]
    async fn builder_registers_runtime_support_tools() {
        let registry = Arc::new(ToolRegistry::with_standard_tools());
        let builder = RuntimeToolRegistryBuilder::new(registry.as_ref());
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let memory_dir = tempdir().expect("temp dir");
        let task_tool = TaskTool::new(
            Arc::clone(&registry),
            "openai".to_string(),
            "https://example.com".to_string(),
            "test-key".to_string(),
            "gpt-4o-mini".to_string(),
        );

        builder.register_runtime_support_tools(task_tool, db, memory_dir.path().to_path_buf());

        for tool_name in [
            "task",
            "clawhub_search",
            "clawhub_recommend",
            "github_repo_download",
            "employee_manage",
            "memory",
        ] {
            assert!(
                registry.get(tool_name).is_some(),
                "expected runtime registry to contain {tool_name}"
            );
        }
    }

    struct FakeTool {
        name: String,
        description: String,
        schema: Value,
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

        fn execute(&self, _input: Value, _ctx: &ToolContext) -> Result<String> {
            Ok("search result".to_string())
        }
    }

    #[test]
    fn builder_registers_search_like_mcp_fallback() {
        let registry = ToolRegistry::new();
        let builder = RuntimeToolRegistryBuilder::new(&registry);
        registry.register(Arc::new(FakeTool {
            name: "mcp_filesystem_read".to_string(),
            description: "Read files".to_string(),
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                }
            }),
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
        }));

        let source_label = builder
            .register_mcp_search_fallback()
            .expect("mcp fallback should register");

        assert_eq!(source_label, "brave-search_web_search");
        assert!(registry.get("web_search").is_some());
    }
}
