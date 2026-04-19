use super::runtime_observability::trim_recent_entries;
use super::runtime_registry::{
    fail_pending_runtime_requests, resolve_pending_runtime_request, PendingRuntimeRequestMap,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeCommandErrorDelivery {
    pub delivered: bool,
    pub failed_count: usize,
}

pub(crate) fn deliver_runtime_result<T>(
    pending_requests: &mut PendingRuntimeRequestMap<T>,
    request_id: &str,
    result: T,
) -> bool {
    resolve_pending_runtime_request(pending_requests, request_id, Ok(result))
}

pub(crate) fn register_pending_runtime_request_with_status<T>(
    pending_requests: &mut PendingRuntimeRequestMap<T>,
    request_id: &str,
    last_event_at: &mut Option<String>,
    recent_logs: &mut Vec<String>,
    queued_log: String,
    now_rfc3339: String,
    recent_log_limit: usize,
) -> Result<std::sync::mpsc::Receiver<Result<T, String>>, String> {
    let receiver =
        super::runtime_registry::register_pending_runtime_request(pending_requests, request_id)?;
    *last_event_at = Some(now_rfc3339);
    recent_logs.push(queued_log);
    trim_recent_entries(recent_logs, recent_log_limit);
    Ok(receiver)
}

pub(crate) fn deliver_runtime_result_with_status<T>(
    pending_requests: &mut PendingRuntimeRequestMap<T>,
    request_id: &str,
    result: T,
    last_event_at: &mut Option<String>,
    recent_logs: &mut Vec<String>,
    delivered_log: String,
    dropped_log: Option<String>,
    now_rfc3339: String,
    recent_log_limit: usize,
) -> bool {
    *last_event_at = Some(now_rfc3339);
    recent_logs.push(delivered_log);
    let delivered = deliver_runtime_result(pending_requests, request_id, result);
    if !delivered {
        if let Some(entry) = dropped_log {
            recent_logs.push(entry);
        }
    }
    trim_recent_entries(recent_logs, recent_log_limit);
    delivered
}

pub(crate) fn deliver_runtime_command_error<T>(
    pending_requests: &mut PendingRuntimeRequestMap<T>,
    request_id: Option<&str>,
    error_message: &str,
    runtime_name: &str,
    empty_error_fallback: &str,
) -> RuntimeCommandErrorDelivery {
    let normalized_error_message = error_message.trim();
    let final_error = if normalized_error_message.is_empty() {
        empty_error_fallback.to_string()
    } else {
        format!("{runtime_name} command error: {normalized_error_message}")
    };

    let normalized_request_id = request_id.map(str::trim).filter(|value| !value.is_empty());

    if let Some(request_id) = normalized_request_id {
        return RuntimeCommandErrorDelivery {
            delivered: resolve_pending_runtime_request(
                pending_requests,
                request_id,
                Err(final_error),
            ),
            failed_count: 0,
        };
    }

    RuntimeCommandErrorDelivery {
        delivered: false,
        failed_count: fail_pending_runtime_requests(pending_requests, final_error),
    }
}

pub(crate) fn fail_pending_runtime_requests_with_status<T>(
    pending_requests: &mut PendingRuntimeRequestMap<T>,
    error: String,
    last_event_at: &mut Option<String>,
    recent_logs: &mut Vec<String>,
    error_log: String,
    now_rfc3339: String,
    recent_log_limit: usize,
) -> usize {
    *last_event_at = Some(now_rfc3339);
    recent_logs.push(error_log);
    trim_recent_entries(recent_logs, recent_log_limit);
    fail_pending_runtime_requests(pending_requests, error)
}

pub(crate) fn drop_pending_runtime_request_with_status<T>(
    pending_requests: &mut PendingRuntimeRequestMap<T>,
    request_id: &str,
    last_event_at: &mut Option<String>,
    recent_logs: &mut Vec<String>,
    dropped_log: String,
    now_rfc3339: String,
    recent_log_limit: usize,
) -> bool {
    let removed = pending_requests.remove(request_id).is_some();
    *last_event_at = Some(now_rfc3339);
    recent_logs.push(dropped_log);
    trim_recent_entries(recent_logs, recent_log_limit);
    removed
}

#[cfg(test)]
mod tests {
    use super::{
        deliver_runtime_command_error, deliver_runtime_result, deliver_runtime_result_with_status,
        drop_pending_runtime_request_with_status, fail_pending_runtime_requests_with_status,
        register_pending_runtime_request_with_status, PendingRuntimeRequestMap,
    };
    use crate::commands::im_host::runtime_registry::register_pending_runtime_request;

    #[test]
    fn deliver_runtime_result_resolves_pending_waiter() {
        let mut pending = PendingRuntimeRequestMap::<String>::new();
        let receiver =
            register_pending_runtime_request(&mut pending, "request-1").expect("register");

        let delivered = deliver_runtime_result(&mut pending, "request-1", "ok".to_string());

        assert!(delivered);
        assert_eq!(
            receiver.recv().expect("receive result").expect("success"),
            "ok"
        );
    }

    #[test]
    fn deliver_runtime_command_error_fails_one_request_when_request_id_exists() {
        let mut pending = PendingRuntimeRequestMap::<String>::new();
        let receiver =
            register_pending_runtime_request(&mut pending, "request-1").expect("register");

        let outcome = deliver_runtime_command_error(
            &mut pending,
            Some("request-1"),
            "bad target",
            "test runtime",
            "fallback",
        );

        assert!(outcome.delivered);
        assert_eq!(outcome.failed_count, 0);
        assert!(receiver
            .recv()
            .expect("receive result")
            .expect_err("should fail")
            .contains("bad target"));
    }

    #[test]
    fn deliver_runtime_command_error_fails_all_when_request_id_missing() {
        let mut pending = PendingRuntimeRequestMap::<String>::new();
        let _receiver_a =
            register_pending_runtime_request(&mut pending, "request-a").expect("register a");
        let _receiver_b =
            register_pending_runtime_request(&mut pending, "request-b").expect("register b");

        let outcome = deliver_runtime_command_error(
            &mut pending,
            None,
            "",
            "test runtime",
            "fallback command error",
        );

        assert!(!outcome.delivered);
        assert_eq!(outcome.failed_count, 2);
        assert!(pending.is_empty());
    }

    #[test]
    fn register_and_deliver_runtime_result_with_status_updates_logs() {
        let mut pending = PendingRuntimeRequestMap::<String>::new();
        let mut last_event_at = None;
        let mut recent_logs = vec![];
        let receiver = register_pending_runtime_request_with_status(
            &mut pending,
            "request-1",
            &mut last_event_at,
            &mut recent_logs,
            "[outbound] queued requestId=request-1".to_string(),
            "2026-04-16T00:00:00Z".to_string(),
            40,
        )
        .expect("register");

        let delivered = deliver_runtime_result_with_status(
            &mut pending,
            "request-1",
            "ok".to_string(),
            &mut last_event_at,
            &mut recent_logs,
            "[outbound] send_result requestId=request-1".to_string(),
            Some("[warn] dropped requestId=request-1".to_string()),
            "2026-04-16T00:00:01Z".to_string(),
            40,
        );

        assert!(delivered);
        assert_eq!(last_event_at.as_deref(), Some("2026-04-16T00:00:01Z"));
        assert_eq!(
            receiver.recv().expect("receive result").expect("success"),
            "ok"
        );
        assert_eq!(recent_logs.len(), 2);
    }

    #[test]
    fn fail_and_drop_runtime_requests_with_status_update_logs() {
        let mut pending = PendingRuntimeRequestMap::<String>::new();
        let mut last_event_at = None;
        let mut recent_logs = vec![];
        let _receiver =
            register_pending_runtime_request(&mut pending, "request-1").expect("register");

        let removed = drop_pending_runtime_request_with_status(
            &mut pending,
            "request-1",
            &mut last_event_at,
            &mut recent_logs,
            "[warn] timed out requestId=request-1".to_string(),
            "2026-04-16T00:00:02Z".to_string(),
            40,
        );
        assert!(removed);
        assert!(pending.is_empty());

        let _receiver =
            register_pending_runtime_request(&mut pending, "request-2").expect("register");
        let failed = fail_pending_runtime_requests_with_status(
            &mut pending,
            "runtime disconnected".to_string(),
            &mut last_event_at,
            &mut recent_logs,
            "[outbound] runtime disconnected".to_string(),
            "2026-04-16T00:00:03Z".to_string(),
            40,
        );
        assert_eq!(failed, 1);
        assert_eq!(last_event_at.as_deref(), Some("2026-04-16T00:00:03Z"));
        assert_eq!(recent_logs.len(), 2);
    }
}
