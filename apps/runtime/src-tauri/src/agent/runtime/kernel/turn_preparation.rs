use super::execution_plan::{ExecutionContext, TurnContext};
use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::{RunBudgetPolicy, RunBudgetScope};
use crate::agent::runtime::repo::{PoolChatEmployeeDirectory, PoolChatSettingsRepository};
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
    let continuation_runtime_notes =
        load_recent_compaction_runtime_notes(params.app, params.session_id).await;

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
        skill_system_prompt: &effective_skill_system_prompt,
        skill_allowed_tools: effective_skill_allowed_tools.clone(),
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
        capability_snapshot: prepared_runtime_tools.capability_snapshot,
        system_prompt: prepared_runtime_tools.system_prompt,
        continuation_runtime_notes,
        permission_mode,
        executor_work_dir: execution_preparation_service
            .resolve_executor_work_dir(&execution_guidance),
        max_iterations: Some(max_iter),
        max_call_depth: chat_preparation.max_call_depth,
        node_timeout_seconds: chat_preparation.node_timeout_seconds,
        route_retry_count: chat_preparation.retry_count,
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
            requested_capability,
            route_candidates,
            per_candidate_retry_count,
            messages,
        },
        execution_context,
    ))
}

async fn load_recent_compaction_runtime_notes(app: &AppHandle, session_id: &str) -> Vec<String> {
    let Some(journal) = app.try_state::<SessionJournalStateHandle>() else {
        return Vec::new();
    };

    journal
        .0
        .read_state(session_id)
        .await
        .map(|state| resolve_recent_compaction_runtime_notes(&state))
        .unwrap_or_default()
}

fn resolve_recent_compaction_runtime_notes(state: &SessionJournalState) -> Vec<String> {
    state
        .runs
        .iter()
        .rev()
        .find_map(|run| {
            let turn_state = run.turn_state.as_ref()?;
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
    max_iterations: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::{
        append_current_turn_message, parse_user_skill_command,
        resolve_explicit_prompt_following_skill, rewrite_user_skill_command_for_model,
        resolve_recent_compaction_runtime_notes,
    };
    use crate::agent::runtime::runtime_io as chat_io;
    use crate::agent::runtime::runtime_io::{WorkspaceSkillContent, WorkspaceSkillRuntimeEntry};
    use crate::session_journal::{
        SessionJournalState, SessionRunSnapshot, SessionRunStatus,
        SessionRunTurnStateCompactionBoundary, SessionRunTurnStateSnapshot,
    };
    use runtime_skill_core::{SkillConfig, SkillInvocationPolicy};
    use serde_json::json;

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

        let notes = resolve_recent_compaction_runtime_notes(&state);

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

        assert!(resolve_recent_compaction_runtime_notes(&state).is_empty());
    }
}
