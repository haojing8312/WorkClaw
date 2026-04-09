use super::execution_plan::{
    ContinuationKind, ContinuationPreference, ContinuationTurnPolicy, ExecutionContext, TurnContext,
};
use super::route_lane::{
    parse_skill_allowed_tool_categories, parse_skill_allowed_tool_sources,
    parse_skill_denied_tool_categories, parse_skill_denied_tool_sources,
    skill_allowed_mcp_servers,
};
use super::session_profile::{SessionExecutionProfile, SessionSurfaceKind};
use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::{RunBudgetPolicy, RunBudgetScope};
use crate::agent::runtime::effective_tool_set::runtime_default_tool_policy_input;
use crate::agent::runtime::kernel::capability_snapshot::CapabilitySnapshot;
use crate::agent::runtime::kernel::context_bundle::ContextBundle;
use crate::agent::runtime::repo::{
    load_runtime_tool_policy_defaults, PoolChatEmployeeDirectory, PoolChatSettingsRepository,
};
use crate::agent::runtime::runtime_io as chat_io;
use crate::agent::runtime::runtime_io::WorkspaceSkillRuntimeEntry;
use crate::agent::runtime::skill_routing::index::SkillRouteIndex;
use crate::agent::runtime::tool_setup::{prepare_runtime_tools, ToolSetupParams};
use crate::agent::runtime::RuntimeTranscript;
use crate::agent::AgentExecutor;
use crate::model_transport::{resolve_model_transport, ModelTransportKind};
use crate::session_journal::{SessionJournalState, SessionJournalStateHandle, SessionRunStatus};
use runtime_chat_app::{ChatExecutionPreparationRequest, ChatExecutionPreparationService};
use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[derive(Clone)]
pub(crate) struct PrepareLocalTurnParams<'a> {
    pub app: &'a AppHandle,
    pub db: &'a sqlx::SqlitePool,
    pub agent_executor: &'a Arc<AgentExecutor>,
    pub session_id: &'a str,
    pub user_message: &'a str,
    pub user_message_parts: &'a [Value],
    pub max_iterations_override: Option<usize>,
}

pub(crate) async fn prepare_local_turn(
    params: PrepareLocalTurnParams<'_>,
) -> Result<(TurnContext, ExecutionContext), String> {
    let (skill_id, model_id, perm_str, work_dir, session_employee_id) =
        chat_io::load_session_runtime_inputs_with_pool(params.db, params.session_id).await?;

    let chat_repo = PoolChatSettingsRepository::new(params.db);
    let execution_request = ChatExecutionPreparationRequest {
        user_message: params.user_message.to_string(),
        user_message_parts: Some(params.user_message_parts.to_vec()),
        session_id: Some(params.session_id.to_string()),
        permission_mode: Some(perm_str.clone()),
        session_mode: None,
        team_id: None,
        employee_id: Some(session_employee_id.clone()),
        requested_capability: None,
        work_dir: Some(work_dir.clone()),
        imported_mcp_server_ids: Vec::new(),
    };
    let employee_directory = PoolChatEmployeeDirectory::new(params.db);
    let execution_preparation_service = ChatExecutionPreparationService::new();
    let prepared_execution = execution_preparation_service
        .prepare_execution_with_directory(
            &chat_repo,
            &employee_directory,
            &model_id,
            &execution_request,
        )
        .await?;
    let chat_preparation = prepared_execution.chat_preparation;
    let prepared_execution_context = prepared_execution.execution_context;
    let execution_guidance = prepared_execution.execution_guidance;
    let prepared_routes = prepared_execution.route_decisions;
    let employee_collaboration_guidance = prepared_execution.employee_collaboration_guidance;
    let permission_mode =
        parse_permission_mode_for_runtime(&chat_preparation.permission_mode_storage);
    let runtime_tool_policy_defaults = load_runtime_tool_policy_defaults(params.db).await?;
    let runtime_default_tool_policy = runtime_default_tool_policy_input(
        runtime_tool_policy_defaults.label,
        runtime_tool_policy_defaults.denied_tool_names,
        runtime_tool_policy_defaults.denied_categories,
        runtime_tool_policy_defaults.allowed_sources,
        runtime_tool_policy_defaults.allowed_mcp_servers,
    );

    let (manifest_json, username, pack_path, source_type) =
        chat_io::load_installed_skill_source_with_pool(params.db, &skill_id).await?;
    let raw_prompt = chat_io::load_skill_prompt(
        &skill_id,
        &manifest_json,
        &username,
        &pack_path,
        &source_type,
    )?;
    let history = chat_io::load_session_history_with_pool(params.db, params.session_id).await?;
    let workspace_skill_entries =
        chat_io::load_workspace_skill_runtime_entries_with_pool(params.db).await?;
    let route_index = SkillRouteIndex::build(&workspace_skill_entries);

    let per_candidate_retry_count = prepared_routes.retry_count_per_candidate;
    let mut route_candidates: Vec<(String, String, String, String, String)> = prepared_routes
        .candidates
        .into_iter()
        .map(|candidate| {
            let transport = resolve_model_transport(
                &candidate.protocol_type,
                &candidate.base_url,
                Some(candidate.provider_key.as_str()).filter(|value| !value.trim().is_empty()),
            );
            let effective_api_format = if candidate.protocol_type.trim().is_empty() {
                match transport.kind {
                    ModelTransportKind::AnthropicMessages => "anthropic".to_string(),
                    ModelTransportKind::OpenAiCompletions | ModelTransportKind::OpenAiResponses => {
                        "openai".to_string()
                    }
                }
            } else {
                candidate.protocol_type.clone()
            };
            (
                candidate.provider_key,
                effective_api_format,
                candidate.base_url,
                candidate.model_name,
                candidate.api_key,
            )
        })
        .collect();
    let requested_capability = chat_preparation.capability.clone();

    if route_candidates.is_empty() {
        if requested_capability == "vision" {
            return Err("VISION_MODEL_NOT_CONFIGURED: 请先在设置中配置图片理解模型".to_string());
        }
        return Err(format!(
            "模型 API Key 为空，请在设置中重新配置 (model_id={model_id})"
        ));
    }

    route_candidates.dedup();
    eprintln!(
        "[routing] capability={}, candidates={}, retry_per_candidate={}",
        requested_capability,
        route_candidates.len(),
        per_candidate_retry_count
    );

    let (_, api_format, base_url, model_name, api_key) = route_candidates[0].clone();
    let mut messages = RuntimeTranscript::sanitize_reconstructed_messages(
        RuntimeTranscript::reconstruct_history_messages(&history, &api_format),
        &api_format,
    );
    if let Some(current_turn) =
        RuntimeTranscript::build_current_turn_message(&api_format, params.user_message_parts)
    {
        append_current_turn_message(&mut messages, current_turn);
    }
    let skill_config = crate::agent::skill_config::SkillConfig::parse(&raw_prompt);
    let explicit_skill_selection =
        resolve_explicit_prompt_following_skill(params.user_message, &workspace_skill_entries);
    let effective_skill_id = explicit_skill_selection
        .as_ref()
        .map(|selection| selection.skill_id.clone())
        .unwrap_or_else(|| skill_id.clone());
    let effective_skill_system_prompt = explicit_skill_selection
        .as_ref()
        .map(|selection| selection.system_prompt.clone())
        .unwrap_or_else(|| skill_config.system_prompt.clone());
    let effective_skill_allowed_tools = explicit_skill_selection
        .as_ref()
        .and_then(|selection| selection.allowed_tools.clone())
        .or_else(|| skill_config.allowed_tools.clone());
    let effective_skill_denied_tools = explicit_skill_selection
        .as_ref()
        .and_then(|selection| selection.denied_tools.clone())
        .or_else(|| skill_config.denied_tools.clone());
    let effective_skill_allowed_tool_sources = explicit_skill_selection
        .as_ref()
        .and_then(|selection| selection.allowed_tool_sources.clone())
        .or_else(|| parse_skill_allowed_tool_sources(&skill_config));
    let effective_skill_denied_tool_sources = explicit_skill_selection
        .as_ref()
        .and_then(|selection| selection.denied_tool_sources.clone())
        .or_else(|| parse_skill_denied_tool_sources(&skill_config));
    let effective_skill_allowed_tool_categories = explicit_skill_selection
        .as_ref()
        .and_then(|selection| selection.allowed_tool_categories.clone())
        .or_else(|| parse_skill_allowed_tool_categories(&skill_config));
    let effective_skill_denied_tool_categories = explicit_skill_selection
        .as_ref()
        .and_then(|selection| selection.denied_tool_categories.clone())
        .or_else(|| parse_skill_denied_tool_categories(&skill_config));
    let effective_skill_allowed_mcp_servers = explicit_skill_selection
        .as_ref()
        .and_then(|selection| selection.allowed_mcp_servers.clone())
        .or_else(|| {
            let servers = skill_config
                .mcp_servers
                .iter()
                .map(|server| server.name.trim())
                .filter(|name| !name.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            (!servers.is_empty()).then_some(servers)
        });
    let effective_skill_max_iterations = explicit_skill_selection
        .as_ref()
        .and_then(|selection| selection.max_iterations)
        .or(skill_config.max_iterations);
    let budget_scope = if effective_skill_id
        .trim()
        .eq_ignore_ascii_case("builtin-general")
    {
        RunBudgetScope::GeneralChat
    } else {
        RunBudgetScope::Skill
    };
    let default_max_iter =
        RunBudgetPolicy::resolve(budget_scope, effective_skill_max_iterations).max_turns;
    let max_iter = params
        .max_iterations_override
        .map(|override_value| override_value.max(1))
        .unwrap_or(default_max_iter);
    let session_profile = build_local_chat_session_profile();
    let recent_continuation_context = load_recent_continuation_context(
        params.app,
        params.session_id,
        params.user_message,
        &session_profile,
    )
    .await;
    let continuation_runtime_notes = recent_continuation_context.runtime_notes;
    let continuation_preference = recent_continuation_context.continuation_preference;
    let (per_candidate_retry_count, route_retry_count) = apply_continuation_turn_policy(
        per_candidate_retry_count,
        chat_preparation.retry_count,
        continuation_preference.as_ref(),
    );

    let prepared_runtime_tools = prepare_runtime_tools(ToolSetupParams {
        app: params.app,
        db: params.db,
        agent_executor: params.agent_executor,
        workspace_skill_entries: &workspace_skill_entries,
        session_id: params.session_id,
        api_format: &api_format,
        base_url: &base_url,
        model_name: &model_name,
        api_key: &api_key,
        skill_id: &effective_skill_id,
        source_type: &source_type,
        pack_path: &pack_path,
        permission_mode,
        runtime_default_tool_policy: runtime_default_tool_policy.clone(),
        skill_system_prompt: &effective_skill_system_prompt,
        skill_allowed_tools: effective_skill_allowed_tools.clone(),
        skill_denied_tools: effective_skill_denied_tools,
        skill_allowed_tool_sources: effective_skill_allowed_tool_sources,
        skill_denied_tool_sources: effective_skill_denied_tool_sources,
        skill_allowed_tool_categories: effective_skill_allowed_tool_categories,
        skill_denied_tool_categories: effective_skill_denied_tool_categories,
        skill_allowed_mcp_servers: effective_skill_allowed_mcp_servers,
        tool_discovery_query: Some(params.user_message),
        max_iter,
        max_call_depth: chat_preparation.max_call_depth,
        suppress_workspace_skills_prompt: explicit_skill_selection.is_some(),
        execution_preparation_service: &execution_preparation_service,
        execution_guidance: &execution_guidance,
        memory_bucket_employee_id: execution_preparation_service
            .resolve_memory_bucket_employee_id(&prepared_execution_context),
        employee_collaboration_guidance: employee_collaboration_guidance.as_deref(),
        supplemental_runtime_notes: &continuation_runtime_notes,
    })
    .await?;

    if let Some(rewritten_body) = rewrite_user_skill_command_for_model(
        params.user_message,
        &prepared_runtime_tools
            .capability_snapshot
            .skill_command_specs,
    ) {
        let rewritten_parts = vec![serde_json::json!({
            "type": "text",
            "text": rewritten_body,
        })];
        if let Some(current_turn) =
            RuntimeTranscript::build_current_turn_message(&api_format, &rewritten_parts)
        {
            let _ = messages.pop();
            append_current_turn_message(&mut messages, current_turn);
        }
    }

    let execution_context = ExecutionContext {
        session_profile,
        capability_snapshot: prepared_runtime_tools.capability_snapshot,
        system_prompt: prepared_runtime_tools.system_prompt,
        continuation_runtime_notes,
        permission_mode,
        runtime_default_tool_policy,
        executor_work_dir: execution_preparation_service
            .resolve_executor_work_dir(&execution_guidance),
        max_iterations: Some(max_iter),
        max_call_depth: chat_preparation.max_call_depth,
        node_timeout_seconds: chat_preparation.node_timeout_seconds,
        route_retry_count,
        execution_guidance,
        memory_bucket_employee_id: execution_preparation_service
            .resolve_memory_bucket_employee_id(&prepared_execution_context)
            .to_string(),
        employee_collaboration_guidance,
        workspace_skill_entries,
        route_index,
    };

    Ok((
        TurnContext {
            user_message: params.user_message.to_string(),
            requested_capability,
            route_candidates,
            per_candidate_retry_count,
            messages,
            continuation_preference,
        },
        execution_context,
    ))
}

pub(crate) fn prepare_hidden_child_turn(
    agent_executor: &Arc<AgentExecutor>,
    prompt: &str,
    agent_type: &str,
    delegate_display_name: &str,
    api_format: &str,
    base_url: &str,
    api_key: &str,
    model: &str,
    allowed_tools: Option<Vec<String>>,
    max_iterations: usize,
    work_dir: Option<String>,
) -> (TurnContext, ExecutionContext) {
    let messages = vec![serde_json::json!({
        "role": "user",
        "content": prompt,
    })];
    let resolved_tool_names = chat_io::resolve_tool_name_list(&allowed_tools, agent_executor);
    let capability_snapshot = CapabilitySnapshot::build(
        allowed_tools.clone(),
        resolved_tool_names,
        Vec::new(),
        vec!["当前处于隐藏子会话中，请面向上级代理直接返回必要结论。".to_string()],
    );

    (
        TurnContext {
            user_message: prompt.to_string(),
            requested_capability: "general".to_string(),
            route_candidates: vec![(
                String::new(),
                api_format.to_string(),
                base_url.to_string(),
                model.to_string(),
                api_key.to_string(),
            )],
            per_candidate_retry_count: 0,
            messages,
            continuation_preference: None,
        },
        ExecutionContext {
            session_profile: build_hidden_child_session_profile(),
            capability_snapshot,
            system_prompt: build_hidden_child_system_prompt(agent_type, delegate_display_name),
            continuation_runtime_notes: Vec::new(),
            permission_mode: PermissionMode::Unrestricted,
            runtime_default_tool_policy: ExecutionContext::default().runtime_default_tool_policy,
            executor_work_dir: work_dir,
            max_iterations: Some(max_iterations.max(1)),
            max_call_depth: 0,
            node_timeout_seconds: 60,
            route_retry_count: 0,
            execution_guidance: runtime_chat_app::ChatExecutionGuidance {
                effective_work_dir: String::new(),
                local_timezone: String::new(),
                local_date: String::new(),
                local_tomorrow: String::new(),
                local_month_range: String::new(),
            },
            memory_bucket_employee_id: String::new(),
            employee_collaboration_guidance: None,
            workspace_skill_entries: Vec::new(),
            route_index: SkillRouteIndex::default(),
        },
    )
}

pub(crate) fn prepare_employee_step_turn(
    agent_executor: &Arc<AgentExecutor>,
    user_prompt: &str,
    employee_step_system_prompt: &str,
    api_format: &str,
    base_url: &str,
    api_key: &str,
    model: &str,
    allowed_tools: Option<Vec<String>>,
    max_iterations: usize,
    work_dir: Option<String>,
) -> (TurnContext, ExecutionContext) {
    let capability_snapshot = CapabilitySnapshot::build(
        allowed_tools.clone(),
        chat_io::resolve_tool_name_list(&allowed_tools, agent_executor),
        Vec::new(),
        Vec::new(),
    );
    let execution_guidance = build_minimal_execution_guidance(work_dir.as_deref());
    let context_bundle = ContextBundle::build(
        employee_step_system_prompt,
        &capability_snapshot,
        model,
        max_iterations.max(1),
        &execution_guidance,
        None,
        None,
        None,
    );

    (
        TurnContext {
            user_message: user_prompt.to_string(),
            requested_capability: "general".to_string(),
            route_candidates: vec![(
                String::new(),
                api_format.to_string(),
                base_url.to_string(),
                model.to_string(),
                api_key.to_string(),
            )],
            per_candidate_retry_count: 0,
            messages: vec![serde_json::json!({
                "role": "user",
                "content": user_prompt,
            })],
            continuation_preference: None,
        },
        ExecutionContext {
            session_profile: build_employee_step_session_profile(),
            capability_snapshot,
            system_prompt: context_bundle.system_prompt,
            continuation_runtime_notes: Vec::new(),
            permission_mode: PermissionMode::Unrestricted,
            runtime_default_tool_policy: ExecutionContext::default().runtime_default_tool_policy,
            executor_work_dir: work_dir,
            max_iterations: Some(max_iterations.max(1)),
            max_call_depth: 0,
            node_timeout_seconds: 60,
            route_retry_count: 0,
            execution_guidance,
            memory_bucket_employee_id: String::new(),
            employee_collaboration_guidance: None,
            workspace_skill_entries: Vec::new(),
            route_index: SkillRouteIndex::default(),
        },
    )
}

fn build_local_chat_session_profile() -> SessionExecutionProfile {
    SessionExecutionProfile::local_chat()
}

fn build_hidden_child_session_profile() -> SessionExecutionProfile {
    SessionExecutionProfile::hidden_child_session()
}

fn build_employee_step_session_profile() -> SessionExecutionProfile {
    SessionExecutionProfile::employee_step_session()
}

fn build_hidden_child_system_prompt(agent_type: &str, delegate_display_name: &str) -> String {
    format!(
        "你是一个专注的子 Agent (类型: {})，当前承接角色: {}。完成以下任务后返回结果。简洁地报告你的发现。",
        agent_type, delegate_display_name
    )
}

fn build_minimal_execution_guidance(
    work_dir: Option<&str>,
) -> runtime_chat_app::ChatExecutionGuidance {
    runtime_chat_app::ChatExecutionGuidance {
        effective_work_dir: work_dir.unwrap_or_default().to_string(),
        local_timezone: String::new(),
        local_date: String::new(),
        local_tomorrow: String::new(),
        local_month_range: String::new(),
    }
}

#[derive(Debug, Default)]
struct RecentContinuationContext {
    runtime_notes: Vec<String>,
    continuation_preference: Option<ContinuationPreference>,
}

async fn load_recent_continuation_context(
    app: &AppHandle,
    session_id: &str,
    user_message: &str,
    session_profile: &SessionExecutionProfile,
) -> RecentContinuationContext {
    let Some(journal) = app.try_state::<SessionJournalStateHandle>() else {
        return RecentContinuationContext::default();
    };

    journal
        .0
        .read_state(session_id)
        .await
        .map(|state| RecentContinuationContext {
            runtime_notes: resolve_recent_compaction_runtime_notes(session_profile, &state),
            continuation_preference: resolve_session_continuation_preference(
                user_message,
                &state,
                session_profile,
            ),
        })
        .unwrap_or_default()
}

fn resolve_recent_compaction_runtime_notes(
    session_profile: &SessionExecutionProfile,
    state: &SessionJournalState,
) -> Vec<String> {
    if !session_profile
        .continuation_mode
        .allows_compaction_runtime_notes()
    {
        return Vec::new();
    }

    state
        .runs
        .iter()
        .rev()
        .find_map(|run| {
            let turn_state = run.turn_state.as_ref()?;
            if resolve_session_run_surface(turn_state.session_surface.as_deref())
                != session_profile.surface
            {
                return None;
            }
            let boundary = turn_state.compaction_boundary.as_ref()?;
            let mut lines = vec![format!(
                "当前会话最近一次上下文压缩已生效：{} -> {} tokens。当前提供给你的历史消息已经是压缩后的恢复上下文，不要要求用户重复提供压缩前的完整历史。",
                boundary.original_tokens, boundary.compacted_tokens
            )];
            if !boundary.summary.trim().is_empty() {
                lines.push(format!("压缩摘要：{}", boundary.summary.trim()));
            }
            if let Some(reconstructed_history_len) = turn_state.reconstructed_history_len {
                lines.push(format!("重建历史消息数：{}", reconstructed_history_len));
            }
            if matches!(run.status, SessionRunStatus::Failed | SessionRunStatus::Cancelled)
                || run.last_error_kind.as_deref() == Some("max_turns")
            {
                lines.push(
                    "若用户要求“继续”或“继续执行”，应基于当前压缩后的恢复上下文直接继续完成剩余任务。"
                        .to_string(),
                );
            }
            Some(lines.join("\n"))
        })
        .into_iter()
        .collect()
}

fn resolve_compaction_continuation_preference(
    user_message: &str,
    state: &SessionJournalState,
) -> Option<ContinuationPreference> {
    resolve_session_continuation_preference(
        user_message,
        state,
        &build_local_chat_session_profile(),
    )
}

fn resolve_session_continuation_preference(
    user_message: &str,
    state: &SessionJournalState,
    session_profile: &SessionExecutionProfile,
) -> Option<ContinuationPreference> {
    if !is_compaction_continuation_request(user_message) {
        return None;
    }

    state.runs.iter().rev().find_map(|run| {
        let turn_state = run.turn_state.as_ref()?;
        if resolve_session_run_surface(turn_state.session_surface.as_deref())
            != session_profile.surface
        {
            return None;
        }

        let kind = resolve_continuation_kind(run, turn_state, session_profile)?;
        let selected_skill = turn_state
            .selected_skill
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if matches!(kind, ContinuationKind::CompactionRecovery) && selected_skill.is_none() {
            return None;
        }

        Some(ContinuationPreference {
            kind,
            selected_skill,
            selected_runner: turn_state.selected_runner.clone(),
            reconstructed_history_len: turn_state.reconstructed_history_len,
            turn_policy: resolve_continuation_turn_policy(run),
        })
    })
}

fn resolve_continuation_kind(
    run: &crate::session_journal::SessionRunSnapshot,
    turn_state: &crate::session_journal::SessionRunTurnStateSnapshot,
    session_profile: &SessionExecutionProfile,
) -> Option<ContinuationKind> {
    if !is_recoverable_continuation_run(run) {
        return None;
    }

    match session_profile.continuation_mode {
        super::session_profile::SessionContinuationProfile::LocalChat => turn_state
            .compaction_boundary
            .as_ref()
            .map(|_| ContinuationKind::CompactionRecovery),
        super::session_profile::SessionContinuationProfile::HiddenChildSession => {
            Some(ContinuationKind::HiddenChildSession)
        }
        super::session_profile::SessionContinuationProfile::EmployeeStepSession => {
            Some(ContinuationKind::EmployeeStepSession)
        }
    }
}

fn is_recoverable_continuation_run(run: &crate::session_journal::SessionRunSnapshot) -> bool {
    matches!(
        run.status,
        SessionRunStatus::Failed | SessionRunStatus::Cancelled
    ) || matches!(
        run.last_error_kind.as_deref(),
        Some("max_turns" | "loop_detected" | "no_progress" | "tool_failure_circuit_breaker")
    )
}

fn resolve_session_run_surface(session_surface: Option<&str>) -> SessionSurfaceKind {
    SessionSurfaceKind::from_journal_key(session_surface)
}

fn resolve_continuation_turn_policy(
    run: &crate::session_journal::SessionRunSnapshot,
) -> ContinuationTurnPolicy {
    let should_clamp_retries = matches!(
        run.last_error_kind.as_deref(),
        Some(
            "max_turns"
                | "loop_detected"
                | "no_progress"
                | "tool_failure_circuit_breaker"
                | "auth"
        )
    );

    if should_clamp_retries {
        ContinuationTurnPolicy {
            per_candidate_retry_count: Some(0),
            route_retry_count: Some(0),
        }
    } else {
        ContinuationTurnPolicy::default()
    }
}

fn apply_continuation_turn_policy(
    per_candidate_retry_count: usize,
    route_retry_count: usize,
    continuation_preference: Option<&ContinuationPreference>,
) -> (usize, usize) {
    let Some(preference) = continuation_preference else {
        return (per_candidate_retry_count, route_retry_count);
    };

    (
        preference
            .turn_policy
            .per_candidate_retry_count
            .unwrap_or(per_candidate_retry_count),
        preference
            .turn_policy
            .route_retry_count
            .unwrap_or(route_retry_count),
    )
}

fn is_compaction_continuation_request(user_message: &str) -> bool {
    let normalized = canonicalize_continuation_match(user_message);
    if normalized.is_empty() {
        return false;
    }

    const EXACT_CONTINUATION_REQUESTS: &[&str] = &[
        "continue",
        "continueplease",
        "pleasecontinue",
        "继续",
        "继续执行",
        "继续上次",
        "继续刚才",
        "继续处理",
        "接着做",
        "接着来",
    ];

    EXACT_CONTINUATION_REQUESTS
        .iter()
        .any(|candidate| normalized == canonicalize_continuation_match(candidate))
        || (normalized.starts_with("继续") && normalized.chars().count() <= 12)
        || (normalized.starts_with("continue") && normalized.chars().count() <= 24)
}

fn canonicalize_continuation_match(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|ch| ch.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(ch))
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

pub(crate) fn parse_user_skill_command(user_message: &str) -> Option<(String, String)> {
    let trimmed = user_message.trim();
    let without_slash = trimmed.strip_prefix('/')?;
    let command = without_slash
        .split_whitespace()
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let args = without_slash[command.len()..].trim_start().to_string();
    Some((command.to_ascii_lowercase(), args))
}

fn parse_permission_mode_for_runtime(permission_mode: &str) -> PermissionMode {
    match permission_mode {
        "standard" | "default" | "accept_edits" => PermissionMode::AcceptEdits,
        "full_access" | "unrestricted" => PermissionMode::Unrestricted,
        _ => PermissionMode::AcceptEdits,
    }
}

fn resolve_explicit_prompt_following_skill(
    user_message: &str,
    entries: &[WorkspaceSkillRuntimeEntry],
) -> Option<ExplicitPromptSkillSelection> {
    let message = user_message.trim();
    if message.is_empty() {
        return None;
    }

    let message_lower = message.to_ascii_lowercase();
    let has_explicit_skill_marker = ["技能", "skill", "使用", "调用", "执行", "run"]
        .iter()
        .any(|marker| message_lower.contains(marker));

    let mut matches = entries
        .iter()
        .filter(|entry| {
            entry.invocation.user_invocable
                && !entry.invocation.disable_model_invocation
                && entry.command_dispatch.is_none()
        })
        .filter(|entry| {
            let skill_id = entry.skill_id.trim().to_ascii_lowercase();
            let skill_name = entry.name.trim().to_ascii_lowercase();
            (!skill_id.is_empty() && message_lower.contains(&skill_id))
                || (has_explicit_skill_marker
                    && !skill_name.is_empty()
                    && message_lower.contains(&skill_name))
        })
        .map(|entry| ExplicitPromptSkillSelection {
            skill_id: entry.skill_id.clone(),
            skill_name: entry.name.clone(),
            system_prompt: entry.config.system_prompt.clone(),
            allowed_tools: entry.config.allowed_tools.clone(),
            denied_tools: entry.config.denied_tools.clone(),
            allowed_tool_sources: parse_skill_allowed_tool_sources(&entry.config),
            denied_tool_sources: parse_skill_denied_tool_sources(&entry.config),
            allowed_tool_categories: parse_skill_allowed_tool_categories(&entry.config),
            denied_tool_categories: parse_skill_denied_tool_categories(&entry.config),
            allowed_mcp_servers: skill_allowed_mcp_servers(entry),
            max_iterations: entry.config.max_iterations,
        })
        .collect::<Vec<_>>();

    matches.dedup_by(|left, right| left.skill_id == right.skill_id);
    if matches.len() == 1 {
        matches.pop()
    } else {
        None
    }
}

fn rewrite_user_skill_command_for_model(
    user_message: &str,
    skill_command_specs: &[chat_io::WorkspaceSkillCommandSpec],
) -> Option<String> {
    let (command_name, raw_args) = parse_user_skill_command(user_message)?;
    let spec = skill_command_specs
        .iter()
        .find(|spec| spec.name.eq_ignore_ascii_case(&command_name) && spec.dispatch.is_none())?;

    let mut parts = vec![format!(
        "Use the \"{}\" skill for this request.",
        spec.skill_name
    )];
    if !raw_args.trim().is_empty() {
        parts.push(format!("User input:\n{}", raw_args.trim()));
    }
    Some(parts.join("\n\n"))
}

fn append_current_turn_message(messages: &mut Vec<Value>, current_turn: Value) {
    messages.push(current_turn);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExplicitPromptSkillSelection {
    skill_id: String,
    skill_name: String,
    system_prompt: String,
    allowed_tools: Option<Vec<String>>,
    denied_tools: Option<Vec<String>>,
    allowed_tool_sources: Option<Vec<crate::agent::tool_manifest::ToolSource>>,
    denied_tool_sources: Option<Vec<crate::agent::tool_manifest::ToolSource>>,
    allowed_tool_categories: Option<Vec<crate::agent::tool_manifest::ToolCategory>>,
    denied_tool_categories: Option<Vec<crate::agent::tool_manifest::ToolCategory>>,
    allowed_mcp_servers: Option<Vec<String>>,
    max_iterations: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::{
        append_current_turn_message, apply_continuation_turn_policy,
        build_employee_step_session_profile, build_hidden_child_session_profile,
        build_local_chat_session_profile,
        parse_user_skill_command, prepare_employee_step_turn, prepare_hidden_child_turn,
        resolve_compaction_continuation_preference, resolve_explicit_prompt_following_skill,
        resolve_recent_compaction_runtime_notes, resolve_session_continuation_preference,
        rewrite_user_skill_command_for_model,
    };
    use crate::agent::registry::ToolRegistry;
    use crate::agent::runtime::kernel::execution_plan::{
        ContinuationKind, ContinuationPreference, ContinuationTurnPolicy,
    };
    use crate::agent::runtime::kernel::session_profile::SessionSurfaceKind;
    use crate::agent::runtime::runtime_io as chat_io;
    use crate::agent::runtime::runtime_io::{WorkspaceSkillContent, WorkspaceSkillRuntimeEntry};
    use crate::agent::AgentExecutor;
    use crate::session_journal::{
        SessionJournalState, SessionRunSnapshot, SessionRunStatus,
        SessionRunTurnStateCompactionBoundary, SessionRunTurnStateSnapshot,
    };
    use runtime_skill_core::{SkillConfig, SkillInvocationPolicy};
    use serde_json::json;
    use std::sync::Arc;

    fn prompt_following_entry(skill_id: &str, name: &str) -> WorkspaceSkillRuntimeEntry {
        WorkspaceSkillRuntimeEntry {
            skill_id: skill_id.to_string(),
            name: name.to_string(),
            description: format!("Use {name}"),
            source_type: "local".to_string(),
            projected_dir_name: skill_id.to_string(),
            config: SkillConfig {
                system_prompt: format!("Prompt for {name}"),
                allowed_tools: Some(vec!["skill".to_string(), "exec".to_string()]),
                max_iterations: Some(7),
                ..SkillConfig::default()
            },
            invocation: SkillInvocationPolicy {
                user_invocable: true,
                disable_model_invocation: false,
            },
            metadata: None,
            command_dispatch: None,
            content: WorkspaceSkillContent::FileTree(std::collections::HashMap::new()),
        }
    }

    #[test]
    fn parse_user_skill_command_extracts_command_and_raw_args() {
        let parsed = parse_user_skill_command("  /pm_summary  --employee xt --date 2026-03-27 ");
        assert_eq!(
            parsed,
            Some((
                "pm_summary".to_string(),
                "--employee xt --date 2026-03-27".to_string(),
            ))
        );
    }

    #[test]
    fn parse_user_skill_command_ignores_non_command_messages() {
        assert_eq!(parse_user_skill_command("pm_summary"), None);
        assert_eq!(parse_user_skill_command("/"), None);
    }

    #[test]
    fn local_turn_preparation_still_parses_user_skill_commands() {
        assert_eq!(
            parse_user_skill_command("/pm_summary --employee xt"),
            Some(("pm_summary".to_string(), "--employee xt".to_string()))
        );
    }

    #[test]
    fn local_turn_preparation_uses_local_chat_surface_profile() {
        let profile = build_local_chat_session_profile();

        assert_eq!(profile.surface, SessionSurfaceKind::LocalChat);
    }

    #[test]
    fn hidden_child_turn_preparation_uses_hidden_child_surface_profile() {
        let agent_executor = Arc::new(AgentExecutor::new(Arc::new(ToolRegistry::new())));

        let (turn_context, execution_context) = prepare_hidden_child_turn(
            &agent_executor,
            "请继续完成剩余调查",
            "research",
            "研究子智能体",
            "openai",
            "https://api.example.com/v1",
            "test-key",
            "gpt-test",
            Some(vec!["read".to_string(), "exec".to_string()]),
            6,
            Some("E:/workspace/demo".to_string()),
        );

        assert_eq!(
            execution_context.session_profile.surface,
            SessionSurfaceKind::HiddenChildSession
        );
        assert_eq!(turn_context.route_candidates.len(), 1);
        assert_eq!(turn_context.messages.len(), 1);
        assert_eq!(execution_context.max_iterations, Some(6));
        assert!(execution_context.system_prompt.contains("研究子智能体"));
        assert_eq!(
            execution_context
                .capability_snapshot
                .allowed_tools
                .as_deref(),
            Some(&["read".to_string(), "exec".to_string()][..])
        );
    }

    #[test]
    fn employee_step_turn_preparation_uses_employee_step_surface_profile() {
        let agent_executor = Arc::new(AgentExecutor::new(Arc::new(ToolRegistry::new())));

        let (turn_context, execution_context) = prepare_employee_step_turn(
            &agent_executor,
            "你正在执行多员工团队中的 execute 步骤。",
            "你是一名专业、可靠、注重交付结果的 AI 员工。",
            "openai",
            "https://api.example.com/v1",
            "test-key",
            "gpt-test",
            Some(vec!["read_file".to_string(), "bash".to_string()]),
            8,
            Some("E:/workspace/demo".to_string()),
        );

        assert_eq!(
            execution_context.session_profile.surface,
            SessionSurfaceKind::EmployeeStepSession
        );
        assert_eq!(turn_context.route_candidates.len(), 1);
        assert_eq!(turn_context.messages.len(), 1);
        assert_eq!(execution_context.max_iterations, Some(8));
        assert!(execution_context.system_prompt.contains("AI 员工"));
        assert_eq!(
            execution_context
                .capability_snapshot
                .allowed_tools
                .as_deref(),
            Some(&["read_file".to_string(), "bash".to_string()][..])
        );
    }

    #[test]
    fn rewrite_user_skill_command_for_model_rewrites_prompt_following_skill_commands() {
        let specs = vec![chat_io::WorkspaceSkillCommandSpec {
            name: "pm_summary".to_string(),
            skill_id: "skill-1".to_string(),
            skill_name: "PM Summary".to_string(),
            description: "Summarize PM updates".to_string(),
            dispatch: None,
        }];

        let rewritten = rewrite_user_skill_command_for_model(
            "/pm_summary --employee xt --date 2026-03-27",
            &specs,
        );

        assert_eq!(
            rewritten.as_deref(),
            Some(
                "Use the \"PM Summary\" skill for this request.\n\nUser input:\n--employee xt --date 2026-03-27"
            )
        );
    }

    #[test]
    fn rewrite_user_skill_command_for_model_ignores_dispatchable_commands() {
        let specs = vec![chat_io::WorkspaceSkillCommandSpec {
            name: "pm_summary".to_string(),
            skill_id: "skill-1".to_string(),
            skill_name: "PM Summary".to_string(),
            description: "Summarize PM updates".to_string(),
            dispatch: Some(runtime_skill_core::SkillCommandDispatchSpec {
                kind: runtime_skill_core::SkillCommandDispatchKind::Tool,
                tool_name: "exec".to_string(),
                arg_mode: runtime_skill_core::SkillCommandArgMode::Raw,
            }),
        }];

        assert_eq!(
            rewrite_user_skill_command_for_model("/pm_summary --employee xt", &specs),
            None
        );
    }

    #[test]
    fn append_current_turn_message_keeps_previous_user_turns() {
        let mut messages = vec![
            json!({"role": "user", "content": "你是谁"}),
            json!({"role": "assistant", "content": "我是 WorkClaw 助手"}),
        ];

        append_current_turn_message(
            &mut messages,
            json!({"role": "user", "content": "你能做什么"}),
        );

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0]["role"].as_str(), Some("user"));
        assert_eq!(messages[0]["content"].as_str(), Some("你是谁"));
        assert_eq!(messages[2]["role"].as_str(), Some("user"));
        assert_eq!(messages[2]["content"].as_str(), Some("你能做什么"));
    }

    #[test]
    fn resolve_explicit_prompt_following_skill_matches_skill_id_mentions() {
        let entries = vec![prompt_following_entry("feishu-pm-hub", "Feishu PM Hub")];

        let matched = resolve_explicit_prompt_following_skill(
            "请使用 feishu-pm-hub 技能帮我查询谢涛上周日报",
            &entries,
        )
        .expect("should match explicit skill id");

        assert_eq!(matched.skill_id, "feishu-pm-hub");
        assert_eq!(matched.skill_name, "Feishu PM Hub");
        assert_eq!(matched.max_iterations, Some(7));
        assert_eq!(
            matched.allowed_tools,
            Some(vec!["skill".to_string(), "exec".to_string()])
        );
    }

    #[test]
    fn resolve_explicit_prompt_following_skill_returns_none_for_implicit_requests() {
        let entries = vec![prompt_following_entry("feishu-pm-hub", "Feishu PM Hub")];

        assert_eq!(
            resolve_explicit_prompt_following_skill("帮我查询谢涛上周工作日报", &entries),
            None
        );
    }

    #[test]
    fn resolve_explicit_prompt_following_skill_returns_none_when_multiple_skills_match() {
        let entries = vec![
            prompt_following_entry("feishu-pm-hub", "Feishu PM Hub"),
            prompt_following_entry("feishu-pm-task-query", "Feishu PM Task Query"),
        ];

        assert_eq!(
            resolve_explicit_prompt_following_skill(
                "请使用 feishu-pm-hub 和 feishu-pm-task-query 技能帮我查一下",
                &entries,
            ),
            None
        );
    }

    #[test]
    fn resolve_recent_compaction_runtime_notes_describes_latest_boundary() {
        let state = SessionJournalState {
            session_id: "session-1".to_string(),
            current_run_id: None,
            runs: vec![SessionRunSnapshot {
                run_id: "run-1".to_string(),
                user_message_id: "user-1".to_string(),
                status: SessionRunStatus::Failed,
                buffered_text: "已保留当前执行上下文".to_string(),
                last_error_kind: Some("max_turns".to_string()),
                last_error_message: Some("已达到执行步数上限".to_string()),
                turn_state: Some(SessionRunTurnStateSnapshot {
                    session_surface: None,
                    execution_lane: Some("open_task".to_string()),
                    selected_runner: Some("OpenTaskRunner".to_string()),
                    selected_skill: Some("builtin-general".to_string()),
                    fallback_reason: None,
                    allowed_tools: vec!["read".to_string(), "exec".to_string()],
                    invoked_skills: vec!["builtin-general".to_string()],
                    partial_assistant_text: "已保留当前执行上下文".to_string(),
                    tool_failure_streak: 0,
                    reconstructed_history_len: Some(7),
                    compaction_boundary: Some(SessionRunTurnStateCompactionBoundary {
                        transcript_path: "temp/transcripts/run-1.json".to_string(),
                        original_tokens: 4096,
                        compacted_tokens: 1024,
                        summary: "保留最近的文件修改计划和工具结果".to_string(),
                    }),
                }),
            }],
        };

        let notes =
            resolve_recent_compaction_runtime_notes(&build_local_chat_session_profile(), &state);

        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("4096 -> 1024"));
        assert!(notes[0].contains("压缩后的恢复上下文"));
        assert!(notes[0].contains("保留最近的文件修改计划和工具结果"));
        assert!(notes[0].contains("重建历史消息数：7"));
    }

    #[test]
    fn resolve_recent_compaction_runtime_notes_ignores_runs_without_boundary() {
        let state = SessionJournalState {
            session_id: "session-1".to_string(),
            current_run_id: None,
            runs: vec![SessionRunSnapshot {
                run_id: "run-1".to_string(),
                user_message_id: "user-1".to_string(),
                status: SessionRunStatus::Failed,
                buffered_text: String::new(),
                last_error_kind: Some("max_turns".to_string()),
                last_error_message: Some("已达到执行步数上限".to_string()),
                turn_state: Some(SessionRunTurnStateSnapshot {
                    session_surface: None,
                    execution_lane: None,
                    selected_runner: None,
                    selected_skill: None,
                    fallback_reason: None,
                    allowed_tools: Vec::new(),
                    invoked_skills: Vec::new(),
                    partial_assistant_text: String::new(),
                    tool_failure_streak: 0,
                    reconstructed_history_len: Some(3),
                    compaction_boundary: None,
                }),
            }],
        };

        assert!(resolve_recent_compaction_runtime_notes(
            &build_local_chat_session_profile(),
            &state
        )
        .is_empty());
    }

    #[test]
    fn resolve_compaction_continuation_preference_prefers_recent_prompt_skill() {
        let state = SessionJournalState {
            session_id: "session-1".to_string(),
            current_run_id: None,
            runs: vec![SessionRunSnapshot {
                run_id: "run-1".to_string(),
                user_message_id: "user-1".to_string(),
                status: SessionRunStatus::Failed,
                buffered_text: "已保留当前执行上下文".to_string(),
                last_error_kind: Some("max_turns".to_string()),
                last_error_message: Some("已达到执行步数上限".to_string()),
                turn_state: Some(SessionRunTurnStateSnapshot {
                    session_surface: None,
                    execution_lane: Some("prompt_fork".to_string()),
                    selected_runner: Some("prompt_skill_fork".to_string()),
                    selected_skill: Some("feishu-pm-weekly-work-summary".to_string()),
                    fallback_reason: None,
                    allowed_tools: vec!["read".to_string(), "exec".to_string()],
                    invoked_skills: vec!["feishu-pm-weekly-work-summary".to_string()],
                    partial_assistant_text: "还差最后的日报汇总和任务整理".to_string(),
                    tool_failure_streak: 0,
                    reconstructed_history_len: Some(6),
                    compaction_boundary: Some(SessionRunTurnStateCompactionBoundary {
                        transcript_path: "temp/transcripts/run-1.json".to_string(),
                        original_tokens: 4096,
                        compacted_tokens: 1024,
                        summary: "保留最近的日报汇总计划和工具结果".to_string(),
                    }),
                }),
            }],
        };

        let preference =
            resolve_compaction_continuation_preference("继续执行", &state).expect("preference");

        assert_eq!(preference.kind, ContinuationKind::CompactionRecovery);
        assert_eq!(
            preference.selected_skill,
            Some("feishu-pm-weekly-work-summary".to_string())
        );
        assert_eq!(
            preference.selected_runner.as_deref(),
            Some("prompt_skill_fork")
        );
        assert_eq!(preference.reconstructed_history_len, Some(6));
        assert_eq!(preference.turn_policy.per_candidate_retry_count, Some(0));
        assert_eq!(preference.turn_policy.route_retry_count, Some(0));
    }

    #[test]
    fn resolve_session_continuation_preference_respects_session_surface_profile() {
        let state = SessionJournalState {
            session_id: "employee-step-session".to_string(),
            current_run_id: None,
            runs: vec![SessionRunSnapshot {
                run_id: "run-1".to_string(),
                user_message_id: "user-1".to_string(),
                status: SessionRunStatus::Failed,
                buffered_text: "继续整理剩余执行项".to_string(),
                last_error_kind: Some("max_turns".to_string()),
                last_error_message: Some("已达到执行步数上限".to_string()),
                turn_state: Some(SessionRunTurnStateSnapshot {
                    session_surface: Some("employee_step_session".to_string()),
                    execution_lane: Some("open_task".to_string()),
                    selected_runner: Some("OpenTaskRunner".to_string()),
                    selected_skill: None,
                    fallback_reason: None,
                    allowed_tools: vec!["read".to_string()],
                    invoked_skills: Vec::new(),
                    partial_assistant_text: "还差最后一段执行说明".to_string(),
                    tool_failure_streak: 0,
                    reconstructed_history_len: Some(3),
                    compaction_boundary: None,
                }),
            }],
        };

        assert!(resolve_session_continuation_preference(
            "继续",
            &state,
            &build_local_chat_session_profile()
        )
        .is_none());

        let preference = resolve_session_continuation_preference(
            "继续",
            &state,
            &build_employee_step_session_profile(),
        )
        .expect("employee step continuation preference");

        assert_eq!(preference.kind, ContinuationKind::EmployeeStepSession);
        assert_eq!(preference.selected_skill, None);
    }

    #[test]
    fn resolve_session_continuation_preference_treats_cancelled_hidden_child_runs_as_recoverable() {
        let state = SessionJournalState {
            session_id: "child-session".to_string(),
            current_run_id: None,
            runs: vec![SessionRunSnapshot {
                run_id: "run-1".to_string(),
                user_message_id: "user-1".to_string(),
                status: SessionRunStatus::Cancelled,
                buffered_text: "已停止，保留当前分析上下文".to_string(),
                last_error_kind: Some("cancelled".to_string()),
                last_error_message: Some("user cancelled".to_string()),
                turn_state: Some(SessionRunTurnStateSnapshot {
                    session_surface: Some("hidden_child_session".to_string()),
                    execution_lane: Some("open_task".to_string()),
                    selected_runner: Some("OpenTaskRunner".to_string()),
                    selected_skill: None,
                    fallback_reason: None,
                    allowed_tools: vec!["read".to_string()],
                    invoked_skills: Vec::new(),
                    partial_assistant_text: "还差最后一段结论".to_string(),
                    tool_failure_streak: 0,
                    reconstructed_history_len: Some(2),
                    compaction_boundary: None,
                }),
            }],
        };

        let preference = resolve_session_continuation_preference(
            "继续",
            &state,
            &build_hidden_child_session_profile(),
        )
        .expect("hidden child continuation preference");

        assert_eq!(preference.kind, ContinuationKind::HiddenChildSession);
        assert_eq!(preference.selected_skill, None);
        assert_eq!(
            preference.selected_runner.as_deref(),
            Some("OpenTaskRunner")
        );
    }

    #[test]
    fn resolve_session_continuation_preference_clamps_retry_budgets_for_permission_errors() {
        let state = SessionJournalState {
            session_id: "employee-step-session".to_string(),
            current_run_id: None,
            runs: vec![SessionRunSnapshot {
                run_id: "run-1".to_string(),
                user_message_id: "user-1".to_string(),
                status: SessionRunStatus::Failed,
                buffered_text: "继续时需要保留当前受限工具上下文".to_string(),
                last_error_kind: Some("auth".to_string()),
                last_error_message: Some("permission denied".to_string()),
                turn_state: Some(SessionRunTurnStateSnapshot {
                    session_surface: Some("employee_step_session".to_string()),
                    execution_lane: Some("open_task".to_string()),
                    selected_runner: Some("OpenTaskRunner".to_string()),
                    selected_skill: None,
                    fallback_reason: None,
                    allowed_tools: vec!["read".to_string()],
                    invoked_skills: Vec::new(),
                    partial_assistant_text: "当前操作仍受权限限制".to_string(),
                    tool_failure_streak: 0,
                    reconstructed_history_len: Some(4),
                    compaction_boundary: None,
                }),
            }],
        };

        let preference = resolve_session_continuation_preference(
            "继续",
            &state,
            &build_employee_step_session_profile(),
        )
        .expect("employee step continuation preference");

        assert_eq!(preference.kind, ContinuationKind::EmployeeStepSession);
        assert_eq!(preference.turn_policy.per_candidate_retry_count, Some(0));
        assert_eq!(preference.turn_policy.route_retry_count, Some(0));
    }

    #[test]
    fn apply_continuation_turn_policy_clamps_retry_budgets_for_recovery_turns() {
        let preference = ContinuationPreference {
            kind: ContinuationKind::CompactionRecovery,
            selected_skill: Some("feishu-pm-weekly-work-summary".to_string()),
            selected_runner: Some("prompt_skill_fork".to_string()),
            reconstructed_history_len: Some(6),
            turn_policy: ContinuationTurnPolicy {
                per_candidate_retry_count: Some(0),
                route_retry_count: Some(0),
            },
        };

        let (per_candidate_retry_count, route_retry_count) =
            apply_continuation_turn_policy(2, 2, Some(&preference));

        assert_eq!(per_candidate_retry_count, 0);
        assert_eq!(route_retry_count, 0);
    }
}
