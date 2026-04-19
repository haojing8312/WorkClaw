use super::runtime_events::runtime_event_name;
use super::runtime_observability::trim_recent_entries;

pub(crate) fn merge_runtime_status_event(
    value: &serde_json::Value,
    last_event_at: &mut Option<String>,
    last_error: &mut Option<String>,
    recent_logs: &mut Vec<String>,
    account_id: &mut String,
    port: &mut Option<u16>,
    now_rfc3339: String,
    recent_log_limit: usize,
) {
    let Some(event) = runtime_event_name(value) else {
        return;
    };

    match event {
        "status" => {
            *last_event_at = Some(now_rfc3339);
            if let Some(patch) = value.get("patch").and_then(|entry| entry.as_object()) {
                if let Some(account) = patch.get("accountId").and_then(|entry| entry.as_str()) {
                    *account_id = account.to_string();
                }
                if let Some(port_value) = patch.get("port").and_then(|entry| entry.as_u64()) {
                    *port = Some(port_value as u16);
                }
                if let Some(last_error_value) =
                    patch.get("lastError").and_then(|entry| entry.as_str())
                {
                    let normalized = last_error_value.trim();
                    *last_error = if normalized.is_empty() {
                        None
                    } else {
                        Some(normalized.to_string())
                    };
                } else if !patch.is_empty() {
                    *last_error = None;
                }
            }
        }
        "log" => {
            *last_event_at = Some(now_rfc3339);
            let level = value
                .get("level")
                .and_then(|entry| entry.as_str())
                .unwrap_or("info")
                .trim()
                .to_string();
            let scope = value
                .get("scope")
                .and_then(|entry| entry.as_str())
                .unwrap_or("runtime")
                .trim()
                .to_string();
            let message = value
                .get("message")
                .and_then(|entry| entry.as_str())
                .unwrap_or("")
                .trim()
                .to_string();

            if !message.is_empty() {
                let entry = format!("[{level}] {scope}: {message}");
                recent_logs.push(entry.clone());
                trim_recent_entries(recent_logs, recent_log_limit);
                if level == "error" {
                    *last_error = Some(entry);
                }
            }
        }
        "fatal" => {
            *last_event_at = Some(now_rfc3339);
            if let Some(error_value) = value.get("error").and_then(|entry| entry.as_str()) {
                let normalized = error_value.trim();
                if !normalized.is_empty() {
                    *last_error = Some(normalized.to_string());
                    recent_logs.push(format!("[fatal] runtime: {normalized}"));
                    trim_recent_entries(recent_logs, recent_log_limit);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::merge_runtime_status_event;

    #[test]
    fn merges_status_patch_fields() {
        let mut last_event_at = None;
        let mut last_error = None;
        let mut recent_logs = vec![];
        let mut account_id = "default".to_string();
        let mut port = None;

        merge_runtime_status_event(
            &serde_json::json!({
                "event": "status",
                "patch": {
                    "accountId": "workspace",
                    "port": 3100,
                    "lastError": ""
                }
            }),
            &mut last_event_at,
            &mut last_error,
            &mut recent_logs,
            &mut account_id,
            &mut port,
            "2026-04-14T00:00:00Z".to_string(),
            40,
        );

        assert_eq!(account_id, "workspace");
        assert_eq!(port, Some(3100));
        assert!(last_error.is_none());
        assert_eq!(last_event_at.as_deref(), Some("2026-04-14T00:00:00Z"));
    }
}
