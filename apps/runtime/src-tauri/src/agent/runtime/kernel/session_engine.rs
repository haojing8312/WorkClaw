use super::execution_plan::{ExecutionOutcome, SessionEngineError};
use crate::agent::runtime::attempt_runner::{
    execute_route_candidates, RouteExecutionParams,
};
use crate::agent::runtime::events::ToolConfirmResponder;
use crate::agent::runtime::runtime_io as chat_io;
use crate::agent::run_guard::parse_run_stop_reason;
use crate::agent::runtime::skill_routing::runner::{
    execute_implicit_route_plan, plan_implicit_route_with_observation, RouteRunOutcome,
};
use crate::agent::runtime::session_runtime::{
    PrepareSendMessageParams, SessionRuntime,
};
use crate::agent::AgentExecutor;
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct SessionEngine;

impl SessionEngine {
    pub(crate) async fn run_local_turn(
        app: &AppHandle,
        agent_executor: &Arc<AgentExecutor>,
        db: &sqlx::SqlitePool,
        journal: &crate::session_journal::SessionJournalStore,
        session_id: &str,
        run_id: &str,
        user_message_id: &str,
        user_message: &str,
        user_message_parts: &[Value],
        max_iterations_override: Option<usize>,
        cancel_flag: Arc<AtomicBool>,
        tool_confirm_responder: ToolConfirmResponder,
    ) -> Result<ExecutionOutcome, SessionEngineError> {
        let prepared_context = SessionRuntime::prepare_send_message_context(PrepareSendMessageParams {
            app,
            db,
            agent_executor,
            session_id,
            user_message,
            user_message_parts,
            max_iterations_override,
        })
        .await
        .map_err(SessionEngineError::Generic)?;

        chat_io::append_run_started_with_pool(db, journal, session_id, run_id, user_message_id)
            .await
            .map_err(SessionEngineError::Generic)?;

        match SessionRuntime::maybe_execute_user_skill_command(
            app,
            agent_executor,
            session_id,
            run_id,
            user_message,
            &prepared_context,
            cancel_flag.clone(),
            tool_confirm_responder.clone(),
        )
        .await
        {
            Ok(Some(output)) => return Ok(ExecutionOutcome::DirectDispatch(output)),
            Ok(None) => {}
            Err(error) => {
                return Ok(match parse_run_stop_reason(&error) {
                    Some(stop_reason) => ExecutionOutcome::SkillCommandStopped {
                        stop_reason,
                        error,
                    },
                    None => ExecutionOutcome::SkillCommandFailed(error),
                });
            }
        }

        let planned_route = plan_implicit_route_with_observation(
            &prepared_context.route_index,
            &prepared_context.workspace_skill_entries,
            &prepared_context.prepared_runtime_tools.skill_command_specs,
            user_message,
        );
        chat_io::append_skill_route_recorded_with_pool(
            db,
            journal,
            session_id,
            run_id,
            &planned_route.observation,
        )
        .await
        .map_err(SessionEngineError::Generic)?;

        let route_execution = match execute_implicit_route_plan(
            app,
            agent_executor,
            db,
            session_id,
            run_id,
            &prepared_context,
            planned_route.route_plan,
            cancel_flag.clone(),
            tool_confirm_responder.clone(),
        )
        .await
        .map_err(SessionEngineError::Generic)?
        {
            RouteRunOutcome::OpenTask => execute_route_candidates(RouteExecutionParams {
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
            .await,
            RouteRunOutcome::DirectDispatch(output) => {
                return Ok(ExecutionOutcome::DirectDispatch(output));
            }
            RouteRunOutcome::Prompt {
                route_execution,
                reconstructed_history_len,
            } => {
                return Ok(ExecutionOutcome::RouteExecution {
                    route_execution,
                    reconstructed_history_len,
                });
            }
        };

        Ok(ExecutionOutcome::RouteExecution {
            route_execution,
            reconstructed_history_len: prepared_context.messages.len(),
        })
    }
}
