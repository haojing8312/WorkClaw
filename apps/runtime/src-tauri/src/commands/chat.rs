use super::chat_repo::{PoolChatEmployeeDirectory, PoolChatSettingsRepository};
use super::chat_runtime_io as chat_io;
use super::runtime_preferences::resolve_default_work_dir_with_pool;
use super::skills::DbState;
use crate::agent::compactor;
use crate::agent::permissions::PermissionMode;
use crate::agent::tools::search_providers::cache::SearchCache;
use crate::agent::tools::{
    browser_tools::register_browser_tools, AskUserResponder, AskUserTool, BashKillTool,
    BashOutputTool, BashTool, ClawhubRecommendTool, ClawhubSearchTool, CompactTool,
    EmployeeManageTool, GithubRepoDownloadTool, MemoryTool, ProcessManager, SkillInvokeTool,
    TaskTool, WebSearchTool,
};
use crate::agent::AgentExecutor;
use crate::session_journal::{
    SessionJournalState, SessionJournalStateHandle, SessionJournalStore, SessionRunStatus,
};
use chrono::Utc;
use runtime_chat_app::{
    compose_system_prompt, ChatExecutionPreparationRequest, ChatExecutionPreparationService,
    ChatPreparationService, SessionCreationRequest,
};
use runtime_executor_core::estimate_tokens;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

/// 全局 AskUser 响应通道（用于 answer_user_question command）
pub struct AskUserState(pub AskUserResponder);

/// 工具确认通道（用于 confirm_tool_execution command）
pub type ToolConfirmResponder =
    std::sync::Arc<std::sync::Mutex<Option<std::sync::mpsc::Sender<bool>>>>;
pub struct ToolConfirmState(pub ToolConfirmResponder);

/// 全局搜索缓存（跨会话共享，在 lib.rs 中创建）
pub struct SearchCacheState(pub Arc<SearchCache>);

/// Agent 取消标志（用于 cancel_agent command 停止正在执行的 Agent）
pub struct CancelFlagState(pub Arc<AtomicBool>);

#[cfg(test)]
fn normalize_permission_mode_for_storage(permission_mode: Option<&str>) -> &'static str {
    match permission_mode.unwrap_or("").trim() {
        "standard" | "default" | "accept_edits" => "standard",
        "full_access" | "unrestricted" => "full_access",
        _ => "standard",
    }
}

#[cfg(test)]
fn normalize_session_mode_for_storage(session_mode: Option<&str>) -> &'static str {
    match session_mode.unwrap_or("").trim() {
        "employee_direct" => "employee_direct",
        "team_entry" => "team_entry",
        "general" => "general",
        _ => "general",
    }
}

#[cfg(test)]
fn normalize_team_id_for_storage(session_mode: &str, team_id: Option<&str>) -> String {
    if session_mode == "team_entry" {
        team_id.unwrap_or("").trim().to_string()
    } else {
        String::new()
    }
}

fn parse_permission_mode_for_runtime(permission_mode: &str) -> PermissionMode {
    match permission_mode {
        "standard" | "default" | "accept_edits" => PermissionMode::AcceptEdits,
        "full_access" | "unrestricted" => PermissionMode::Unrestricted,
        _ => PermissionMode::AcceptEdits,
    }
}

fn permission_mode_label_for_display(permission_mode: &str) -> &'static str {
    match permission_mode {
        "standard" => "标准模式",
        "full_access" => "全自动模式",
        "default" => "标准模式",
        "unrestricted" => "全自动模式",
        _ => "标准模式",
    }
}

fn resolve_im_session_source(channel: Option<&str>) -> (String, String) {
    match channel.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "" => ("local".to_string(), String::new()),
        "wecom" => ("wecom".to_string(), "企业微信".to_string()),
        "feishu" => ("feishu".to_string(), "飞书".to_string()),
        other => (other.to_string(), other.to_string()),
    }
}

#[cfg(test)]
fn is_supported_protocol(protocol: &str) -> bool {
    matches!(protocol, "openai" | "anthropic")
}

#[cfg(test)]
fn infer_capability_from_user_message(message: &str) -> &'static str {
    let m = message.to_ascii_lowercase();
    if m.contains("识图")
        || m.contains("看图")
        || m.contains("图片理解")
        || m.contains("vision")
        || m.contains("analyze image")
    {
        return "vision";
    }
    if m.contains("生图")
        || m.contains("画图")
        || m.contains("生成图片")
        || m.contains("image generation")
        || m.contains("generate image")
    {
        return "image_gen";
    }
    if m.contains("语音转文字")
        || m.contains("语音识别")
        || m.contains("stt")
        || m.contains("transcribe")
        || m.contains("speech to text")
    {
        return "audio_stt";
    }
    if m.contains("文字转语音")
        || m.contains("tts")
        || m.contains("text to speech")
        || m.contains("语音合成")
    {
        return "audio_tts";
    }
    "chat"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelRouteErrorKind {
    Billing,
    Auth,
    RateLimit,
    Timeout,
    Network,
    Unknown,
}

fn classify_model_route_error(error_message: &str) -> ModelRouteErrorKind {
    let lower = error_message.to_ascii_lowercase();
    if lower.contains("insufficient_balance")
        || lower.contains("insufficient balance")
        || lower.contains("balance too low")
        || lower.contains("account balance too low")
        || lower.contains("insufficient_quota")
        || lower.contains("insufficient quota")
        || lower.contains("billing")
        || lower.contains("payment required")
        || lower.contains("credit balance")
        || lower.contains("余额不足")
        || lower.contains("欠费")
    {
        return ModelRouteErrorKind::Billing;
    }
    if lower.contains("api key")
        || lower.contains("unauthorized")
        || lower.contains("invalid_api_key")
        || lower.contains("authentication")
        || lower.contains("permission denied")
        || lower.contains("forbidden")
    {
        return ModelRouteErrorKind::Auth;
    }
    if lower.contains("rate limit")
        || lower.contains("too many requests")
        || lower.contains("429")
        || lower.contains("quota")
    {
        return ModelRouteErrorKind::RateLimit;
    }
    if lower.contains("timeout") || lower.contains("timed out") || lower.contains("deadline") {
        return ModelRouteErrorKind::Timeout;
    }
    if lower.contains("connection")
        || lower.contains("network")
        || lower.contains("dns")
        || lower.contains("connect")
        || lower.contains("socket")
        || lower.contains("error sending request for url")
        || lower.contains("sending request for url")
    {
        return ModelRouteErrorKind::Network;
    }
    ModelRouteErrorKind::Unknown
}

fn should_retry_same_candidate(kind: ModelRouteErrorKind) -> bool {
    matches!(
        kind,
        ModelRouteErrorKind::RateLimit
            | ModelRouteErrorKind::Timeout
            | ModelRouteErrorKind::Network
    )
}

fn retry_budget_for_error(kind: ModelRouteErrorKind, configured_retry_count: usize) -> usize {
    if kind == ModelRouteErrorKind::Network {
        configured_retry_count.max(1)
    } else {
        configured_retry_count
    }
}

fn retry_backoff_ms(kind: ModelRouteErrorKind, attempt_idx: usize) -> u64 {
    let base_ms = match kind {
        ModelRouteErrorKind::RateLimit => 1200u64,
        ModelRouteErrorKind::Timeout => 700u64,
        ModelRouteErrorKind::Network => 400u64,
        _ => 0u64,
    };
    if base_ms == 0 {
        return 0;
    }
    let exp = attempt_idx.min(3) as u32;
    base_ms.saturating_mul(1u64 << exp).min(5000)
}

fn model_route_error_kind_key(kind: ModelRouteErrorKind) -> &'static str {
    match kind {
        ModelRouteErrorKind::Billing => "billing",
        ModelRouteErrorKind::Auth => "auth",
        ModelRouteErrorKind::RateLimit => "rate_limit",
        ModelRouteErrorKind::Timeout => "timeout",
        ModelRouteErrorKind::Network => "network",
        ModelRouteErrorKind::Unknown => "unknown",
    }
}

#[cfg(test)]
fn parse_fallback_chain_targets(raw: &str) -> Vec<(String, String)> {
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|item| {
            let provider_id = item.get("provider_id")?.as_str()?.to_string();
            let model = item
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            Some((provider_id, model))
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn build_group_orchestrator_report_preview(
    request: crate::agent::group_orchestrator::GroupRunRequest,
) -> String {
    let outcome = crate::agent::group_orchestrator::simulate_group_run(request);
    outcome.final_report
}

#[derive(serde::Serialize, Clone)]
pub(crate) struct StreamToken {
    pub(crate) session_id: String,
    pub(crate) token: String,
    pub(crate) done: bool,
    #[serde(default)]
    pub(crate) sub_agent: bool,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct SkillRouteEvent {
    pub session_id: String,
    pub route_run_id: String,
    pub node_id: String,
    pub parent_node_id: Option<String>,
    pub skill_name: String,
    pub depth: usize,
    pub status: String,
    pub duration_ms: Option<u64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

pub fn emit_skill_route_event(app: &AppHandle, event: SkillRouteEvent) {
    let _ = app.emit("skill-route-node-updated", event);
}

#[tauri::command]
pub async fn create_session(
    skill_id: String,
    model_id: String,
    work_dir: Option<String>,
    employee_id: Option<String>,
    title: Option<String>,
    permission_mode: Option<String>,
    session_mode: Option<String>,
    team_id: Option<String>,
    db: State<'_, DbState>,
) -> Result<String, String> {
    create_session_with_pool(
        &db.0,
        skill_id,
        model_id,
        work_dir,
        employee_id,
        title,
        permission_mode,
        session_mode,
        team_id,
    )
    .await
}

pub async fn create_session_with_pool(
    pool: &sqlx::SqlitePool,
    skill_id: String,
    model_id: String,
    work_dir: Option<String>,
    employee_id: Option<String>,
    title: Option<String>,
    permission_mode: Option<String>,
    session_mode: Option<String>,
    team_id: Option<String>,
) -> Result<String, String> {
    let session_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let prepared = ChatPreparationService::new().prepare_session_creation(SessionCreationRequest {
        permission_mode,
        session_mode,
        team_id,
        title,
        work_dir,
        employee_id,
    });
    let resolved_work_dir = if prepared.normalized_work_dir.is_empty() {
        resolve_default_work_dir_with_pool(pool).await?
    } else {
        prepared.normalized_work_dir
    };
    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&session_id)
    .bind(&skill_id)
    .bind(&prepared.normalized_title)
    .bind(&now)
    .bind(&model_id)
    .bind(&prepared.permission_mode_storage)
    .bind(&resolved_work_dir)
    .bind(&prepared.normalized_employee_id)
    .bind(&prepared.session_mode_storage)
    .bind(&prepared.normalized_team_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(session_id)
}

#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    session_id: String,
    user_message: String,
    db: State<'_, DbState>,
    agent_executor: State<'_, Arc<AgentExecutor>>,
    journal: State<'_, SessionJournalStateHandle>,
    cancel_flag: State<'_, CancelFlagState>,
) -> Result<(), String> {
    // 重置取消标志
    cancel_flag.0.store(false, Ordering::SeqCst);
    let cancel_flag_clone = cancel_flag.0.clone();

    // 保存用户消息
    let msg_id =
        chat_io::insert_session_message_with_pool(&db.0, &session_id, "user", &user_message)
            .await?;
    chat_io::maybe_update_session_title_from_first_user_message_with_pool(
        &db.0,
        &session_id,
        &user_message,
    )
        .await?;

    if chat_io::maybe_handle_team_entry_pre_execution_with_pool(
        &app,
        &db.0,
        journal.0.as_ref(),
        &session_id,
        &msg_id,
        &user_message,
    )
    .await?
    {
        return Ok(());
    }

    // 加载会话信息（含权限模式和工作目录）
    let (skill_id, model_id, perm_str, work_dir, session_employee_id) =
        chat_io::load_session_runtime_inputs_with_pool(&db.0, &session_id).await?;

    let chat_repo = PoolChatSettingsRepository::new(&db.0);
    let execution_request = ChatExecutionPreparationRequest {
        user_message: user_message.clone(),
        session_id: Some(session_id.clone()),
        permission_mode: Some(perm_str.clone()),
        session_mode: None,
        team_id: None,
        employee_id: Some(session_employee_id.clone()),
        requested_capability: None,
        work_dir: Some(work_dir.clone()),
        imported_mcp_server_ids: Vec::new(),
    };
    let employee_directory = PoolChatEmployeeDirectory::new(&db.0);
    let execution_preparation_service = ChatExecutionPreparationService::new();
    let prepared_execution = execution_preparation_service
        .prepare_execution_with_directory(
            &chat_repo,
            &employee_directory,
            &model_id,
            &execution_request,
        )
        .await?;
    let chat_preparation = prepared_execution.chat_preparation;
    let execution_context = prepared_execution.execution_context;
    let execution_guidance = prepared_execution.execution_guidance;
    let prepared_routes = prepared_execution.route_decisions;
    let employee_collaboration_guidance = prepared_execution.employee_collaboration_guidance;
    let permission_mode =
        parse_permission_mode_for_runtime(&chat_preparation.permission_mode_storage);

    // 加载 Skill 信息（含 pack_path 和 source_type，用 COALESCE 兼容旧数据）
    let (manifest_json, username, pack_path, source_type) =
        chat_io::load_installed_skill_source_with_pool(&db.0, &skill_id).await?;

    // 根据 source_type 决定如何读取 SKILL.md 内容
    let raw_prompt = chat_io::load_skill_prompt(
        &skill_id,
        &manifest_json,
        &username,
        &pack_path,
        &source_type,
    )?;

    // 加载消息历史
    let history = chat_io::load_session_history_with_pool(&db.0, &session_id).await?;

    let per_candidate_retry_count = prepared_routes.retry_count_per_candidate;
    let mut route_candidates: Vec<(String, String, String, String)> = prepared_routes
        .candidates
        .into_iter()
        .map(|candidate| {
            (
                candidate.protocol_type,
                candidate.base_url,
                candidate.model_name,
                candidate.api_key,
            )
        })
        .collect();
    let requested_capability = chat_preparation.capability.clone();

    if route_candidates.is_empty() {
        return Err(format!(
            "模型 API Key 为空，请在设置中重新配置 (model_id={model_id})"
        ));
    }

    // 去重，避免 fallback 与会话配置重复
    route_candidates.dedup();
    eprintln!(
        "[routing] capability={}, candidates={}, retry_per_candidate={}",
        requested_capability,
        route_candidates.len(),
        per_candidate_retry_count
    );

    // 当前回合默认使用首个候选的 api_format 做消息重建
    let (api_format, base_url, model_name, api_key) = route_candidates[0].clone();

    // 重建 LLM 历史消息：将 JSON 包装的 assistant content 还原为 tool_use/tool_result 消息对
    let messages = chat_io::reconstruct_history_messages(&history, &api_format);

    // 解析 Skill 元数据（frontmatter + system prompt）
    let skill_config = crate::agent::skill_config::SkillConfig::parse(&raw_prompt);

    // 确定工具白名单
    let allowed_tools = skill_config.allowed_tools.clone();

    let max_iter = skill_config.max_iterations.unwrap_or(10);

    // 动态注册运行时工具（在计算 tool_names 之前完成，确保列表完整）

    // L3: 注册后台进程管理工具
    let process_manager = Arc::new(ProcessManager::new());
    agent_executor
        .registry()
        .register(Arc::new(BashOutputTool::new(Arc::clone(&process_manager))));
    agent_executor
        .registry()
        .register(Arc::new(BashKillTool::new(Arc::clone(&process_manager))));
    // 替换默认 bash 工具为支持后台模式的版本
    agent_executor.registry().unregister("bash");
    agent_executor
        .registry()
        .register(Arc::new(BashTool::with_process_manager(Arc::clone(
            &process_manager,
        ))));

    // L4: 注册浏览器自动化工具（通过 Sidecar 桥接）
    register_browser_tools(agent_executor.registry(), "http://localhost:8765");

    let task_tool = TaskTool::new(
        agent_executor.registry_arc(),
        api_format.clone(),
        base_url.clone(),
        api_key.clone(),
        model_name.clone(),
    )
    .with_app_handle(app.clone(), session_id.clone());
    agent_executor.registry().register(Arc::new(task_tool));
    agent_executor
        .registry()
        .register(Arc::new(ClawhubSearchTool));
    agent_executor
        .registry()
        .register(Arc::new(ClawhubRecommendTool));
    agent_executor
        .registry()
        .register(Arc::new(GithubRepoDownloadTool::new()));
    agent_executor
        .registry()
        .register(Arc::new(EmployeeManageTool::new(db.0.clone())));

    // 注册 WebSearch 工具（从 DB 加载搜索 Provider 配置，使用全局缓存）
    {
        use crate::agent::tools::search_providers::create_provider;

        let search_cache = app.state::<SearchCacheState>().0.clone();

        let search_config = chat_io::load_default_search_provider_config_with_pool(&db.0).await?;

        if let Some((search_api_format, search_base_url, search_api_key, search_model_name)) =
            search_config
        {
            match create_provider(
                &search_api_format,
                &search_base_url,
                &search_api_key,
                &search_model_name,
            ) {
                Ok(provider) => {
                    let web_search = WebSearchTool::with_provider(provider, search_cache);
                    agent_executor.registry().register(Arc::new(web_search));
                }
                Err(e) => {
                    eprintln!("[search] 创建搜索 Provider 失败: {}", e);
                }
            }
        }
        // 无搜索配置时不注册 web_search 工具，Agent 不调用搜索
    }

    // 注册 Memory 工具（基于 Skill ID 的持久存储）
    let app_data_dir = app.path().app_data_dir().unwrap_or_default();
    let memory_dir = chat_io::build_memory_dir_for_session(
        &app_data_dir,
        &skill_id,
        execution_preparation_service.resolve_memory_bucket_employee_id(&execution_context),
    );
    let memory_tool = MemoryTool::new(memory_dir.clone());
    agent_executor.registry().register(Arc::new(memory_tool));

    // 注册 Skill 调用工具（支持 Skill 之间按需互调）
    let skill_roots = chat_io::build_skill_roots(
        execution_preparation_service.resolve_skill_root_work_dir(&execution_guidance),
        &source_type,
        &pack_path,
    );
    let skill_tool = SkillInvokeTool::new(session_id.clone(), skill_roots)
        .with_max_depth(chat_preparation.max_call_depth);
    agent_executor.registry().register(Arc::new(skill_tool));

    // 注册 Compact 工具（手动触发上下文压缩）
    let compact_tool = CompactTool::new();
    agent_executor.registry().register(Arc::new(compact_tool));

    // 注册 AskUser 工具（使用全局响应通道，在 lib.rs 中创建）
    let ask_user_responder = app.state::<AskUserState>().0.clone();
    let ask_user_tool = AskUserTool::new(app.clone(), session_id.clone(), ask_user_responder);
    agent_executor.registry().register(Arc::new(ask_user_tool));

    // 获取工具名称列表（在所有工具注册完成后计算，确保列表完整）
    let tool_names = chat_io::resolve_tool_names(&allowed_tools, &agent_executor);

    let imported_external_mcp_guidance = execution_preparation_service
        .resolve_imported_mcp_guidance(&execution_guidance)
        .map(str::to_string);

    // 如果存在 MEMORY.md，注入到 system prompt
    let memory_content = chat_io::load_memory_content(&memory_dir);
    let system_prompt = compose_system_prompt(
        &skill_config.system_prompt,
        &tool_names,
        &model_name,
        max_iter,
        &execution_guidance,
        employee_collaboration_guidance.as_deref(),
        imported_external_mcp_guidance.as_deref(),
        Some(&memory_content),
    );

    // 使用全局工具确认通道（在 lib.rs 中创建）
    let tool_confirm_responder = app.state::<ToolConfirmState>().0.clone();
    let run_id = Uuid::new_v4().to_string();
    chat_io::append_run_started_with_pool(
        &db.0,
        journal.0.as_ref(),
        &session_id,
        &run_id,
        &msg_id,
    )
    .await?;

    // 始终走 Agent 模式；失败时按候选链重试
    let mut final_messages_opt: Option<Vec<Value>> = None;
    let mut last_error: Option<String> = None;
    let mut last_error_kind: Option<String> = None;
    let streamed_text = Arc::new(std::sync::Mutex::new(String::new()));
    for (candidate_api_format, candidate_base_url, candidate_model_name, candidate_api_key) in
        &route_candidates
    {
        let mut attempt_idx = 0usize;
        loop {
            if let Ok(mut buffer) = streamed_text.lock() {
                buffer.clear();
            }
            let app_clone = app.clone();
            let session_id_clone = session_id.clone();
            let streamed_text_clone = Arc::clone(&streamed_text);
            let attempt = agent_executor
                .execute_turn(
                    candidate_api_format,
                    candidate_base_url,
                    candidate_api_key,
                    candidate_model_name,
                    &system_prompt,
                    messages.clone(),
                    move |token: String| {
                        if let Ok(mut buffer) = streamed_text_clone.lock() {
                            buffer.push_str(&token);
                        }
                        let _ = app_clone.emit(
                            "stream-token",
                            StreamToken {
                                session_id: session_id_clone.clone(),
                                token,
                                done: false,
                                sub_agent: false,
                            },
                        );
                    },
                    Some(&app),
                    Some(&session_id),
                    allowed_tools.as_deref(),
                    permission_mode,
                    Some(tool_confirm_responder.clone()),
                    execution_preparation_service.resolve_executor_work_dir(&execution_guidance),
                    skill_config.max_iterations,
                    Some(cancel_flag_clone.clone()),
                    Some(chat_preparation.node_timeout_seconds),
                    Some(chat_preparation.retry_count),
                )
                .await;

            match attempt {
                Ok(messages_out) => {
                    chat_io::record_route_attempt_log_with_pool(
                        &db.0,
                        &session_id,
                        &requested_capability,
                        candidate_api_format,
                        candidate_model_name,
                        attempt_idx + 1,
                        attempt_idx,
                        "ok",
                        true,
                        "",
                    )
                    .await;
                    final_messages_opt = Some(messages_out);
                    break;
                }
                Err(err) => {
                    let err_text = err.to_string();
                    let kind = classify_model_route_error(&err_text);
                    let kind_text = model_route_error_kind_key(kind);
                    chat_io::record_route_attempt_log_with_pool(
                        &db.0,
                        &session_id,
                        &requested_capability,
                        candidate_api_format,
                        candidate_model_name,
                        attempt_idx + 1,
                        attempt_idx,
                        kind_text,
                        false,
                        &err_text,
                    )
                    .await;
                    last_error = Some(err_text.clone());
                    last_error_kind = Some(kind_text.to_string());
                    eprintln!(
                        "[routing] 候选模型执行失败: format={}, model={}, attempt={}, kind={:?}, err={}",
                        candidate_api_format,
                        candidate_model_name,
                        attempt_idx + 1,
                        kind,
                        err_text
                    );

                    let retry_budget = retry_budget_for_error(kind, per_candidate_retry_count);
                    if should_retry_same_candidate(kind) && attempt_idx < retry_budget {
                        let backoff_ms = retry_backoff_ms(kind, attempt_idx);
                        if backoff_ms > 0 {
                            eprintln!(
                                "[routing] 同候选重试等待: format={}, model={}, wait_ms={}, next_attempt={}",
                                candidate_api_format,
                                candidate_model_name,
                                backoff_ms,
                                attempt_idx + 2
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                        }
                        attempt_idx += 1;
                        continue;
                    }
                    break;
                }
            }
        }
        if final_messages_opt.is_some() {
            break;
        }
    }

    let final_messages = match final_messages_opt {
        Some(messages) => messages,
        None => {
            let partial_text = streamed_text
                .lock()
                .map(|buffer| buffer.clone())
                .unwrap_or_default();
            if !partial_text.is_empty() {
                chat_io::append_partial_assistant_chunk_with_pool(
                    &db.0,
                    journal.0.as_ref(),
                    &session_id,
                    &run_id,
                    &partial_text,
                )
                .await;
            }

            let error_message = last_error.unwrap_or_else(|| "所有候选模型执行失败".to_string());
            let error_kind = last_error_kind.unwrap_or_else(|| "unknown".to_string());
            chat_io::append_run_failed_with_pool(
                &db.0,
                journal.0.as_ref(),
                &session_id,
                &run_id,
                &error_kind,
                &error_message,
            )
            .await;
            let _ = app.emit(
                "stream-token",
                StreamToken {
                    session_id: session_id.clone(),
                    token: String::new(),
                    done: true,
                    sub_agent: false,
                },
            );
            return Err(error_message);
        }
    };

    let (final_text, has_tool_calls, content) =
        chat_io::build_assistant_content_from_final_messages(&final_messages, messages.len());

    let finalize_result = chat_io::finalize_run_success_with_pool(
        &db.0,
        journal.0.as_ref(),
        &session_id,
        &run_id,
        &final_text,
        has_tool_calls,
        &content,
    )
    .await;

    if let Err(err) = finalize_result {
        chat_io::append_run_failed_with_pool(
            &db.0,
            journal.0.as_ref(),
            &session_id,
            &run_id,
            "persistence",
            &err,
        )
        .await;
        let _ = app.emit(
            "stream-token",
            StreamToken {
                session_id: session_id.clone(),
                token: String::new(),
                done: true,
                sub_agent: false,
            },
        );
        return Err(err);
    }

    let _ = app.emit(
        "stream-token",
        StreamToken {
            session_id: session_id.clone(),
            token: String::new(),
            done: true,
            sub_agent: false,
        },
    );

    Ok(())
}

/// 将旧格式扁平 tool_call items 转换为前端期望的嵌套 toolCall 格式
///
/// 旧格式：`{"type":"tool_call","id":"...","name":"...","input":{...},"output":"...","status":"completed"}`
/// 新格式：`{"type":"tool_call","toolCall":{"id":"...","name":"...","input":{...},"output":"...","status":"completed"}}`
fn normalize_stream_items(items: &Value) -> Value {
    if let Some(arr) = items.as_array() {
        Value::Array(
            arr.iter()
                .map(|item| {
                    if item["type"].as_str() == Some("tool_call") && item.get("toolCall").is_none()
                    {
                        // 旧格式：扁平结构 → 包装为嵌套格式
                        json!({
                            "type": "tool_call",
                            "toolCall": {
                                "id": item["id"],
                                "name": item["name"],
                                "input": item["input"],
                                "output": item["output"],
                                "status": item["status"]
                            }
                        })
                    } else {
                        item.clone()
                    }
                })
                .collect(),
        )
    } else {
        items.clone()
    }
}

#[tauri::command]
pub async fn get_messages(
    session_id: String,
    db: State<'_, DbState>,
) -> Result<Vec<serde_json::Value>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
        "SELECT
            m.id,
            m.role,
            m.content,
            m.created_at,
            NULLIF(sr.id, '') AS run_id
         FROM messages m
         LEFT JOIN session_runs sr ON sr.assistant_message_id = m.id
         WHERE m.session_id = ?
         ORDER BY m.created_at ASC",
    )
    .bind(&session_id)
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(|(id, role, content, created_at, run_id)| {
            // 对 assistant 消息尝试解析结构化 content
            if role == "assistant" {
                if let Ok(parsed) = serde_json::from_str::<Value>(content) {
                    if let Some(text) = parsed.get("text") {
                        // 包含有序 items 列表
                        if let Some(items) = parsed.get("items") {
                            // 向后兼容：将旧格式扁平 tool_call 转换为嵌套 toolCall 格式
                            let normalized = normalize_stream_items(items);
                            return json!({
                                "id": id,
                                "role": role,
                                "content": text,
                                "created_at": created_at,
                                "runId": run_id,
                                "streamItems": normalized,
                            });
                        }
                        // 旧格式：包含 tool_calls 列表（向后兼容）
                        let tool_calls = parsed.get("tool_calls").cloned().unwrap_or(Value::Null);
                        return json!({
                            "id": id,
                            "role": role,
                            "content": text,
                            "created_at": created_at,
                            "runId": run_id,
                            "tool_calls": tool_calls,
                        });
                    }
                }
            }
            // 其他情况直接返回原始 content
            json!({
                "id": id,
                "role": role,
                "content": content,
                "created_at": created_at,
                "runId": run_id,
            })
        })
        .collect())
}

#[tauri::command]
pub async fn list_sessions(db: State<'_, DbState>) -> Result<Vec<serde_json::Value>, String> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        ),
    >(
        "SELECT
            s.id,
            s.title,
            s.created_at,
            s.model_id,
            COALESCE(s.work_dir, ''),
            COALESCE(s.employee_id, ''),
            COALESCE(s.permission_mode, 'standard'),
            COALESCE(s.session_mode, 'general'),
            COALESCE(s.team_id, ''),
            COALESCE((
                SELECT ts.channel
                FROM im_thread_sessions ts
                WHERE ts.session_id = s.id
                ORDER BY ts.updated_at DESC, ts.created_at DESC
                LIMIT 1
            ), '') AS im_source_channel
         FROM sessions s
         ORDER BY s.created_at DESC",
    )
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(
            |(
                id,
                title,
                created_at,
                model_id,
                work_dir,
                employee_id,
                permission_mode,
                session_mode,
                team_id,
                im_source_channel,
            )| {
                let (source_channel, source_label) =
                    resolve_im_session_source(Some(im_source_channel));
                json!({
                    "id": id,
                    "title": title,
                    "created_at": created_at,
                    "model_id": model_id,
                    "work_dir": work_dir,
                    "employee_id": employee_id,
                    "permission_mode": permission_mode,
                    "session_mode": session_mode,
                    "team_id": team_id,
                    "permission_mode_label": permission_mode_label_for_display(permission_mode),
                    "source_channel": source_channel,
                    "source_label": source_label,
                })
            },
        )
        .collect())
}

#[tauri::command]
pub async fn get_sessions(
    skill_id: String,
    db: State<'_, DbState>,
) -> Result<Vec<serde_json::Value>, String> {
    // 兼容旧调用，skill_id 不再参与过滤
    let _ = &skill_id;
    list_sessions(db).await
}

#[cfg(test)]
mod tests {
    use super::{
        build_group_orchestrator_report_preview, chat_runtime_io,
        classify_model_route_error, infer_capability_from_user_message,
        is_supported_protocol, normalize_permission_mode_for_storage,
        normalize_session_mode_for_storage, normalize_team_id_for_storage,
        parse_fallback_chain_targets, parse_permission_mode_for_runtime,
        permission_mode_label_for_display, retry_backoff_ms, retry_budget_for_error,
        should_retry_same_candidate, ModelRouteErrorKind,
    };
    use crate::agent::permissions::PermissionMode;
    use std::collections::HashMap;
    use std::path::Path;

    #[test]
    fn normalize_permission_mode_defaults_to_standard() {
        assert_eq!(normalize_permission_mode_for_storage(None), "standard");
        assert_eq!(normalize_permission_mode_for_storage(Some("")), "standard");
        assert_eq!(
            normalize_permission_mode_for_storage(Some("invalid")),
            "standard"
        );
    }

    #[test]
    fn normalize_permission_mode_maps_legacy_values_to_modern_storage() {
        assert_eq!(
            normalize_permission_mode_for_storage(Some("standard")),
            "standard"
        );
        assert_eq!(
            normalize_permission_mode_for_storage(Some("full_access")),
            "full_access"
        );
        assert_eq!(
            normalize_permission_mode_for_storage(Some("default")),
            "standard"
        );
        assert_eq!(
            normalize_permission_mode_for_storage(Some("accept_edits")),
            "standard"
        );
        assert_eq!(
            normalize_permission_mode_for_storage(Some("unrestricted")),
            "full_access"
        );
    }

    #[test]
    fn normalize_session_mode_defaults_to_general() {
        assert_eq!(normalize_session_mode_for_storage(None), "general");
        assert_eq!(normalize_session_mode_for_storage(Some("")), "general");
        assert_eq!(
            normalize_session_mode_for_storage(Some("invalid")),
            "general"
        );
    }

    #[test]
    fn normalize_session_mode_keeps_supported_values() {
        assert_eq!(
            normalize_session_mode_for_storage(Some("general")),
            "general"
        );
        assert_eq!(
            normalize_session_mode_for_storage(Some("employee_direct")),
            "employee_direct"
        );
        assert_eq!(
            normalize_session_mode_for_storage(Some("team_entry")),
            "team_entry"
        );
    }

    #[test]
    fn normalize_team_id_only_keeps_team_entry_values() {
        assert_eq!(
            normalize_team_id_for_storage("general", Some("group-1")),
            ""
        );
        assert_eq!(
            normalize_team_id_for_storage("employee_direct", Some("group-1")),
            ""
        );
        assert_eq!(
            normalize_team_id_for_storage("team_entry", Some(" group-1 ")),
            "group-1"
        );
    }

    #[test]
    fn parse_permission_mode_for_runtime_defaults_to_standard_behavior() {
        assert_eq!(
            parse_permission_mode_for_runtime(""),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            parse_permission_mode_for_runtime("invalid"),
            PermissionMode::AcceptEdits
        );
    }

    #[test]
    fn parse_permission_mode_for_runtime_supports_modern_and_legacy_values() {
        assert_eq!(
            parse_permission_mode_for_runtime("standard"),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            parse_permission_mode_for_runtime("full_access"),
            PermissionMode::Unrestricted
        );
        assert_eq!(
            parse_permission_mode_for_runtime("default"),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            parse_permission_mode_for_runtime("accept_edits"),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            parse_permission_mode_for_runtime("unrestricted"),
            PermissionMode::Unrestricted
        );
    }

    #[test]
    fn permission_mode_label_is_user_friendly() {
        assert_eq!(permission_mode_label_for_display("standard"), "标准模式");
        assert_eq!(
            permission_mode_label_for_display("full_access"),
            "全自动模式"
        );
        assert_eq!(
            permission_mode_label_for_display("accept_edits"),
            "标准模式"
        );
        assert_eq!(permission_mode_label_for_display("default"), "标准模式");
        assert_eq!(
            permission_mode_label_for_display("unrestricted"),
            "全自动模式"
        );
    }

    #[test]
    fn supported_protocols_are_openai_and_anthropic_only() {
        assert!(is_supported_protocol("openai"));
        assert!(is_supported_protocol("anthropic"));
        assert!(!is_supported_protocol("gemini"));
        assert!(!is_supported_protocol(""));
    }

    #[test]
    fn parse_fallback_chain_targets_handles_json_array() {
        let raw = r#"[{"provider_id":"p1","model":"m1"},{"provider_id":"p2","model":"m2"}]"#;
        let parsed = parse_fallback_chain_targets(raw);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, "p1");
        assert_eq!(parsed[0].1, "m1");
        assert_eq!(parsed[1].0, "p2");
        assert_eq!(parsed[1].1, "m2");
    }

    #[test]
    fn classify_model_route_error_detects_auth() {
        let kind = classify_model_route_error("Unauthorized: invalid_api_key");
        assert_eq!(kind, ModelRouteErrorKind::Auth);
        assert!(!should_retry_same_candidate(kind));
    }

    #[test]
    fn classify_model_route_error_detects_billing() {
        let kind = classify_model_route_error("insufficient_balance: account balance too low");
        assert_eq!(kind, ModelRouteErrorKind::Billing);
        assert_eq!(model_route_error_kind_key(kind), "billing");
        assert!(!should_retry_same_candidate(kind));
    }

    #[test]
    fn classify_model_route_error_detects_retryable_kinds() {
        let rate = classify_model_route_error("429 Too Many Requests");
        let timeout = classify_model_route_error("request timeout while calling provider");
        let network = classify_model_route_error("network connection reset");
        assert_eq!(rate, ModelRouteErrorKind::RateLimit);
        assert_eq!(timeout, ModelRouteErrorKind::Timeout);
        assert_eq!(network, ModelRouteErrorKind::Network);
        assert!(should_retry_same_candidate(rate));
        assert!(should_retry_same_candidate(timeout));
        assert!(should_retry_same_candidate(network));
    }

    #[test]
    fn classify_model_route_error_detects_transport_send_failures_as_network() {
        let kind = classify_model_route_error(
            "error sending request for url (https://api.minimax.io/anthropic/v1/messages)",
        );
        assert_eq!(kind, ModelRouteErrorKind::Network);
    }

    #[test]
    fn retry_budget_for_error_guarantees_one_retry_for_network() {
        assert_eq!(retry_budget_for_error(ModelRouteErrorKind::Network, 0), 1);
        assert_eq!(retry_budget_for_error(ModelRouteErrorKind::Network, 2), 2);
        assert_eq!(retry_budget_for_error(ModelRouteErrorKind::RateLimit, 0), 0);
    }

    #[test]
    fn retry_backoff_is_exponential_and_capped() {
        assert_eq!(retry_backoff_ms(ModelRouteErrorKind::Network, 0), 400);
        assert_eq!(retry_backoff_ms(ModelRouteErrorKind::Network, 2), 1600);
        assert_eq!(retry_backoff_ms(ModelRouteErrorKind::RateLimit, 3), 5000);
        assert_eq!(retry_backoff_ms(ModelRouteErrorKind::Unknown, 1), 0);
    }

    #[test]
    fn infer_capability_from_user_message_detects_modalities() {
        assert_eq!(infer_capability_from_user_message("请帮我识图"), "vision");
        assert_eq!(
            infer_capability_from_user_message("帮我生成图片"),
            "image_gen"
        );
        assert_eq!(
            infer_capability_from_user_message("这段音频做语音转文字"),
            "audio_stt"
        );
        assert_eq!(
            infer_capability_from_user_message("这段文案做文字转语音"),
            "audio_tts"
        );
        assert_eq!(infer_capability_from_user_message("解释这个报错"), "chat");
    }

    #[test]
    fn build_memory_dir_for_session_keeps_legacy_skill_bucket_without_employee() {
        let base = Path::new("C:/workclaw/app-data");
        let dir = chat_runtime_io::build_memory_dir_for_session(base, "builtin-general", "");
        assert_eq!(
            dir,
            Path::new("C:/workclaw/app-data")
                .join("memory")
                .join("builtin-general")
        );
    }

    #[test]
    fn build_memory_dir_for_session_isolates_by_employee_when_provided() {
        let base = Path::new("C:/workclaw/app-data");
        let dir = chat_runtime_io::build_memory_dir_for_session(
            base,
            "builtin-general",
            "Sales Lead/华东",
        );
        assert_eq!(
            dir,
            Path::new("C:/workclaw/app-data")
                .join("memory")
                .join("employees")
                .join("sales_lead")
                .join("skills")
                .join("builtin-general")
        );
    }

    #[test]
    fn extract_skill_prompt_supports_lowercase_skill_md() {
        let mut files = HashMap::new();
        files.insert("skill.md".to_string(), b"# lowercase skill".to_vec());
        let content = chat_runtime_io::extract_skill_prompt_from_decrypted_files(&files);
        assert_eq!(content.as_deref(), Some("# lowercase skill"));
    }

    #[test]
    fn group_orchestrator_preview_contains_plan_execute_summary_sections() {
        let report = build_group_orchestrator_report_preview(
            crate::agent::group_orchestrator::GroupRunRequest {
                group_id: "group-1".to_string(),
                coordinator_employee_id: "project_manager".to_string(),
                planner_employee_id: None,
                reviewer_employee_id: None,
                member_employee_ids: vec![
                    "project_manager".to_string(),
                    "dev_team".to_string(),
                    "qa_team".to_string(),
                ],
                execute_targets: Vec::new(),
                user_goal: "实现群组协作编排".to_string(),
                execution_window: 3,
                timeout_employee_ids: Vec::new(),
                max_retry_per_step: 1,
            },
        );

        assert!(report.contains("计划"));
        assert!(report.contains("执行"));
        assert!(report.contains("汇报"));
    }
}

#[tauri::command]
pub async fn update_session_workspace(
    session_id: String,
    workspace: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    sqlx::query("UPDATE sessions SET work_dir = ? WHERE id = ?")
        .bind(&workspace)
        .bind(&session_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_session(session_id: String, db: State<'_, DbState>) -> Result<(), String> {
    // 先删除该会话下的所有消息
    sqlx::query("DELETE FROM messages WHERE session_id = ?")
        .bind(&session_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    // 再删除会话本身
    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(&session_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// 搜索会话标题和消息内容
#[tauri::command]
pub async fn search_sessions_global(
    query: String,
    db: State<'_, DbState>,
) -> Result<Vec<serde_json::Value>, String> {
    let pattern = format!("%{}%", query);
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        "SELECT DISTINCT
            s.id,
            s.title,
            s.created_at,
            s.model_id,
            COALESCE(s.work_dir, ''),
            COALESCE(s.employee_id, ''),
            COALESCE((
                SELECT ts.channel
                FROM im_thread_sessions ts
                WHERE ts.session_id = s.id
                ORDER BY ts.updated_at DESC, ts.created_at DESC
                LIMIT 1
            ), '') AS im_source_channel
         FROM sessions s
         LEFT JOIN messages m ON m.session_id = s.id
         WHERE (s.title LIKE ? OR m.content LIKE ?)
         ORDER BY s.created_at DESC",
    )
    .bind(&pattern)
    .bind(&pattern)
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(
            |(id, title, created_at, model_id, work_dir, employee_id, im_source_channel)| {
                let (source_channel, source_label) =
                    resolve_im_session_source(Some(im_source_channel));
                json!({
                    "id": id,
                    "title": title,
                    "created_at": created_at,
                    "model_id": model_id,
                    "work_dir": work_dir,
                    "employee_id": employee_id,
                    "source_channel": source_channel,
                    "source_label": source_label
                })
            },
        )
        .collect())
}

#[tauri::command]
pub async fn search_sessions(
    skill_id: String,
    query: String,
    db: State<'_, DbState>,
) -> Result<Vec<serde_json::Value>, String> {
    // 兼容旧调用，skill_id 不再参与过滤
    let _ = &skill_id;
    search_sessions_global(query, db).await
}

/// 将会话消息导出为 Markdown 字符串
#[tauri::command]
pub async fn export_session(
    session_id: String,
    db: State<'_, DbState>,
    journal: State<'_, SessionJournalStateHandle>,
) -> Result<String, String> {
    export_session_markdown_with_pool(&db.0, &session_id, Some(journal.0.as_ref())).await
}

pub async fn export_session_markdown_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    journal: Option<&SessionJournalStore>,
) -> Result<String, String> {
    let (title,): (String,) = sqlx::query_as("SELECT title FROM sessions WHERE id = ?")
        .bind(session_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

    let messages = sqlx::query_as::<_, (String, String, String)>(
        "SELECT role, content, created_at FROM messages WHERE session_id = ? ORDER BY created_at ASC"
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut md = format!("# {}\n\n", title);
    for (role, content, created_at) in &messages {
        let label = if role == "user" { "用户" } else { "助手" };
        let rendered_content = render_export_message_content(role, content);
        md.push_str(&format!(
            "## {} ({})\n\n{}\n\n---\n\n",
            label, created_at, rendered_content
        ));
    }

    if let Some(journal_store) = journal {
        if let Ok(state) = journal_store.read_state(session_id).await {
            let recovered = render_recovered_run_sections(&messages, &state);
            if !recovered.is_empty() {
                md.push_str("## 恢复的运行记录\n\n");
                md.push_str(&recovered);
            }
        }
    }

    Ok(md)
}

#[cfg(test)]
mod session_source_tests {
    use super::resolve_im_session_source;

    #[test]
    fn resolve_im_session_source_maps_wecom_and_feishu_labels() {
        assert_eq!(
            resolve_im_session_source(Some("wecom")),
            ("wecom".to_string(), "企业微信".to_string())
        );
        assert_eq!(
            resolve_im_session_source(Some("feishu")),
            ("feishu".to_string(), "飞书".to_string())
        );
        assert_eq!(
            resolve_im_session_source(Some("")),
            ("local".to_string(), String::new())
        );
        assert_eq!(
            resolve_im_session_source(None),
            ("local".to_string(), String::new())
        );
    }
}

fn render_export_message_content(role: &str, content: &str) -> String {
    if role != "assistant" {
        return content.to_string();
    }

    let Ok(parsed) = serde_json::from_str::<Value>(content) else {
        return content.to_string();
    };

    let mut sections: Vec<String> = Vec::new();
    let final_text = parsed["text"].as_str().unwrap_or("").trim();
    if !final_text.is_empty() {
        sections.push(final_text.to_string());
    }

    if let Some(items) = parsed["items"].as_array() {
        let item_text = items
            .iter()
            .filter_map(|item| {
                if item["type"].as_str() == Some("text") {
                    return item["content"]
                        .as_str()
                        .map(str::trim)
                        .filter(|text| !text.is_empty())
                        .map(str::to_string);
                }
                None
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !item_text.is_empty() && !sections.iter().any(|section| section.contains(&item_text)) {
            sections.push(item_text);
        }
    }

    if sections.is_empty() {
        content.to_string()
    } else {
        sections.join("\n\n")
    }
}

fn render_recovered_run_sections(
    messages: &[(String, String, String)],
    state: &SessionJournalState,
) -> String {
    let assistant_contents: Vec<&str> = messages
        .iter()
        .filter_map(|(role, content, _)| (role == "assistant").then_some(content.as_str()))
        .collect();

    let mut sections = Vec::new();
    for run in &state.runs {
        let buffered = run.buffered_text.trim();
        let error_message = run.last_error_message.as_deref().unwrap_or("").trim();
        let buffered_already_exported = !buffered.is_empty()
            && assistant_contents
                .iter()
                .any(|content| content.contains(buffered));
        let error_already_exported = !error_message.is_empty()
            && assistant_contents
                .iter()
                .any(|content| content.contains(error_message));
        let should_recover = (!buffered.is_empty() && !buffered_already_exported)
            || (!error_message.is_empty() && !error_already_exported)
            || matches!(
                &run.status,
                SessionRunStatus::Failed | SessionRunStatus::Cancelled
            );

        if !should_recover {
            continue;
        }

        sections.push(format!(
            "### Run {} ({})",
            run.run_id,
            export_status_label(&run.status)
        ));
        sections.push(String::new());
        if !buffered.is_empty() && !buffered_already_exported {
            sections.push("#### 已保留的部分输出".to_string());
            sections.push(String::new());
            sections.push(buffered.to_string());
            sections.push(String::new());
        }
        if let Some(error_kind) = &run.last_error_kind {
            if !error_kind.trim().is_empty() {
                sections.push(format!("- error_kind: {}", error_kind));
            }
        }
        if !error_message.is_empty() && !error_already_exported {
            sections.push(format!("- error_message: {}", error_message));
        }
        sections.push("\n---\n".to_string());
    }

    sections.join("\n")
}

fn export_status_label(status: &SessionRunStatus) -> &'static str {
    match status {
        SessionRunStatus::Queued => "queued",
        SessionRunStatus::Thinking => "thinking",
        SessionRunStatus::ToolCalling => "tool_calling",
        SessionRunStatus::WaitingUser => "waiting_user",
        SessionRunStatus::Completed => "completed",
        SessionRunStatus::Failed => "failed",
        SessionRunStatus::Cancelled => "cancelled",
    }
}

/// 写入导出文件
#[tauri::command]
pub async fn write_export_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, &content).map_err(|e| format!("写入失败: {}", e))
}

/// 用户回答 AskUser 工具的问题
#[tauri::command]
pub async fn answer_user_question(
    answer: String,
    ask_user_state: State<'_, AskUserState>,
) -> Result<(), String> {
    let guard = ask_user_state
        .0
        .lock()
        .map_err(|e| format!("锁获取失败: {}", e))?;

    if let Some(sender) = guard.as_ref() {
        sender
            .send(answer)
            .map_err(|e| format!("发送响应失败: {}", e))?;
        Ok(())
    } else {
        Err("没有等待中的用户问题".to_string())
    }
}

/// 用户确认或拒绝工具执行
#[tauri::command]
pub async fn confirm_tool_execution(
    confirmed: bool,
    tool_confirm_state: State<'_, ToolConfirmState>,
) -> Result<(), String> {
    let guard = tool_confirm_state
        .0
        .lock()
        .map_err(|e| format!("锁获取失败: {}", e))?;
    if let Some(sender) = guard.as_ref() {
        sender
            .send(confirmed)
            .map_err(|e| format!("发送确认失败: {}", e))?;
        Ok(())
    } else {
        Err("没有等待中的工具确认请求".to_string())
    }
}

/// 取消正在执行的 Agent
#[tauri::command]
pub async fn cancel_agent(cancel_flag: State<'_, CancelFlagState>) -> Result<(), String> {
    cancel_flag.0.store(true, Ordering::SeqCst);
    eprintln!("[agent] 收到取消信号");
    Ok(())
}

/// 压缩结果
#[derive(serde::Serialize)]
pub struct CompactionResult {
    original_tokens: usize,
    new_tokens: usize,
    summary: String,
}

/// 手动触发上下文压缩
#[tauri::command]
pub async fn compact_context(
    session_id: String,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<CompactionResult, String> {
    // 1. 获取会话消息
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT role, content FROM messages WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(&session_id)
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    let messages: Vec<Value> = rows
        .iter()
        .map(|(role, content)| json!({ "role": role, "content": content }))
        .collect();

    // 2. 估算原始 token 数
    let original_tokens = estimate_tokens(&messages);

    // 3. 获取模型配置
    let (model_id,): (String,) = sqlx::query_as("SELECT model_id FROM sessions WHERE id = ?")
        .bind(&session_id)
        .fetch_one(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    let (api_format, base_url, api_key, model_name) =
        sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT api_format, base_url, api_key, model_name FROM model_configs WHERE id = ?",
        )
        .bind(&model_id)
        .fetch_one(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    // 4. 创建 transcript 目录
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let transcript_dir = app_data_dir.join("transcripts");
    std::fs::create_dir_all(&transcript_dir).map_err(|e| e.to_string())?;

    // 5. 保存完整记录并压缩
    let transcript_path = compactor::save_transcript(&transcript_dir, &session_id, &messages)
        .map_err(|e| e.to_string())?;

    let compacted = compactor::auto_compact(
        &api_format,
        &base_url,
        &api_key,
        &model_name,
        &messages,
        &transcript_path.to_string_lossy(),
    )
    .await
    .map_err(|e| e.to_string())?;

    // 6. 更新会话消息（删除旧消息，插入压缩后的消息）
    sqlx::query("DELETE FROM messages WHERE session_id = ?")
        .bind(&session_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    let now = Utc::now().to_rfc3339();
    for msg in &compacted {
        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(msg["role"].as_str().unwrap_or("user"))
        .bind(msg["content"].as_str().unwrap_or(""))
        .bind(&now)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;
    }

    // 7. 返回结果
    let new_tokens = estimate_tokens(&compacted);
    let summary = compacted
        .iter()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("")
        .to_string();

    Ok(CompactionResult {
        original_tokens,
        new_tokens,
        summary,
    })
}
