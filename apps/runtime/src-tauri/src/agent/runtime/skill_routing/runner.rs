use super::adjudicator::adjudicate_route;
use super::index::SkillRouteIndex;
use super::intent::RouteFallbackReason;
use super::recall::recall_skill_candidates;
use crate::agent::context::build_tool_context;
use crate::agent::run_guard::{RunBudgetPolicy, RunBudgetScope};
use crate::agent::runtime::attempt_runner::{execute_route_candidates, RouteExecutionOutcome, RouteExecutionParams};
use crate::agent::runtime::runtime_io::{
    WorkspaceSkillCommandSpec, WorkspaceSkillRuntimeEntry,
};
use crate::agent::runtime::tool_dispatch::{dispatch_skill_command, ToolDispatchContext};
use crate::agent::runtime::tool_setup::{prepare_runtime_tools, ToolSetupParams};
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
    pub max_iterations: Option<usize>,
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
    Prompt(RouteExecutionOutcome),
}

pub(crate) fn build_routed_skill_tool_setup(entry: &WorkspaceSkillRuntimeEntry) -> RoutedSkillToolSetup {
    RoutedSkillToolSetup {
        skill_id: entry.skill_id.clone(),
        skill_system_prompt: entry.config.system_prompt.clone(),
        skill_allowed_tools: entry.config.allowed_tools.clone(),
        max_iterations: entry.config.max_iterations,
    }
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
        .find_map(|prefix| strip_command_prefix(trimmed, &prefix))
        .unwrap_or(trimmed)
        .trim();

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
    let candidates = recall_skill_candidates(route_index, user_message);
    let decision = adjudicate_route(&candidates);

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
            let Some(entry) = workspace_skill_entries
                .iter()
                .find(|entry| entry.skill_id == skill_id)
            else {
                return RouteRunPlan::OpenTask {
                    fallback_reason: Some(RouteFallbackReason::InvalidSkillContract),
                };
            };
            let setup = build_routed_skill_tool_setup(entry);
            let Some(command_spec) = command_specs
                .iter()
                .find(|spec| spec.skill_id == skill_id && spec.dispatch.is_some())
                .cloned()
            else {
                return RouteRunPlan::OpenTask {
                    fallback_reason: Some(RouteFallbackReason::InvalidSkillContract),
                };
            };
            let Some(raw_args) = resolve_direct_dispatch_raw_args(user_message, &command_spec, entry) else {
                return RouteRunPlan::OpenTask {
                    fallback_reason: Some(RouteFallbackReason::DispatchArgumentResolutionFailed),
                };
            };

            RouteRunPlan::DirectDispatchSkill {
                skill_id,
                setup,
                command_spec,
                raw_args,
            }
        }
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
            let tool_ctx = build_tool_context(
                Some(session_id),
                prepared_context
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
                allowed_tools: setup.skill_allowed_tools.as_deref(),
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
        RouteRunPlan::PromptSkillInline { skill_id, setup }
        | RouteRunPlan::PromptSkillFork { skill_id, setup } => {
            let execution_preparation_service = ChatExecutionPreparationService::new();
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
                source_type: &prepared_context.source_type,
                pack_path: &prepared_context.pack_path,
                skill_system_prompt: &setup.skill_system_prompt,
                skill_allowed_tools: setup.skill_allowed_tools.clone(),
                max_iter,
                max_call_depth: prepared_context.max_call_depth,
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
                permission_mode: prepared_context.permission_mode,
                tool_confirm_responder,
                executor_work_dir: prepared_context.executor_work_dir.clone(),
                max_iterations: Some(max_iter),
                cancel_flag,
                node_timeout_seconds: prepared_context.node_timeout_seconds,
                route_retry_count: prepared_context.route_retry_count,
            })
            .await;

            Ok(RouteRunOutcome::Prompt(route_execution))
        }
    }
}

fn strip_command_prefix<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let mut parts = value.splitn(2, char::is_whitespace);
    let first = parts.next()?.trim();
    if !first.eq_ignore_ascii_case(prefix) {
        return None;
    }
    Some(parts.next().unwrap_or("").trim_start())
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
        if ch.is_control() || !ch.is_ascii() {
            return false;
        }
        if ch.is_ascii_alphanumeric()
            || ch.is_ascii_whitespace()
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

#[cfg(test)]
mod tests {
    use super::*;
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

    fn build_command_specs(entries: &[WorkspaceSkillRuntimeEntry]) -> Vec<WorkspaceSkillCommandSpec> {
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
            }
            other => panic!("expected prompt-fork plan, got {:?}", other),
        }
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
}
