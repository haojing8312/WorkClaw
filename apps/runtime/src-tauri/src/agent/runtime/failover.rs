use crate::agent::run_guard::{parse_run_stop_reason, RunStopReason, RunStopReasonKind};
use crate::agent::runtime::kernel::turn_state::TurnCompactionBoundary;
use crate::model_errors::normalize_model_error;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeFailoverErrorKind {
    Billing,
    Auth,
    RateLimit,
    Timeout,
    Network,
    DeferredTools,
    PolicyBlocked,
    MaxTurns,
    LoopDetected,
    NoProgress,
    Unknown,
}

#[derive(Debug, Clone)]
pub(crate) struct CandidateAttemptOutcome {
    pub final_messages: Option<Vec<Value>>,
    pub last_error: Option<String>,
    pub last_error_kind: Option<String>,
    pub error_kind: Option<RuntimeFailoverErrorKind>,
    pub last_stop_reason: Option<RunStopReason>,
    pub partial_text: String,
    pub reasoning_text: String,
    pub reasoning_duration_ms: Option<u64>,
    pub tool_exposure_expanded: bool,
    pub tool_exposure_expansion_reason: Option<String>,
    pub compaction_boundary: Option<TurnCompactionBoundary>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeFailoverOutcome {
    pub final_messages: Option<Vec<Value>>,
    pub last_error: Option<String>,
    pub last_error_kind: Option<String>,
    pub last_stop_reason: Option<RunStopReason>,
    pub partial_text: String,
    pub reasoning_text: String,
    pub reasoning_duration_ms: Option<u64>,
    pub tool_exposure_expanded: bool,
    pub tool_exposure_expansion_reason: Option<String>,
    pub compaction_boundary: Option<TurnCompactionBoundary>,
}

pub(crate) struct RuntimeFailoverParams<'a> {
    pub route_candidates: &'a [(String, String, String, String, String)],
    pub per_candidate_retry_count: usize,
    pub on_same_candidate_retry:
        Option<Box<dyn FnMut(RuntimeFailoverErrorKind, usize, usize) + Send + 'a>>,
    pub on_error_kind: Option<Box<dyn FnMut(RuntimeFailoverErrorKind) + Send + 'a>>,
    pub attempt_once: Box<
        dyn FnMut(
                &'a str,
                &'a str,
                &'a str,
                &'a str,
                &'a str,
                usize,
            ) -> Pin<Box<dyn Future<Output = CandidateAttemptOutcome> + Send + 'a>>
            + Send
            + 'a,
    >,
}

pub struct RuntimeFailover;

impl RuntimeFailover {
    pub(crate) async fn execute_candidates(
        mut params: RuntimeFailoverParams<'_>,
    ) -> RuntimeFailoverOutcome {
        let mut final_messages_opt: Option<Vec<Value>> = None;
        let mut last_error: Option<String> = None;
        let mut last_error_kind: Option<String> = None;
        let mut last_stop_reason: Option<RunStopReason> = None;
        let mut streamed_text = String::new();
        let mut streamed_reasoning = String::new();
        let mut tool_exposure_expanded = false;
        let mut tool_exposure_expansion_reason: Option<String> = None;
        let mut compaction_boundary: Option<TurnCompactionBoundary> = None;

        for (
            candidate_provider_key,
            candidate_api_format,
            candidate_base_url,
            candidate_model_name,
            candidate_api_key,
        ) in params.route_candidates
        {
            let mut attempt_idx = 0usize;
            loop {
                let attempt = (params.attempt_once)(
                    candidate_provider_key,
                    candidate_api_format,
                    candidate_base_url,
                    candidate_model_name,
                    candidate_api_key,
                    attempt_idx,
                )
                .await;

                if let Some(messages_out) = attempt.final_messages {
                    final_messages_opt = Some(messages_out);
                    streamed_text = attempt.partial_text;
                    streamed_reasoning = attempt.reasoning_text;
                    last_error = attempt.last_error;
                    last_error_kind = attempt.last_error_kind;
                    last_stop_reason = attempt.last_stop_reason;
                    compaction_boundary = attempt.compaction_boundary;
                    return RuntimeFailoverOutcome {
                        final_messages: final_messages_opt,
                        last_error,
                        last_error_kind,
                        last_stop_reason,
                        partial_text: streamed_text,
                        reasoning_text: streamed_reasoning,
                        reasoning_duration_ms: attempt.reasoning_duration_ms,
                        tool_exposure_expanded: attempt.tool_exposure_expanded
                            || tool_exposure_expanded,
                        tool_exposure_expansion_reason: attempt
                            .tool_exposure_expansion_reason
                            .or(tool_exposure_expansion_reason),
                        compaction_boundary,
                    };
                }

                streamed_text = attempt.partial_text;
                streamed_reasoning = attempt.reasoning_text;
                last_error = attempt.last_error.or(last_error);
                last_error_kind = attempt.last_error_kind.or(last_error_kind);
                last_stop_reason = attempt.last_stop_reason.or(last_stop_reason);
                tool_exposure_expanded |= attempt.tool_exposure_expanded;
                tool_exposure_expansion_reason = attempt
                    .tool_exposure_expansion_reason
                    .or(tool_exposure_expansion_reason);
                if let Some(boundary) = attempt.compaction_boundary {
                    compaction_boundary = Some(boundary);
                }

                let current_kind = attempt
                    .error_kind
                    .or_else(|| {
                        last_error_kind
                            .as_deref()
                            .and_then(runtime_failover_kind_from_key)
                            .or_else(|| {
                                last_error
                                    .as_deref()
                                    .map(runtime_failover_error_kind_from_error_text)
                            })
                    })
                    .unwrap_or(RuntimeFailoverErrorKind::Unknown);
                if let Some(on_error_kind) = params.on_error_kind.as_mut() {
                    on_error_kind(current_kind);
                }
                let retry_budget =
                    runtime_retry_budget_for_error(current_kind, params.per_candidate_retry_count);
                if runtime_should_retry_same_candidate(current_kind) && attempt_idx < retry_budget {
                    if let Some(on_retry) = params.on_same_candidate_retry.as_mut() {
                        on_retry(current_kind, attempt_idx + 1, retry_budget);
                    }
                    let backoff_ms = runtime_retry_backoff_ms(current_kind, attempt_idx);
                    if backoff_ms > 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                    }
                    attempt_idx += 1;
                    continue;
                }

                break;
            }
        }

        RuntimeFailoverOutcome {
            final_messages: final_messages_opt,
            last_error,
            last_error_kind,
            last_stop_reason,
            partial_text: streamed_text,
            reasoning_text: streamed_reasoning,
            reasoning_duration_ms: None,
            tool_exposure_expanded,
            tool_exposure_expansion_reason,
            compaction_boundary,
        }
    }
}

fn runtime_failover_kind_from_key(key: &str) -> Option<RuntimeFailoverErrorKind> {
    Some(match key {
        "billing" => RuntimeFailoverErrorKind::Billing,
        "auth" => RuntimeFailoverErrorKind::Auth,
        "rate_limit" => RuntimeFailoverErrorKind::RateLimit,
        "timeout" => RuntimeFailoverErrorKind::Timeout,
        "network" => RuntimeFailoverErrorKind::Network,
        "deferred_tools" => RuntimeFailoverErrorKind::DeferredTools,
        "policy_blocked" => RuntimeFailoverErrorKind::PolicyBlocked,
        "max_turns" => RuntimeFailoverErrorKind::MaxTurns,
        "loop_detected" => RuntimeFailoverErrorKind::LoopDetected,
        "no_progress" => RuntimeFailoverErrorKind::NoProgress,
        "unknown" => RuntimeFailoverErrorKind::Unknown,
        _ => return None,
    })
}

pub(crate) fn runtime_failover_error_kind_from_error_text(
    error_message: &str,
) -> RuntimeFailoverErrorKind {
    if let Some(reason) = parse_run_stop_reason(error_message) {
        return runtime_failover_error_kind_from_stop_reason_kind(reason.kind);
    }

    let lower = error_message.to_ascii_lowercase();
    if lower.contains("达到最大迭代次数") || lower.contains("最大迭代次数") {
        return RuntimeFailoverErrorKind::MaxTurns;
    }
    if lower.contains("loop_detected") {
        return RuntimeFailoverErrorKind::LoopDetected;
    }
    if lower.contains("no_progress") || lower.contains("没有进展") {
        return RuntimeFailoverErrorKind::NoProgress;
    }
    if lower.contains("insufficient_balance")
        || lower.contains("insufficient balance")
        || lower.contains("balance too low")
        || lower.contains("account balance too low")
        || lower.contains("insufficient_quota")
        || lower.contains("insufficient quota")
        || lower.contains("billing")
        || lower.contains("payment required")
        || lower.contains("credit balance")
        || lower.contains("余额不足")
        || lower.contains("欠费")
    {
        return RuntimeFailoverErrorKind::Billing;
    }
    if lower.contains("api key")
        || lower.contains("unauthorized")
        || lower.contains("invalid_api_key")
        || lower.contains("authentication")
        || lower.contains("permission denied")
        || lower.contains("forbidden")
    {
        return RuntimeFailoverErrorKind::Auth;
    }
    if lower.contains("rate limit")
        || lower.contains("too many requests")
        || lower.contains("429")
        || lower.contains("quota")
    {
        return RuntimeFailoverErrorKind::RateLimit;
    }
    if lower.contains("timeout") || lower.contains("timed out") || lower.contains("deadline") {
        return RuntimeFailoverErrorKind::Timeout;
    }
    if lower.contains("connection")
        || lower.contains("network")
        || lower.contains("dns")
        || lower.contains("connect")
        || lower.contains("socket")
        || lower.contains("error sending request for url")
        || lower.contains("sending request for url")
    {
        return RuntimeFailoverErrorKind::Network;
    }

    match normalize_model_error(error_message).kind {
        crate::model_errors::ModelErrorKind::Billing => RuntimeFailoverErrorKind::Billing,
        crate::model_errors::ModelErrorKind::Auth => RuntimeFailoverErrorKind::Auth,
        crate::model_errors::ModelErrorKind::RateLimit => RuntimeFailoverErrorKind::RateLimit,
        crate::model_errors::ModelErrorKind::Timeout => RuntimeFailoverErrorKind::Timeout,
        crate::model_errors::ModelErrorKind::Network => RuntimeFailoverErrorKind::Network,
        crate::model_errors::ModelErrorKind::Unknown => RuntimeFailoverErrorKind::Unknown,
    }
}

pub(crate) fn runtime_failover_error_kind_from_stop_reason_kind(
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

pub(crate) fn runtime_failover_error_kind_key(kind: RuntimeFailoverErrorKind) -> &'static str {
    match kind {
        RuntimeFailoverErrorKind::Billing => "billing",
        RuntimeFailoverErrorKind::Auth => "auth",
        RuntimeFailoverErrorKind::RateLimit => "rate_limit",
        RuntimeFailoverErrorKind::Timeout => "timeout",
        RuntimeFailoverErrorKind::Network => "network",
        RuntimeFailoverErrorKind::DeferredTools => "deferred_tools",
        RuntimeFailoverErrorKind::PolicyBlocked => "policy_blocked",
        RuntimeFailoverErrorKind::MaxTurns => "max_turns",
        RuntimeFailoverErrorKind::LoopDetected => "loop_detected",
        RuntimeFailoverErrorKind::NoProgress => "no_progress",
        RuntimeFailoverErrorKind::Unknown => "unknown",
    }
}

fn runtime_should_retry_same_candidate(kind: RuntimeFailoverErrorKind) -> bool {
    matches!(
        kind,
        RuntimeFailoverErrorKind::RateLimit
            | RuntimeFailoverErrorKind::Timeout
            | RuntimeFailoverErrorKind::Network
            | RuntimeFailoverErrorKind::DeferredTools
    )
}

fn runtime_retry_budget_for_error(
    kind: RuntimeFailoverErrorKind,
    configured_retry_count: usize,
) -> usize {
    if kind == RuntimeFailoverErrorKind::Network {
        configured_retry_count.max(5)
    } else if kind == RuntimeFailoverErrorKind::DeferredTools {
        configured_retry_count.max(1)
    } else {
        configured_retry_count
    }
}

fn runtime_retry_backoff_ms(kind: RuntimeFailoverErrorKind, attempt_idx: usize) -> u64 {
    let base_ms = match kind {
        RuntimeFailoverErrorKind::RateLimit => 1200u64,
        RuntimeFailoverErrorKind::Timeout => 700u64,
        RuntimeFailoverErrorKind::Network => 400u64,
        RuntimeFailoverErrorKind::DeferredTools => 0u64,
        _ => 0u64,
    };
    if base_ms == 0 {
        return 0;
    }
    let exp = attempt_idx.min(3) as u32;
    base_ms.saturating_mul(1u64 << exp).min(5000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::runtime::kernel::turn_state::TurnCompactionBoundary;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn execute_candidates_retries_first_candidate_then_advances() {
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let attempts_clone = Arc::clone(&attempts);
        let route_candidates = vec![
            (
                String::new(),
                "openai".to_string(),
                "https://a.example".to_string(),
                "model-a".to_string(),
                "key-a".to_string(),
            ),
            (
                String::new(),
                "anthropic".to_string(),
                "https://b.example".to_string(),
                "model-b".to_string(),
                "key-b".to_string(),
            ),
        ];

        let result = RuntimeFailover::execute_candidates(RuntimeFailoverParams {
            route_candidates: &route_candidates,
            per_candidate_retry_count: 1,
            on_same_candidate_retry: None,
            on_error_kind: None,
            attempt_once: Box::new(
                move |_provider_key, api_format, _base_url, model_name, _api_key, attempt_idx| {
                    let attempts = Arc::clone(&attempts_clone);
                    Box::pin(async move {
                        attempts
                            .lock()
                            .expect("attempt log lock")
                            .push(format!("{api_format}:{model_name}:{attempt_idx}"));

                        if api_format == "anthropic" {
                            return CandidateAttemptOutcome {
                                final_messages: Some(vec![json!({
                                    "role": "assistant",
                                    "content": "ok",
                                })]),
                                last_error: None,
                                last_error_kind: None,
                                error_kind: None,
                                last_stop_reason: None,
                                partial_text: "done".to_string(),
                                reasoning_text: String::new(),
                                reasoning_duration_ms: Some(12),
                                tool_exposure_expanded: false,
                                tool_exposure_expansion_reason: None,
                                compaction_boundary: Some(TurnCompactionBoundary {
                                    transcript_path: "temp/transcripts/retry-success.json"
                                        .to_string(),
                                    original_tokens: 6400,
                                    compacted_tokens: 1600,
                                    summary: "summary".to_string(),
                                }),
                            };
                        }

                        CandidateAttemptOutcome {
                            final_messages: None,
                            last_error: Some("request timeout".to_string()),
                            last_error_kind: Some("timeout".to_string()),
                            error_kind: Some(RuntimeFailoverErrorKind::Timeout),
                            last_stop_reason: None,
                            partial_text: "partial".to_string(),
                            reasoning_text: "thinking".to_string(),
                            reasoning_duration_ms: None,
                            tool_exposure_expanded: false,
                            tool_exposure_expansion_reason: None,
                            compaction_boundary: None,
                        }
                    })
                },
            ),
        })
        .await;

        assert_eq!(
            attempts.lock().expect("attempt log lock").as_slice(),
            &[
                "openai:model-a:0".to_string(),
                "openai:model-a:1".to_string(),
                "anthropic:model-b:0".to_string(),
            ]
        );
        assert!(result.final_messages.is_some());
        assert_eq!(result.partial_text, "done");
        assert_eq!(result.reasoning_text, "");
        assert_eq!(result.reasoning_duration_ms, Some(12));
        assert_eq!(
            result.compaction_boundary,
            Some(TurnCompactionBoundary {
                transcript_path: "temp/transcripts/retry-success.json".to_string(),
                original_tokens: 6400,
                compacted_tokens: 1600,
                summary: "summary".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn execute_candidates_guarantees_five_network_retries() {
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let attempts_clone = Arc::clone(&attempts);
        let route_candidates = vec![(
            String::new(),
            "openai".to_string(),
            "https://a.example".to_string(),
            "model-a".to_string(),
            "key-a".to_string(),
        )];

        let result = RuntimeFailover::execute_candidates(RuntimeFailoverParams {
            route_candidates: &route_candidates,
            per_candidate_retry_count: 0,
            on_same_candidate_retry: None,
            on_error_kind: None,
            attempt_once: Box::new(
                move |_provider_key, api_format, _base_url, model_name, _api_key, attempt_idx| {
                    let attempts = Arc::clone(&attempts_clone);
                    Box::pin(async move {
                        attempts
                            .lock()
                            .expect("attempt log lock")
                            .push(format!("{api_format}:{model_name}:{attempt_idx}"));

                        CandidateAttemptOutcome {
                            final_messages: None,
                            last_error: Some("network connection reset".to_string()),
                            last_error_kind: Some("network".to_string()),
                            error_kind: Some(RuntimeFailoverErrorKind::Network),
                            last_stop_reason: None,
                            partial_text: String::new(),
                            reasoning_text: String::new(),
                            reasoning_duration_ms: None,
                            tool_exposure_expanded: false,
                            tool_exposure_expansion_reason: None,
                            compaction_boundary: None,
                        }
                    })
                },
            ),
        })
        .await;

        assert!(result.final_messages.is_none());
        assert_eq!(
            attempts.lock().expect("attempt log lock").as_slice(),
            &[
                "openai:model-a:0".to_string(),
                "openai:model-a:1".to_string(),
                "openai:model-a:2".to_string(),
                "openai:model-a:3".to_string(),
                "openai:model-a:4".to_string(),
                "openai:model-a:5".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn execute_candidates_reports_error_kinds_to_observer() {
        let observed = Arc::new(Mutex::new(Vec::new()));
        let observed_clone = Arc::clone(&observed);
        let route_candidates = vec![(
            String::new(),
            "openai".to_string(),
            "https://a.example".to_string(),
            "model-a".to_string(),
            "key-a".to_string(),
        )];

        let _ = RuntimeFailover::execute_candidates(RuntimeFailoverParams {
            route_candidates: &route_candidates,
            per_candidate_retry_count: 0,
            on_same_candidate_retry: None,
            on_error_kind: Some(Box::new(move |kind| {
                observed_clone.lock().expect("observed lock").push(kind);
            })),
            attempt_once: Box::new(
                move |_provider_key,
                      _api_format,
                      _base_url,
                      _model_name,
                      _api_key,
                      _attempt_idx| {
                    Box::pin(async move {
                        CandidateAttemptOutcome {
                            final_messages: None,
                            last_error: Some("insufficient balance".to_string()),
                            last_error_kind: Some("billing".to_string()),
                            error_kind: Some(RuntimeFailoverErrorKind::Billing),
                            last_stop_reason: None,
                            partial_text: String::new(),
                            reasoning_text: String::new(),
                            reasoning_duration_ms: None,
                            tool_exposure_expanded: false,
                            tool_exposure_expansion_reason: None,
                            compaction_boundary: None,
                        }
                    })
                },
            ),
        })
        .await;

        assert_eq!(
            observed.lock().expect("observed lock").as_slice(),
            &[RuntimeFailoverErrorKind::Billing]
        );
    }

    #[tokio::test]
    async fn execute_candidates_reports_network_retry_progress() {
        let retry_progress = Arc::new(Mutex::new(Vec::new()));
        let retry_progress_clone = Arc::clone(&retry_progress);
        let route_candidates = vec![(
            String::new(),
            "openai".to_string(),
            "https://a.example".to_string(),
            "model-a".to_string(),
            "key-a".to_string(),
        )];

        let _ = RuntimeFailover::execute_candidates(RuntimeFailoverParams {
            route_candidates: &route_candidates,
            per_candidate_retry_count: 0,
            on_same_candidate_retry: Some(Box::new(move |kind, retry_attempt, total_retries| {
                retry_progress_clone
                    .lock()
                    .expect("retry progress lock")
                    .push((kind, retry_attempt, total_retries));
            })),
            on_error_kind: None,
            attempt_once: Box::new(
                move |_provider_key,
                      _api_format,
                      _base_url,
                      _model_name,
                      _api_key,
                      _attempt_idx| {
                    Box::pin(async move {
                        CandidateAttemptOutcome {
                            final_messages: None,
                            last_error: Some("network connection reset".to_string()),
                            last_error_kind: Some("network".to_string()),
                            error_kind: Some(RuntimeFailoverErrorKind::Network),
                            last_stop_reason: None,
                            partial_text: String::new(),
                            reasoning_text: String::new(),
                            reasoning_duration_ms: None,
                            tool_exposure_expanded: false,
                            tool_exposure_expansion_reason: None,
                            compaction_boundary: None,
                        }
                    })
                },
            ),
        })
        .await;

        assert_eq!(
            retry_progress
                .lock()
                .expect("retry progress lock")
                .as_slice(),
            &[
                (RuntimeFailoverErrorKind::Network, 1, 5),
                (RuntimeFailoverErrorKind::Network, 2, 5),
                (RuntimeFailoverErrorKind::Network, 3, 5),
                (RuntimeFailoverErrorKind::Network, 4, 5),
                (RuntimeFailoverErrorKind::Network, 5, 5),
            ]
        );
    }

    #[test]
    fn runtime_failover_error_kind_helpers_stay_in_sync() {
        assert_eq!(
            runtime_failover_error_kind_from_stop_reason_kind(RunStopReasonKind::Timeout),
            RuntimeFailoverErrorKind::Timeout
        );
        assert_eq!(
            runtime_failover_error_kind_from_error_text("insufficient balance on account"),
            RuntimeFailoverErrorKind::Billing
        );
        assert_eq!(
            runtime_failover_error_kind_key(RuntimeFailoverErrorKind::LoopDetected),
            "loop_detected"
        );
    }
}
