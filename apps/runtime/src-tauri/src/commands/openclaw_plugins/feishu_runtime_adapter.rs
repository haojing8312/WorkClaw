use super::runtime_service::{
    handle_feishu_runtime_pairing_request_event,
    handle_openclaw_plugin_feishu_runtime_command_error_event,
    handle_openclaw_plugin_feishu_runtime_send_result_event,
    merge_feishu_runtime_reply_lifecycle_event, merge_feishu_runtime_status_event,
    parse_feishu_runtime_dispatch_event_with_pool, trim_recent_runtime_logs,
};
use super::{now_rfc3339, OpenClawPluginFeishuRuntimeState, OpenClawPluginFeishuRuntimeStatus};
use crate::commands::feishu_gateway::dispatch_feishu_inbound_to_workclaw_with_pool_and_app;
use crate::commands::im_host::{handle_runtime_stdout_line_with_adapter, ImRuntimeStdoutAdapter};
use sqlx::SqlitePool;
use tauri::AppHandle;

fn handle_feishu_runtime_dispatch_request_event(
    pool: &SqlitePool,
    status: &mut OpenClawPluginFeishuRuntimeStatus,
    app: Option<&AppHandle>,
    value: &serde_json::Value,
) {
    match tauri::async_runtime::block_on(parse_feishu_runtime_dispatch_event_with_pool(pool, value))
    {
        Ok(inbound) => {
            if let Some(app_handle) = app.as_ref() {
                match tauri::async_runtime::block_on(
                    dispatch_feishu_inbound_to_workclaw_with_pool_and_app(
                        pool, app_handle, &inbound, None,
                    ),
                ) {
                    Ok(result) => {
                        status.last_error = None;
                        status.recent_logs.push(format!(
                            "[dispatch] feishu: accepted={} deduped={} thread={}",
                            result.accepted, result.deduped, inbound.thread_id
                        ));
                    }
                    Err(error) => {
                        status.last_error = Some(format!(
                            "failed to bridge official feishu dispatch: {error}"
                        ));
                        status.recent_logs.push(format!(
                            "[error] runtime: failed to bridge official feishu dispatch: {error}"
                        ));
                    }
                }
            } else {
                status.recent_logs.push(
                    "[warn] runtime: dispatch_request ignored because no app handle was available"
                        .to_string(),
                );
            }
        }
        Err(error) => {
            status.last_error = Some(format!("invalid official feishu dispatch event: {error}"));
            status.recent_logs.push(format!(
                "[error] runtime: invalid official feishu dispatch event: {error}"
            ));
        }
    }
    trim_recent_runtime_logs(status);
}

struct FeishuRuntimeStdoutAdapter<'a> {
    pool: &'a SqlitePool,
    state: &'a OpenClawPluginFeishuRuntimeState,
    app: Option<&'a AppHandle>,
}

impl FeishuRuntimeStdoutAdapter<'_> {
    fn record_dropped_runtime_result(&self, kind: &str, value: &serde_json::Value) {
        let request_id = value
            .get("requestId")
            .and_then(|entry| entry.as_str())
            .unwrap_or("unknown");
        if let Ok(mut guard) = self.state.0.lock() {
            guard.status.recent_logs.push(format!(
                "[warn] runtime: dropped {kind} requestId={request_id}"
            ));
            trim_recent_runtime_logs(&mut guard.status);
            guard.status.last_event_at = Some(now_rfc3339());
        }
    }
}

impl ImRuntimeStdoutAdapter for FeishuRuntimeStdoutAdapter<'_> {
    fn handle_send_result(&mut self, value: &serde_json::Value) -> bool {
        let handled = handle_openclaw_plugin_feishu_runtime_send_result_event(self.state, value);
        if !handled {
            self.record_dropped_runtime_result("send_result", value);
        }
        true
    }

    fn handle_command_error(&mut self, value: &serde_json::Value) -> bool {
        let handled = handle_openclaw_plugin_feishu_runtime_command_error_event(self.state, value);
        if !handled {
            self.record_dropped_runtime_result("command_error", value);
        }
        true
    }

    fn handle_reply_lifecycle(&mut self, value: &serde_json::Value) -> bool {
        if let Ok(mut guard) = self.state.0.lock() {
            if let Err(error) = merge_feishu_runtime_reply_lifecycle_event(&mut guard.status, value)
            {
                guard.status.last_error = Some(error.clone());
                guard
                    .status
                    .recent_logs
                    .push(format!("[error] runtime: {error}"));
                trim_recent_runtime_logs(&mut guard.status);
            }
        }
        true
    }

    fn handle_pairing_request(&mut self, value: &serde_json::Value) {
        if let Ok(mut guard) = self.state.0.lock() {
            handle_feishu_runtime_pairing_request_event(self.pool, &mut guard.status, value);
        }
    }

    fn handle_dispatch_request(&mut self, value: &serde_json::Value) {
        if let Ok(mut guard) = self.state.0.lock() {
            handle_feishu_runtime_dispatch_request_event(
                self.pool,
                &mut guard.status,
                self.app,
                value,
            );
        }
    }

    fn handle_other(&mut self, value: &serde_json::Value) {
        if let Ok(mut guard) = self.state.0.lock() {
            merge_feishu_runtime_status_event(&mut guard.status, value);
        }
    }
}

pub(crate) fn handle_openclaw_plugin_feishu_runtime_stdout_line(
    pool: &SqlitePool,
    state: &OpenClawPluginFeishuRuntimeState,
    app: Option<&AppHandle>,
    trimmed: &str,
) {
    let mut adapter = FeishuRuntimeStdoutAdapter { pool, state, app };
    let _ = handle_runtime_stdout_line_with_adapter(&mut adapter, trimmed);
}
