use super::events::{StreamToken, ToolConfirmResponder};
use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::{RunBudgetPolicy, RunBudgetScope};
use crate::agent::AgentExecutor;
use crate::agent::runtime::attempt_runner::{
    execute_route_candidates, RouteExecutionOutcome, RouteExecutionParams,
};
use crate::agent::runtime::repo::{PoolChatEmployeeDirectory, PoolChatSettingsRepository};
use crate::agent::runtime::tool_setup::{
    prepare_runtime_tools, PreparedRuntimeTools, ToolSetupParams,
};
use crate::agent::runtime::RuntimeTranscript;
use crate::session_journal::SessionJournalStore;
use runtime_chat_app::{
    ChatExecutionPreparationRequest, ChatExecutionPreparationService,
};
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use super::runtime_io as chat_io;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SessionRuntime;

#[derive(Clone)]
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

#[derive(Clone)]
pub(crate) struct PrepareSendMessageParams<'a> {
    pub app: &'a AppHandle,
    pub db: &'a sqlx::SqlitePool,
    pub agent_executor: &'a Arc<AgentExecutor>,
    pub session_id: &'a str,
    pub user_message: &'a str,
    pub user_message_parts: &'a [Value],
    pub max_iterations_override: Option<usize>,
}

impl SessionRuntime {
    pub fn new() -> Self {
        Self
    }

    fn parse_permission_mode_for_runtime(permission_mode: &str) -> PermissionMode {
        match permission_mode {
            "standard" | "default" | "accept_edits" => PermissionMode::AcceptEdits,
            "full_access" | "unrestricted" => PermissionMode::Unrestricted,
            _ => PermissionMode::AcceptEdits,
        }
    }

    pub(crate) async fn run_send_message(
        app: &AppHandle,
        agent_executor: &Arc<AgentExecutor>,
        db: &sqlx::SqlitePool,
        journal: &SessionJournalStore,
        session_id: &str,
        user_message_id: &str,
        user_message: &str,
        user_message_parts: &[Value],
        max_iterations_override: Option<usize>,
        cancel_flag: Arc<AtomicBool>,
        tool_confirm_responder: ToolConfirmResponder,
    ) -> Result<(), String> {
        let prepared_context = Self::prepare_send_message_context(PrepareSendMessageParams {
            app,
            db,
            agent_executor,
            session_id,
            user_message,
            user_message_parts,
            max_iterations_override,
        })
        .await?;

        let run_id = Uuid::new_v4().to_string();
        chat_io::append_run_started_with_pool(db, journal, session_id, &run_id, user_message_id)
            .await?;

        let route_execution = execute_route_candidates(RouteExecutionParams {
            app,
            agent_executor: agent_executor.as_ref(),
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
        })
        .await;

        Self::finalize_send_message_execution(
            app,
            db,
            journal,
            session_id,
            &run_id,
            route_execution,
            prepared_context.messages.len(),
        )
        .await
    }

    pub(crate) async fn prepare_send_message_context(
        params: PrepareSendMessageParams<'_>,
    ) -> Result<PreparedSendMessageContext, String> {
        let (skill_id, model_id, perm_str, work_dir, session_employee_id) =
            chat_io::load_session_runtime_inputs_with_pool(params.db, params.session_id).await?;

        let chat_repo = PoolChatSettingsRepository::new(params.db);
        let execution_request = ChatExecutionPreparationRequest {
            user_message: params.user_message.to_string(),
            user_message_parts: Some(params.user_message_parts.to_vec()),
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
            Self::parse_permission_mode_for_runtime(&chat_preparation.permission_mode_storage);

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
            if requested_capability == "vision" {
                return Err("VISION_MODEL_NOT_CONFIGURED: 请先在设置中配置图片理解模型".to_string());
            }
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
        let mut messages = RuntimeTranscript::sanitize_reconstructed_messages(
            RuntimeTranscript::reconstruct_history_messages(&history, &api_format),
            &api_format,
        );
        if let Some(current_turn) =
            RuntimeTranscript::build_current_turn_message(&api_format, params.user_message_parts)
        {
            if let Some(existing) = messages
                .iter_mut()
                .rev()
                .find(|message| message["role"].as_str() == Some("user"))
            {
                *existing = current_turn;
            } else {
                messages.push(current_turn);
            }
        }
        let skill_config = crate::agent::skill_config::SkillConfig::parse(&raw_prompt);
        let budget_scope = if skill_id.trim().eq_ignore_ascii_case("builtin-general") {
            RunBudgetScope::GeneralChat
        } else {
            RunBudgetScope::Skill
        };
        let default_max_iter =
            RunBudgetPolicy::resolve(budget_scope, skill_config.max_iterations).max_turns;
        let max_iter = params
            .max_iterations_override
            .map(|override_value| override_value.max(1))
            .unwrap_or(default_max_iter);

        let prepared_runtime_tools = prepare_runtime_tools(ToolSetupParams {
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
            max_iterations: Some(max_iter),
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
                if let Some(stop_reason) = route_execution.last_stop_reason.as_ref() {
                    let _ = chat_io::append_run_stopped_with_pool(
                        db,
                        journal,
                        session_id,
                        run_id,
                        stop_reason,
                    )
                    .await;
                } else {
                    chat_io::append_run_failed_with_pool(
                        db,
                        journal,
                        session_id,
                        run_id,
                        &error_kind,
                        &error_message,
                    )
                    .await;
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
                return Err(error_message);
            }
        };

        let (final_text, has_tool_calls, content) = crate::agent::runtime::RuntimeTranscript::build_assistant_content_from_final_messages(
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
            &route_execution.reasoning_text,
            route_execution.reasoning_duration_ms,
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

}
