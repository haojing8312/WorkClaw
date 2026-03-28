use super::events::{StreamToken, ToolConfirmResponder};
use super::observability::RuntimeObservabilityState;
use super::failover::{
    runtime_failover_error_kind_from_error_text, runtime_failover_error_kind_from_stop_reason_kind,
    runtime_failover_error_kind_key, CandidateAttemptOutcome, RuntimeFailover,
    RuntimeFailoverOutcome, RuntimeFailoverParams,
};
use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::parse_run_stop_reason;
use crate::agent::types::{AgentStateEvent, StreamDelta};
use crate::agent::AgentExecutor;
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

use super::runtime_io as chat_io;
use crate::model_transport::resolve_model_transport;

pub(crate) type RouteExecutionOutcome = RuntimeFailoverOutcome;

#[derive(Clone)]
pub(crate) struct RouteExecutionParams<'a> {
    pub app: &'a AppHandle,
    pub agent_executor: &'a AgentExecutor,
    pub db: &'a sqlx::SqlitePool,
    pub session_id: &'a str,
    pub requested_capability: &'a str,
    pub route_candidates: &'a [(String, String, String, String, String)],
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
    let runtime_observability = params
        .app
        .try_state::<RuntimeObservabilityState>()
        .map(|state| state.0.clone());
    RuntimeFailover::execute_candidates(RuntimeFailoverParams {
        route_candidates: params.route_candidates,
        per_candidate_retry_count: params.per_candidate_retry_count,
        on_same_candidate_retry: Some(Box::new(move |kind, retry_attempt, total_retries| {
            if let Some(detail) =
                build_retrying_agent_state_detail(kind, retry_attempt, total_retries)
            {
                let _ = params_ref.app.emit(
                    "agent-state-event",
                    AgentStateEvent::basic(
                        params_ref.session_id,
                        "retrying",
                        Some(detail),
                        retry_attempt,
                    ),
                );
            }
        })),
        on_error_kind: runtime_observability.map(|observability| {
            Box::new(move |kind| {
                observability.record_failover_error_kind(runtime_failover_error_kind_key(kind));
            }) as Box<dyn FnMut(super::failover::RuntimeFailoverErrorKind) + Send>
        }),
        attempt_once: Box::new(
            move |candidate_api_format,
                  candidate_provider_key,
                  candidate_base_url,
                  candidate_model_name,
                  candidate_api_key,
                  attempt_idx| {
                Box::pin(execute_candidate_attempt(
                    params_ref,
                    candidate_api_format,
                    candidate_provider_key,
                    candidate_base_url,
                    candidate_model_name,
                    candidate_api_key,
                    attempt_idx,
                ))
            },
        ),
    })
    .await
}

fn build_retrying_agent_state_detail(
    kind: super::failover::RuntimeFailoverErrorKind,
    retry_attempt: usize,
    total_retries: usize,
) -> Option<String> {
    if total_retries == 0 {
        return None;
    }

    match kind {
        super::failover::RuntimeFailoverErrorKind::Network => Some(format!(
            "网络异常，正在自动重试（第 {retry_attempt}/{total_retries} 次）"
        )),
        _ => None,
    }
}

async fn execute_candidate_attempt(
    params: &RouteExecutionParams<'_>,
    candidate_api_format: &str,
    candidate_provider_key: &str,
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

    let transport = resolve_model_transport(
        candidate_api_format,
        candidate_base_url,
        Some(candidate_provider_key).filter(|value| !value.trim().is_empty()),
    );

    let attempt = params
        .agent_executor
        .execute_turn_with_transport(
            transport,
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
            let reasoning_duration_ms = reasoning_started_at
                .lock()
                .ok()
                .and_then(|started| started.map(|instant| instant.elapsed().as_millis() as u64));
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
                .map(|reason| runtime_failover_error_kind_from_stop_reason_kind(reason.kind))
                .unwrap_or_else(|| runtime_failover_error_kind_from_error_text(&err_text));
            let kind_text = runtime_failover_error_kind_key(kind);
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

#[cfg(test)]
mod tests {
    use super::build_retrying_agent_state_detail;
    use crate::agent::runtime::failover::RuntimeFailoverErrorKind;

    #[test]
    fn build_retrying_agent_state_detail_formats_network_retries() {
        assert_eq!(
            build_retrying_agent_state_detail(RuntimeFailoverErrorKind::Network, 2, 5).as_deref(),
            Some("网络异常，正在自动重试（第 2/5 次）")
        );
        assert_eq!(
            build_retrying_agent_state_detail(RuntimeFailoverErrorKind::Timeout, 1, 5),
            None
        );
    }
}
