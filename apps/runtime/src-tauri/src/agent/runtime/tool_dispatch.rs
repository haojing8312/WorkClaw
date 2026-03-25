use crate::agent::browser_progress::BrowserProgressSnapshot;
use crate::agent::event_bridge::{append_tool_run_event, build_skill_route_event};
use crate::agent::executor::AgentExecutor;
use crate::agent::permissions::PermissionMode;
use crate::agent::progress::{json_progress_signature, text_progress_signature};
use crate::agent::run_guard::{encode_run_stop_reason, ProgressFingerprint};
use crate::agent::safety::classify_policy_blocked_tool_error;
use crate::agent::types::{AgentStateEvent, Tool, ToolCall, ToolCallEvent, ToolContext, ToolResult};
use crate::agent::runtime::approval_gate::gate_tool_approval;
use crate::session_journal::SessionRunEvent;
use anyhow::{anyhow, Result};
use runtime_executor_core::{
    extract_tool_call_parse_error, split_error_code_and_message, truncate_tool_output,
    update_tool_failure_streak, ToolFailureStreak, MAX_TOOL_OUTPUT_CHARS,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

pub(crate) struct ToolDispatchContext<'a> {
    pub agent: &'a AgentExecutor,
    pub app_handle: Option<&'a AppHandle>,
    pub session_id: Option<&'a str>,
    pub persisted_run_id: Option<&'a str>,
    pub allowed_tools: Option<&'a [String]>,
    pub permission_mode: PermissionMode,
    pub tool_ctx: &'a ToolContext,
    pub tool_confirm_tx: Option<&'a Arc<Mutex<Option<std::sync::mpsc::Sender<bool>>>>>,
    pub cancel_flag: Option<Arc<AtomicBool>>,
    pub route_run_id: &'a str,
    pub route_node_timeout_secs: u64,
    pub route_retry_count: usize,
    pub iteration: usize,
}

pub(crate) struct ToolDispatchState<'a> {
    pub tool_results: &'a mut Vec<ToolResult>,
    pub repeated_failure_summary: &'a mut Option<String>,
    pub tool_failure_streak: &'a mut Option<ToolFailureStreak>,
    pub progress_history: &'a mut Vec<ProgressFingerprint>,
    pub latest_browser_progress: &'a mut Option<BrowserProgressSnapshot>,
}

pub(crate) enum ToolDispatchOutcome {
    Continue,
    Cancelled,
}

pub(crate) async fn dispatch_tool_call(
    ctx: &ToolDispatchContext<'_>,
    state: &mut ToolDispatchState<'_>,
    call_index: usize,
    call: &ToolCall,
) -> Result<ToolDispatchOutcome> {
    let skill_name = call
        .input
        .get("skill_name")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let is_skill_call = call.name == "skill";
    let node_id = format!("{}-{}-{}", ctx.iteration, call_index, call.id);
    let started_at = std::time::Instant::now();

    if is_skill_call {
        if let (Some(app), Some(sid)) = (ctx.app_handle, ctx.session_id) {
            let _ = app.emit(
                "skill-route-node-updated",
                build_skill_route_event(
                    sid,
                    ctx.route_run_id,
                    &node_id,
                    None,
                    &skill_name,
                    1,
                    "routing",
                    None,
                    None,
                    None,
                ),
            );
        }
    }

    if let Some(ref flag) = ctx.cancel_flag {
        if flag.load(Ordering::SeqCst) {
            eprintln!("[agent] 工具执行中被用户取消");
            return Ok(ToolDispatchOutcome::Cancelled);
        }
    }

    eprintln!("[agent] Calling tool: {}", call.name);

    if is_skill_call {
        if let (Some(app), Some(sid)) = (ctx.app_handle, ctx.session_id) {
            let _ = app.emit(
                "skill-route-node-updated",
                build_skill_route_event(
                    sid,
                    ctx.route_run_id,
                    &node_id,
                    None,
                    &skill_name,
                    1,
                    "executing",
                    None,
                    None,
                    None,
                ),
            );
        }
    }

    if let (Some(app), Some(sid)) = (ctx.app_handle, ctx.session_id) {
        let _ = app.emit(
            "tool-call-event",
            ToolCallEvent {
                session_id: sid.to_string(),
                tool_name: call.name.clone(),
                tool_input: call.input.clone(),
                tool_output: None,
                status: "started".to_string(),
            },
        );
        if let Some(run_id) = ctx.persisted_run_id {
            let _ = append_tool_run_event(
                app,
                sid,
                SessionRunEvent::ToolStarted {
                    run_id: run_id.to_string(),
                    tool_name: call.name.clone(),
                    call_id: call.id.clone(),
                    input: call.input.clone(),
                },
            )
            .await;
        }
    }

    if ctx
        .permission_mode
        .needs_confirmation(&call.name, &call.input, ctx.tool_ctx.work_dir.as_deref())
    {
        let approval_decision: Result<Option<crate::approval_bus::ApprovalDecision>> =
            gate_tool_approval(
            ctx.app_handle,
            ctx.session_id,
            ctx.persisted_run_id,
            call,
            ctx.tool_ctx.work_dir.as_deref(),
            ctx.tool_confirm_tx,
            ctx.cancel_flag.clone(),
        )
        .await;

        let Some(approval_decision) = (match approval_decision {
            Ok(Some(decision)) => Some(decision),
            Ok(None) => {
                state.tool_results.push(ToolResult {
                    tool_use_id: call.id.clone(),
                    content: "工具确认超时，已取消此操作".to_string(),
                });
                return Ok(ToolDispatchOutcome::Continue);
            }
            Err(err) => {
                let rejection_message = err.to_string();
                if let (Some(app), Some(sid)) = (ctx.app_handle, ctx.session_id) {
                    let _ = app.emit(
                        "tool-call-event",
                        ToolCallEvent {
                            session_id: sid.to_string(),
                            tool_name: call.name.clone(),
                            tool_input: call.input.clone(),
                            tool_output: Some(rejection_message.clone()),
                            status: "error".to_string(),
                        },
                    );
                    if let Some(run_id) = ctx.persisted_run_id {
                        let _ = append_tool_run_event(
                            app,
                            sid,
                            SessionRunEvent::ToolCompleted {
                                run_id: run_id.to_string(),
                                tool_name: call.name.clone(),
                                call_id: call.id.clone(),
                                input: call.input.clone(),
                                output: rejection_message.clone(),
                                is_error: true,
                            },
                        )
                        .await;
                    }
                }
                state.tool_results.push(ToolResult {
                    tool_use_id: call.id.clone(),
                    content: rejection_message,
                });
                return Ok(ToolDispatchOutcome::Continue);
            }
        }) else {
            return Ok(ToolDispatchOutcome::Continue);
        };

        if approval_decision == crate::approval_bus::ApprovalDecision::Deny {
            let rejection_message = "用户拒绝了此操作";
            if let (Some(app), Some(sid)) = (ctx.app_handle, ctx.session_id) {
                let _ = app.emit(
                    "tool-call-event",
                    ToolCallEvent {
                        session_id: sid.to_string(),
                        tool_name: call.name.clone(),
                        tool_input: call.input.clone(),
                        tool_output: Some(rejection_message.to_string()),
                        status: "error".to_string(),
                    },
                );
                if let Some(run_id) = ctx.persisted_run_id {
                    let _ = append_tool_run_event(
                        app,
                        sid,
                        SessionRunEvent::ToolCompleted {
                            run_id: run_id.to_string(),
                            tool_name: call.name.clone(),
                            call_id: call.id.clone(),
                            input: call.input.clone(),
                            output: rejection_message.to_string(),
                            is_error: true,
                        },
                    )
                    .await;
                }
            }
            state.tool_results.push(ToolResult {
                tool_use_id: call.id.clone(),
                content: rejection_message.to_string(),
            });
            return Ok(ToolDispatchOutcome::Continue);
        }
    }

    let max_attempts = if is_skill_call {
        ctx.route_retry_count + 1
    } else {
        1
    };
    let mut attempt = 0usize;
    let (result, is_error) = loop {
        attempt += 1;
        let (result, is_error) = if let Some(parse_error) = extract_tool_call_parse_error(&call.input)
        {
            (
                format!(
                    "工具参数错误: {}。请提供完整且合法的 JSON 参数后再重试。",
                    parse_error
                ),
                true,
            )
        } else {
            match ctx.agent.registry.get(&call.name) {
                Some(tool) => {
                    if let Some(whitelist) = ctx.allowed_tools {
                        if !whitelist.iter().any(|w| w == &call.name) {
                            (
                                format!("此 Skill 不允许使用工具: {}", call.name),
                                true,
                            )
                        } else {
                            run_tool(tool, call, ctx.tool_ctx, ctx.cancel_flag.clone(), ctx.route_node_timeout_secs, is_skill_call).await
                        }
                    } else {
                        run_tool(tool, call, ctx.tool_ctx, ctx.cancel_flag.clone(), ctx.route_node_timeout_secs, is_skill_call).await
                    }
                }
                None => {
                    let available: Vec<String> = ctx
                        .agent
                        .registry
                        .get_tool_definitions()
                        .iter()
                        .filter_map(|t| t["name"].as_str().map(String::from))
                        .collect();
                    (
                        format!(
                            "错误: 工具 '{}' 不存在。请勿再次调用此工具。可用工具: {}",
                            call.name,
                            available.join(", ")
                        ),
                        true,
                    )
                }
            }
        };
        if !is_error || attempt >= max_attempts {
            break (result, is_error);
        }
    };

    let result = truncate_tool_output(&result, MAX_TOOL_OUTPUT_CHARS);

    if is_error {
        if let Some(summary) = update_tool_failure_streak(
            state.tool_failure_streak,
            &call.name,
            &call.input,
            &result,
        ) {
            *state.repeated_failure_summary = Some(summary);
        }
    } else {
        *state.tool_failure_streak = None;
    }

    if let (Some(app), Some(sid)) = (ctx.app_handle, ctx.session_id) {
        let _ = app.emit(
            "tool-call-event",
            ToolCallEvent {
                session_id: sid.to_string(),
                tool_name: call.name.clone(),
                tool_input: call.input.clone(),
                tool_output: Some(result.clone()),
                status: if is_error {
                    "error".to_string()
                } else {
                    "completed".to_string()
                },
            },
        );
        if let Some(run_id) = ctx.persisted_run_id {
            let _ = append_tool_run_event(
                app,
                sid,
                SessionRunEvent::ToolCompleted {
                    run_id: run_id.to_string(),
                    tool_name: call.name.clone(),
                    call_id: call.id.clone(),
                    input: call.input.clone(),
                    output: result.clone(),
                    is_error,
                },
            )
            .await;
        }
    }

    if is_skill_call {
        if let (Some(app), Some(sid)) = (ctx.app_handle, ctx.session_id) {
            let duration_ms = started_at.elapsed().as_millis() as u64;
            let parsed_error = if is_error {
                Some(split_error_code_and_message(&result))
            } else {
                None
            };
            let _ = app.emit(
                "skill-route-node-updated",
                build_skill_route_event(
                    sid,
                    ctx.route_run_id,
                    &node_id,
                    None,
                    &skill_name,
                    1,
                    if is_error { "failed" } else { "completed" },
                    Some(duration_ms),
                    parsed_error.as_ref().map(|(code, _)| code.as_str()),
                    parsed_error.as_ref().map(|(_, msg)| msg.as_str()),
                ),
            );
        }
    }

    if is_error {
        if let Some(mut stop_reason) = classify_policy_blocked_tool_error(&call.name, &result) {
            if let Some(last_completed_step) = state
                .latest_browser_progress
                .as_ref()
                .and_then(BrowserProgressSnapshot::last_completed_step)
            {
                stop_reason = stop_reason.with_last_completed_step(last_completed_step);
            }
            if let (Some(app), Some(sid)) = (ctx.app_handle, ctx.session_id) {
                let _ = app.emit(
                    "agent-state-event",
                    AgentStateEvent::stopped(sid, ctx.iteration, &stop_reason),
                );
            }
            return Err(anyhow!(encode_run_stop_reason(&stop_reason)));
        }
    }

    let input_signature = json_progress_signature(&call.input);
    let browser_progress_snapshot = if is_error {
        None
    } else {
        BrowserProgressSnapshot::from_tool_output(&call.name, &result)
    };
    let output_signature = if let Some(snapshot) = browser_progress_snapshot.as_ref() {
        snapshot.progress_signature()
    } else {
        let progress_text = if is_error {
            format!("error:{result}")
        } else {
            result.clone()
        };
        text_progress_signature(&progress_text)
    };
    if let Some(snapshot) = browser_progress_snapshot {
        *state.latest_browser_progress = Some(snapshot);
    }
    state.progress_history.push(ProgressFingerprint::tool_result(
        call.name.clone(),
        input_signature,
        output_signature,
    ));

    state.tool_results.push(ToolResult {
        tool_use_id: call.id.clone(),
        content: result,
    });

    Ok(ToolDispatchOutcome::Continue)
}

async fn run_tool(
    tool: Arc<dyn Tool>,
    call: &ToolCall,
    tool_ctx: &ToolContext,
    cancel_flag: Option<Arc<AtomicBool>>,
    route_node_timeout_secs: u64,
    is_skill_call: bool,
) -> (String, bool) {
    let tool_clone = Arc::clone(&tool);
    let input_clone = call.input.clone();
    let ctx_clone = tool_ctx.clone();
    let handle = tokio::task::spawn_blocking(move || tool_clone.execute(input_clone, &ctx_clone));
    let exec_future = async move {
        tokio::select! {
            res = handle => {
                match res {
                    Ok(Ok(output)) => (output, false),
                    Ok(Err(e)) => (format!("工具执行错误: {}", e), true),
                    Err(e) => (format!("工具执行线程异常: {}", e), true),
                }
            }
            _ = AgentExecutor::wait_for_cancel(&cancel_flag) => {
                ("工具执行被用户取消".to_string(), true)
            }
        }
    };

    if is_skill_call {
        match tokio::time::timeout(
            std::time::Duration::from_secs(route_node_timeout_secs),
            exec_future,
        )
        .await
        {
            Ok(v) => v,
            Err(_) => ("TIMEOUT: 子 Skill 执行超时".to_string(), true),
        }
    } else {
        exec_future.await
    }
}
