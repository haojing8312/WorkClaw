use super::execution_plan::{ExecutionLane, ExecutionOutcome, SessionEngineError};
use super::session_profile::SessionSurfaceKind;
use crate::agent::run_guard::parse_run_stop_reason;
use crate::agent::runtime::attempt_runner::RouteExecutionOutcome;
use crate::agent::runtime::events::ToolConfirmResponder;
use crate::agent::runtime::kernel::lane_executor::{execute_execution_lane, LaneExecutionParams};
use crate::agent::runtime::kernel::turn_preparation::{prepare_local_turn, PrepareLocalTurnParams};
use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
use crate::agent::runtime::runtime_io as chat_io;
use crate::agent::runtime::session_runtime::SessionRuntime;
use crate::agent::runtime::skill_routing::runner::plan_implicit_route_with_observation;
use crate::agent::types::StreamDelta;
use crate::agent::AgentExecutor;
use crate::model_transport::resolve_model_transport;
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex;
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
        let prepared_context = prepare_local_turn(PrepareLocalTurnParams {
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
        let (turn_context, execution_context) = &prepared_context;
        debug_assert_eq!(
            execution_context.session_profile.surface,
            SessionSurfaceKind::LocalChat
        );

        chat_io::append_run_started_with_pool(db, journal, session_id, run_id, user_message_id)
            .await
            .map_err(SessionEngineError::Generic)?;

        match SessionRuntime::maybe_execute_user_skill_command(
            app,
            agent_executor,
            session_id,
            run_id,
            user_message,
            execution_context,
            cancel_flag.clone(),
            tool_confirm_responder.clone(),
        )
        .await
        {
            Ok(Some(dispatch_outcome)) => {
                let turn_state = TurnStateSnapshot::new(
                    execution_context
                        .allowed_tools()
                        .map(|tools| tools.to_vec()),
                )
                .with_session_surface(execution_context.session_profile.surface)
                .with_execution_lane(ExecutionLane::DirectDispatch)
                .with_invoked_skill(dispatch_outcome.skill_id);
                return Ok(ExecutionOutcome::DirectDispatch {
                    output: dispatch_outcome.output,
                    turn_state,
                });
            }
            Ok(None) => {}
            Err(dispatch_error) => {
                let error = dispatch_error.error;
                let turn_state = TurnStateSnapshot::new(
                    execution_context
                        .allowed_tools()
                        .map(|tools| tools.to_vec()),
                )
                .with_session_surface(execution_context.session_profile.surface)
                .with_execution_lane(ExecutionLane::DirectDispatch)
                .with_invoked_skill(dispatch_error.skill_id);
                return Ok(match parse_run_stop_reason(&error) {
                    Some(stop_reason) => ExecutionOutcome::SkillCommandStopped {
                        turn_state: turn_state.with_stop_reason(stop_reason.clone()),
                        stop_reason,
                        error,
                    },
                    None => ExecutionOutcome::SkillCommandFailed { error, turn_state },
                });
            }
        }

        let planned_route = plan_implicit_route_with_observation(
            &execution_context.route_index,
            &execution_context.workspace_skill_entries,
            execution_context.skill_command_specs(),
            user_message,
            turn_context.continuation_preference.as_ref(),
        );
        let execution_plan = planned_route.execution_plan.clone();
        chat_io::append_skill_route_recorded_with_pool(
            db,
            journal,
            session_id,
            run_id,
            &planned_route.observation,
            execution_context.tool_plan_record(),
        )
        .await
        .map_err(SessionEngineError::Generic)?;

        let mut turn_state = TurnStateSnapshot::default()
            .with_session_surface(execution_context.session_profile.surface)
            .with_route_observation(planned_route.observation.clone())
            .with_execution_lane(execution_plan.lane);
        if let Some(skill_id) = planned_route.observation.selected_skill.as_deref() {
            turn_state = turn_state.with_invoked_skill(skill_id);
        }

        execute_execution_lane(LaneExecutionParams {
            app,
            agent_executor,
            db,
            session_id,
            run_id,
            turn_context,
            execution_context,
            execution_plan: &execution_plan,
            turn_state,
            cancel_flag,
            tool_confirm_responder,
        })
        .await
        .map_err(SessionEngineError::Generic)
    }

    pub(crate) async fn run_hidden_child_turn(
        app_handle: Option<&AppHandle>,
        agent_executor: &Arc<AgentExecutor>,
        session_id: &str,
        turn_context: &super::execution_plan::TurnContext,
        execution_context: &super::execution_plan::ExecutionContext,
        on_token: impl Fn(StreamDelta) + Send + Clone + 'static,
    ) -> Result<ExecutionOutcome, SessionEngineError> {
        Self::run_prepared_surface_turn(
            SessionSurfaceKind::HiddenChildSession,
            "隐藏子会话缺少可执行模型候选",
            "child_session",
            app_handle,
            agent_executor,
            session_id,
            turn_context,
            execution_context,
            on_token,
        )
        .await
    }

    pub(crate) async fn run_employee_step_turn(
        app_handle: Option<&AppHandle>,
        agent_executor: &Arc<AgentExecutor>,
        session_id: &str,
        turn_context: &super::execution_plan::TurnContext,
        execution_context: &super::execution_plan::ExecutionContext,
        on_token: impl Fn(StreamDelta) + Send + Clone + 'static,
    ) -> Result<ExecutionOutcome, SessionEngineError> {
        Self::run_prepared_surface_turn(
            SessionSurfaceKind::EmployeeStepSession,
            "员工步骤会话缺少可执行模型候选",
            "employee_step",
            app_handle,
            agent_executor,
            session_id,
            turn_context,
            execution_context,
            on_token,
        )
        .await
    }

    async fn run_prepared_surface_turn(
        expected_surface: SessionSurfaceKind,
        missing_candidate_message: &str,
        default_error_kind: &str,
        app_handle: Option<&AppHandle>,
        agent_executor: &Arc<AgentExecutor>,
        session_id: &str,
        turn_context: &super::execution_plan::TurnContext,
        execution_context: &super::execution_plan::ExecutionContext,
        on_token: impl Fn(StreamDelta) + Send + Clone + 'static,
    ) -> Result<ExecutionOutcome, SessionEngineError> {
        debug_assert_eq!(execution_context.session_profile.surface, expected_surface);

        let Some((provider_key, api_format, base_url, model_name, api_key)) =
            turn_context.primary_route_candidate()
        else {
            return Err(SessionEngineError::Generic(
                missing_candidate_message.to_string(),
            ));
        };

        let transport = resolve_model_transport(
            api_format,
            base_url,
            Some(provider_key.as_str()).filter(|value| !value.trim().is_empty()),
        );
        let streamed_text = Arc::new(Mutex::new(String::new()));
        let streamed_text_for_callback = Arc::clone(&streamed_text);
        let callback = on_token.clone();

        let route_execution = match agent_executor
            .execute_turn_with_transport_outcome(
                transport,
                api_format,
                base_url,
                api_key,
                model_name,
                &execution_context.system_prompt,
                turn_context.messages.clone(),
                move |delta| {
                    if let StreamDelta::Text(token) = &delta {
                        if let Ok(mut buffer) = streamed_text_for_callback.lock() {
                            buffer.push_str(token);
                        }
                    }
                    callback(delta);
                },
                app_handle,
                Some(session_id),
                execution_context.allowed_tools(),
                execution_context.permission_mode,
                None,
                execution_context.executor_work_dir.clone(),
                execution_context.max_iterations,
                None,
                Some(execution_context.node_timeout_seconds),
                Some(execution_context.route_retry_count),
            )
            .await
        {
            Ok(outcome) => RouteExecutionOutcome {
                final_messages: Some(outcome.messages),
                last_error: None,
                last_error_kind: None,
                last_stop_reason: None,
                partial_text: streamed_text
                    .lock()
                    .map(|buffer| buffer.clone())
                    .unwrap_or_default(),
                reasoning_text: String::new(),
                reasoning_duration_ms: None,
                tool_exposure_expanded: false,
                tool_exposure_expansion_reason: None,
                compaction_boundary: outcome.compaction_outcome.as_ref().map(Into::into),
            },
            Err(error) => {
                let error_text = error.error.to_string();
                let stop_reason = parse_run_stop_reason(&error_text);
                RouteExecutionOutcome {
                    final_messages: None,
                    last_error: Some(error_text),
                    last_error_kind: Some(
                        stop_reason
                            .as_ref()
                            .map(|reason| reason.kind.as_key().to_string())
                            .unwrap_or_else(|| default_error_kind.to_string()),
                    ),
                    last_stop_reason: stop_reason,
                    partial_text: streamed_text
                        .lock()
                        .map(|buffer| buffer.clone())
                        .unwrap_or_default(),
                    reasoning_text: String::new(),
                    reasoning_duration_ms: None,
                    tool_exposure_expanded: false,
                    tool_exposure_expansion_reason: None,
                    compaction_boundary: error.compaction_outcome.as_ref().map(Into::into),
                }
            }
        };

        let reconstructed_history_len = turn_context.messages.len();
        let turn_state = TurnStateSnapshot::new(
            execution_context
                .allowed_tools()
                .map(|tools| tools.to_vec()),
        )
        .with_session_surface(execution_context.session_profile.surface)
        .with_execution_lane(ExecutionLane::OpenTask)
        .with_route_execution(&route_execution, reconstructed_history_len);

        Ok(ExecutionOutcome::RouteExecution {
            route_execution,
            reconstructed_history_len,
            turn_state,
        })
    }
}
