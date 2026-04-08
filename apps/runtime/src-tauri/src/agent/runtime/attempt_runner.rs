use super::events::{StreamToken, ToolConfirmResponder};
use super::failover::{
    runtime_failover_error_kind_from_error_text, runtime_failover_error_kind_from_stop_reason_kind,
    runtime_failover_error_kind_key, CandidateAttemptOutcome, RuntimeFailover,
    RuntimeFailoverOutcome, RuntimeFailoverParams,
};
use super::observability::RuntimeObservabilityState;
use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::parse_run_stop_reason;
use crate::agent::types::{AgentStateEvent, StreamDelta};
use crate::agent::AgentExecutor;
use crate::diagnostics::{self, LogLevel, ManagedDiagnosticsState};
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

use super::runtime_io as chat_io;
use crate::model_transport::{resolve_model_transport, ModelTransportKind};

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
    pub full_allowed_tools: Option<&'a [String]>,
    pub has_deferred_tools: bool,
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

fn compute_reasoning_duration_ms(
    reasoning_started_at: Option<std::time::Instant>,
    last_reasoning_at: Option<std::time::Instant>,
) -> Option<u64> {
    let started_at = reasoning_started_at?;
    let ended_at = last_reasoning_at.unwrap_or(started_at);
    Some(ended_at.saturating_duration_since(started_at).as_millis() as u64)
}

fn emit_reasoning_completed_if_needed(
    app: &AppHandle,
    session_id: &str,
    reasoning_started_at: &std::sync::Mutex<Option<std::time::Instant>>,
    last_reasoning_at: &std::sync::Mutex<Option<std::time::Instant>>,
    completion_emitted: &std::sync::Mutex<bool>,
) -> Option<u64> {
    let mut emitted_guard = completion_emitted.lock().ok()?;
    if *emitted_guard {
        return reasoning_started_at.lock().ok().and_then(|started| {
            last_reasoning_at
                .lock()
                .ok()
                .and_then(|ended| compute_reasoning_duration_ms(*started, *ended))
        });
    }

    let duration_ms = reasoning_started_at.lock().ok().and_then(|started| {
        last_reasoning_at
            .lock()
            .ok()
            .and_then(|ended| compute_reasoning_duration_ms(*started, *ended))
    })?;

    let _ = app.emit(
        "assistant-reasoning-completed",
        serde_json::json!({
            "session_id": session_id,
            "duration_ms": duration_ms,
        }),
    );
    *emitted_guard = true;
    Some(duration_ms)
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
    let using_full_tool_exposure = attempt_idx > 0 && params.has_deferred_tools;
    let effective_allowed_tools = if using_full_tool_exposure {
        params.full_allowed_tools.or(params.allowed_tools)
    } else {
        params.allowed_tools
    };
    let streamed_text = Arc::new(std::sync::Mutex::new(String::new()));
    let streamed_reasoning = Arc::new(std::sync::Mutex::new(String::new()));
    let reasoning_started_at = Arc::new(std::sync::Mutex::new(None::<std::time::Instant>));
    let last_reasoning_at = Arc::new(std::sync::Mutex::new(None::<std::time::Instant>));
    let reasoning_completion_emitted = Arc::new(std::sync::Mutex::new(false));
    let app_clone = params.app.clone();
    let session_id_clone = params.session_id.to_string();
    let streamed_text_clone = Arc::clone(&streamed_text);
    let streamed_reasoning_clone = Arc::clone(&streamed_reasoning);
    let reasoning_started_at_clone = Arc::clone(&reasoning_started_at);
    let last_reasoning_at_clone = Arc::clone(&last_reasoning_at);
    let reasoning_completion_emitted_clone = Arc::clone(&reasoning_completion_emitted);

    let transport = resolve_model_transport(
        candidate_api_format,
        candidate_base_url,
        Some(candidate_provider_key).filter(|value| !value.trim().is_empty()),
    );
    let effective_api_format = if candidate_api_format.trim().is_empty() {
        match transport.kind {
            ModelTransportKind::AnthropicMessages => "anthropic",
            ModelTransportKind::OpenAiCompletions | ModelTransportKind::OpenAiResponses => "openai",
        }
    } else {
        candidate_api_format
    };
    if candidate_api_format.trim().is_empty() {
        if let Some(diagnostics_state) = params.app.try_state::<ManagedDiagnosticsState>() {
            let _ = diagnostics::write_log_record(
                &diagnostics_state.0.paths,
                LogLevel::Warn,
                "chat",
                "empty_route_protocol_fallback",
                "route candidate protocol_type was empty; falling back from base_url/transport detection",
                Some(serde_json::json!({
                    "session_id": params.session_id,
                    "provider_key": candidate_provider_key,
                    "base_url": candidate_base_url,
                    "model_name": candidate_model_name,
                    "effective_api_format": effective_api_format,
                    "transport_kind": format!("{:?}", transport.kind),
                })),
            );
        }
    }

    let attempt = params
        .agent_executor
        .execute_turn_with_transport(
            transport,
            effective_api_format,
            candidate_base_url,
            candidate_api_key,
            candidate_model_name,
            params.system_prompt,
            params.messages.to_vec(),
            move |delta: StreamDelta| match delta {
                StreamDelta::Text(token) => {
                    let _ = emit_reasoning_completed_if_needed(
                        &app_clone,
                        &session_id_clone,
                        &reasoning_started_at_clone,
                        &last_reasoning_at_clone,
                        &reasoning_completion_emitted_clone,
                    );
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
                    if let Ok(mut last_at) = last_reasoning_at_clone.lock() {
                        *last_at = Some(std::time::Instant::now());
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
            effective_allowed_tools,
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
                effective_api_format,
                candidate_model_name,
                attempt_idx + 1,
                attempt_idx,
                "ok",
                true,
                "",
            )
            .await;
            let reasoning_duration_ms = emit_reasoning_completed_if_needed(
                params.app,
                params.session_id,
                &reasoning_started_at,
                &last_reasoning_at,
                &reasoning_completion_emitted,
            );
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
                tool_exposure_expanded: using_full_tool_exposure,
                tool_exposure_expansion_reason: using_full_tool_exposure
                    .then(|| "deferred_tools_retry".to_string()),
            }
        }
        Err(err) => {
            let err_text = err.to_string();
            let parsed_stop_reason = parse_run_stop_reason(&err_text);
            let kind = parsed_stop_reason
                .as_ref()
                .map(|reason| runtime_failover_error_kind_from_stop_reason_kind(reason.kind))
                .unwrap_or_else(|| runtime_failover_error_kind_from_error_text(&err_text));
            let kind = if should_expand_tool_exposure(
                &err_text,
                kind,
                params.has_deferred_tools,
                using_full_tool_exposure,
            ) {
                super::failover::RuntimeFailoverErrorKind::DeferredTools
            } else {
                kind
            };
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
                effective_api_format,
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
                tool_exposure_expanded: using_full_tool_exposure,
                tool_exposure_expansion_reason: using_full_tool_exposure
                    .then(|| "deferred_tools_retry".to_string()),
            }
        }
    }
}

fn should_expand_tool_exposure(
    err_text: &str,
    kind: super::failover::RuntimeFailoverErrorKind,
    has_deferred_tools: bool,
    using_full_tool_exposure: bool,
) -> bool {
    if !has_deferred_tools || using_full_tool_exposure {
        return false;
    }

    let lower = err_text.to_ascii_lowercase();
    matches!(
        kind,
        super::failover::RuntimeFailoverErrorKind::MaxTurns
            | super::failover::RuntimeFailoverErrorKind::NoProgress
            | super::failover::RuntimeFailoverErrorKind::PolicyBlocked
    ) || lower.contains("此 skill 不允许使用工具")
        || lower.contains("tool not allowed")
}

#[cfg(test)]
mod tests {
    use super::{build_retrying_agent_state_detail, compute_reasoning_duration_ms};
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

    #[test]
    fn compute_reasoning_duration_uses_last_reasoning_delta_boundary() {
        let started = std::time::Instant::now();
        let ended = started + std::time::Duration::from_millis(420);

        assert_eq!(
            compute_reasoning_duration_ms(Some(started), Some(ended)),
            Some(420)
        );
    }

    #[test]
    fn compute_reasoning_duration_defaults_to_zero_without_end_boundary() {
        let started = std::time::Instant::now();

        assert_eq!(compute_reasoning_duration_ms(Some(started), None), Some(0));
        assert_eq!(compute_reasoning_duration_ms(None, None), None);
    }
}
