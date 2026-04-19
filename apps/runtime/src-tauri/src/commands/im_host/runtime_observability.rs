use super::contract::ImReplyLifecycleEvent;

pub(crate) fn trim_recent_entries<T>(entries: &mut Vec<T>, limit: usize) {
    if entries.len() > limit {
        let overflow = entries.len() - limit;
        entries.drain(0..overflow);
    }
}

pub(crate) fn merge_runtime_reply_lifecycle_event(
    value: &serde_json::Value,
    last_event_at: &mut Option<String>,
    recent_logs: &mut Vec<String>,
    recent_reply_lifecycle: &mut Vec<ImReplyLifecycleEvent>,
    now_rfc3339: String,
    recent_log_limit: usize,
    recent_reply_lifecycle_limit: usize,
) -> Result<(), String> {
    let event: ImReplyLifecycleEvent = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid reply_lifecycle event: {error}"))?;

    *last_event_at = Some(now_rfc3339);
    recent_logs.push(format!(
        "[reply] {} phase={} thread={}",
        event.logical_reply_id,
        format!("{:?}", event.phase),
        event.thread_id.as_deref().unwrap_or("")
    ));
    trim_recent_entries(recent_logs, recent_log_limit);
    recent_reply_lifecycle.push(event);
    trim_recent_entries(recent_reply_lifecycle, recent_reply_lifecycle_limit);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{merge_runtime_reply_lifecycle_event, trim_recent_entries};
    use crate::commands::im_host::ImReplyLifecyclePhase;

    #[test]
    fn trim_recent_entries_keeps_tail() {
        let mut entries = vec![1, 2, 3, 4];
        trim_recent_entries(&mut entries, 2);
        assert_eq!(entries, vec![3, 4]);
    }

    #[test]
    fn merge_reply_lifecycle_appends_event_and_log() {
        let mut last_event_at = None;
        let mut recent_logs = vec![];
        let mut recent_reply_lifecycle = vec![];
        let value = serde_json::json!({
            "logicalReplyId": "reply-1",
            "phase": "processing_started",
            "channel": "feishu",
            "accountId": "default",
            "threadId": "oc_chat_1",
            "messageId": "om_1"
        });

        merge_runtime_reply_lifecycle_event(
            &value,
            &mut last_event_at,
            &mut recent_logs,
            &mut recent_reply_lifecycle,
            "2026-04-13T08:00:00Z".to_string(),
            40,
            20,
        )
        .expect("merge lifecycle event");

        assert_eq!(last_event_at.as_deref(), Some("2026-04-13T08:00:00Z"));
        assert_eq!(recent_logs.len(), 1);
        assert!(recent_logs[0].contains("reply-1"));
        assert_eq!(recent_reply_lifecycle.len(), 1);
        assert_eq!(
            recent_reply_lifecycle[0].phase,
            ImReplyLifecyclePhase::ProcessingStarted
        );
    }
}
