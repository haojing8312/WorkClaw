use super::execution_plan::{ExecutionLane, ExecutionOutcome, SessionEngineError};
use super::session_profile::SessionSurfaceKind;
use crate::agent::run_guard::parse_run_stop_reason;
use crate::agent::runtime::events::ToolConfirmResponder;
use crate::agent::runtime::kernel::lane_executor::{execute_execution_lane, LaneExecutionParams};
use crate::agent::runtime::kernel::turn_preparation::{prepare_local_turn, PrepareLocalTurnParams};
use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
use crate::agent::runtime::runtime_io as chat_io;
use crate::agent::runtime::session_runtime::SessionRuntime;
use crate::agent::runtime::skill_routing::runner::plan_implicit_route_with_observation;
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
        )
        .await
        .map_err(SessionEngineError::Generic)?;

        let mut turn_state = TurnStateSnapshot::default()
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
}
