use super::adjudicator::adjudicate_route;
use super::index::SkillRouteIndex;
use super::intent::RouteFallbackReason;
use super::observability::{build_implicit_route_observation, PlannedImplicitRoute};
use super::recall::recall_skill_candidates;
use crate::agent::runtime::kernel::direct_dispatch::execute_direct_dispatch_skill;
use crate::agent::runtime::kernel::execution_plan::{
    ContinuationPreference, ExecutionContext, ExecutionPlan, TurnContext,
};
use crate::agent::runtime::kernel::route_lane::{
    build_routed_skill_tool_setup, resolve_skill_allowed_tools, RouteRunOutcome, RouteRunPlan,
};
use crate::agent::runtime::kernel::routed_prompt::{
    execute_routed_prompt, prepare_routed_prompt, RoutedPromptExecutionParams,
    RoutedPromptPreparationParams,
};
use crate::agent::runtime::runtime_io::{
    WorkspaceSkillCommandSpec, WorkspaceSkillRouteExecutionMode, WorkspaceSkillRuntimeEntry,
};
use crate::agent::AgentExecutor;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;

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
        None,
    )
    .execution_plan
    .route_plan
}

pub(crate) fn plan_implicit_route_with_observation(
    route_index: &SkillRouteIndex,
    workspace_skill_entries: &[WorkspaceSkillRuntimeEntry],
    command_specs: &[WorkspaceSkillCommandSpec],
    user_message: &str,
    continuation_preference: Option<&ContinuationPreference>,
) -> PlannedImplicitRoute {
    let started_at = std::time::Instant::now();
    let candidates = recall_skill_candidates(route_index, user_message);
    let route_plan = resolve_continuation_route_plan(
        route_index,
        workspace_skill_entries,
        continuation_preference,
    )
    .unwrap_or_else(|| {
        let decision = adjudicate_route(&candidates);
        route_plan_from_decision(
            workspace_skill_entries,
            command_specs,
            user_message,
            decision,
        )
    });
    let execution_plan = ExecutionPlan::from_route_plan(route_plan.clone());

    PlannedImplicitRoute {
        observation: build_implicit_route_observation(
            &route_plan,
            candidates.len(),
            started_at.elapsed().as_millis() as u64,
        ),
        execution_plan,
    }
}

fn resolve_continuation_route_plan(
    route_index: &SkillRouteIndex,
    workspace_skill_entries: &[WorkspaceSkillRuntimeEntry],
    continuation_preference: Option<&ContinuationPreference>,
) -> Option<RouteRunPlan> {
    let preference = continuation_preference?;
    let selected_skill = preference.selected_skill.as_deref()?;
    let projection = route_index.get(selected_skill)?;
    let entry = workspace_skill_entries
        .iter()
        .find(|entry| entry.skill_id == selected_skill)?;
    let setup = build_routed_skill_tool_setup(entry);

    match projection.execution_mode {
        WorkspaceSkillRouteExecutionMode::Inline => Some(RouteRunPlan::PromptSkillInline {
            skill_id: selected_skill.to_string(),
            setup,
        }),
        WorkspaceSkillRouteExecutionMode::Fork => Some(RouteRunPlan::PromptSkillFork {
            skill_id: selected_skill.to_string(),
            setup,
        }),
        WorkspaceSkillRouteExecutionMode::DirectDispatch => None,
    }
}

fn route_plan_from_decision(
    workspace_skill_entries: &[WorkspaceSkillRuntimeEntry],
    command_specs: &[WorkspaceSkillCommandSpec],
    user_message: &str,
    decision: super::intent::RouteDecision,
) -> RouteRunPlan {
    match decision {
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
    }
}

pub(crate) async fn execute_planned_route(
    app: &AppHandle,
    agent_executor: &Arc<AgentExecutor>,
    db: &sqlx::SqlitePool,
    session_id: &str,
    run_id: &str,
    turn_context: &TurnContext,
    execution_context: &ExecutionContext,
    execution_plan: &ExecutionPlan,
    cancel_flag: Arc<AtomicBool>,
    tool_confirm_responder: crate::agent::runtime::events::ToolConfirmResponder,
) -> Result<RouteRunOutcome, String> {
    match execution_plan.lane {
        crate::agent::runtime::kernel::execution_plan::ExecutionLane::OpenTask => {
            Ok(RouteRunOutcome::OpenTask)
        }
        crate::agent::runtime::kernel::execution_plan::ExecutionLane::PromptInline
        | crate::agent::runtime::kernel::execution_plan::ExecutionLane::PromptFork
        | crate::agent::runtime::kernel::execution_plan::ExecutionLane::DirectDispatch => {
            execute_implicit_route_plan(
                app,
                agent_executor,
                db,
                session_id,
                run_id,
                turn_context,
                execution_context,
                execution_plan.route_plan.clone(),
                cancel_flag,
                tool_confirm_responder,
            )
            .await
        }
    }
}

pub(crate) async fn execute_implicit_route_plan(
    app: &AppHandle,
    agent_executor: &Arc<AgentExecutor>,
    db: &sqlx::SqlitePool,
    session_id: &str,
    run_id: &str,
    turn_context: &TurnContext,
    execution_context: &ExecutionContext,
    route_plan: RouteRunPlan,
    cancel_flag: Arc<AtomicBool>,
    tool_confirm_responder: crate::agent::runtime::events::ToolConfirmResponder,
) -> Result<RouteRunOutcome, String> {
    let primary_route_candidate = turn_context
        .primary_route_candidate()
        .ok_or_else(|| "route planning produced no model candidates".to_string())?;

    match route_plan {
        RouteRunPlan::OpenTask { .. } => Ok(RouteRunOutcome::OpenTask),
        RouteRunPlan::DirectDispatchSkill {
            skill_id: _skill_id,
            mut setup,
            command_spec,
            raw_args,
        } => {
            setup.skill_allowed_tools = resolve_skill_allowed_tools(
                agent_executor.registry(),
                &setup,
                &execution_context.runtime_default_tool_policy,
                execution_context.permission_mode,
            );
            let output = execute_direct_dispatch_skill(
                app,
                agent_executor,
                session_id,
                run_id,
                execution_context,
                &setup,
                &command_spec,
                &raw_args,
                cancel_flag,
                &tool_confirm_responder,
            )
            .await?;
            Ok(RouteRunOutcome::DirectDispatch(output))
        }
        RouteRunPlan::PromptSkillInline { skill_id, setup } => {
            let resolved_allowed_tools = resolve_skill_allowed_tools(
                agent_executor.registry(),
                &setup,
                &execution_context.runtime_default_tool_policy,
                execution_context.permission_mode,
            );
            let prepared_prompt = prepare_routed_prompt(RoutedPromptPreparationParams {
                app,
                db,
                agent_executor,
                session_id,
                turn_context,
                execution_context,
                api_format: &primary_route_candidate.1,
                base_url: &primary_route_candidate.2,
                model_name: &primary_route_candidate.3,
                api_key: &primary_route_candidate.4,
                skill_id: &skill_id,
                skill_system_prompt: &setup.skill_system_prompt,
                skill_allowed_tools: resolved_allowed_tools,
                skill_denied_tools: setup.skill_denied_tools.clone(),
                skill_allowed_tool_sources: setup.skill_allowed_tool_sources.clone(),
                skill_denied_tool_sources: setup.skill_denied_tool_sources.clone(),
                skill_allowed_tool_categories: setup.skill_allowed_tool_categories.clone(),
                skill_denied_tool_categories: setup.skill_denied_tool_categories.clone(),
                skill_allowed_mcp_servers: setup.skill_allowed_mcp_servers.clone(),
                skill_max_iterations: setup.max_iterations,
                source_type: &setup.source_type,
                pack_path: &setup.pack_path,
            })
            .await?;

            let route_execution = execute_routed_prompt(RoutedPromptExecutionParams {
                app,
                agent_executor,
                db,
                session_id,
                turn_context,
                execution_context,
                prepared_prompt: &prepared_prompt,
                messages: &turn_context.messages,
                tool_confirm_responder,
                cancel_flag,
            })
            .await;

            Ok(RouteRunOutcome::Prompt {
                route_execution,
                reconstructed_history_len: turn_context.messages.len(),
            })
        }
        RouteRunPlan::PromptSkillFork { skill_id, setup } => {
            let resolved_allowed_tools = resolve_skill_allowed_tools(
                agent_executor.registry(),
                &setup,
                &execution_context.runtime_default_tool_policy,
                execution_context.permission_mode,
            );
            let prepared_prompt = prepare_routed_prompt(RoutedPromptPreparationParams {
                app,
                db,
                agent_executor,
                session_id,
                turn_context,
                execution_context,
                api_format: &primary_route_candidate.1,
                base_url: &primary_route_candidate.2,
                model_name: &primary_route_candidate.3,
                api_key: &primary_route_candidate.4,
                skill_id: &skill_id,
                skill_system_prompt: &setup.skill_system_prompt,
                skill_allowed_tools: resolved_allowed_tools,
                skill_denied_tools: setup.skill_denied_tools.clone(),
                skill_allowed_tool_sources: setup.skill_allowed_tool_sources.clone(),
                skill_denied_tool_sources: setup.skill_denied_tool_sources.clone(),
                skill_allowed_tool_categories: setup.skill_allowed_tool_categories.clone(),
                skill_denied_tool_categories: setup.skill_denied_tool_categories.clone(),
                skill_allowed_mcp_servers: setup.skill_allowed_mcp_servers.clone(),
                skill_max_iterations: setup.max_iterations,
                source_type: &setup.source_type,
                pack_path: &setup.pack_path,
            })
            .await?;

            let fork_messages = build_fork_messages(&turn_context.messages);
            let route_execution = execute_routed_prompt(RoutedPromptExecutionParams {
                app,
                agent_executor,
                db,
                session_id,
                turn_context,
                execution_context,
                prepared_prompt: &prepared_prompt,
                messages: &fork_messages,
                tool_confirm_responder,
                cancel_flag,
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
    let head = trimmed.get(..prefix.len())?;
    let tail = trimmed.get(prefix.len()..)?;
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
    use crate::agent::runtime::kernel::execution_plan::{
        ContinuationKind, ContinuationPreference, ContinuationTurnPolicy,
    };
    use crate::agent::runtime::runtime_io::{
        WorkspaceSkillCommandSpec, WorkspaceSkillContent, WorkspaceSkillRuntimeEntry,
    };
    use crate::agent::runtime::skill_routing::index::SkillRouteIndex;
    use crate::agent::runtime::skill_routing::intent::RouteFallbackReason;
    use runtime_skill_core::{
        OpenClawSkillMetadata, SkillCommandArgMode, SkillCommandDispatchKind,
        SkillCommandDispatchSpec, SkillConfig, SkillInvocationPolicy,
    };

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
    fn strip_command_prefix_returns_none_for_unicode_sentence_without_panicking() {
        let resolved = strip_command_prefix(
            "获取谢涛2026年3月30日到4月4日的工作日报并汇总成简报",
            "PM Task Dispatch",
        );

        assert!(resolved.is_none());
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
    fn plan_implicit_route_with_observation_attaches_execution_plan() {
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

        let planned_route = plan_implicit_route_with_observation(
            &index,
            &entries,
            &command_specs,
            "帮我同步项管日报到看板",
            None,
        );

        assert_eq!(
            planned_route.execution_plan.lane,
            crate::agent::runtime::kernel::execution_plan::ExecutionLane::PromptInline
        );
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

    #[test]
    fn plan_implicit_route_with_observation_prefers_recent_prompt_skill_for_continuation() {
        let entries = vec![
            build_entry(
                "feishu-pm-fork-sync",
                "PM Fork Sync",
                "同步项管日报到看板",
                "## When to Use\n- 同步项管日报到看板并更新状态。\n",
                Some("fork"),
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
        let continuation_preference = ContinuationPreference {
            kind: ContinuationKind::CompactionRecovery,
            selected_skill: Some("feishu-pm-fork-sync".to_string()),
            selected_runner: Some("prompt_skill_fork".to_string()),
            reconstructed_history_len: Some(6),
            turn_policy: ContinuationTurnPolicy {
                per_candidate_retry_count: Some(0),
                route_retry_count: Some(0),
            },
        };

        let planned_route = plan_implicit_route_with_observation(
            &index,
            &entries,
            &command_specs,
            "继续",
            Some(&continuation_preference),
        );

        assert_eq!(
            planned_route.observation.selected_skill.as_deref(),
            Some("feishu-pm-fork-sync")
        );
        assert_eq!(
            planned_route.observation.selected_runner,
            "prompt_skill_fork"
        );
        assert!(matches!(
            planned_route.execution_plan.route_plan,
            RouteRunPlan::PromptSkillFork { ref skill_id, .. }
                if skill_id == "feishu-pm-fork-sync"
        ));
    }
}
