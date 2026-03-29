use super::events::{StreamToken, ToolConfirmResponder};
use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::{parse_run_stop_reason, RunBudgetPolicy, RunBudgetScope};
use crate::agent::runtime::attempt_runner::{
    execute_route_candidates, RouteExecutionOutcome, RouteExecutionParams,
};
use crate::agent::runtime::repo::{PoolChatEmployeeDirectory, PoolChatSettingsRepository};
use crate::agent::context::build_tool_context;
use crate::agent::runtime::tool_setup::{
    prepare_runtime_tools, PreparedRuntimeTools, ToolSetupParams,
};
use crate::agent::runtime::RuntimeTranscript;
use crate::agent::AgentExecutor;
use crate::session_journal::SessionJournalStore;
use runtime_chat_app::{ChatExecutionPreparationRequest, ChatExecutionPreparationService};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use super::runtime_io as chat_io;
use crate::model_transport::{resolve_model_transport, ModelTransportKind};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SessionRuntime;

#[derive(Clone)]
pub(crate) struct PreparedSendMessageContext {
    pub requested_capability: String,
    pub route_candidates: Vec<(String, String, String, String, String)>,
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

    fn parse_user_skill_command(user_message: &str) -> Option<(String, String)> {
        let trimmed = user_message.trim();
        let without_slash = trimmed.strip_prefix('/')?;
        let command = without_slash
            .split_whitespace()
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())?;
        let args = without_slash[command.len()..].trim_start().to_string();
        Some((command.to_ascii_lowercase(), args))
    }

    fn rewrite_user_skill_command_for_model(
        user_message: &str,
        skill_command_specs: &[chat_io::WorkspaceSkillCommandSpec],
    ) -> Option<String> {
        let (command_name, raw_args) = Self::parse_user_skill_command(user_message)?;
        let spec = skill_command_specs
            .iter()
            .find(|spec| spec.name.eq_ignore_ascii_case(&command_name) && spec.dispatch.is_none())?;

        let mut parts = vec![format!(
            "Use the \"{}\" skill for this request.",
            spec.skill_name
        )];
        if !raw_args.trim().is_empty() {
            parts.push(format!("User input:\n{}", raw_args.trim()));
        }
        Some(parts.join("\n\n"))
    }

    fn append_current_turn_message(messages: &mut Vec<Value>, current_turn: Value) {
        messages.push(current_turn);
    }

    async fn maybe_execute_user_skill_command(
        app: &AppHandle,
        agent_executor: &Arc<AgentExecutor>,
        session_id: &str,
        run_id: &str,
        user_message: &str,
        prepared_context: &PreparedSendMessageContext,
        cancel_flag: Arc<AtomicBool>,
        tool_confirm_responder: ToolConfirmResponder,
    ) -> Result<Option<String>, String> {
        let Some((command_name, raw_args)) = Self::parse_user_skill_command(user_message) else {
            return Ok(None);
        };
        let Some(spec) = prepared_context
            .prepared_runtime_tools
            .skill_command_specs
            .iter()
            .find(|spec| spec.name.eq_ignore_ascii_case(&command_name) && spec.dispatch.is_some())
        else {
            return Ok(None);
        };

        let tool_ctx = build_tool_context(
            Some(session_id),
            prepared_context
                .executor_work_dir
                .as_ref()
                .map(PathBuf::from),
            prepared_context.prepared_runtime_tools.allowed_tools.as_deref(),
        )
        .map_err(|err| err.to_string())?;
        let dispatch_context = crate::agent::runtime::tool_dispatch::ToolDispatchContext {
            registry: agent_executor.registry(),
            app_handle: Some(app),
            session_id: Some(session_id),
            persisted_run_id: Some(run_id),
            allowed_tools: prepared_context.prepared_runtime_tools.allowed_tools.as_deref(),
            permission_mode: prepared_context.permission_mode,
            tool_ctx: &tool_ctx,
            tool_confirm_tx: Some(&tool_confirm_responder),
            cancel_flag: Some(cancel_flag),
            route_run_id: run_id,
            route_node_timeout_secs: prepared_context.node_timeout_seconds,
            route_retry_count: 0,
            iteration: 1,
            run_budget_policy: RunBudgetPolicy::for_scope(RunBudgetScope::Skill),
        };

        crate::agent::runtime::tool_dispatch::dispatch_skill_command(
            &dispatch_context,
            spec,
            &raw_args,
        )
        .await
        .map(Some)
        .map_err(|err| err.to_string())
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

        match Self::maybe_execute_user_skill_command(
            app,
            agent_executor,
            session_id,
            &run_id,
            user_message,
            &prepared_context,
            cancel_flag.clone(),
            tool_confirm_responder.clone(),
        )
        .await
        {
            Ok(Some(output)) => {
                chat_io::finalize_run_success_with_pool(
                    db,
                    journal,
                    session_id,
                    &run_id,
                    &output,
                    false,
                    &output,
                    "",
                    None,
                )
                .await?;
                let _ = app.emit(
                    "stream-token",
                    StreamToken {
                        session_id: session_id.to_string(),
                        token: output,
                        done: false,
                        sub_agent: false,
                    },
                );
                let _ = app.emit(
                    "stream-token",
                    StreamToken {
                        session_id: session_id.to_string(),
                        token: String::new(),
                        done: true,
                        sub_agent: false,
                    },
                );
                return Ok(());
            }
            Ok(None) => {}
            Err(error) => {
                if let Some(stop_reason) = parse_run_stop_reason(&error) {
                    let _ = chat_io::append_run_stopped_with_pool(
                        db,
                        journal,
                        session_id,
                        &run_id,
                        &stop_reason,
                    )
                    .await;
                } else {
                    chat_io::append_run_failed_with_pool(
                        db,
                        journal,
                        session_id,
                        &run_id,
                        "skill_command_dispatch",
                        &error,
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
                return Err(error);
            }
        }

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
            allowed_tools: prepared_context
                .prepared_runtime_tools
                .allowed_tools
                .as_deref(),
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
        let mut route_candidates: Vec<(String, String, String, String, String)> = prepared_routes
            .candidates
            .into_iter()
            .map(|candidate| {
                let transport = resolve_model_transport(
                    &candidate.protocol_type,
                    &candidate.base_url,
                    Some(candidate.provider_key.as_str()).filter(|value| !value.trim().is_empty()),
                );
                let effective_api_format = if candidate.protocol_type.trim().is_empty() {
                    match transport.kind {
                        ModelTransportKind::AnthropicMessages => "anthropic".to_string(),
                        ModelTransportKind::OpenAiCompletions
                        | ModelTransportKind::OpenAiResponses => "openai".to_string(),
                    }
                } else {
                    candidate.protocol_type.clone()
                };
                (
                    candidate.provider_key,
                    effective_api_format,
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

        let (_, api_format, base_url, model_name, api_key) = route_candidates[0].clone();
        let mut messages = RuntimeTranscript::sanitize_reconstructed_messages(
            RuntimeTranscript::reconstruct_history_messages(&history, &api_format),
            &api_format,
        );
        if let Some(current_turn) =
            RuntimeTranscript::build_current_turn_message(&api_format, params.user_message_parts)
        {
            Self::append_current_turn_message(&mut messages, current_turn);
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

        if let Some(rewritten_body) =
            Self::rewrite_user_skill_command_for_model(params.user_message, &prepared_runtime_tools.skill_command_specs)
        {
            let rewritten_parts = vec![serde_json::json!({
                "type": "text",
                "text": rewritten_body,
            })];
            if let Some(current_turn) =
                RuntimeTranscript::build_current_turn_message(&api_format, &rewritten_parts)
            {
                let _ = messages.pop();
                Self::append_current_turn_message(&mut messages, current_turn);
            }
        }

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
                let _ = chat_io::persist_partial_assistant_message_for_run_with_pool(
                    db,
                    session_id,
                    run_id,
                    &partial_text,
                )
                .await;

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

        let (final_text, has_tool_calls, content) =
            crate::agent::runtime::RuntimeTranscript::build_assistant_content_from_final_messages(
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
            chat_io::append_run_failed_with_pool(
                db,
                journal,
                session_id,
                run_id,
                "persistence",
                &err,
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

#[cfg(test)]
mod tests {
    use super::SessionRuntime;
    use crate::agent::runtime::runtime_io as chat_io;
    use serde_json::json;

    #[test]
    fn parse_user_skill_command_extracts_command_and_raw_args() {
        let parsed = SessionRuntime::parse_user_skill_command("  /pm_summary  --employee xt --date 2026-03-27 ");
        assert_eq!(
            parsed,
            Some((
                "pm_summary".to_string(),
                "--employee xt --date 2026-03-27".to_string(),
            ))
        );
    }

    #[test]
    fn parse_user_skill_command_ignores_non_command_messages() {
        assert_eq!(SessionRuntime::parse_user_skill_command("pm_summary"), None);
        assert_eq!(SessionRuntime::parse_user_skill_command("/"), None);
    }

    #[test]
    fn rewrite_user_skill_command_for_model_rewrites_prompt_following_skill_commands() {
        let specs = vec![chat_io::WorkspaceSkillCommandSpec {
            name: "pm_summary".to_string(),
            skill_id: "skill-1".to_string(),
            skill_name: "PM Summary".to_string(),
            description: "Summarize PM updates".to_string(),
            dispatch: None,
        }];

        let rewritten = SessionRuntime::rewrite_user_skill_command_for_model(
            "/pm_summary --employee xt --date 2026-03-27",
            &specs,
        );

        assert_eq!(
            rewritten.as_deref(),
            Some(
                "Use the \"PM Summary\" skill for this request.\n\nUser input:\n--employee xt --date 2026-03-27"
            )
        );
    }

    #[test]
    fn rewrite_user_skill_command_for_model_ignores_dispatchable_commands() {
        let specs = vec![chat_io::WorkspaceSkillCommandSpec {
            name: "pm_summary".to_string(),
            skill_id: "skill-1".to_string(),
            skill_name: "PM Summary".to_string(),
            description: "Summarize PM updates".to_string(),
            dispatch: Some(runtime_skill_core::SkillCommandDispatchSpec {
                kind: runtime_skill_core::SkillCommandDispatchKind::Tool,
                tool_name: "exec".to_string(),
                arg_mode: runtime_skill_core::SkillCommandArgMode::Raw,
            }),
        }];

        assert_eq!(
            SessionRuntime::rewrite_user_skill_command_for_model("/pm_summary --employee xt", &specs),
            None
        );
    }

    #[test]
    fn append_current_turn_message_keeps_previous_user_turns() {
        let mut messages = vec![
            json!({"role": "user", "content": "你是谁"}),
            json!({"role": "assistant", "content": "我是 WorkClaw 助手"}),
        ];

        SessionRuntime::append_current_turn_message(
            &mut messages,
            json!({"role": "user", "content": "你能做什么"}),
        );

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0]["role"].as_str(), Some("user"));
        assert_eq!(messages[0]["content"].as_str(), Some("你是谁"));
        assert_eq!(messages[2]["role"].as_str(), Some("user"));
        assert_eq!(messages[2]["content"].as_str(), Some("你能做什么"));
    }
}
