use super::adjudicator::adjudicate_route;
use super::index::SkillRouteIndex;
use super::intent::RouteFallbackReason;
use super::observability::{build_implicit_route_observation, PlannedImplicitRoute};
use super::recall::recall_skill_candidates;
use crate::agent::context::build_tool_context;
use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::{RunBudgetPolicy, RunBudgetScope};
use crate::agent::runtime::attempt_runner::{
    execute_route_candidates, RouteExecutionOutcome, RouteExecutionParams,
};
use crate::agent::runtime::effective_tool_set::{
    resolve_effective_tool_set, session_tool_policy_input, skill_tool_policy_input,
    EffectiveToolPolicyInput,
};
use crate::agent::runtime::runtime_io::{
    WorkspaceSkillCommandSpec, WorkspaceSkillContent, WorkspaceSkillRuntimeEntry,
};
use crate::agent::runtime::tool_dispatch::{dispatch_skill_command, ToolDispatchContext};
use crate::agent::runtime::tool_profiles::ToolProfileName;
use crate::agent::runtime::tool_setup::{prepare_runtime_tools, ToolSetupParams};
use crate::agent::tool_manifest::{ToolCategory, ToolSource};
use crate::agent::AgentExecutor;
use runtime_chat_app::ChatExecutionPreparationService;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RoutedSkillToolSetup {
    pub skill_id: String,
    pub skill_system_prompt: String,
    pub skill_allowed_tools: Option<Vec<String>>,
    pub skill_denied_tools: Option<Vec<String>>,
    pub skill_allowed_tool_sources: Option<Vec<ToolSource>>,
    pub skill_denied_tool_sources: Option<Vec<ToolSource>>,
    pub skill_allowed_tool_categories: Option<Vec<ToolCategory>>,
    pub skill_denied_tool_categories: Option<Vec<ToolCategory>>,
    pub skill_allowed_mcp_servers: Option<Vec<String>>,
    pub tool_profile: Option<ToolProfileName>,
    pub max_iterations: Option<usize>,
    pub source_type: String,
    pub pack_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RouteRunPlan {
    OpenTask {
        fallback_reason: Option<RouteFallbackReason>,
    },
    PromptSkillInline {
        skill_id: String,
        setup: RoutedSkillToolSetup,
    },
    PromptSkillFork {
        skill_id: String,
        setup: RoutedSkillToolSetup,
    },
    DirectDispatchSkill {
        skill_id: String,
        setup: RoutedSkillToolSetup,
        command_spec: WorkspaceSkillCommandSpec,
        raw_args: String,
    },
}

#[derive(Debug)]
pub(crate) enum RouteRunOutcome {
    OpenTask,
    DirectDispatch(String),
    Prompt {
        route_execution: RouteExecutionOutcome,
        reconstructed_history_len: usize,
    },
}

pub(crate) fn build_routed_skill_tool_setup(
    entry: &WorkspaceSkillRuntimeEntry,
) -> RoutedSkillToolSetup {
    RoutedSkillToolSetup {
        skill_id: entry.skill_id.clone(),
        skill_system_prompt: entry.config.system_prompt.clone(),
        skill_allowed_tools: entry.config.allowed_tools.clone(),
        skill_denied_tools: entry.config.denied_tools.clone(),
        skill_allowed_tool_sources: parse_skill_allowed_tool_sources(&entry.config),
        skill_denied_tool_sources: parse_skill_denied_tool_sources(&entry.config),
        skill_allowed_tool_categories: parse_skill_allowed_tool_categories(&entry.config),
        skill_denied_tool_categories: parse_skill_denied_tool_categories(&entry.config),
        skill_allowed_mcp_servers: skill_allowed_mcp_servers(entry),
        tool_profile: infer_skill_tool_profile(entry),
        max_iterations: entry.config.max_iterations,
        source_type: entry.source_type.clone(),
        pack_path: match &entry.content {
            WorkspaceSkillContent::LocalDir(path) => path.to_string_lossy().to_string(),
            WorkspaceSkillContent::FileTree(_) => String::new(),
        },
    }
}

fn skill_allowed_mcp_servers(entry: &WorkspaceSkillRuntimeEntry) -> Option<Vec<String>> {
    let servers = entry
        .config
        .mcp_servers
        .iter()
        .map(|server| server.name.trim())
        .filter(|name| !name.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    (!servers.is_empty()).then_some(servers)
}

fn infer_skill_tool_profile(entry: &WorkspaceSkillRuntimeEntry) -> Option<ToolProfileName> {
    if entry.config.allowed_tools.is_some() {
        return None;
    }

    let haystack = format!(
        "{} {} {}",
        entry.skill_id.to_ascii_lowercase(),
        entry.name.to_ascii_lowercase(),
        entry.description.to_ascii_lowercase()
    );

    if haystack.contains("browser") {
        return Some(ToolProfileName::Browser);
    }
    if haystack.contains("employee") || haystack.contains("team") {
        return Some(ToolProfileName::Employee);
    }
    if haystack.contains("coding") || haystack.contains("code") || haystack.contains("developer") {
        return Some(ToolProfileName::Coding);
    }

    None
}

fn parse_skill_denied_tool_categories(
    config: &runtime_skill_core::SkillConfig,
) -> Option<Vec<ToolCategory>> {
    let categories = config
        .denied_tool_categories
        .as_ref()
        .into_iter()
        .flatten()
        .filter_map(|name| parse_tool_category_name(name))
        .collect::<Vec<_>>();

    (!categories.is_empty()).then_some(categories)
}

fn parse_skill_allowed_tool_categories(
    config: &runtime_skill_core::SkillConfig,
) -> Option<Vec<ToolCategory>> {
    let categories = config
        .allowed_tool_categories
        .as_ref()
        .into_iter()
        .flatten()
        .filter_map(|name| parse_tool_category_name(name))
        .collect::<Vec<_>>();

    (!categories.is_empty()).then_some(categories)
}

fn parse_skill_allowed_tool_sources(
    config: &runtime_skill_core::SkillConfig,
) -> Option<Vec<ToolSource>> {
    let sources = config
        .allowed_tool_sources
        .as_ref()
        .into_iter()
        .flatten()
        .filter_map(|name| parse_tool_source_name(name))
        .collect::<Vec<_>>();

    (!sources.is_empty()).then_some(sources)
}

fn parse_skill_denied_tool_sources(
    config: &runtime_skill_core::SkillConfig,
) -> Option<Vec<ToolSource>> {
    let sources = config
        .denied_tool_sources
        .as_ref()
        .into_iter()
        .flatten()
        .filter_map(|name| parse_tool_source_name(name))
        .collect::<Vec<_>>();

    (!sources.is_empty()).then_some(sources)
}

fn parse_tool_category_name(raw: &str) -> Option<ToolCategory> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "file" => Some(ToolCategory::File),
        "shell" => Some(ToolCategory::Shell),
        "web" => Some(ToolCategory::Web),
        "browser" => Some(ToolCategory::Browser),
        "system" => Some(ToolCategory::System),
        "planning" => Some(ToolCategory::Planning),
        "agent" => Some(ToolCategory::Agent),
        "memory" => Some(ToolCategory::Memory),
        "search" => Some(ToolCategory::Search),
        "integration" => Some(ToolCategory::Integration),
        "other" => Some(ToolCategory::Other),
        _ => None,
    }
}

fn parse_tool_source_name(raw: &str) -> Option<ToolSource> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "native" => Some(ToolSource::Native),
        "runtime" => Some(ToolSource::Runtime),
        "sidecar" => Some(ToolSource::Sidecar),
        "mcp" => Some(ToolSource::Mcp),
        "plugin" => Some(ToolSource::Plugin),
        "alias" => Some(ToolSource::Alias),
        _ => None,
    }
}

fn resolve_skill_allowed_tools(
    registry: &crate::agent::ToolRegistry,
    setup: &RoutedSkillToolSetup,
    runtime_default_tool_policy: &EffectiveToolPolicyInput,
    permission_mode: PermissionMode,
) -> Option<Vec<String>> {
    resolve_effective_tool_set(
        registry,
        setup.skill_allowed_tools.clone(),
        setup.tool_profile,
        &[
            runtime_default_tool_policy.clone(),
            session_tool_policy_input(permission_mode),
            skill_tool_policy_input(
                setup.skill_denied_tools.clone().unwrap_or_default(),
                setup
                    .skill_denied_tool_categories
                    .clone()
                    .unwrap_or_default(),
                setup.skill_allowed_tool_categories.clone(),
                setup.skill_allowed_tool_sources.clone(),
                setup.skill_denied_tool_sources.clone().unwrap_or_default(),
                setup.skill_allowed_mcp_servers.clone(),
            ),
        ],
    )
    .allowed_tools
}

pub(crate) fn resolve_direct_dispatch_raw_args(
    user_message: &str,
    command_spec: &WorkspaceSkillCommandSpec,
    entry: &WorkspaceSkillRuntimeEntry,
) -> Option<String> {
    let trimmed = user_message.trim();
    if trimmed.is_empty() {
        return None;
    }

    let candidate = direct_dispatch_prefixes(command_spec, entry)
        .into_iter()
        .find_map(|prefix| strip_command_prefix(trimmed, &prefix))?;
    let candidate = candidate.trim();

    if candidate.is_empty() || !is_safe_dispatch_fragment(candidate) {
        return None;
    }

    Some(candidate.to_string())
}

pub(crate) fn plan_implicit_route(
    route_index: &SkillRouteIndex,
    workspace_skill_entries: &[WorkspaceSkillRuntimeEntry],
    command_specs: &[WorkspaceSkillCommandSpec],
    user_message: &str,
) -> RouteRunPlan {
    plan_implicit_route_with_observation(
        route_index,
        workspace_skill_entries,
        command_specs,
        user_message,
    )
    .route_plan
}

pub(crate) fn plan_implicit_route_with_observation(
    route_index: &SkillRouteIndex,
    workspace_skill_entries: &[WorkspaceSkillRuntimeEntry],
    command_specs: &[WorkspaceSkillCommandSpec],
    user_message: &str,
) -> PlannedImplicitRoute {
    let started_at = std::time::Instant::now();
    let candidates = recall_skill_candidates(route_index, user_message);
    let decision = adjudicate_route(&candidates);

    let route_plan = match decision {
        super::intent::RouteDecision::OpenTask {
            fallback_reason, ..
        } => RouteRunPlan::OpenTask { fallback_reason },
        super::intent::RouteDecision::PromptSkillInline { skill_id, .. } => {
            match workspace_skill_entries
                .iter()
                .find(|entry| entry.skill_id == skill_id)
                .map(build_routed_skill_tool_setup)
            {
                Some(setup) => RouteRunPlan::PromptSkillInline { skill_id, setup },
                None => RouteRunPlan::OpenTask {
                    fallback_reason: Some(RouteFallbackReason::InvalidSkillContract),
                },
            }
        }
        super::intent::RouteDecision::PromptSkillFork { skill_id, .. } => {
            match workspace_skill_entries
                .iter()
                .find(|entry| entry.skill_id == skill_id)
                .map(build_routed_skill_tool_setup)
            {
                Some(setup) => RouteRunPlan::PromptSkillFork { skill_id, setup },
                None => RouteRunPlan::OpenTask {
                    fallback_reason: Some(RouteFallbackReason::InvalidSkillContract),
                },
            }
        }
        super::intent::RouteDecision::DirectDispatchSkill { skill_id, .. } => {
            match workspace_skill_entries
                .iter()
                .find(|entry| entry.skill_id == skill_id)
            {
                Some(entry) => {
                    let setup = build_routed_skill_tool_setup(entry);
                    match command_specs
                        .iter()
                        .find(|spec| spec.skill_id == skill_id && spec.dispatch.is_some())
                        .cloned()
                    {
                        Some(command_spec) => {
                            match resolve_direct_dispatch_raw_args(
                                user_message,
                                &command_spec,
                                entry,
                            ) {
                                Some(raw_args) => RouteRunPlan::DirectDispatchSkill {
                                    skill_id,
                                    setup,
                                    command_spec,
                                    raw_args,
                                },
                                None => RouteRunPlan::OpenTask {
                                    fallback_reason: Some(
                                        RouteFallbackReason::DispatchArgumentResolutionFailed,
                                    ),
                                },
                            }
                        }
                        None => RouteRunPlan::OpenTask {
                            fallback_reason: Some(RouteFallbackReason::InvalidSkillContract),
                        },
                    }
                }
                None => RouteRunPlan::OpenTask {
                    fallback_reason: Some(RouteFallbackReason::InvalidSkillContract),
                },
            }
        }
    };

    PlannedImplicitRoute {
        observation: build_implicit_route_observation(
            &route_plan,
            candidates.len(),
            started_at.elapsed().as_millis() as u64,
        ),
        route_plan,
    }
}

pub(crate) async fn execute_implicit_route_plan(
    app: &AppHandle,
    agent_executor: &Arc<AgentExecutor>,
    db: &sqlx::SqlitePool,
    session_id: &str,
    run_id: &str,
    prepared_context: &crate::agent::runtime::session_runtime::PreparedSendMessageContext,
    route_plan: RouteRunPlan,
    cancel_flag: Arc<AtomicBool>,
    tool_confirm_responder: crate::agent::runtime::events::ToolConfirmResponder,
) -> Result<RouteRunOutcome, String> {
    match route_plan {
        RouteRunPlan::OpenTask { .. } => Ok(RouteRunOutcome::OpenTask),
        RouteRunPlan::DirectDispatchSkill {
            skill_id: _skill_id,
            setup,
            command_spec,
            raw_args,
        } => {
            let resolved_allowed_tools = resolve_skill_allowed_tools(
                agent_executor.registry(),
                &setup,
                &prepared_context.runtime_default_tool_policy,
                prepared_context.permission_mode,
            );
            let tool_ctx = build_tool_context(
                Some(session_id),
                prepared_context
                    .executor_work_dir
                    .as_ref()
                    .map(std::path::PathBuf::from),
                resolved_allowed_tools.as_deref(),
            )
            .map_err(|err| err.to_string())?;
            let dispatch_context = ToolDispatchContext {
                registry: agent_executor.registry(),
                app_handle: Some(app),
                session_id: Some(session_id),
                persisted_run_id: Some(run_id),
                allowed_tools: resolved_allowed_tools.as_deref(),
                effective_tool_plan: None,
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

            let output = dispatch_skill_command(&dispatch_context, &command_spec, &raw_args)
                .await
                .map_err(|err| err.to_string())?;
            Ok(RouteRunOutcome::DirectDispatch(output))
        }
        RouteRunPlan::PromptSkillInline { skill_id, setup } => {
            let execution_preparation_service = ChatExecutionPreparationService::new();
            let resolved_allowed_tools = resolve_skill_allowed_tools(
                agent_executor.registry(),
                &setup,
                &prepared_context.runtime_default_tool_policy,
                prepared_context.permission_mode,
            );
            let max_iter = RunBudgetPolicy::resolve(
                if skill_id.eq_ignore_ascii_case("builtin-general") {
                    RunBudgetScope::GeneralChat
                } else {
                    RunBudgetScope::Skill
                },
                setup.max_iterations,
            )
            .max_turns;

            let prepared_runtime_tools = prepare_runtime_tools(ToolSetupParams {
                app,
                db,
                agent_executor,
                workspace_skill_entries: &prepared_context.workspace_skill_entries,
                session_id,
                api_format: &prepared_context.route_candidates[0].1,
                base_url: &prepared_context.route_candidates[0].2,
                model_name: &prepared_context.route_candidates[0].3,
                api_key: &prepared_context.route_candidates[0].4,
                skill_id: &skill_id,
                source_type: &setup.source_type,
                pack_path: &setup.pack_path,
                permission_mode: prepared_context.permission_mode,
                runtime_default_tool_policy: prepared_context.runtime_default_tool_policy.clone(),
                skill_system_prompt: &setup.skill_system_prompt,
                skill_allowed_tools: resolved_allowed_tools.clone(),
                skill_denied_tools: setup.skill_denied_tools.clone(),
                skill_allowed_tool_sources: setup.skill_allowed_tool_sources.clone(),
                skill_denied_tool_sources: setup.skill_denied_tool_sources.clone(),
                skill_allowed_tool_categories: setup.skill_allowed_tool_categories.clone(),
                skill_denied_tool_categories: setup.skill_denied_tool_categories.clone(),
                skill_allowed_mcp_servers: setup.skill_allowed_mcp_servers.clone(),
                tool_discovery_query: Some(&prepared_context.user_message),
                max_iter,
                max_call_depth: prepared_context.max_call_depth,
                suppress_workspace_skills_prompt: false,
                execution_preparation_service: &execution_preparation_service,
                execution_guidance: &prepared_context.execution_guidance,
                memory_bucket_employee_id: &prepared_context.memory_bucket_employee_id,
                employee_collaboration_guidance: prepared_context
                    .employee_collaboration_guidance
                    .as_deref(),
            })
            .await?;

            let route_execution = execute_route_candidates(RouteExecutionParams {
                app,
                agent_executor: agent_executor.as_ref(),
                db,
                session_id,
                requested_capability: &prepared_context.requested_capability,
                route_candidates: &prepared_context.route_candidates,
                per_candidate_retry_count: prepared_context.per_candidate_retry_count,
                system_prompt: &prepared_runtime_tools.system_prompt,
                messages: &prepared_context.messages,
                allowed_tools: prepared_runtime_tools.allowed_tools.as_deref(),
                full_allowed_tools: Some(&prepared_runtime_tools.full_allowed_tools),
                has_deferred_tools: prepared_runtime_tools
                    .effective_tool_plan
                    .has_deferred_tools(),
                permission_mode: prepared_context.permission_mode,
                tool_confirm_responder,
                executor_work_dir: prepared_context.executor_work_dir.clone(),
                max_iterations: Some(max_iter),
                cancel_flag,
                node_timeout_seconds: prepared_context.node_timeout_seconds,
                route_retry_count: prepared_context.route_retry_count,
            })
            .await;

            Ok(RouteRunOutcome::Prompt {
                route_execution,
                reconstructed_history_len: prepared_context.messages.len(),
            })
        }
        RouteRunPlan::PromptSkillFork { skill_id, setup } => {
            let execution_preparation_service = ChatExecutionPreparationService::new();
            let resolved_allowed_tools = resolve_skill_allowed_tools(
                agent_executor.registry(),
                &setup,
                &prepared_context.runtime_default_tool_policy,
                prepared_context.permission_mode,
            );
            let max_iter = RunBudgetPolicy::resolve(
                if skill_id.eq_ignore_ascii_case("builtin-general") {
                    RunBudgetScope::GeneralChat
                } else {
                    RunBudgetScope::Skill
                },
                setup.max_iterations,
            )
            .max_turns;

            let prepared_runtime_tools = prepare_runtime_tools(ToolSetupParams {
                app,
                db,
                agent_executor,
                workspace_skill_entries: &prepared_context.workspace_skill_entries,
                session_id,
                api_format: &prepared_context.route_candidates[0].1,
                base_url: &prepared_context.route_candidates[0].2,
                model_name: &prepared_context.route_candidates[0].3,
                api_key: &prepared_context.route_candidates[0].4,
                skill_id: &skill_id,
                source_type: &setup.source_type,
                pack_path: &setup.pack_path,
                permission_mode: prepared_context.permission_mode,
                runtime_default_tool_policy: prepared_context.runtime_default_tool_policy.clone(),
                skill_system_prompt: &setup.skill_system_prompt,
                skill_allowed_tools: resolved_allowed_tools.clone(),
                skill_denied_tools: setup.skill_denied_tools.clone(),
                skill_allowed_tool_sources: setup.skill_allowed_tool_sources.clone(),
                skill_denied_tool_sources: setup.skill_denied_tool_sources.clone(),
                skill_allowed_tool_categories: setup.skill_allowed_tool_categories.clone(),
                skill_denied_tool_categories: setup.skill_denied_tool_categories.clone(),
                skill_allowed_mcp_servers: setup.skill_allowed_mcp_servers.clone(),
                tool_discovery_query: Some(&prepared_context.user_message),
                max_iter,
                max_call_depth: prepared_context.max_call_depth,
                suppress_workspace_skills_prompt: false,
                execution_preparation_service: &execution_preparation_service,
                execution_guidance: &prepared_context.execution_guidance,
                memory_bucket_employee_id: &prepared_context.memory_bucket_employee_id,
                employee_collaboration_guidance: prepared_context
                    .employee_collaboration_guidance
                    .as_deref(),
            })
            .await?;

            let fork_messages = build_fork_messages(&prepared_context.messages);
            let route_execution = execute_route_candidates(RouteExecutionParams {
                app,
                agent_executor: agent_executor.as_ref(),
                db,
                session_id,
                requested_capability: &prepared_context.requested_capability,
                route_candidates: &prepared_context.route_candidates,
                per_candidate_retry_count: prepared_context.per_candidate_retry_count,
                system_prompt: &prepared_runtime_tools.system_prompt,
                messages: &fork_messages,
                allowed_tools: prepared_runtime_tools.allowed_tools.as_deref(),
                full_allowed_tools: Some(&prepared_runtime_tools.full_allowed_tools),
                has_deferred_tools: prepared_runtime_tools
                    .effective_tool_plan
                    .has_deferred_tools(),
                permission_mode: prepared_context.permission_mode,
                tool_confirm_responder,
                executor_work_dir: prepared_context.executor_work_dir.clone(),
                max_iterations: Some(max_iter),
                cancel_flag,
                node_timeout_seconds: prepared_context.node_timeout_seconds,
                route_retry_count: prepared_context.route_retry_count,
            })
            .await;

            Ok(RouteRunOutcome::Prompt {
                route_execution,
                reconstructed_history_len: fork_messages.len(),
            })
        }
    }
}

fn strip_command_prefix<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let trimmed = value.trim_start();
    if trimmed.len() < prefix.len() {
        return None;
    }
    let (head, tail) = trimmed.split_at(prefix.len());
    if !head.eq_ignore_ascii_case(prefix) {
        return None;
    }
    if let Some(next) = tail.chars().next() {
        if !next.is_whitespace() {
            return None;
        }
    }
    Some(tail.trim_start())
}

fn direct_dispatch_prefixes(
    command_spec: &WorkspaceSkillCommandSpec,
    entry: &WorkspaceSkillRuntimeEntry,
) -> Vec<String> {
    let mut prefixes = vec![command_spec.name.clone(), command_spec.skill_name.clone()];
    prefixes.push(entry.skill_id.clone());
    prefixes.push(entry.name.clone());
    if let Some(metadata) = entry.metadata.as_ref() {
        if let Some(skill_key) = metadata.skill_key.as_ref() {
            prefixes.push(skill_key.clone());
        }
    }
    prefixes.retain(|value| !value.trim().is_empty());
    prefixes
}

fn is_safe_dispatch_fragment(fragment: &str) -> bool {
    let mut saw_signal = false;
    for ch in fragment.chars() {
        if ch.is_control() {
            return false;
        }
        if ch.is_alphanumeric()
            || ch.is_whitespace()
            || "-_=.,:/\\'\"[]{}()<>+|?&%#~!$".contains(ch)
        {
            if matches!(ch, '-' | '=' | ':') {
                saw_signal = true;
            }
            continue;
        }
        return false;
    }

    saw_signal
}

fn build_fork_messages(messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
    messages
        .last()
        .cloned()
        .map(|message| vec![message])
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::permissions::PermissionMode;
    use crate::agent::runtime::runtime_io::{
        WorkspaceSkillCommandSpec, WorkspaceSkillContent, WorkspaceSkillRuntimeEntry,
    };
    use crate::agent::runtime::skill_routing::index::SkillRouteIndex;
    use crate::agent::runtime::skill_routing::intent::RouteFallbackReason;
    use crate::agent::runtime::tool_registry_builder::{
        RuntimeToolRegistryBuilder, DEFAULT_BROWSER_SIDECAR_URL,
    };
    use crate::agent::tools::{ProcessManager, TaskTool};
    use crate::agent::ToolRegistry;
    use runtime_skill_core::{
        McpServerDep, OpenClawSkillMetadata, SkillCommandArgMode, SkillCommandDispatchKind,
        SkillCommandDispatchSpec, SkillConfig, SkillInvocationPolicy,
    };
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn build_entry(
        skill_id: &str,
        name: &str,
        description: &str,
        system_prompt: &str,
        context: Option<&str>,
        allowed_tools: Option<Vec<&str>>,
        max_iterations: Option<usize>,
        invocation: SkillInvocationPolicy,
        metadata_skill_key: Option<&str>,
        command_dispatch: Option<SkillCommandDispatchSpec>,
    ) -> WorkspaceSkillRuntimeEntry {
        let command_dispatch_for_config = command_dispatch.clone();
        let allowed_tools_for_config = allowed_tools
            .clone()
            .map(|values| values.into_iter().map(|value| value.to_string()).collect());
        WorkspaceSkillRuntimeEntry {
            skill_id: skill_id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            source_type: "local".to_string(),
            projected_dir_name: skill_id.to_string(),
            config: SkillConfig {
                name: Some(name.to_string()),
                description: Some(description.to_string()),
                allowed_tools: allowed_tools_for_config,
                denied_tools: None,
                allowed_tool_sources: None,
                denied_tool_sources: None,
                allowed_tool_categories: None,
                denied_tool_categories: None,
                model: None,
                max_iterations,
                argument_hint: None,
                disable_model_invocation: invocation.disable_model_invocation,
                user_invocable: invocation.user_invocable,
                invocation: invocation.clone(),
                metadata: metadata_skill_key.map(|skill_key| OpenClawSkillMetadata {
                    skill_key: Some(skill_key.to_string()),
                    ..Default::default()
                }),
                command_dispatch: command_dispatch_for_config,
                context: context.map(|value| value.to_string()),
                agent: None,
                mcp_servers: vec![],
                system_prompt: system_prompt.to_string(),
            },
            invocation,
            metadata: metadata_skill_key.map(|skill_key| OpenClawSkillMetadata {
                skill_key: Some(skill_key.to_string()),
                ..Default::default()
            }),
            command_dispatch,
            content: WorkspaceSkillContent::FileTree(std::collections::HashMap::new()),
        }
    }

    fn build_index(entries: Vec<WorkspaceSkillRuntimeEntry>) -> SkillRouteIndex {
        SkillRouteIndex::build(&entries)
    }

    fn build_command_specs(
        entries: &[WorkspaceSkillRuntimeEntry],
    ) -> Vec<WorkspaceSkillCommandSpec> {
        crate::agent::runtime::runtime_io::build_workspace_skill_command_specs(entries)
    }

    #[test]
    fn plan_implicit_route_routes_direct_dispatch_with_safe_args() {
        let entries = vec![
            build_entry(
                "feishu-pm-task-dispatch",
                "PM Task Dispatch",
                "Create or dispatch PM follow-up tasks",
                "## When to Use\n- Dispatch a correction task for a leader.\n",
                Some("fork"),
                Some(vec!["exec", "read_file"]),
                Some(11),
                SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: true,
                },
                Some("task-dispatch"),
                Some(SkillCommandDispatchSpec {
                    kind: SkillCommandDispatchKind::Tool,
                    tool_name: "exec".to_string(),
                    arg_mode: SkillCommandArgMode::Raw,
                }),
            ),
            build_entry(
                "feishu-pm-weekly-work-summary",
                "PM Weekly Summary",
                "Organize weekly reporting",
                "## When to Use\n- Keep reporting aligned.\n",
                None,
                Some(vec!["read_file"]),
                Some(3),
                SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: false,
                },
                Some("weekly-summary"),
                None,
            ),
        ];
        let index = build_index(entries.clone());
        let command_specs = build_command_specs(&entries);

        let plan = plan_implicit_route(
            &index,
            &entries,
            &command_specs,
            "task-dispatch --employee xt --date 2026-03-27",
        );

        match plan {
            RouteRunPlan::DirectDispatchSkill {
                skill_id,
                setup,
                command_spec,
                raw_args,
            } => {
                assert_eq!(skill_id, "feishu-pm-task-dispatch");
                assert_eq!(setup.skill_id, "feishu-pm-task-dispatch");
                assert_eq!(setup.source_type, "local");
                assert!(setup.pack_path.is_empty());
                assert_eq!(command_spec.skill_id, "feishu-pm-task-dispatch");
                assert_eq!(command_spec.name, "pm_task_dispatch");
                assert_eq!(raw_args, "--employee xt --date 2026-03-27");
            }
            other => panic!("expected direct-dispatch plan, got {:?}", other),
        }
    }

    #[test]
    fn plan_implicit_route_falls_back_when_dispatch_args_are_unsafe() {
        let entries = vec![
            build_entry(
                "feishu-pm-task-dispatch",
                "PM Task Dispatch",
                "Create or dispatch PM follow-up tasks",
                "## When to Use\n- Dispatch a correction task for a leader.\n",
                Some("fork"),
                Some(vec!["exec", "read_file"]),
                Some(11),
                SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: true,
                },
                Some("task-dispatch"),
                Some(SkillCommandDispatchSpec {
                    kind: SkillCommandDispatchKind::Tool,
                    tool_name: "exec".to_string(),
                    arg_mode: SkillCommandArgMode::Raw,
                }),
            ),
            build_entry(
                "feishu-pm-weekly-work-summary",
                "PM Weekly Summary",
                "Organize weekly reporting",
                "## When to Use\n- Keep reporting aligned.\n",
                None,
                Some(vec!["read_file"]),
                Some(3),
                SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: false,
                },
                Some("weekly-summary"),
                None,
            ),
        ];
        let index = build_index(entries.clone());
        let command_specs = build_command_specs(&entries);

        let plan = plan_implicit_route(&index, &entries, &command_specs, "task-dispatch");

        match plan {
            RouteRunPlan::OpenTask {
                fallback_reason: Some(RouteFallbackReason::DispatchArgumentResolutionFailed),
            } => {}
            other => panic!("expected fallback plan, got {:?}", other),
        }
    }

    #[test]
    fn resolve_direct_dispatch_raw_args_rejects_unprefixed_ascii_sentence() {
        let entry = build_entry(
            "feishu-pm-task-dispatch",
            "PM Task Dispatch",
            "Create or dispatch PM follow-up tasks",
            "## When to Use\n- Dispatch a correction task for a leader.\n",
            Some("fork"),
            Some(vec!["exec", "read_file"]),
            Some(11),
            SkillInvocationPolicy {
                user_invocable: true,
                disable_model_invocation: true,
            },
            Some("task-dispatch"),
            Some(SkillCommandDispatchSpec {
                kind: SkillCommandDispatchKind::Tool,
                tool_name: "exec".to_string(),
                arg_mode: SkillCommandArgMode::Raw,
            }),
        );
        let command_spec = build_command_specs(std::slice::from_ref(&entry))
            .into_iter()
            .find(|spec| spec.skill_id == "feishu-pm-task-dispatch")
            .expect("dispatch spec");

        let resolved = resolve_direct_dispatch_raw_args(
            "please assign xt --date 2026-03-27 and remind the owner",
            &command_spec,
            &entry,
        );

        assert!(resolved.is_none(), "unexpected raw args: {resolved:?}");
    }

    #[test]
    fn resolve_direct_dispatch_raw_args_accepts_multi_word_prefix_and_unicode_flags() {
        let entry = build_entry(
            "feishu-pm-task-dispatch",
            "PM Task Dispatch",
            "Create or dispatch PM follow-up tasks",
            "## When to Use\n- Dispatch a correction task for a leader.\n",
            Some("fork"),
            Some(vec!["exec", "read_file"]),
            Some(11),
            SkillInvocationPolicy {
                user_invocable: true,
                disable_model_invocation: true,
            },
            Some("task-dispatch"),
            Some(SkillCommandDispatchSpec {
                kind: SkillCommandDispatchKind::Tool,
                tool_name: "exec".to_string(),
                arg_mode: SkillCommandArgMode::Raw,
            }),
        );
        let command_spec = build_command_specs(std::slice::from_ref(&entry))
            .into_iter()
            .find(|spec| spec.skill_id == "feishu-pm-task-dispatch")
            .expect("dispatch spec");

        let resolved = resolve_direct_dispatch_raw_args(
            "PM Task Dispatch --employee 郝敬 --title 测试任务",
            &command_spec,
            &entry,
        );

        assert_eq!(
            resolved.as_deref(),
            Some("--employee 郝敬 --title 测试任务")
        );
    }

    #[test]
    fn plan_implicit_route_uses_route_entry_config_for_prompt_inline() {
        let entries = vec![
            build_entry(
                "feishu-pm-daily-sync",
                "PM Daily Sync",
                "同步项管日报到看板",
                "## When to Use\n- 同步项管日报到看板并更新状态。\n",
                None,
                Some(vec!["read_file", "edit"]),
                Some(4),
                SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: false,
                },
                Some("daily-sync"),
                None,
            ),
            build_entry(
                "feishu-pm-weekly-work-summary",
                "PM Weekly Summary",
                "项管日报汇总",
                "## When to Use\n- 汇总项管日报并整理任务。\n",
                None,
                Some(vec!["read_file"]),
                Some(3),
                SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: false,
                },
                Some("weekly-summary"),
                None,
            ),
        ];
        let index = build_index(entries.clone());
        let command_specs = build_command_specs(&entries);

        let plan = plan_implicit_route(&index, &entries, &command_specs, "帮我同步项管日报到看板");

        match plan {
            RouteRunPlan::PromptSkillInline { skill_id, setup } => {
                assert_eq!(skill_id, "feishu-pm-daily-sync");
                assert_eq!(setup.skill_id, "feishu-pm-daily-sync");
                assert_eq!(setup.source_type, "local");
                assert!(setup.pack_path.is_empty());
                assert_eq!(
                    setup.skill_system_prompt,
                    "## When to Use\n- 同步项管日报到看板并更新状态。\n"
                );
                assert_eq!(
                    setup.skill_allowed_tools,
                    Some(vec!["read_file".to_string(), "edit".to_string()])
                );
                assert_eq!(setup.max_iterations, Some(4));
            }
            other => panic!("expected prompt-inline plan, got {:?}", other),
        }
    }

    #[test]
    fn plan_implicit_route_uses_fork_lane_for_mixed_case_context() {
        let entries = vec![
            build_entry(
                "feishu-pm-fork-sync",
                "PM Fork Sync",
                "同步项管日报到看板",
                "## When to Use\n- 同步项管日报到看板并更新状态。\n",
                Some("FoRk"),
                Some(vec!["read_file"]),
                Some(4),
                SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: false,
                },
                Some("fork-sync"),
                None,
            ),
            build_entry(
                "feishu-pm-weekly-work-summary",
                "PM Weekly Summary",
                "项管日报汇总",
                "## When to Use\n- 汇总项管日报并整理任务。\n",
                None,
                Some(vec!["read_file"]),
                Some(3),
                SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: false,
                },
                Some("weekly-summary"),
                None,
            ),
        ];
        let index = build_index(entries.clone());
        let command_specs = build_command_specs(&entries);

        let plan = plan_implicit_route(&index, &entries, &command_specs, "帮我同步项管日报到看板");

        match plan {
            RouteRunPlan::PromptSkillFork { skill_id, setup } => {
                assert_eq!(skill_id, "feishu-pm-fork-sync");
                assert_eq!(setup.skill_id, "feishu-pm-fork-sync");
                assert_eq!(setup.source_type, "local");
                assert!(setup.pack_path.is_empty());
            }
            other => panic!("expected prompt-fork plan, got {:?}", other),
        }
    }

    #[test]
    fn build_fork_messages_keeps_only_latest_turn() {
        let messages = vec![
            serde_json::json!({"role": "user", "content": "earlier"}),
            serde_json::json!({"role": "assistant", "content": "middle"}),
            serde_json::json!({"role": "user", "content": "latest"}),
        ];

        let fork_messages = build_fork_messages(&messages);

        assert_eq!(fork_messages.len(), 1);
        assert_eq!(fork_messages[0]["content"].as_str(), Some("latest"));
    }

    #[test]
    fn plan_implicit_route_returns_open_task_when_no_candidates() {
        let entries = vec![build_entry(
            "feishu-pm-task-dispatch",
            "PM Task Dispatch",
            "Create or dispatch PM follow-up tasks",
            "## When to Use\n- Dispatch a correction task for a leader.\n",
            Some("fork"),
            Some(vec!["exec", "read_file"]),
            Some(11),
            SkillInvocationPolicy {
                user_invocable: true,
                disable_model_invocation: true,
            },
            Some("task-dispatch"),
            Some(SkillCommandDispatchSpec {
                kind: SkillCommandDispatchKind::Tool,
                tool_name: "exec".to_string(),
                arg_mode: SkillCommandArgMode::Raw,
            }),
        )];
        let index = build_index(entries.clone());
        let command_specs = build_command_specs(&entries);

        let plan = plan_implicit_route(&index, &entries, &command_specs, "完全无关的查询");

        match plan {
            RouteRunPlan::OpenTask {
                fallback_reason: Some(RouteFallbackReason::NoCandidates),
            } => {}
            other => panic!("expected open-task plan, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn resolve_skill_allowed_tools_uses_inferred_browser_profile_when_explicit_list_missing()
    {
        let entry = build_entry(
            "browser-research-skill",
            "Browser Research Skill",
            "Research with browser automation",
            "## When to Use\n- Use browser automation for research.\n",
            None,
            None,
            Some(5),
            SkillInvocationPolicy {
                user_invocable: true,
                disable_model_invocation: false,
            },
            None,
            None,
        );
        let setup = build_routed_skill_tool_setup(&entry);
        let registry = Arc::new(ToolRegistry::with_standard_tools());
        let builder = RuntimeToolRegistryBuilder::new(registry.as_ref());
        builder.register_process_shell_tools(Arc::new(ProcessManager::new()));
        builder.register_browser_and_alias_tools(DEFAULT_BROWSER_SIDECAR_URL);
        builder.register_skill_and_compaction_tools("sess-profile", Vec::new(), 2);

        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let memory_dir = tempdir().expect("temp dir");
        let task_tool = TaskTool::new(
            Arc::clone(&registry),
            "openai".to_string(),
            "https://example.com".to_string(),
            "test-key".to_string(),
            "gpt-4o-mini".to_string(),
        );
        builder.register_runtime_support_tools(task_tool, db, memory_dir.path().to_path_buf());

        let allowed_tools = resolve_skill_allowed_tools(
            registry.as_ref(),
            &setup,
            &crate::agent::runtime::effective_tool_set::runtime_default_tool_policy_input(
                "runtime_preferences:standard".to_string(),
                Vec::new(),
                Vec::new(),
                None,
                None,
            ),
            PermissionMode::AcceptEdits,
        )
        .expect("profile allowed tools");

        assert_eq!(setup.tool_profile, Some(ToolProfileName::Browser));
        assert!(allowed_tools.contains(&"browser_launch".to_string()));
        assert!(allowed_tools.contains(&"browser_snapshot".to_string()));
        assert!(!allowed_tools.contains(&"bash".to_string()));
    }

    #[test]
    fn resolve_skill_allowed_tools_prefers_explicit_list_over_profile() {
        let entry = build_entry(
            "browser-research-skill",
            "Browser Research Skill",
            "Research with browser automation",
            "## When to Use\n- Use browser automation for research.\n",
            None,
            Some(vec!["read_file", "web_fetch"]),
            Some(5),
            SkillInvocationPolicy {
                user_invocable: true,
                disable_model_invocation: false,
            },
            None,
            None,
        );
        let setup = build_routed_skill_tool_setup(&entry);
        let registry = ToolRegistry::with_standard_tools();

        let allowed_tools = resolve_skill_allowed_tools(
            &registry,
            &setup,
            &crate::agent::runtime::effective_tool_set::runtime_default_tool_policy_input(
                "runtime_preferences:standard".to_string(),
                Vec::new(),
                Vec::new(),
                None,
                None,
            ),
            PermissionMode::AcceptEdits,
        )
        .expect("explicit allowed tools");

        assert_eq!(setup.tool_profile, None);
        assert_eq!(
            allowed_tools,
            vec!["read_file".to_string(), "web_fetch".to_string()]
        );
    }

    #[test]
    fn resolve_skill_allowed_tools_applies_declared_denied_tools() {
        let entry = WorkspaceSkillRuntimeEntry {
            skill_id: "deny-bash-skill".to_string(),
            name: "Deny Bash Skill".to_string(),
            description: "Prevent bash usage".to_string(),
            source_type: "local".to_string(),
            projected_dir_name: "deny-bash-skill".to_string(),
            config: SkillConfig {
                name: Some("Deny Bash Skill".to_string()),
                description: Some("Prevent bash usage".to_string()),
                allowed_tools: Some(vec!["read_file".to_string(), "bash".to_string()]),
                denied_tools: Some(vec!["bash".to_string()]),
                allowed_tool_sources: None,
                denied_tool_sources: None,
                allowed_tool_categories: None,
                denied_tool_categories: Some(vec!["shell".to_string()]),
                model: None,
                max_iterations: Some(3),
                argument_hint: None,
                disable_model_invocation: false,
                user_invocable: true,
                invocation: SkillInvocationPolicy::default(),
                metadata: None,
                command_dispatch: None,
                context: None,
                agent: None,
                mcp_servers: vec![],
                system_prompt: "Use safe tools".to_string(),
            },
            invocation: SkillInvocationPolicy::default(),
            metadata: None,
            command_dispatch: None,
            content: WorkspaceSkillContent::FileTree(std::collections::HashMap::new()),
        };
        let setup = build_routed_skill_tool_setup(&entry);
        let registry = ToolRegistry::with_standard_tools();

        let allowed_tools = resolve_skill_allowed_tools(
            &registry,
            &setup,
            &crate::agent::runtime::effective_tool_set::runtime_default_tool_policy_input(
                "runtime_preferences:standard".to_string(),
                Vec::new(),
                Vec::new(),
                None,
                None,
            ),
            PermissionMode::AcceptEdits,
        )
        .expect("resolved allowed tools");

        assert_eq!(setup.skill_denied_tools, Some(vec!["bash".to_string()]));
        assert_eq!(allowed_tools, vec!["read_file".to_string()]);
    }

    #[test]
    fn resolve_skill_allowed_tools_applies_declared_allowed_tool_sources() {
        let entry = WorkspaceSkillRuntimeEntry {
            skill_id: "mcp-only-skill".to_string(),
            name: "MCP Only Skill".to_string(),
            description: "Allow only MCP-backed tools".to_string(),
            source_type: "local".to_string(),
            projected_dir_name: "mcp-only-skill".to_string(),
            config: SkillConfig {
                name: Some("MCP Only Skill".to_string()),
                description: Some("Allow only MCP-backed tools".to_string()),
                allowed_tools: Some(vec![
                    "read_file".to_string(),
                    "mcp_repo_files_read".to_string(),
                ]),
                denied_tools: None,
                allowed_tool_sources: Some(vec!["mcp".to_string()]),
                denied_tool_sources: None,
                allowed_tool_categories: None,
                denied_tool_categories: None,
                model: None,
                max_iterations: Some(3),
                argument_hint: None,
                disable_model_invocation: false,
                user_invocable: true,
                invocation: SkillInvocationPolicy::default(),
                metadata: None,
                command_dispatch: None,
                context: None,
                agent: None,
                mcp_servers: vec![],
                system_prompt: "Use MCP tools only".to_string(),
            },
            invocation: SkillInvocationPolicy::default(),
            metadata: None,
            command_dispatch: None,
            content: WorkspaceSkillContent::FileTree(std::collections::HashMap::new()),
        };
        let setup = build_routed_skill_tool_setup(&entry);
        let registry = ToolRegistry::new();
        registry.register(Arc::new(FakeTool {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            source: ToolSource::Native,
            category: ToolCategory::File,
        }));
        registry.register(Arc::new(FakeTool {
            name: "mcp_repo_files_read".to_string(),
            description: "Read from repo MCP".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            source: ToolSource::Mcp,
            category: ToolCategory::Integration,
        }));

        let allowed_tools = resolve_skill_allowed_tools(
            &registry,
            &setup,
            &crate::agent::runtime::effective_tool_set::runtime_default_tool_policy_input(
                "runtime_preferences:standard".to_string(),
                Vec::new(),
                Vec::new(),
                None,
                None,
            ),
            PermissionMode::AcceptEdits,
        )
        .expect("resolved allowed tools");

        assert_eq!(
            setup.skill_allowed_tool_sources,
            Some(vec![ToolSource::Mcp])
        );
        assert_eq!(allowed_tools, vec!["mcp_repo_files_read".to_string()]);
    }

    #[test]
    fn resolve_skill_allowed_tools_applies_declared_denied_tool_sources() {
        let entry = WorkspaceSkillRuntimeEntry {
            skill_id: "deny-runtime-sources".to_string(),
            name: "Deny Runtime Sources".to_string(),
            description: "Filter out runtime-backed tools".to_string(),
            source_type: "local".to_string(),
            projected_dir_name: "deny-runtime-sources".to_string(),
            config: SkillConfig {
                name: Some("Deny Runtime Sources".to_string()),
                description: Some("Filter out runtime-backed tools".to_string()),
                allowed_tools: Some(vec!["read_file".to_string(), "bash".to_string()]),
                denied_tools: None,
                allowed_tool_sources: None,
                denied_tool_sources: Some(vec!["runtime".to_string()]),
                allowed_tool_categories: None,
                denied_tool_categories: None,
                model: None,
                max_iterations: Some(3),
                argument_hint: None,
                disable_model_invocation: false,
                user_invocable: true,
                invocation: SkillInvocationPolicy::default(),
                metadata: None,
                command_dispatch: None,
                context: None,
                agent: None,
                mcp_servers: vec![],
                system_prompt: "Avoid runtime tools".to_string(),
            },
            invocation: SkillInvocationPolicy::default(),
            metadata: None,
            command_dispatch: None,
            content: WorkspaceSkillContent::FileTree(std::collections::HashMap::new()),
        };
        let setup = build_routed_skill_tool_setup(&entry);
        let registry = ToolRegistry::with_standard_tools();

        let allowed_tools = resolve_skill_allowed_tools(
            &registry,
            &setup,
            &crate::agent::runtime::effective_tool_set::runtime_default_tool_policy_input(
                "runtime_preferences:standard".to_string(),
                Vec::new(),
                Vec::new(),
                None,
                None,
            ),
            PermissionMode::AcceptEdits,
        )
        .expect("resolved allowed tools");

        assert_eq!(
            setup.skill_denied_tool_sources,
            Some(vec![ToolSource::Runtime])
        );
        assert_eq!(allowed_tools, vec!["read_file".to_string()]);
    }

    #[test]
    fn resolve_skill_allowed_tools_applies_declared_allowed_tool_categories() {
        let entry = WorkspaceSkillRuntimeEntry {
            skill_id: "file-only-skill".to_string(),
            name: "File Only Skill".to_string(),
            description: "Allow only file-category tools".to_string(),
            source_type: "local".to_string(),
            projected_dir_name: "file-only-skill".to_string(),
            config: SkillConfig {
                name: Some("File Only Skill".to_string()),
                description: Some("Allow only file-category tools".to_string()),
                allowed_tools: Some(vec!["read_file".to_string(), "web_fetch".to_string()]),
                denied_tools: None,
                allowed_tool_sources: None,
                denied_tool_sources: None,
                allowed_tool_categories: Some(vec!["file".to_string()]),
                denied_tool_categories: None,
                model: None,
                max_iterations: Some(3),
                argument_hint: None,
                disable_model_invocation: false,
                user_invocable: true,
                invocation: SkillInvocationPolicy::default(),
                metadata: None,
                command_dispatch: None,
                context: None,
                agent: None,
                mcp_servers: vec![],
                system_prompt: "Use file tools only".to_string(),
            },
            invocation: SkillInvocationPolicy::default(),
            metadata: None,
            command_dispatch: None,
            content: WorkspaceSkillContent::FileTree(std::collections::HashMap::new()),
        };
        let setup = build_routed_skill_tool_setup(&entry);
        let registry = ToolRegistry::with_standard_tools();

        let allowed_tools = resolve_skill_allowed_tools(
            &registry,
            &setup,
            &crate::agent::runtime::effective_tool_set::runtime_default_tool_policy_input(
                "runtime_preferences:standard".to_string(),
                Vec::new(),
                Vec::new(),
                None,
                None,
            ),
            PermissionMode::AcceptEdits,
        )
        .expect("resolved allowed tools");

        assert_eq!(
            setup.skill_allowed_tool_categories,
            Some(vec![ToolCategory::File])
        );
        assert_eq!(allowed_tools, vec!["read_file".to_string()]);
    }

    #[test]
    fn build_routed_skill_tool_setup_carries_declared_mcp_server_names() {
        let mut entry = build_entry(
            "repo-research-skill",
            "Repo Research Skill",
            "Research with repo MCP tools",
            "## When to Use\n- Use MCP tools for repo research.\n",
            None,
            None,
            Some(5),
            SkillInvocationPolicy {
                user_invocable: true,
                disable_model_invocation: false,
            },
            None,
            None,
        );
        entry.config.mcp_servers = vec![
            McpServerDep {
                name: "repo-files".to_string(),
                command: None,
                args: None,
                env: None,
            },
            McpServerDep {
                name: "brave search".to_string(),
                command: None,
                args: None,
                env: None,
            },
        ];

        let setup = build_routed_skill_tool_setup(&entry);

        assert_eq!(
            setup.skill_allowed_mcp_servers,
            Some(vec!["repo-files".to_string(), "brave search".to_string()])
        );
    }

    struct FakeTool {
        name: String,
        description: String,
        schema: serde_json::Value,
        source: ToolSource,
        category: ToolCategory,
    }

    impl crate::agent::Tool for FakeTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        fn input_schema(&self) -> serde_json::Value {
            self.schema.clone()
        }

        fn execute(
            &self,
            _input: serde_json::Value,
            _ctx: &crate::agent::ToolContext,
        ) -> anyhow::Result<String> {
            Ok("ok".to_string())
        }

        fn metadata(&self) -> crate::agent::tool_manifest::ToolMetadata {
            crate::agent::tool_manifest::ToolMetadata {
                source: self.source,
                category: self.category,
                ..Default::default()
            }
        }
    }
}
