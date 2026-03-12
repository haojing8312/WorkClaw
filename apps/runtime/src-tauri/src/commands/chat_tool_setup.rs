use super::chat::{AskUserState, SearchCacheState};
use super::chat_runtime_io as chat_io;
use crate::agent::tools::search_providers::create_provider;
use crate::agent::tools::{
    browser_tools::register_browser_tools, AskUserTool, BashKillTool, BashOutputTool, BashTool,
    ClawhubRecommendTool, ClawhubSearchTool, CompactTool, EmployeeManageTool,
    GithubRepoDownloadTool, MemoryTool, ProcessManager, SkillInvokeTool, TaskTool, WebSearchTool,
};
use crate::agent::AgentExecutor;
use runtime_chat_app::{compose_system_prompt, ChatExecutionGuidance, ChatExecutionPreparationService};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub(crate) struct PreparedRuntimeTools {
    pub allowed_tools: Option<Vec<String>>,
    pub system_prompt: String,
}

pub(crate) struct ToolSetupParams<'a> {
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
    pub skill_system_prompt: &'a str,
    pub skill_allowed_tools: Option<Vec<String>>,
    pub max_iter: usize,
    pub max_call_depth: usize,
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

    register_browser_tools(params.agent_executor.registry(), "http://localhost:8765");

    let task_tool = TaskTool::new(
        params.agent_executor.registry_arc(),
        params.api_format.to_string(),
        params.base_url.to_string(),
        params.api_key.to_string(),
        params.model_name.to_string(),
    )
    .with_app_handle(params.app.clone(), params.session_id.to_string());
    params.agent_executor.registry().register(Arc::new(task_tool));
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
                params.agent_executor.registry().register(Arc::new(web_search));
            }
            Err(e) => {
                eprintln!("[search] 创建搜索 Provider 失败: {}", e);
            }
        }
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
    params.agent_executor.registry().register(Arc::new(skill_tool));

    params
        .agent_executor
        .registry()
        .register(Arc::new(CompactTool::new()));

    let ask_user_responder = params.app.state::<AskUserState>().0.clone();
    let ask_user_tool =
        AskUserTool::new(params.app.clone(), params.session_id.to_string(), ask_user_responder);
    params.agent_executor.registry().register(Arc::new(ask_user_tool));

    let tool_names = chat_io::resolve_tool_names(&params.skill_allowed_tools, params.agent_executor);
    let imported_external_mcp_guidance = params
        .execution_preparation_service
        .resolve_imported_mcp_guidance(params.execution_guidance)
        .map(str::to_string);
    let memory_content = chat_io::load_memory_content(&memory_dir);
    let system_prompt = compose_system_prompt(
        params.skill_system_prompt,
        &tool_names,
        params.model_name,
        params.max_iter,
        params.execution_guidance,
        params.employee_collaboration_guidance,
        imported_external_mcp_guidance.as_deref(),
        Some(&memory_content),
    );

    Ok(PreparedRuntimeTools {
        allowed_tools: params.skill_allowed_tools,
        system_prompt,
    })
}
