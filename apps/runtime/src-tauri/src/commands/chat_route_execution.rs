use super::chat::{StreamToken, ToolConfirmResponder};
use super::chat_policy::{self, ModelRouteErrorKind};
use super::chat_runtime_io as chat_io;
use crate::agent::permissions::PermissionMode;
use crate::agent::runtime::{
    CandidateAttemptOutcome, RuntimeFailover, RuntimeFailoverErrorKind, RuntimeFailoverOutcome,
    RuntimeFailoverParams,
};
use crate::agent::run_guard::{parse_run_stop_reason, RunStopReasonKind};
use crate::agent::types::StreamDelta;
use crate::agent::AgentExecutor;
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub(crate) type RouteExecutionOutcome = RuntimeFailoverOutcome;

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
}

pub(crate) async fn execute_route_candidates(
    params: RouteExecutionParams<'_>,
) -> RouteExecutionOutcome {
    let params_ref = &params;
    RuntimeFailover::execute_candidates(RuntimeFailoverParams {
        route_candidates: params.route_candidates,
        per_candidate_retry_count: params.per_candidate_retry_count,
        attempt_once: Box::new(move |candidate_api_format, candidate_base_url, candidate_model_name, candidate_api_key, attempt_idx| {
            Box::pin(execute_candidate_attempt(
                params_ref,
                candidate_api_format,
                candidate_base_url,
                candidate_model_name,
                candidate_api_key,
                attempt_idx,
            ))
        }),
    })
    .await
}

async fn execute_candidate_attempt(
    params: &RouteExecutionParams<'_>,
    candidate_api_format: &str,
    candidate_base_url: &str,
    candidate_model_name: &str,
    candidate_api_key: &str,
    attempt_idx: usize,
) -> CandidateAttemptOutcome {
    let streamed_text = Arc::new(std::sync::Mutex::new(String::new()));
    let streamed_reasoning = Arc::new(std::sync::Mutex::new(String::new()));
    let reasoning_started_at = Arc::new(std::sync::Mutex::new(None::<std::time::Instant>));
    let app_clone = params.app.clone();
    let session_id_clone = params.session_id.to_string();
    let streamed_text_clone = Arc::clone(&streamed_text);
    let streamed_reasoning_clone = Arc::clone(&streamed_reasoning);
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
                    let emit_started = if let Ok(mut started) = reasoning_started_at_clone.lock() {
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
            let reasoning_duration_ms = reasoning_started_at.lock().ok().and_then(|started| {
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
            CandidateAttemptOutcome {
                final_messages: Some(messages_out),
                last_error: None,
                last_error_kind: None,
                error_kind: None,
                last_stop_reason: None,
                partial_text: streamed_text
                    .lock()
                    .map(|buffer| buffer.clone())
                    .unwrap_or_default(),
                reasoning_text: streamed_reasoning
                    .lock()
                    .map(|buffer| buffer.clone())
                    .unwrap_or_default(),
                reasoning_duration_ms,
            }
        }
        Err(err) => {
            let err_text = err.to_string();
            let parsed_stop_reason = parse_run_stop_reason(&err_text);
            let kind = parsed_stop_reason
                .as_ref()
                .map(|reason| runtime_failover_error_kind_for_stop_reason_kind(reason.kind))
                .unwrap_or_else(|| runtime_failover_error_kind_from_command_kind(
                    chat_policy::classify_model_route_error(&err_text),
                ));
            let kind_text = runtime_error_kind_key(kind);
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
            CandidateAttemptOutcome {
                final_messages: None,
                last_error: Some(user_facing_error),
                last_error_kind: Some(kind_text.to_string()),
                error_kind: Some(kind),
                last_stop_reason: parsed_stop_reason,
                partial_text: streamed_text
                    .lock()
                    .map(|buffer| buffer.clone())
                    .unwrap_or_default(),
                reasoning_text: streamed_reasoning
                    .lock()
                    .map(|buffer| buffer.clone())
                    .unwrap_or_default(),
                reasoning_duration_ms: None,
            }
        }
    }
}

fn runtime_failover_error_kind_from_command_kind(
    kind: ModelRouteErrorKind,
) -> RuntimeFailoverErrorKind {
    match kind {
        ModelRouteErrorKind::Billing => RuntimeFailoverErrorKind::Billing,
        ModelRouteErrorKind::Auth => RuntimeFailoverErrorKind::Auth,
        ModelRouteErrorKind::RateLimit => RuntimeFailoverErrorKind::RateLimit,
        ModelRouteErrorKind::Timeout => RuntimeFailoverErrorKind::Timeout,
        ModelRouteErrorKind::Network => RuntimeFailoverErrorKind::Network,
        ModelRouteErrorKind::PolicyBlocked => RuntimeFailoverErrorKind::PolicyBlocked,
        ModelRouteErrorKind::MaxTurns => RuntimeFailoverErrorKind::MaxTurns,
        ModelRouteErrorKind::LoopDetected => RuntimeFailoverErrorKind::LoopDetected,
        ModelRouteErrorKind::NoProgress => RuntimeFailoverErrorKind::NoProgress,
        ModelRouteErrorKind::Unknown => RuntimeFailoverErrorKind::Unknown,
    }
}

fn runtime_failover_error_kind_for_stop_reason_kind(
    kind: RunStopReasonKind,
) -> RuntimeFailoverErrorKind {
    match kind {
        RunStopReasonKind::Timeout => RuntimeFailoverErrorKind::Timeout,
        RunStopReasonKind::PolicyBlocked => RuntimeFailoverErrorKind::PolicyBlocked,
        RunStopReasonKind::MaxTurns | RunStopReasonKind::MaxSessionTurns => {
            RuntimeFailoverErrorKind::MaxTurns
        }
        RunStopReasonKind::LoopDetected | RunStopReasonKind::ToolFailureCircuitBreaker => {
            RuntimeFailoverErrorKind::LoopDetected
        }
        RunStopReasonKind::NoProgress => RuntimeFailoverErrorKind::NoProgress,
        _ => RuntimeFailoverErrorKind::Unknown,
    }
}

fn runtime_error_kind_key(kind: RuntimeFailoverErrorKind) -> &'static str {
    match kind {
        RuntimeFailoverErrorKind::Billing => "billing",
        RuntimeFailoverErrorKind::Auth => "auth",
        RuntimeFailoverErrorKind::RateLimit => "rate_limit",
        RuntimeFailoverErrorKind::Timeout => "timeout",
        RuntimeFailoverErrorKind::Network => "network",
        RuntimeFailoverErrorKind::PolicyBlocked => "policy_blocked",
        RuntimeFailoverErrorKind::MaxTurns => "max_turns",
        RuntimeFailoverErrorKind::LoopDetected => "loop_detected",
        RuntimeFailoverErrorKind::NoProgress => "no_progress",
        RuntimeFailoverErrorKind::Unknown => "unknown",
    }
}
