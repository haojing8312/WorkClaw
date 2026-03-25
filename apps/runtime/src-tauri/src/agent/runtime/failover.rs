use crate::agent::run_guard::RunStopReason;
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
}

pub(crate) struct RuntimeFailoverParams<'a> {
    pub route_candidates: &'a [(String, String, String, String)],
    pub per_candidate_retry_count: usize,
    pub attempt_once: Box<
        dyn FnMut(
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

        for (candidate_api_format, candidate_base_url, candidate_model_name, candidate_api_key) in
            params.route_candidates
        {
            let mut attempt_idx = 0usize;
            loop {
                let attempt = (params.attempt_once)(
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
                    return RuntimeFailoverOutcome {
                        final_messages: final_messages_opt,
                        last_error,
                        last_error_kind,
                        last_stop_reason,
                        partial_text: streamed_text,
                        reasoning_text: streamed_reasoning,
                        reasoning_duration_ms: attempt.reasoning_duration_ms,
                    };
                }

                streamed_text = attempt.partial_text;
                streamed_reasoning = attempt.reasoning_text;
                last_error = attempt.last_error.or(last_error);
                last_error_kind = attempt.last_error_kind.or(last_error_kind);
                last_stop_reason = attempt.last_stop_reason.or(last_stop_reason);

                let current_kind = attempt
                    .error_kind
                    .or_else(|| {
                        last_error_kind
                            .as_deref()
                            .and_then(runtime_failover_kind_from_key)
                    })
                    .unwrap_or(RuntimeFailoverErrorKind::Unknown);
                let retry_budget = runtime_retry_budget_for_error(
                    current_kind,
                    params.per_candidate_retry_count,
                );
                if runtime_should_retry_same_candidate(current_kind) && attempt_idx < retry_budget
                {
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
        "policy_blocked" => RuntimeFailoverErrorKind::PolicyBlocked,
        "max_turns" => RuntimeFailoverErrorKind::MaxTurns,
        "loop_detected" => RuntimeFailoverErrorKind::LoopDetected,
        "no_progress" => RuntimeFailoverErrorKind::NoProgress,
        "unknown" => RuntimeFailoverErrorKind::Unknown,
        _ => return None,
    })
}

fn runtime_should_retry_same_candidate(kind: RuntimeFailoverErrorKind) -> bool {
    matches!(
        kind,
        RuntimeFailoverErrorKind::RateLimit
            | RuntimeFailoverErrorKind::Timeout
            | RuntimeFailoverErrorKind::Network
    )
}

fn runtime_retry_budget_for_error(
    kind: RuntimeFailoverErrorKind,
    configured_retry_count: usize,
) -> usize {
    if kind == RuntimeFailoverErrorKind::Network {
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
        _ => 0u64,
    };
    if base_ms == 0 {
        return 0;
    }
    let exp = attempt_idx.min(3) as u32;
    base_ms.saturating_mul(1u64 << exp).min(5000)
}
