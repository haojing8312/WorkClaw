use super::events::SearchCacheState;
use super::runtime_io as chat_io;
use super::tool_registry_builder::{
    RuntimeToolRegistryBuilder, DEFAULT_BROWSER_SIDECAR_URL,
};
use crate::agent::ToolManifestEntry;
use crate::agent::runtime::runtime_io::WorkspaceSkillRuntimeEntry;
use crate::agent::tools::search_providers::create_provider;
use crate::agent::tools::{
    ExecTool, ProcessManager, TaskTool, WebSearchTool,
};
use crate::agent::AgentExecutor;
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
    pub tool_manifest: Vec<ToolManifestEntry>,
    pub system_prompt: String,
    pub skill_command_specs: Vec<chat_io::WorkspaceSkillCommandSpec>,
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
    registry_builder.register_runtime_support_tools(task_tool, params.db.clone(), memory_dir.clone());

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

    let tool_names =
        chat_io::resolve_tool_names(&params.skill_allowed_tools, params.agent_executor);
    let tool_manifest =
        chat_io::resolve_tool_manifest(&params.skill_allowed_tools, params.agent_executor);
    let memory_content = chat_io::load_memory_content(&memory_dir);
    let system_prompt = compose_system_prompt(
        params.skill_system_prompt,
        &tool_names,
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

    Ok(PreparedRuntimeTools {
        allowed_tools: params.skill_allowed_tools,
        tool_manifest,
        system_prompt,
        skill_command_specs,
    })
}

#[cfg(test)]
mod tests {
    use super::PreparedRuntimeTools;

    #[test]
    fn prepared_runtime_tools_keeps_allowed_tool_list() {
        let prepared = PreparedRuntimeTools {
            allowed_tools: Some(vec!["read_file".to_string(), "bash".to_string()]),
            tool_manifest: Vec::new(),
            system_prompt: "system prompt".to_string(),
            skill_command_specs: Vec::new(),
        };

        assert_eq!(
            prepared.allowed_tools,
            Some(vec!["read_file".to_string(), "bash".to_string()])
        );
        assert!(prepared.tool_manifest.is_empty());
        assert_eq!(prepared.system_prompt, "system prompt");
        assert!(prepared.skill_command_specs.is_empty());
    }
}
