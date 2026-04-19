use crate::commands::im_host::{
    dispatch_im_inbound_to_workclaw_with_pool_and_app, handle_runtime_stdout_line_with_adapter,
    parse_normalized_im_event_value, ImRuntimeStdoutAdapter,
};
#[cfg(test)]
use crate::commands::im_host::parse_sidecar_channel_health;
use sqlx::SqlitePool;
use tauri::AppHandle;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct WecomRuntimeAdapterStatus {
    pub running: bool,
    pub instance_id: Option<String>,
    pub started_at: Option<String>,
    pub last_event_at: Option<String>,
    pub last_error: Option<String>,
    pub reconnect_attempts: i64,
    pub queue_depth: i64,
    pub recent_logs: Vec<String>,
}

fn trim_recent_logs(status: &mut WecomRuntimeAdapterStatus) {
    if status.recent_logs.len() > 40 {
        let overflow = status.recent_logs.len() - 40;
        status.recent_logs.drain(0..overflow);
    }
}

pub(crate) fn merge_wecom_runtime_status(
    current: &mut WecomRuntimeAdapterStatus,
    patch: &WecomRuntimeAdapterStatus,
) {
    current.running = patch.running;
    if patch.instance_id.is_some() {
        current.instance_id = patch.instance_id.clone();
    }
    if patch.started_at.is_some() {
        current.started_at = patch.started_at.clone();
    }
    if patch.last_event_at.is_some() {
        current.last_event_at = patch.last_event_at.clone();
    }
    if patch.last_error.is_some() || current.last_error.is_none() {
        current.last_error = patch.last_error.clone();
    }
    current.reconnect_attempts = patch.reconnect_attempts;
    current.queue_depth = patch.queue_depth;
    if !patch.recent_logs.is_empty() {
        current.recent_logs.extend(patch.recent_logs.clone());
        trim_recent_logs(current);
    }
}

pub(crate) fn parse_wecom_runtime_status_value(
    value: &serde_json::Value,
) -> Result<WecomRuntimeAdapterStatus, String> {
    serde_json::from_value(value.clone())
        .map_err(|error| format!("parse wecom runtime status failed: {error}"))
}

#[cfg(test)]
pub(crate) fn merge_wecom_runtime_status_snapshot(
    status: &mut WecomRuntimeAdapterStatus,
    value: &serde_json::Value,
    now_rfc3339: &dyn Fn() -> String,
) {
    let snapshot = parse_sidecar_channel_health(value, "wecom:wecom-main");
    status.running = snapshot.running;
    status.instance_id = Some(snapshot.instance_id.clone());
    status.started_at = snapshot.started_at;
    status.last_error = snapshot.last_error.clone();
    status.reconnect_attempts = snapshot.reconnect_attempts;
    status.queue_depth = snapshot.queue_depth;
    status.last_event_at = Some(now_rfc3339());
    status.recent_logs.push(format!(
        "[wecom] sidecar state={} queue_depth={} reconnect_attempts={}",
        snapshot.state, snapshot.queue_depth, snapshot.reconnect_attempts
    ));
    trim_recent_logs(status);
}

pub(crate) fn build_wecom_runtime_status_value(
    status: &WecomRuntimeAdapterStatus,
) -> serde_json::Value {
    serde_json::json!({
        "running": status.running,
        "instance_id": status.instance_id,
        "started_at": status.started_at,
        "last_event_at": status.last_event_at,
        "last_error": status.last_error,
        "reconnect_attempts": status.reconnect_attempts,
        "queue_depth": status.queue_depth,
        "recent_logs": status.recent_logs,
    })
}

struct WecomRuntimeStdoutAdapter<'a> {
    pool: Option<&'a SqlitePool>,
    app: Option<&'a AppHandle>,
    status: &'a mut WecomRuntimeAdapterStatus,
    now_rfc3339: &'a dyn Fn() -> String,
}

impl WecomRuntimeStdoutAdapter<'_> {
    fn record_event(&mut self, message: String) {
        self.status.last_event_at = Some((self.now_rfc3339)());
        self.status.recent_logs.push(message);
        trim_recent_logs(self.status);
    }
}

impl ImRuntimeStdoutAdapter for WecomRuntimeStdoutAdapter<'_> {
    fn handle_send_result(&mut self, _value: &serde_json::Value) -> bool {
        self.record_event("[wecom] send_result".to_string());
        true
    }

    fn handle_command_error(&mut self, value: &serde_json::Value) -> bool {
        let error = value
            .get("error")
            .and_then(|entry| entry.as_str())
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .unwrap_or("unknown command error")
            .to_string();
        self.status.last_error = Some(error.clone());
        self.record_event(format!("[wecom] command_error: {error}"));
        true
    }

    fn handle_reply_lifecycle(&mut self, value: &serde_json::Value) -> bool {
        let phase = value
            .get("phase")
            .and_then(|entry| entry.as_str())
            .unwrap_or("unknown");
        self.record_event(format!("[wecom] reply_lifecycle phase={phase}"));
        true
    }

    fn handle_pairing_request(&mut self, _value: &serde_json::Value) {
        self.record_event("[wecom] pairing_request".to_string());
    }

    fn handle_dispatch_request(&mut self, value: &serde_json::Value) {
        match parse_normalized_im_event_value(value) {
            Ok(event) => {
                if let (Some(pool), Some(app)) = (self.pool, self.app) {
                    match tauri::async_runtime::block_on(
                        dispatch_im_inbound_to_workclaw_with_pool_and_app(pool, app, &event),
                    ) {
                        Ok(result) => {
                            self.status.last_error = None;
                            self.record_event(format!(
                                "[wecom] dispatch_request accepted={} deduped={} thread={}",
                                result.accepted, result.deduped, event.thread_id
                            ));
                        }
                        Err(error) => {
                            self.status.last_error = Some(error.clone());
                            self.record_event(format!(
                                "[wecom] dispatch_request bridge failed: {error}"
                            ));
                        }
                    }
                } else {
                    self.record_event(format!(
                        "[wecom] dispatch_request thread={} (no app bridge)",
                        event.thread_id
                    ));
                }
            }
            Err(error) => {
                self.status.last_error = Some(error.clone());
                self.record_event(format!("[wecom] invalid dispatch_request: {error}"));
            }
        }
    }

    fn handle_other(&mut self, value: &serde_json::Value) {
        let event = value
            .get("event")
            .and_then(|entry| entry.as_str())
            .unwrap_or("unknown");
        self.record_event(format!("[wecom] other event={event}"));
    }
}

pub(crate) fn handle_openclaw_plugin_wecom_runtime_stdout_line(
    status: &mut WecomRuntimeAdapterStatus,
    trimmed: &str,
    now_rfc3339: &dyn Fn() -> String,
) {
    let mut adapter = WecomRuntimeStdoutAdapter {
        pool: None,
        app: None,
        status,
        now_rfc3339,
    };
    let _ = handle_runtime_stdout_line_with_adapter(&mut adapter, trimmed);
}

pub(crate) fn handle_openclaw_plugin_wecom_runtime_stdout_line_with_bridge(
    pool: &SqlitePool,
    app: &AppHandle,
    status: &mut WecomRuntimeAdapterStatus,
    trimmed: &str,
    now_rfc3339: &dyn Fn() -> String,
) {
    let mut adapter = WecomRuntimeStdoutAdapter {
        pool: Some(pool),
        app: Some(app),
        status,
        now_rfc3339,
    };
    let _ = handle_runtime_stdout_line_with_adapter(&mut adapter, trimmed);
}

#[cfg(test)]
mod tests {
    use super::{
        handle_openclaw_plugin_wecom_runtime_stdout_line, merge_wecom_runtime_status_snapshot,
        WecomRuntimeAdapterStatus,
    };

    #[test]
    fn wecom_adapter_handles_dispatch_request_route() {
        let mut status = WecomRuntimeAdapterStatus::default();
        handle_openclaw_plugin_wecom_runtime_stdout_line(
            &mut status,
            r#"{"event":"dispatch_request","threadId":"wecom-room-1"}"#,
            &|| "2026-04-14T00:00:00Z".to_string(),
        );

        assert_eq!(
            status.last_event_at.as_deref(),
            Some("2026-04-14T00:00:00Z")
        );
        assert!(status
            .recent_logs
            .iter()
            .any(|entry| entry.contains("dispatch_request")));
    }

    #[test]
    fn wecom_adapter_records_command_errors() {
        let mut status = WecomRuntimeAdapterStatus::default();
        handle_openclaw_plugin_wecom_runtime_stdout_line(
            &mut status,
            r#"{"event":"command_error","error":"bad target"}"#,
            &|| "2026-04-14T00:00:00Z".to_string(),
        );

        assert_eq!(status.last_error.as_deref(), Some("bad target"));
        assert!(status
            .recent_logs
            .iter()
            .any(|entry| entry.contains("command_error")));
    }

    #[test]
    fn wecom_adapter_merges_sidecar_health_snapshot() {
        let mut status = WecomRuntimeAdapterStatus::default();
        merge_wecom_runtime_status_snapshot(
            &mut status,
            &serde_json::json!({
                "instance_id": "wecom:wecom-main",
                "state": "degraded",
                "last_ok_at": "2026-04-14T00:00:00Z",
                "last_error": "timeout",
                "reconnect_attempts": 3,
                "queue_depth": 8
            }),
            &|| "2026-04-14T00:00:01Z".to_string(),
        );

        assert!(status.running);
        assert_eq!(status.instance_id.as_deref(), Some("wecom:wecom-main"));
        assert_eq!(status.started_at.as_deref(), Some("2026-04-14T00:00:00Z"));
        assert_eq!(status.last_error.as_deref(), Some("timeout"));
        assert_eq!(status.reconnect_attempts, 3);
        assert_eq!(status.queue_depth, 8);
    }
}
