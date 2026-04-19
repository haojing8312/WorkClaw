#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImSidecarChannelHealthSnapshot {
    pub instance_id: String,
    pub state: String,
    pub running: bool,
    pub started_at: Option<String>,
    pub last_error: Option<String>,
    pub reconnect_attempts: i64,
    pub queue_depth: i64,
}

pub(crate) fn build_sidecar_channel_instance_id(adapter_name: &str, connector_id: &str) -> String {
    format!("{}:{}", adapter_name.trim(), connector_id.trim())
}

pub(crate) fn is_sidecar_channel_running(state: &str) -> bool {
    matches!(state.trim(), "running" | "starting" | "degraded")
}

pub(crate) fn parse_sidecar_channel_health(
    value: &serde_json::Value,
    default_instance_id: &str,
) -> ImSidecarChannelHealthSnapshot {
    let state = value
        .get("state")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .unwrap_or("unknown")
        .to_string();

    ImSidecarChannelHealthSnapshot {
        instance_id: value
            .get("instance_id")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .unwrap_or(default_instance_id)
            .to_string(),
        running: is_sidecar_channel_running(&state),
        state,
        started_at: value
            .get("last_ok_at")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        last_error: value
            .get("last_error")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_string),
        reconnect_attempts: value
            .get("reconnect_attempts")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0),
        queue_depth: value
            .get("queue_depth")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0),
    }
}

pub(crate) fn build_sidecar_text_message_request(
    instance_id: &str,
    conversation_id: &str,
    text: &str,
) -> serde_json::Value {
    serde_json::json!({
        "instance_id": instance_id,
        "request": {
            "thread_id": conversation_id,
            "reply_target": conversation_id,
            "text": text,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_sidecar_channel_instance_id, build_sidecar_text_message_request,
        is_sidecar_channel_running, parse_sidecar_channel_health,
    };

    #[test]
    fn builds_sidecar_instance_id() {
        assert_eq!(
            build_sidecar_channel_instance_id("wecom", "main"),
            "wecom:main"
        );
    }

    #[test]
    fn running_state_accepts_degraded() {
        assert!(is_sidecar_channel_running("degraded"));
        assert!(!is_sidecar_channel_running("stopped"));
    }

    #[test]
    fn parses_sidecar_health_with_fallback_instance_id() {
        let snapshot = parse_sidecar_channel_health(
            &serde_json::json!({
                "state": "running",
                "last_ok_at": "2026-04-14T00:00:00Z",
                "reconnect_attempts": 2,
                "queue_depth": 5
            }),
            "wecom:main",
        );

        assert_eq!(snapshot.instance_id, "wecom:main");
        assert!(snapshot.running);
        assert_eq!(snapshot.started_at.as_deref(), Some("2026-04-14T00:00:00Z"));
        assert_eq!(snapshot.reconnect_attempts, 2);
        assert_eq!(snapshot.queue_depth, 5);
    }

    #[test]
    fn builds_sidecar_send_message_request() {
        let payload = build_sidecar_text_message_request("wecom:main", "room-1", "hello");
        assert_eq!(payload["instance_id"], "wecom:main");
        assert_eq!(payload["request"]["thread_id"], "room-1");
        assert_eq!(payload["request"]["reply_target"], "room-1");
        assert_eq!(payload["request"]["text"], "hello");
    }
}
