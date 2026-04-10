use crate::agent::context::build_tool_context;
use crate::agent::run_guard::{RunBudgetPolicy, RunBudgetScope};
use crate::agent::runtime::kernel::execution_plan::ExecutionContext;
use crate::agent::runtime::kernel::route_lane::RoutedSkillToolSetup;
use crate::agent::runtime::runtime_io::WorkspaceSkillCommandSpec;
use crate::agent::runtime::tool_dispatch::{dispatch_skill_command, ToolDispatchContext};
use crate::agent::AgentExecutor;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;

pub(crate) async fn execute_direct_dispatch_skill(
    app: &AppHandle,
    agent_executor: &Arc<AgentExecutor>,
    session_id: &str,
    run_id: &str,
    execution_context: &ExecutionContext,
    setup: &RoutedSkillToolSetup,
    command_spec: &WorkspaceSkillCommandSpec,
    raw_args: &str,
    cancel_flag: Arc<AtomicBool>,
    tool_confirm_responder: &crate::agent::runtime::events::ToolConfirmResponder,
) -> Result<String, String> {
    let tool_ctx = build_tool_context(
        Some(session_id),
        execution_context
            .executor_work_dir
            .as_ref()
            .map(std::path::PathBuf::from),
        setup.skill_allowed_tools.as_deref(),
    )
    .map_err(|err| err.to_string())?;
    let dispatch_context = ToolDispatchContext {
        registry: agent_executor.registry(),
        app_handle: Some(app),
        session_id: Some(session_id),
        persisted_run_id: Some(run_id),
        active_task_identity: execution_context.active_task_identity(),
        active_task_kind: execution_context.active_task_kind,
        active_task_surface: execution_context.active_task_surface,
        active_task_backend: execution_context.active_task_backend,
        active_task_continuation_mode: execution_context.active_task_continuation_mode,
        active_task_continuation_source: execution_context.active_task_continuation_source,
        active_task_continuation_reason: execution_context
            .active_task_continuation_reason
            .as_deref(),
        allowed_tools: setup.skill_allowed_tools.as_deref(),
        effective_tool_plan: execution_context.effective_tool_plan(),
        permission_mode: execution_context.permission_mode,
        tool_ctx: &tool_ctx,
        tool_confirm_tx: Some(tool_confirm_responder),
        cancel_flag: Some(cancel_flag),
        route_run_id: run_id,
        route_node_timeout_secs: execution_context.node_timeout_seconds,
        route_retry_count: 0,
        iteration: 1,
        run_budget_policy: RunBudgetPolicy::for_scope(RunBudgetScope::Skill),
    };

    dispatch_skill_command(&dispatch_context, command_spec, raw_args)
        .await
        .map_err(|err| err.to_string())
}
