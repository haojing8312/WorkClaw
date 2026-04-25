use crate::agent::run_guard::{RunBudgetPolicy, RunBudgetScope};
use crate::agent::runtime::attempt_runner::{
    execute_route_candidates, RouteExecutionOutcome, RouteExecutionParams,
};
use crate::agent::runtime::events::ToolConfirmResponder;
use crate::agent::runtime::kernel::execution_plan::ExecutionContext;
use crate::agent::runtime::kernel::execution_plan::TurnContext;
use crate::agent::runtime::tool_setup::{prepare_runtime_tools, ToolSetupParams};
use crate::agent::AgentExecutor;
use runtime_chat_app::ChatExecutionPreparationService;
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedRoutedPrompt {
    pub allowed_tools: Option<Vec<String>>,
    pub full_allowed_tools: Vec<String>,
    pub has_deferred_tools: bool,
    pub system_prompt: String,
    pub max_iterations: usize,
}

#[derive(Clone)]
pub(crate) struct RoutedPromptPreparationParams<'a> {
    pub app: &'a AppHandle,
    pub db: &'a sqlx::SqlitePool,
    pub agent_executor: &'a Arc<AgentExecutor>,
    pub session_id: &'a str,
    pub turn_context: &'a TurnContext,
    pub execution_context: &'a ExecutionContext,
    pub api_format: &'a str,
    pub base_url: &'a str,
    pub model_name: &'a str,
    pub api_key: &'a str,
    pub skill_id: &'a str,
    pub skill_system_prompt: &'a str,
    pub skill_allowed_tools: Option<Vec<String>>,
    pub skill_denied_tools: Option<Vec<String>>,
    pub skill_allowed_tool_sources: Option<Vec<crate::agent::tool_manifest::ToolSource>>,
    pub skill_denied_tool_sources: Option<Vec<crate::agent::tool_manifest::ToolSource>>,
    pub skill_allowed_tool_categories: Option<Vec<crate::agent::tool_manifest::ToolCategory>>,
    pub skill_denied_tool_categories: Option<Vec<crate::agent::tool_manifest::ToolCategory>>,
    pub skill_allowed_mcp_servers: Option<Vec<String>>,
    pub skill_max_iterations: Option<usize>,
    pub source_type: &'a str,
    pub pack_path: &'a str,
}

#[derive(Clone)]
pub(crate) struct RoutedPromptExecutionParams<'a> {
    pub app: &'a AppHandle,
    pub agent_executor: &'a Arc<AgentExecutor>,
    pub db: &'a sqlx::SqlitePool,
    pub session_id: &'a str,
    pub turn_context: &'a TurnContext,
    pub execution_context: &'a ExecutionContext,
    pub prepared_prompt: &'a PreparedRoutedPrompt,
    pub messages: &'a [Value],
    pub tool_confirm_responder: ToolConfirmResponder,
    pub cancel_flag: Arc<AtomicBool>,
}

pub(crate) fn resolve_routed_prompt_max_iterations(
    skill_id: &str,
    configured_max_iterations: Option<usize>,
) -> usize {
    RunBudgetPolicy::resolve(
        if skill_id.eq_ignore_ascii_case("builtin-general") {
            RunBudgetScope::GeneralChat
        } else {
            RunBudgetScope::Skill
        },
        configured_max_iterations,
    )
    .max_turns
}

pub(crate) async fn prepare_routed_prompt(
    params: RoutedPromptPreparationParams<'_>,
) -> Result<PreparedRoutedPrompt, String> {
    let execution_preparation_service = ChatExecutionPreparationService::new();
    let max_iterations =
        resolve_routed_prompt_max_iterations(params.skill_id, params.skill_max_iterations);

    let prepared_runtime_tools = prepare_runtime_tools(ToolSetupParams {
        app: params.app,
        db: params.db,
        agent_executor: params.agent_executor,
        workspace_skill_entries: &params.execution_context.workspace_skill_entries,
        session_id: params.session_id,
        api_format: params.api_format,
        base_url: params.base_url,
        model_name: params.model_name,
        api_key: params.api_key,
        skill_id: params.skill_id,
        source_type: params.source_type,
        pack_path: params.pack_path,
        permission_mode: params.execution_context.permission_mode,
        runtime_default_tool_policy: params.execution_context.runtime_default_tool_policy.clone(),
        skill_system_prompt: params.skill_system_prompt,
        skill_allowed_tools: params.skill_allowed_tools,
        skill_denied_tools: params.skill_denied_tools,
        skill_allowed_tool_sources: params.skill_allowed_tool_sources,
        skill_denied_tool_sources: params.skill_denied_tool_sources,
        skill_allowed_tool_categories: params.skill_allowed_tool_categories,
        skill_denied_tool_categories: params.skill_denied_tool_categories,
        skill_allowed_mcp_servers: params.skill_allowed_mcp_servers,
        tool_discovery_query: Some(&params.turn_context.user_message),
        max_iter: max_iterations,
        max_call_depth: params.execution_context.max_call_depth,
        suppress_workspace_skills_prompt: false,
        execution_preparation_service: &execution_preparation_service,
        execution_guidance: &params.execution_context.execution_guidance,
        memory_bucket_employee_id: &params.execution_context.memory_bucket_employee_id,
        employee_collaboration_guidance: params
            .execution_context
            .employee_collaboration_guidance
            .as_deref(),
        supplemental_runtime_notes: &params.execution_context.continuation_runtime_notes,
        resource_context: None,
    })
    .await?;

    Ok(PreparedRoutedPrompt {
        allowed_tools: prepared_runtime_tools.allowed_tools,
        full_allowed_tools: prepared_runtime_tools.full_allowed_tools,
        has_deferred_tools: prepared_runtime_tools
            .capability_snapshot
            .has_deferred_tools(),
        system_prompt: prepared_runtime_tools.system_prompt,
        max_iterations,
    })
}

pub(crate) async fn execute_routed_prompt(
    params: RoutedPromptExecutionParams<'_>,
) -> RouteExecutionOutcome {
    execute_route_candidates(RouteExecutionParams {
        app: params.app,
        agent_executor: params.agent_executor.as_ref(),
        db: params.db,
        session_id: params.session_id,
        requested_capability: &params.turn_context.requested_capability,
        route_candidates: &params.turn_context.route_candidates,
        per_candidate_retry_count: params.turn_context.per_candidate_retry_count,
        system_prompt: &params.prepared_prompt.system_prompt,
        messages: params.messages,
        allowed_tools: params.prepared_prompt.allowed_tools.as_deref(),
        full_allowed_tools: Some(&params.prepared_prompt.full_allowed_tools),
        has_deferred_tools: params.prepared_prompt.has_deferred_tools,
        permission_mode: params.execution_context.permission_mode,
        tool_confirm_responder: params.tool_confirm_responder,
        executor_work_dir: params.execution_context.executor_work_dir.clone(),
        max_iterations: Some(params.prepared_prompt.max_iterations),
        cancel_flag: params.cancel_flag,
        node_timeout_seconds: params.execution_context.node_timeout_seconds,
        route_retry_count: params.execution_context.route_retry_count,
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::{resolve_routed_prompt_max_iterations, PreparedRoutedPrompt};
    use crate::agent::run_guard::{RunBudgetPolicy, RunBudgetScope};

    #[test]
    fn resolve_routed_prompt_max_iterations_uses_general_chat_budget_for_builtin_general() {
        let resolved = resolve_routed_prompt_max_iterations("builtin-general", Some(7));

        assert_eq!(
            resolved,
            RunBudgetPolicy::resolve(RunBudgetScope::GeneralChat, Some(7)).max_turns
        );
    }

    #[test]
    fn resolve_routed_prompt_max_iterations_uses_skill_budget_for_regular_skills() {
        let resolved = resolve_routed_prompt_max_iterations("feishu-pm-hub", Some(7));

        assert_eq!(
            resolved,
            RunBudgetPolicy::resolve(RunBudgetScope::Skill, Some(7)).max_turns
        );
    }

    #[test]
    fn prepared_routed_prompt_carries_execution_contract() {
        let prepared = PreparedRoutedPrompt {
            allowed_tools: Some(vec!["read".to_string(), "exec".to_string()]),
            full_allowed_tools: vec!["read".to_string(), "exec".to_string()],
            has_deferred_tools: false,
            system_prompt: "Prompt".to_string(),
            max_iterations: 9,
        };

        assert_eq!(
            prepared.allowed_tools.as_deref(),
            Some(&["read".to_string(), "exec".to_string()][..])
        );
        assert_eq!(
            prepared.full_allowed_tools,
            vec!["read".to_string(), "exec".to_string()]
        );
        assert!(!prepared.has_deferred_tools);
        assert_eq!(prepared.system_prompt, "Prompt");
        assert_eq!(prepared.max_iterations, 9);
    }
}
