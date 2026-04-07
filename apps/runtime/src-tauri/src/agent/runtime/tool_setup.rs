use super::runtime_io as chat_io;
use crate::agent::runtime::kernel::capability_snapshot::CapabilitySnapshot;
use crate::agent::runtime::kernel::context_bundle::ContextBundle;
use crate::agent::runtime::kernel::tool_registry_setup::{
    setup_runtime_tool_registry, ToolRegistrySetupParams,
};
use crate::agent::runtime::kernel::workspace_skill_context::build_workspace_skill_context;
use crate::agent::runtime::runtime_io::WorkspaceSkillRuntimeEntry;
use crate::agent::AgentExecutor;
use runtime_chat_app::{ChatExecutionGuidance, ChatExecutionPreparationService};
use std::sync::Arc;
use tauri::AppHandle;

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

    let resolved_tool_names =
        chat_io::resolve_tool_name_list(&params.skill_allowed_tools, params.agent_executor);
    let capability_snapshot = CapabilitySnapshot::build(
        params.skill_allowed_tools.clone(),
        resolved_tool_names,
        workspace_skill_context.skill_command_specs.clone(),
        registry_setup.runtime_notes,
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

    Ok(PreparedRuntimeTools {
        allowed_tools: params.skill_allowed_tools,
        system_prompt: context_bundle.system_prompt,
        capability_snapshot,
    })
}

#[cfg(test)]
mod tests {
    use crate::agent::registry::ToolRegistry;
    use crate::agent::runtime::kernel::tool_registry_setup::{
        offline_only_search_note, resolve_mcp_search_fallback, search_fallback_runtime_note,
        McpSearchFallbackTool,
    };
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
        let tool = McpSearchFallbackTool::new(
            Arc::new(FakeTool {
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
            "brave-search_web_search".to_string(),
            "query".to_string(),
            Some("limit".to_string()),
            None,
        );

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

    #[test]
    fn offline_only_search_note_mentions_knowledge_only_limit() {
        let note = offline_only_search_note();

        assert!(note.contains("当前没有可用联网检索源"));
        assert!(note.contains("只能基于已有知识回答"));
    }

    #[test]
    fn search_fallback_runtime_note_mentions_web_search_and_mcp_source() {
        let note = search_fallback_runtime_note("brave-search_web_search");

        assert!(note.contains("`web_search`"));
        assert!(note.contains("MCP"));
        assert!(note.contains("brave-search_web_search"));
    }
}
