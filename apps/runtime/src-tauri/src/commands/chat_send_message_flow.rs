use super::chat::{StreamToken, ToolConfirmResponder};
use super::chat_policy::{
    classify_model_route_error, model_route_error_kind_key, parse_permission_mode_for_runtime,
    retry_backoff_ms, retry_budget_for_error, should_retry_same_candidate,
};
use super::chat_repo::{PoolChatEmployeeDirectory, PoolChatSettingsRepository};
use super::chat_route_execution::{self, RouteExecutionOutcome, RouteExecutionParams};
use super::chat_runtime_io as chat_io;
use super::chat_tool_setup::{self, PreparedRuntimeTools, ToolSetupParams};
use crate::agent::permissions::PermissionMode;
use crate::agent::AgentExecutor;
use crate::session_journal::SessionJournalStore;
use runtime_chat_app::{ChatExecutionPreparationRequest, ChatExecutionPreparationService};
use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub(crate) struct PreparedSendMessageContext {
    pub requested_capability: String,
    pub route_candidates: Vec<(String, String, String, String)>,
    pub per_candidate_retry_count: usize,
    pub messages: Vec<Value>,
    pub prepared_runtime_tools: PreparedRuntimeTools,
    pub permission_mode: PermissionMode,
    pub executor_work_dir: Option<String>,
    pub max_iterations: Option<usize>,
    pub node_timeout_seconds: u64,
    pub route_retry_count: usize,
}

pub(crate) struct PrepareSendMessageParams<'a> {
    pub app: &'a AppHandle,
    pub db: &'a sqlx::SqlitePool,
    pub agent_executor: &'a Arc<AgentExecutor>,
    pub session_id: &'a str,
    pub user_message: &'a str,
}

pub(crate) async fn prepare_send_message_context(
    params: PrepareSendMessageParams<'_>,
) -> Result<PreparedSendMessageContext, String> {
    let (skill_id, model_id, perm_str, work_dir, session_employee_id) =
        chat_io::load_session_runtime_inputs_with_pool(params.db, params.session_id).await?;

    let chat_repo = PoolChatSettingsRepository::new(params.db);
    let execution_request = ChatExecutionPreparationRequest {
        user_message: params.user_message.to_string(),
        session_id: Some(params.session_id.to_string()),
        permission_mode: Some(perm_str.clone()),
        session_mode: None,
        team_id: None,
        employee_id: Some(session_employee_id.clone()),
        requested_capability: None,
        work_dir: Some(work_dir.clone()),
        imported_mcp_server_ids: Vec::new(),
    };
    let employee_directory = PoolChatEmployeeDirectory::new(params.db);
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

    let (manifest_json, username, pack_path, source_type) =
        chat_io::load_installed_skill_source_with_pool(params.db, &skill_id).await?;
    let raw_prompt = chat_io::load_skill_prompt(
        &skill_id,
        &manifest_json,
        &username,
        &pack_path,
        &source_type,
    )?;
    let history = chat_io::load_session_history_with_pool(params.db, params.session_id).await?;

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

    route_candidates.dedup();
    eprintln!(
        "[routing] capability={}, candidates={}, retry_per_candidate={}",
        requested_capability,
        route_candidates.len(),
        per_candidate_retry_count
    );

    let (api_format, base_url, model_name, api_key) = route_candidates[0].clone();
    let messages = chat_io::reconstruct_history_messages(&history, &api_format);
    let skill_config = crate::agent::skill_config::SkillConfig::parse(&raw_prompt);
    let max_iter = skill_config.max_iterations.unwrap_or(10);

    let prepared_runtime_tools = chat_tool_setup::prepare_runtime_tools(ToolSetupParams {
        app: params.app,
        db: params.db,
        agent_executor: params.agent_executor,
        session_id: params.session_id,
        api_format: &api_format,
        base_url: &base_url,
        model_name: &model_name,
        api_key: &api_key,
        skill_id: &skill_id,
        source_type: &source_type,
        pack_path: &pack_path,
        skill_system_prompt: &skill_config.system_prompt,
        skill_allowed_tools: skill_config.allowed_tools.clone(),
        max_iter,
        max_call_depth: chat_preparation.max_call_depth,
        execution_preparation_service: &execution_preparation_service,
        execution_guidance: &execution_guidance,
        memory_bucket_employee_id: execution_preparation_service
            .resolve_memory_bucket_employee_id(&execution_context),
        employee_collaboration_guidance: employee_collaboration_guidance.as_deref(),
    })
    .await?;

    Ok(PreparedSendMessageContext {
        requested_capability,
        route_candidates,
        per_candidate_retry_count,
        messages,
        prepared_runtime_tools,
        permission_mode,
        executor_work_dir: execution_preparation_service
            .resolve_executor_work_dir(&execution_guidance),
        max_iterations: skill_config.max_iterations,
        node_timeout_seconds: chat_preparation.node_timeout_seconds,
        route_retry_count: chat_preparation.retry_count,
    })
}

pub(crate) async fn finalize_send_message_execution(
    app: &AppHandle,
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    route_execution: RouteExecutionOutcome,
    reconstructed_history_len: usize,
) -> Result<(), String> {
    let final_messages = match route_execution.final_messages {
        Some(messages) => messages,
        None => {
            let partial_text = route_execution.partial_text;
            if !partial_text.is_empty() {
                chat_io::append_partial_assistant_chunk_with_pool(
                    db,
                    journal,
                    session_id,
                    run_id,
                    &partial_text,
                )
                .await;
            }

            let error_message = route_execution
                .last_error
                .unwrap_or_else(|| "所有候选模型执行失败".to_string());
            let error_kind = route_execution
                .last_error_kind
                .unwrap_or_else(|| "unknown".to_string());
            chat_io::append_run_failed_with_pool(
                db,
                journal,
                session_id,
                run_id,
                &error_kind,
                &error_message,
            )
            .await;
            let _ = app.emit(
                "stream-token",
                StreamToken {
                    session_id: session_id.to_string(),
                    token: String::new(),
                    done: true,
                    sub_agent: false,
                },
            );
            return Err(error_message);
        }
    };

    let (final_text, has_tool_calls, content) =
        chat_io::build_assistant_content_from_final_messages(
            &final_messages,
            reconstructed_history_len,
        );

    let finalize_result = chat_io::finalize_run_success_with_pool(
        db,
        journal,
        session_id,
        run_id,
        &final_text,
        has_tool_calls,
        &content,
    )
    .await;

    if let Err(err) = finalize_result {
        chat_io::append_run_failed_with_pool(db, journal, session_id, run_id, "persistence", &err)
            .await;
        let _ = app.emit(
            "stream-token",
            StreamToken {
                session_id: session_id.to_string(),
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
            session_id: session_id.to_string(),
            token: String::new(),
            done: true,
            sub_agent: false,
        },
    );

    Ok(())
}

pub(crate) async fn execute_send_message_route(
    app: &AppHandle,
    agent_executor: &AgentExecutor,
    db: &sqlx::SqlitePool,
    session_id: &str,
    prepared_context: &PreparedSendMessageContext,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
    tool_confirm_responder: ToolConfirmResponder,
) -> RouteExecutionOutcome {
    chat_route_execution::execute_route_candidates(RouteExecutionParams {
        app,
        agent_executor,
        db,
        session_id,
        requested_capability: &prepared_context.requested_capability,
        route_candidates: &prepared_context.route_candidates,
        per_candidate_retry_count: prepared_context.per_candidate_retry_count,
        system_prompt: &prepared_context.prepared_runtime_tools.system_prompt,
        messages: &prepared_context.messages,
        allowed_tools: prepared_context.prepared_runtime_tools.allowed_tools.as_deref(),
        permission_mode: prepared_context.permission_mode,
        tool_confirm_responder,
        executor_work_dir: prepared_context.executor_work_dir.clone(),
        max_iterations: prepared_context.max_iterations,
        cancel_flag,
        node_timeout_seconds: prepared_context.node_timeout_seconds,
        route_retry_count: prepared_context.route_retry_count,
        classify_error: classify_model_route_error,
        error_kind_key: model_route_error_kind_key,
        should_retry_same_candidate,
        retry_budget_for_error,
        retry_backoff_ms,
    })
    .await
}
