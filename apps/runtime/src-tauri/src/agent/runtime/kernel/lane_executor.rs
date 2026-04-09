use crate::agent::runtime::attempt_runner::{execute_route_candidates, RouteExecutionParams};
use crate::agent::runtime::events::ToolConfirmResponder;
use crate::agent::runtime::kernel::execution_plan::{
    ExecutionContext, ExecutionOutcome, ExecutionPlan, TurnContext,
};
use crate::agent::runtime::kernel::route_lane::{RouteRunOutcome, RouteRunPlan};
use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
use crate::agent::runtime::skill_routing::runner::execute_planned_route;
use crate::agent::AgentExecutor;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;

pub(crate) struct LaneExecutionParams<'a> {
    pub app: &'a AppHandle,
    pub agent_executor: &'a Arc<AgentExecutor>,
    pub db: &'a sqlx::SqlitePool,
    pub session_id: &'a str,
    pub run_id: &'a str,
    pub turn_context: &'a TurnContext,
    pub execution_context: &'a ExecutionContext,
    pub execution_plan: &'a ExecutionPlan,
    pub turn_state: TurnStateSnapshot,
    pub cancel_flag: Arc<AtomicBool>,
    pub tool_confirm_responder: ToolConfirmResponder,
}

fn decorate_turn_state(
    turn_state: TurnStateSnapshot,
    execution_plan: &ExecutionPlan,
    execution_context: &ExecutionContext,
) -> TurnStateSnapshot {
    let turn_state = turn_state.with_session_surface(execution_context.session_profile.surface);

    match &execution_plan.route_plan {
        RouteRunPlan::OpenTask { .. } => turn_state
            .with_execution_lane(execution_plan.lane)
            .with_allowed_tools(
                execution_context
                    .allowed_tools()
                    .map(|tools| tools.to_vec()),
            ),
        RouteRunPlan::PromptSkillInline { skill_id, setup }
        | RouteRunPlan::PromptSkillFork { skill_id, setup } => turn_state
            .with_execution_lane(execution_plan.lane)
            .with_allowed_tools(setup.skill_allowed_tools.clone())
            .with_invoked_skill(skill_id.clone()),
        RouteRunPlan::DirectDispatchSkill {
            skill_id, setup, ..
        } => turn_state
            .with_execution_lane(execution_plan.lane)
            .with_allowed_tools(setup.skill_allowed_tools.clone())
            .with_invoked_skill(skill_id.clone()),
    }
}

pub(crate) async fn execute_execution_lane(
    params: LaneExecutionParams<'_>,
) -> Result<ExecutionOutcome, String> {
    let turn_state = decorate_turn_state(
        params.turn_state,
        params.execution_plan,
        params.execution_context,
    );

    match execute_planned_route(
        params.app,
        params.agent_executor,
        params.db,
        params.session_id,
        params.run_id,
        params.turn_context,
        params.execution_context,
        params.execution_plan,
        params.cancel_flag.clone(),
        params.tool_confirm_responder.clone(),
    )
    .await?
    {
        RouteRunOutcome::OpenTask => {
            let route_execution = execute_route_candidates(RouteExecutionParams {
                app: params.app,
                agent_executor: params.agent_executor.as_ref(),
                db: params.db,
                session_id: params.session_id,
                requested_capability: &params.turn_context.requested_capability,
                route_candidates: &params.turn_context.route_candidates,
                per_candidate_retry_count: params.turn_context.per_candidate_retry_count,
                system_prompt: &params.execution_context.system_prompt,
                messages: &params.turn_context.messages,
                allowed_tools: params.execution_context.allowed_tools(),
                full_allowed_tools: params.execution_context.full_allowed_tools(),
                has_deferred_tools: params.execution_context.has_deferred_tools(),
                permission_mode: params.execution_context.permission_mode,
                tool_confirm_responder: params.tool_confirm_responder,
                executor_work_dir: params.execution_context.executor_work_dir.clone(),
                max_iterations: params.execution_context.max_iterations,
                cancel_flag: params.cancel_flag,
                node_timeout_seconds: params.execution_context.node_timeout_seconds,
                route_retry_count: params.execution_context.route_retry_count,
            })
            .await;
            let turn_state = turn_state
                .with_route_execution(&route_execution, params.turn_context.messages.len());

            Ok(ExecutionOutcome::RouteExecution {
                route_execution,
                reconstructed_history_len: params.turn_context.messages.len(),
                turn_state,
            })
        }
        RouteRunOutcome::DirectDispatch(output) => {
            Ok(ExecutionOutcome::DirectDispatch { output, turn_state })
        }
        RouteRunOutcome::Prompt {
            route_execution,
            reconstructed_history_len,
        } => {
            let turn_state =
                turn_state.with_route_execution(&route_execution, reconstructed_history_len);
            Ok(ExecutionOutcome::RouteExecution {
                route_execution,
                reconstructed_history_len,
                turn_state,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::decorate_turn_state;
    use crate::agent::permissions::PermissionMode;
    use crate::agent::runtime::kernel::capability_snapshot::CapabilitySnapshot;
    use crate::agent::runtime::kernel::execution_plan::{
        ExecutionContext, ExecutionLane, ExecutionPlan,
    };
    use crate::agent::runtime::kernel::route_lane::RouteRunPlan;
    use crate::agent::runtime::kernel::session_profile::{
        SessionExecutionProfile, SessionSurfaceKind,
    };
    use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
    use crate::agent::runtime::skill_routing::index::SkillRouteIndex;
    use runtime_chat_app::ChatExecutionGuidance;

    #[test]
    fn decorate_turn_state_projects_session_surface_from_execution_context() {
        let execution_context = ExecutionContext {
            session_profile: SessionExecutionProfile::for_surface(
                SessionSurfaceKind::HiddenChildSession,
            ),
            capability_snapshot: CapabilitySnapshot::default(),
            system_prompt: String::new(),
            continuation_runtime_notes: Vec::new(),
            permission_mode: PermissionMode::AcceptEdits,
            runtime_default_tool_policy: ExecutionContext::default().runtime_default_tool_policy,
            executor_work_dir: None,
            max_iterations: Some(4),
            max_call_depth: 2,
            node_timeout_seconds: 60,
            route_retry_count: 1,
            execution_guidance: ChatExecutionGuidance {
                effective_work_dir: "E:/workspace/demo".to_string(),
                local_timezone: "Asia/Shanghai".to_string(),
                local_date: "2026-04-08".to_string(),
                local_tomorrow: "2026-04-09".to_string(),
                local_month_range: "2026-04-01 ~ 2026-04-30".to_string(),
            },
            memory_bucket_employee_id: "employee-1".to_string(),
            employee_collaboration_guidance: None,
            workspace_skill_entries: Vec::new(),
            route_index: SkillRouteIndex::default(),
        };
        let execution_plan = ExecutionPlan {
            lane: ExecutionLane::OpenTask,
            route_plan: RouteRunPlan::OpenTask {
                fallback_reason: None,
            },
        };

        let turn_state = decorate_turn_state(
            TurnStateSnapshot::default(),
            &execution_plan,
            &execution_context,
        );

        assert_eq!(
            turn_state.session_surface,
            Some(SessionSurfaceKind::HiddenChildSession)
        );
        assert_eq!(turn_state.execution_lane, Some(ExecutionLane::OpenTask));
    }
}
