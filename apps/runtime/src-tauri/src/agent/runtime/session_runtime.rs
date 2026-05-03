use super::events::ToolConfirmResponder;
use crate::agent::context::build_tool_context_with_permission_mode;
use crate::agent::run_guard::{RunBudgetPolicy, RunBudgetScope};
use crate::agent::runtime::kernel::execution_plan::ExecutionContext;
use crate::agent::runtime::kernel::turn_preparation::parse_user_skill_command;
use crate::agent::runtime::task_entry::{self, PrimaryLocalChatTaskRunAndFinalizeRequest};
use crate::agent::AgentExecutor;
use crate::session_journal::SessionJournalStore;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;
use uuid::Uuid;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SessionRuntime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkillCommandDispatchOutcome {
    pub output: String,
    pub skill_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkillCommandDispatchError {
    pub error: String,
    pub skill_id: String,
}

impl SessionRuntime {
    pub fn new() -> Self {
        Self
    }

    pub(crate) async fn maybe_execute_user_skill_command(
        app: &AppHandle,
        agent_executor: &Arc<AgentExecutor>,
        session_id: &str,
        run_id: &str,
        user_message: &str,
        execution_context: &ExecutionContext,
        cancel_flag: Arc<AtomicBool>,
        tool_confirm_responder: ToolConfirmResponder,
    ) -> Result<Option<SkillCommandDispatchOutcome>, SkillCommandDispatchError> {
        let Some((command_name, raw_args)) = parse_user_skill_command(user_message) else {
            return Ok(None);
        };
        let Some(spec) = execution_context
            .skill_command_specs()
            .iter()
            .find(|spec| spec.name.eq_ignore_ascii_case(&command_name) && spec.dispatch.is_some())
        else {
            return Ok(None);
        };

        let tool_ctx = build_tool_context_with_permission_mode(
            Some(session_id),
            execution_context
                .executor_work_dir
                .as_ref()
                .map(PathBuf::from),
            execution_context.allowed_tools(),
            execution_context.permission_mode,
        )
        .map_err(|err| SkillCommandDispatchError {
            error: err.to_string(),
            skill_id: spec.skill_id.clone(),
        })?;
        let dispatch_context = crate::agent::runtime::tool_dispatch::ToolDispatchContext {
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
            allowed_tools: execution_context.allowed_tools(),
            effective_tool_plan: execution_context.effective_tool_plan(),
            permission_mode: execution_context.permission_mode,
            tool_ctx: &tool_ctx,
            tool_confirm_tx: Some(&tool_confirm_responder),
            cancel_flag: Some(cancel_flag),
            route_run_id: run_id,
            route_node_timeout_secs: execution_context.node_timeout_seconds,
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
        .map(|output| {
            Some(SkillCommandDispatchOutcome {
                output,
                skill_id: spec.skill_id.clone(),
            })
        })
        .map_err(|err| SkillCommandDispatchError {
            error: err.to_string(),
            skill_id: spec.skill_id.clone(),
        })
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
        let run_id = Uuid::new_v4().to_string();
        task_entry::run_and_finalize_primary_local_chat_task(
            PrimaryLocalChatTaskRunAndFinalizeRequest {
                app,
                agent_executor,
                db,
                journal,
                session_id,
                run_id: &run_id,
                user_message_id,
                user_message,
                user_message_parts,
                max_iterations_override,
                cancel_flag: cancel_flag.clone(),
                tool_confirm_responder: tool_confirm_responder.clone(),
            },
        )
        .await
    }
}
