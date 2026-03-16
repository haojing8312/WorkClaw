use super::chat::{StreamToken, ToolConfirmResponder};
use super::chat_policy::{model_route_error_kind_for_stop_reason_kind, ModelRouteErrorKind};
use super::chat_runtime_io as chat_io;
use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::{parse_run_stop_reason, RunStopReason};
use crate::agent::types::StreamDelta;
use crate::agent::AgentExecutor;
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub(crate) struct RouteExecutionOutcome {
    pub final_messages: Option<Vec<Value>>,
    pub last_error: Option<String>,
    pub last_error_kind: Option<String>,
    pub last_stop_reason: Option<RunStopReason>,
    pub partial_text: String,
    pub reasoning_text: String,
    pub reasoning_duration_ms: Option<u64>,
}

pub(crate) struct RouteExecutionParams<'a> {
    pub app: &'a AppHandle,
    pub agent_executor: &'a AgentExecutor,
    pub db: &'a sqlx::SqlitePool,
    pub session_id: &'a str,
    pub requested_capability: &'a str,
    pub route_candidates: &'a [(String, String, String, String)],
    pub per_candidate_retry_count: usize,
    pub system_prompt: &'a str,
    pub messages: &'a [Value],
    pub allowed_tools: Option<&'a [String]>,
    pub permission_mode: PermissionMode,
    pub tool_confirm_responder: ToolConfirmResponder,
    pub executor_work_dir: Option<String>,
    pub max_iterations: Option<usize>,
    pub cancel_flag: Arc<AtomicBool>,
    pub node_timeout_seconds: u64,
    pub route_retry_count: usize,
    pub classify_error: fn(&str) -> ModelRouteErrorKind,
    pub error_kind_key: fn(ModelRouteErrorKind) -> &'static str,
    pub should_retry_same_candidate: fn(ModelRouteErrorKind) -> bool,
    pub retry_budget_for_error: fn(ModelRouteErrorKind, usize) -> usize,
    pub retry_backoff_ms: fn(ModelRouteErrorKind, usize) -> u64,
}

pub(crate) async fn execute_route_candidates(
    params: RouteExecutionParams<'_>,
) -> RouteExecutionOutcome {
    let mut final_messages_opt: Option<Vec<Value>> = None;
    let mut last_error: Option<String> = None;
    let mut last_error_kind: Option<String> = None;
    let mut last_stop_reason: Option<RunStopReason> = None;
    let streamed_text = Arc::new(std::sync::Mutex::new(String::new()));
    let streamed_reasoning = Arc::new(std::sync::Mutex::new(String::new()));

    for (candidate_api_format, candidate_base_url, candidate_model_name, candidate_api_key) in
        params.route_candidates
    {
        let mut attempt_idx = 0usize;
        loop {
            if let Ok(mut buffer) = streamed_text.lock() {
                buffer.clear();
            }
            if let Ok(mut buffer) = streamed_reasoning.lock() {
                buffer.clear();
            }
            let app_clone = params.app.clone();
            let session_id_clone = params.session_id.to_string();
            let streamed_text_clone = Arc::clone(&streamed_text);
            let streamed_reasoning_clone = Arc::clone(&streamed_reasoning);
            let reasoning_started_at = Arc::new(std::sync::Mutex::new(None::<std::time::Instant>));
            let reasoning_started_at_clone = Arc::clone(&reasoning_started_at);
            let attempt = params
                .agent_executor
                .execute_turn(
                    candidate_api_format,
                    candidate_base_url,
                    candidate_api_key,
                    candidate_model_name,
                    params.system_prompt,
                    params.messages.to_vec(),
                    move |delta: StreamDelta| match delta {
                        StreamDelta::Text(token) => {
                            if let Ok(mut buffer) = streamed_text_clone.lock() {
                                buffer.push_str(&token);
                            }
                            let _ = app_clone.emit(
                                "stream-token",
                                StreamToken {
                                    session_id: session_id_clone.clone(),
                                    token,
                                    done: false,
                                    sub_agent: false,
                                },
                            );
                        }
                        StreamDelta::Reasoning(text) => {
                            let emit_started =
                                if let Ok(mut started) = reasoning_started_at_clone.lock() {
                                    if started.is_none() {
                                        *started = Some(std::time::Instant::now());
                                        true
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                };
                            if emit_started {
                                let _ = app_clone.emit(
                                    "assistant-reasoning-started",
                                    serde_json::json!({ "session_id": session_id_clone.clone() }),
                                );
                            }
                            if let Ok(mut buffer) = streamed_reasoning_clone.lock() {
                                buffer.push_str(&text);
                            }
                            let _ = app_clone.emit(
                                "assistant-reasoning-delta",
                                serde_json::json!({
                                    "session_id": session_id_clone.clone(),
                                    "text": text,
                                }),
                            );
                        }
                    },
                    Some(params.app),
                    Some(params.session_id),
                    params.allowed_tools,
                    params.permission_mode,
                    Some(params.tool_confirm_responder.clone()),
                    params.executor_work_dir.clone(),
                    params.max_iterations,
                    Some(params.cancel_flag.clone()),
                    Some(params.node_timeout_seconds),
                    Some(params.route_retry_count),
                )
                .await;

            match attempt {
                Ok(messages_out) => {
                    chat_io::record_route_attempt_log_with_pool(
                        params.db,
                        params.session_id,
                        params.requested_capability,
                        candidate_api_format,
                        candidate_model_name,
                        attempt_idx + 1,
                        attempt_idx,
                        "ok",
                        true,
                        "",
                    )
                    .await;
                    let reasoning_duration_ms =
                        reasoning_started_at.lock().ok().and_then(|started| {
                            started.map(|instant| instant.elapsed().as_millis() as u64)
                        });
                    if let Some(duration_ms) = reasoning_duration_ms {
                        let _ = params.app.emit(
                            "assistant-reasoning-completed",
                            serde_json::json!({
                                "session_id": params.session_id,
                                "duration_ms": duration_ms,
                            }),
                        );
                    }
                    final_messages_opt = Some(messages_out);
                    let reasoning_snapshot = streamed_reasoning
                        .lock()
                        .map(|buffer| buffer.clone())
                        .unwrap_or_default();
                    return RouteExecutionOutcome {
                        final_messages: final_messages_opt,
                        last_error,
                        last_error_kind,
                        last_stop_reason,
                        partial_text: streamed_text
                            .lock()
                            .map(|buffer| buffer.clone())
                            .unwrap_or_default(),
                        reasoning_text: reasoning_snapshot,
                        reasoning_duration_ms,
                    };
                }
                Err(err) => {
                    let err_text = err.to_string();
                    let parsed_stop_reason = parse_run_stop_reason(&err_text);
                    let kind = parsed_stop_reason
                        .as_ref()
                        .map(|reason| model_route_error_kind_for_stop_reason_kind(reason.kind))
                        .unwrap_or_else(|| (params.classify_error)(&err_text));
                    let kind_text = (params.error_kind_key)(kind);
                    let user_facing_error = parsed_stop_reason
                        .as_ref()
                        .map(|reason| {
                            if let Some(step) = reason.last_completed_step.as_deref() {
                                format!("{}\n最后完成步骤：{}", reason.message, step)
                            } else {
                                reason.message.clone()
                            }
                        })
                        .unwrap_or_else(|| err_text.clone());
                    chat_io::record_route_attempt_log_with_pool(
                        params.db,
                        params.session_id,
                        params.requested_capability,
                        candidate_api_format,
                        candidate_model_name,
                        attempt_idx + 1,
                        attempt_idx,
                        kind_text,
                        false,
                        &user_facing_error,
                    )
                    .await;
                    if reasoning_started_at
                        .lock()
                        .ok()
                        .and_then(|started| *started)
                        .is_some()
                    {
                        let _ = params.app.emit(
                            "assistant-reasoning-interrupted",
                            serde_json::json!({ "session_id": params.session_id }),
                        );
                    }
                    last_error = Some(user_facing_error.clone());
                    last_error_kind = Some(kind_text.to_string());
                    last_stop_reason = parsed_stop_reason;
                    eprintln!(
                        "[routing] 候选模型执行失败: format={}, model={}, attempt={}, kind={:?}, err={}",
                        candidate_api_format,
                        candidate_model_name,
                        attempt_idx + 1,
                        kind,
                        err_text
                    );

                    let retry_budget =
                        (params.retry_budget_for_error)(kind, params.per_candidate_retry_count);
                    if (params.should_retry_same_candidate)(kind) && attempt_idx < retry_budget {
                        let backoff_ms = (params.retry_backoff_ms)(kind, attempt_idx);
                        if backoff_ms > 0 {
                            eprintln!(
                                "[routing] 同候选重试等待: format={}, model={}, wait_ms={}, next_attempt={}",
                                candidate_api_format,
                                candidate_model_name,
                                backoff_ms,
                                attempt_idx + 2
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                        }
                        attempt_idx += 1;
                        continue;
                    }
                    break;
                }
            }
        }
    }

    let partial_text = streamed_text
        .lock()
        .map(|buffer| buffer.clone())
        .unwrap_or_default();
    let reasoning_text = streamed_reasoning
        .lock()
        .map(|buffer| buffer.clone())
        .unwrap_or_default();

    RouteExecutionOutcome {
        final_messages: final_messages_opt,
        last_error,
        last_error_kind,
        last_stop_reason,
        partial_text,
        reasoning_text,
        reasoning_duration_ms: None,
    }
}
