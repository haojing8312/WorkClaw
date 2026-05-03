#[cfg(test)]
use super::approval_flow::{
    request_tool_approval_and_wait, wait_for_tool_confirmation, ApprovalWaitRuntime,
    ToolConfirmationDecision,
};
use super::browser_progress::BrowserProgressSnapshot;
use super::context::build_tool_context;
use super::event_bridge::{append_run_guard_warning_event, resolve_current_session_run_id};
#[cfg(test)]
use super::execution_caps::detect_execution_caps;
use super::executor::{AgentExecutor, AgentTurnExecutionError, AgentTurnExecutionOutcome};
use super::permissions::PermissionMode;
use super::run_guard::{
    encode_run_stop_reason, ProgressFingerprint, RunBudgetPolicy, RunBudgetScope, RunStopReason,
};
#[cfg(test)]
use super::safety::classify_policy_blocked_tool_error;
use super::types::{AgentStateEvent, LLMResponse, StreamDelta};
use crate::adapters;
use crate::agent::runtime::RuntimeObservabilityState;
use crate::model_transport::{resolve_model_transport, ModelTransportKind, ResolvedModelTransport};
use crate::runtime_environment::runtime_paths_from_app;
use anyhow::anyhow;
use runtime_executor_core::{
    estimate_tokens, micro_compact, trim_messages, ToolFailureStreak, DEFAULT_TOKEN_BUDGET,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

impl AgentExecutor {
    pub(super) async fn execute_turn_impl(
        &self,
        transport_override: Option<ResolvedModelTransport>,
        api_format: &str,
        base_url: &str,
        api_key: &str,
        model: &str,
        skill_system_prompt: &str,
        mut messages: Vec<Value>,
        on_token: impl Fn(StreamDelta) + Send + Clone,
        app_handle: Option<&AppHandle>,
        session_id: Option<&str>,
        allowed_tools: Option<&[String]>,
        permission_mode: PermissionMode,
        tool_confirm_tx: Option<
            std::sync::Arc<std::sync::Mutex<Option<std::sync::mpsc::Sender<bool>>>>,
        >,
        work_dir: Option<String>,
        max_iterations_override: Option<usize>,
        cancel_flag: Option<Arc<AtomicBool>>,
        route_node_timeout_secs: Option<u64>,
        route_retry_count: Option<usize>,
    ) -> std::result::Result<AgentTurnExecutionOutcome, AgentTurnExecutionError> {
        // 组合系统级 prompt 和 Skill prompt
        let system_prompt = self.system_prompt_builder.build(skill_system_prompt);
        let mut compaction_outcome: Option<
            super::runtime::compaction_pipeline::RuntimeCompactionOutcome,
        > = None;

        let tool_ctx = build_tool_context(session_id, work_dir.map(PathBuf::from), allowed_tools)
            .map_err(|error| {
            AgentTurnExecutionError::from_error(error, compaction_outcome.clone())
        })?;
        let max_iterations = max_iterations_override.unwrap_or(self.max_iterations);
        let mut run_budget_policy = RunBudgetPolicy::for_scope(RunBudgetScope::GeneralChat);
        run_budget_policy.max_turns = max_iterations;
        let route_node_timeout_secs = route_node_timeout_secs.unwrap_or(60).clamp(5, 600);
        let route_retry_count = route_retry_count.unwrap_or(0).clamp(0, 2);
        let mut iteration = 0;
        let route_run_id = Uuid::new_v4().to_string();
        let persisted_run_id = if let (Some(app), Some(sid)) = (app_handle, session_id) {
            resolve_current_session_run_id(app, sid).await
        } else {
            None
        };
        let mut tool_failure_streak: Option<ToolFailureStreak> = None;
        let mut tool_call_history: Vec<ProgressFingerprint> = Vec::new();
        let mut tool_result_history: Vec<ProgressFingerprint> = Vec::new();
        let mut latest_browser_progress: Option<BrowserProgressSnapshot> = None;

        loop {
            // 检查取消标志
            if let Some(ref flag) = cancel_flag {
                if flag.load(Ordering::SeqCst) {
                    eprintln!("[agent] 任务被用户取消");
                    if let (Some(app), Some(sid)) = (app_handle, session_id) {
                        let _ = app.emit(
                            "agent-state-event",
                            AgentStateEvent::basic(
                                sid,
                                "finished",
                                Some("用户取消".to_string()),
                                iteration,
                            ),
                        );
                    }
                    messages.push(json!({
                        "role": "assistant",
                        "content": "任务已被取消。"
                    }));
                    return Ok(AgentTurnExecutionOutcome {
                        messages,
                        compaction_outcome,
                    });
                }
            }

            if iteration >= max_iterations {
                let stop_reason = RunStopReason::max_turns(max_iterations);
                if let (Some(app), Some(sid)) = (app_handle, session_id) {
                    let _ = app.emit(
                        "agent-state-event",
                        AgentStateEvent::stopped(sid, iteration, &stop_reason),
                    );
                }
                return Err(AgentTurnExecutionError::from_error(
                    anyhow!(encode_run_stop_reason(&stop_reason)),
                    compaction_outcome.clone(),
                ));
            }
            iteration += 1;

            eprintln!("[agent] Iteration {}/{}", iteration, max_iterations);

            // 发射 thinking 状态事件
            if let (Some(app), Some(sid)) = (app_handle, session_id) {
                let _ = app.emit(
                    "agent-state-event",
                    AgentStateEvent::basic(sid, "thinking", None, iteration),
                );
            }

            // 自动压缩检查（仅在第二轮及之后，避免首轮触发）
            if iteration > 1 {
                let tokens = estimate_tokens(&messages);
                if super::compactor::needs_auto_compact(tokens) {
                    eprintln!("[agent] Token 数 {} 超过阈值，触发自动压缩", tokens);
                    if let (Some(app), Some(sid)) = (app_handle, session_id) {
                        let _ = app.emit(
                            "context-compaction-event",
                            json!({
                                "session_id": sid,
                                "phase": "started",
                                "detail": format!("正在压缩上下文（约 {} tokens）", tokens),
                                "original_tokens": tokens,
                            }),
                        );
                        let transcript_dir = runtime_paths_from_app(app)
                            .map(|paths| paths.transcripts_dir)
                            .unwrap_or_else(|_| {
                                crate::runtime_paths::RuntimePaths::new(
                                    crate::runtime_paths::resolve_runtime_root(),
                                )
                                .transcripts_dir
                            });
                        let runtime_observability = app
                            .try_state::<RuntimeObservabilityState>()
                            .map(|state| state.0.clone());
                        match super::runtime::compaction_pipeline::maybe_auto_compact(
                            super::runtime::compaction_pipeline::RuntimeCompactionRequest {
                                api_format,
                                base_url,
                                api_key,
                                model,
                                session_id: sid,
                                messages: &messages,
                                transcript_root: &transcript_dir,
                                observability: runtime_observability.as_deref(),
                            },
                        )
                        .await
                        {
                            Ok(Some(outcome)) => {
                                eprintln!(
                                    "[agent] 自动压缩完成，消息数 {} → {}",
                                    messages.len(),
                                    outcome.compacted_messages.len()
                                );
                                let _ = app.emit(
                                    "context-compaction-event",
                                    json!({
                                        "session_id": sid,
                                        "phase": "completed",
                                        "detail": "上下文压缩完成，准备继续执行",
                                        "original_tokens": outcome.original_tokens,
                                        "compacted_tokens": outcome.new_tokens,
                                        "summary": outcome.summary,
                                    }),
                                );
                                compaction_outcome = Some(outcome.clone());
                                messages = outcome.compacted_messages;
                            }
                            Ok(None) => {}
                            Err(e) => {
                                let _ = app.emit(
                                    "context-compaction-event",
                                    json!({
                                        "session_id": sid,
                                        "phase": "failed",
                                        "detail": format!("上下文压缩失败：{}，已继续使用原始上下文", e),
                                    }),
                                );
                                eprintln!("[agent] 自动压缩失败: {}", e)
                            }
                        }
                    }
                }
            }

            // 根据白名单过滤工具定义
            let tools = match allowed_tools {
                Some(whitelist) => self.registry.get_filtered_tool_definitions(whitelist),
                None => self.registry.get_tool_definitions(),
            };

            // 上下文压缩：Layer 1 微压缩 + token 预算裁剪
            let compacted = micro_compact(&messages, 3);
            let trimmed = trim_messages(&compacted, DEFAULT_TOKEN_BUDGET);

            // 调用 LLM（使用组合后的系统 prompt）
            let transport = transport_override
                .clone()
                .unwrap_or_else(|| resolve_model_transport(api_format, base_url, None));
            let response_result = if transport.kind == ModelTransportKind::AnthropicMessages {
                adapters::anthropic::chat_stream_with_tools(
                    base_url,
                    api_key,
                    model,
                    &system_prompt,
                    trimmed.clone(),
                    tools,
                    on_token.clone(),
                )
                .await
            } else {
                adapters::openai::chat_stream_with_tools(
                    &transport,
                    base_url,
                    api_key,
                    model,
                    &system_prompt,
                    trimmed.clone(),
                    tools,
                    on_token.clone(),
                )
                .await
            };

            let response = match response_result {
                Ok(response) => response,
                Err(err) => {
                    if let (Some(app), Some(sid)) = (app_handle, session_id) {
                        let _ = app.emit(
                            "agent-state-event",
                            AgentStateEvent::basic(sid, "error", Some(err.to_string()), iteration),
                        );
                    }
                    return Err(AgentTurnExecutionError::from_error(
                        err,
                        compaction_outcome.clone(),
                    ));
                }
            };

            // 处理响应
            match response {
                LLMResponse::Text(content) => {
                    // 纯文本响应 - 结束循环
                    messages.push(json!({
                        "role": "assistant",
                        "content": content
                    }));
                    eprintln!("[agent] Finished with text response");

                    // 发射 finished 状态事件
                    if let (Some(app), Some(sid)) = (app_handle, session_id) {
                        let _ = app.emit(
                            "agent-state-event",
                            AgentStateEvent::basic(sid, "finished", None, iteration),
                        );
                    }

                    return Ok(AgentTurnExecutionOutcome {
                        messages,
                        compaction_outcome,
                    });
                }
                tc_response
                @ (LLMResponse::ToolCalls(_) | LLMResponse::TextWithToolCalls(_, _)) => {
                    let (companion_text, tool_calls) = match tc_response {
                        LLMResponse::ToolCalls(tc) => (String::new(), tc),
                        LLMResponse::TextWithToolCalls(text, tc) => (text, tc),
                        _ => unreachable!(),
                    };

                    eprintln!(
                        "[agent] Executing {} tool calls (companion_text={})",
                        tool_calls.len(),
                        !companion_text.is_empty()
                    );

                    // 发射 tool_calling 状态事件
                    if let (Some(app), Some(sid)) = (app_handle, session_id) {
                        let tool_names: Vec<&str> =
                            tool_calls.iter().map(|tc| tc.name.as_str()).collect();
                        let _ = app.emit(
                            "agent-state-event",
                            AgentStateEvent::basic(
                                sid,
                                "tool_calling",
                                Some(tool_names.join(", ")),
                                iteration,
                            ),
                        );
                    }

                    // 执行所有工具调用
                    let mut tool_results = vec![];
                    let mut repeated_failure_summary: Option<String> = None;
                    let dispatch_context = super::runtime::tool_dispatch::ToolDispatchContext {
                        registry: self.registry.as_ref(),
                        app_handle,
                        session_id,
                        persisted_run_id: persisted_run_id.as_deref(),
                        active_task_identity: None,
                        active_task_kind: None,
                        active_task_surface: None,
                        active_task_backend: None,
                        active_task_continuation_mode: None,
                        active_task_continuation_source: None,
                        active_task_continuation_reason: None,
                        allowed_tools,
                        effective_tool_plan: None,
                        permission_mode,
                        tool_ctx: &tool_ctx,
                        tool_confirm_tx: tool_confirm_tx.as_ref(),
                        cancel_flag: cancel_flag.clone(),
                        route_run_id: &route_run_id,
                        route_node_timeout_secs,
                        route_retry_count,
                        iteration,
                        run_budget_policy,
                    };
                    for (call_index, call) in tool_calls.iter().enumerate() {
                        let mut dispatch_state = super::runtime::tool_dispatch::ToolDispatchState {
                            tool_results: &mut tool_results,
                            repeated_failure_summary: &mut repeated_failure_summary,
                            tool_failure_streak: &mut tool_failure_streak,
                            tool_call_history: &mut tool_call_history,
                            tool_result_history: &mut tool_result_history,
                            latest_browser_progress: &mut latest_browser_progress,
                        };
                        match super::runtime::tool_dispatch::dispatch_tool_call(
                            &dispatch_context,
                            &mut dispatch_state,
                            call_index,
                            call,
                        )
                        .await
                        .map_err(|error| {
                            AgentTurnExecutionError::from_error(error, compaction_outcome.clone())
                        })? {
                            super::runtime::tool_dispatch::ToolDispatchOutcome::Cancelled => {
                                if let (Some(app), Some(sid)) = (app_handle, session_id) {
                                    let _ = app.emit(
                                        "agent-state-event",
                                        AgentStateEvent::basic(
                                            sid,
                                            "finished",
                                            Some("用户取消".to_string()),
                                            iteration,
                                        ),
                                    );
                                }
                                messages.push(json!({
                                    "role": "assistant",
                                    "content": "任务已被取消。"
                                }));
                                return Ok(AgentTurnExecutionOutcome {
                                    messages,
                                    compaction_outcome,
                                });
                            }
                            super::runtime::tool_dispatch::ToolDispatchOutcome::Continue => {}
                        }

                        if repeated_failure_summary.is_some() {
                            break;
                        }
                    }

                    // 添加工具调用和结果到消息历史（包含伴随文本）
                    if api_format == "anthropic" {
                        // Anthropic 格式: assistant 消息包含 text block + tool_use blocks
                        let mut content_blocks: Vec<Value> = vec![];
                        if !companion_text.is_empty() {
                            content_blocks.push(json!({"type": "text", "text": companion_text}));
                        }
                        for tc in &tool_calls {
                            content_blocks.push(json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.name,
                                "input": tc.input,
                            }));
                        }
                        messages.push(json!({
                            "role": "assistant",
                            "content": content_blocks
                        }));

                        // user 消息包含 tool_result blocks
                        messages.push(json!({
                            "role": "user",
                            "content": tool_results.iter().map(|tr| json!({
                                "type": "tool_result",
                                "tool_use_id": tr.tool_use_id,
                                "content": tr.content,
                            })).collect::<Vec<_>>()
                        }));
                    } else {
                        // OpenAI 格式: companion_text 放 content 字段
                        let content_val = if companion_text.is_empty() {
                            Value::Null
                        } else {
                            Value::String(companion_text.clone())
                        };
                        messages.push(json!({
                            "role": "assistant",
                            "content": content_val,
                            "tool_calls": tool_calls.iter().map(|tc| json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": serde_json::to_string(&tc.input).unwrap_or_default(),
                                }
                            })).collect::<Vec<_>>()
                        }));
                        // OpenAI: 每个工具结果是独立的 "tool" 角色消息
                        for tr in &tool_results {
                            messages.push(json!({
                                "role": "tool",
                                "tool_call_id": tr.tool_use_id,
                                "content": tr.content,
                            }));
                        }
                    }

                    if let Some(summary) = repeated_failure_summary {
                        messages.push(json!({
                            "role": "assistant",
                            "content": summary
                        }));
                        if let (Some(app), Some(sid)) = (app_handle, session_id) {
                            let _ = app.emit(
                                "agent-state-event",
                                AgentStateEvent::basic(
                                    sid,
                                    "finished",
                                    Some("重复工具失败已熔断".to_string()),
                                    iteration,
                                ),
                            );
                        }
                        return Ok(AgentTurnExecutionOutcome {
                            messages,
                            compaction_outcome,
                        });
                    }

                    let progress_evaluation =
                        super::runtime::progress_guard::evaluate_progress_guard(
                            &run_budget_policy,
                            &tool_result_history,
                            latest_browser_progress.as_ref(),
                        );
                    if let Some(warning) = progress_evaluation.warning {
                        if let (Some(app), Some(sid)) = (app_handle, session_id) {
                            let _ = append_run_guard_warning_event(app, sid, &warning).await;
                        }
                    }
                    if let Some(stop_reason) = progress_evaluation.stop_reason {
                        if let (Some(app), Some(sid)) = (app_handle, session_id) {
                            let _ = app.emit(
                                "agent-state-event",
                                AgentStateEvent::stopped(sid, iteration, &stop_reason),
                            );
                        }
                        return Err(AgentTurnExecutionError::from_error(
                            anyhow!(encode_run_stop_reason(&stop_reason)),
                            compaction_outcome.clone(),
                        ));
                    }

                    // 继续下一轮迭代
                    continue;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::classify_policy_blocked_tool_error;
    use super::request_tool_approval_and_wait;
    use super::wait_for_tool_confirmation;
    use super::ApprovalWaitRuntime;
    use super::ToolConfirmationDecision;
    use crate::agent::run_guard::RunStopReasonKind;
    use crate::agent::{FileDeleteTool, Tool, ToolContext};
    use crate::approval_bus::{ApprovalDecision, ApprovalManager};
    use crate::session_journal::SessionJournalStore;
    use serde_json::json;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tempfile::tempdir;

    #[test]
    fn tool_confirmation_timeout_is_treated_as_rejection() {
        let (_tx, rx) = mpsc::channel::<bool>();
        let decision = wait_for_tool_confirmation(&rx, Duration::from_millis(5));
        assert_eq!(decision, ToolConfirmationDecision::TimedOut);
    }

    #[test]
    fn tool_confirmation_false_is_rejected() {
        let (tx, rx) = mpsc::channel::<bool>();
        tx.send(false).expect("send");
        let decision = wait_for_tool_confirmation(&rx, Duration::from_millis(5));
        assert_eq!(decision, ToolConfirmationDecision::Rejected);
    }

    #[test]
    fn workspace_boundary_error_maps_to_policy_blocked() {
        let reason = classify_policy_blocked_tool_error(
            "list_dir",
            "工具执行错误: 路径 C:\\Users\\Administrator\\Desktop 不在工作目录 C:\\Users\\Administrator\\WorkClaw\\workspace 范围内",
        )
        .expect("should classify");

        assert_eq!(reason.kind, RunStopReasonKind::PolicyBlocked);
        assert!(reason
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("切换当前会话的工作目录"));
    }

    #[test]
    fn skill_allowlist_error_is_not_policy_blocked() {
        let reason = classify_policy_blocked_tool_error("bash", "此 Skill 不允许使用工具: bash");

        assert!(reason.is_none());
    }

    #[test]
    fn ordinary_tool_failure_is_not_policy_blocked() {
        let reason = classify_policy_blocked_tool_error(
            "read_file",
            "工具执行错误: 文件不存在: missing.txt",
        );

        assert!(reason.is_none());
    }

    #[test]
    fn tool_context_construction_includes_p0_metadata_slots() {
        let work_dir = Some(PathBuf::from("workspace"));
        let allowed_tools = Some(vec!["read_file".to_string(), "skill".to_string()]);

        let ctx = super::build_tool_context(
            Some("session-123"),
            work_dir.clone(),
            allowed_tools.as_deref(),
        )
        .expect("build tool context");

        assert_eq!(ctx.session_id.as_deref(), Some("session-123"));
        assert_eq!(ctx.work_dir, work_dir);
        assert_eq!(ctx.allowed_tools, allowed_tools);
        let temp_dir = ctx.task_temp_dir.expect("task temp dir");
        let temp_dir_name = temp_dir
            .file_name()
            .and_then(|name| name.to_str())
            .expect("temp dir name");
        assert_eq!(temp_dir_name, "workclaw-task-session-123");
        assert!(temp_dir.exists());

        let caps = ctx.execution_caps.expect("execution caps");
        assert_eq!(caps.platform.as_deref(), Some(std::env::consts::OS));
        assert_eq!(
            caps.preferred_shell.as_deref(),
            Some(if cfg!(target_os = "windows") {
                "powershell"
            } else {
                "bash"
            })
        );
        assert!(caps.python_candidates.is_empty());
        assert!(caps.node_candidates.is_empty());
        assert_eq!(
            caps.notes,
            vec![format!(
                "static P0 detection; command execution should prefer exec ({})",
                if cfg!(target_os = "windows") {
                    "powershell"
                } else {
                    "bash"
                }
            )]
        );
        assert!(ctx.file_task_caps.is_none());
    }

    #[test]
    fn tool_context_reuses_task_temp_dir_for_same_session() {
        let first = super::build_tool_context(Some("session-123"), None, None)
            .expect("first tool context")
            .task_temp_dir
            .expect("first temp dir");
        let second = super::build_tool_context(Some("session-123"), None, None)
            .expect("second tool context")
            .task_temp_dir
            .expect("second temp dir");

        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn approval_bus_blocks_file_delete_until_resolved() {
        let db_dir = tempdir().expect("create db dir");
        let db_url = format!(
            "sqlite://{}?mode=rwc",
            db_dir.path().join("approval-test.db").to_string_lossy()
        );
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect(&db_url)
            .await
            .expect("connect sqlite");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS session_runs (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                user_message_id TEXT NOT NULL DEFAULT '',
                assistant_message_id TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'queued',
                buffered_text TEXT NOT NULL DEFAULT '',
                error_kind TEXT NOT NULL DEFAULT '',
                error_message TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create session_runs");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS session_run_events (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create session_run_events");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS approvals (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                call_id TEXT NOT NULL DEFAULT '',
                tool_name TEXT NOT NULL,
                input_json TEXT NOT NULL DEFAULT '{}',
                summary TEXT NOT NULL DEFAULT '',
                impact TEXT NOT NULL DEFAULT '',
                irreversible INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'pending',
                decision TEXT NOT NULL DEFAULT '',
                notify_targets_json TEXT NOT NULL DEFAULT '[]',
                resume_payload_json TEXT NOT NULL DEFAULT '{}',
                resolved_by_surface TEXT NOT NULL DEFAULT '',
                resolved_by_user TEXT NOT NULL DEFAULT '',
                resolved_at TEXT,
                resumed_at TEXT,
                expires_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create approvals");

        let journal_dir = tempdir().expect("create journal dir");
        let runtime = ApprovalWaitRuntime {
            pool: pool.clone(),
            journal: Arc::new(SessionJournalStore::new(journal_dir.path().to_path_buf())),
            approval_manager: Arc::new(ApprovalManager::default()),
            pending_bridge: Arc::new(Mutex::new(None)),
        };

        let work_dir = tempdir().expect("create work dir");
        let target_dir = work_dir.path().join("danger");
        std::fs::create_dir_all(target_dir.join("nested")).expect("create target tree");
        std::fs::write(target_dir.join("nested").join("file.txt"), "danger")
            .expect("write nested file");

        let input = json!({
            "path": target_dir.to_string_lossy().to_string(),
            "recursive": true,
        });
        let tool_ctx = ToolContext {
            work_dir: Some(PathBuf::from(work_dir.path())),
            path_access: Default::default(),
            allowed_tools: None,
            session_id: Some("sess-approval".to_string()),
            task_temp_dir: Some(PathBuf::from(std::env::temp_dir())),
            execution_caps: Some(super::detect_execution_caps()),
            file_task_caps: None,
        };

        let runtime_clone = runtime.clone();
        let manager = runtime.approval_manager.clone();
        let pool_clone = pool.clone();
        let input_clone = input.clone();
        let tool_ctx_clone = tool_ctx.clone();
        let work_dir_path = work_dir.path().to_path_buf();

        let handle = tokio::spawn(async move {
            let decision = request_tool_approval_and_wait(
                &runtime_clone,
                None,
                "sess-approval",
                Some("run-approval"),
                None,
                None,
                "file_delete",
                "call-approval",
                &input_clone,
                Some(work_dir_path.as_path()),
                None,
            )
            .await
            .expect("approval should resolve");
            assert_eq!(decision, ApprovalDecision::AllowOnce);

            let tool = FileDeleteTool;
            tool.execute(input_clone, &tool_ctx_clone)
        });

        let mut pending_row: Option<(String, String)> = None;
        for _ in 0..20 {
            if let Some(row) = sqlx::query_as::<_, (String, String)>(
                "SELECT id, status FROM approvals WHERE session_id = ?",
            )
            .bind("sess-approval")
            .fetch_optional(&pool)
            .await
            .expect("query pending approval")
            {
                pending_row = Some(row);
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert!(target_dir.exists(), "directory must remain before approval");

        let (approval_id, status) = pending_row.expect("load pending approval");
        assert_eq!(status, "pending");

        manager
            .resolve_with_pool(
                &pool_clone,
                &approval_id,
                ApprovalDecision::AllowOnce,
                "desktop",
                "tester",
            )
            .await
            .expect("resolve pending approval");

        let result = handle
            .await
            .expect("join task")
            .expect("file delete success");
        assert!(result.contains("成功删除"));
        assert!(
            !target_dir.exists(),
            "directory should be removed after approval"
        );
    }
}
