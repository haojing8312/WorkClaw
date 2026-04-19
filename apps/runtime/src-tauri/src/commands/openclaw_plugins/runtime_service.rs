use super::{
    app_setting_string_or_default, build_feishu_openclaw_config_with_pool,
    ensure_supported_feishu_host_node_version, get_feishu_setup_progress_with_pool,
    get_openclaw_plugin_install_by_id_with_pool, handle_openclaw_plugin_feishu_runtime_stdout_line,
    normalize_required, now_rfc3339, resolve_plugin_host_dir, resolve_plugin_host_fixture_root,
    resolve_plugin_host_run_feishu_script, should_auto_restore_feishu_runtime,
    OpenClawPluginFeishuLatestReplyCompletion, OpenClawPluginFeishuReplyCompletionState,
    OpenClawPluginFeishuLifecycleEventRequest, OpenClawPluginFeishuOutboundCommandErrorEvent,
    OpenClawPluginFeishuOutboundSendRequest, OpenClawPluginFeishuOutboundSendResult,
    OpenClawPluginFeishuProcessingStopRequest, OpenClawPluginFeishuRuntimeState,
    OpenClawPluginFeishuRuntimeStatus,
};
use crate::commands::feishu_gateway::upsert_feishu_pairing_request_with_pool;
use crate::commands::im_host::{
    build_runtime_lifecycle_event_command_payload, build_runtime_processing_stop_command_payload,
    build_runtime_text_command_payload, deliver_runtime_command_error,
    deliver_runtime_result_with_status, drop_pending_runtime_request_with_status,
    ensure_runtime_stdin_for_commands, fail_pending_runtime_requests_with_status,
    merge_runtime_reply_lifecycle_event, merge_runtime_status_event, parse_runtime_event,
    register_pending_runtime_request_with_status, resolve_dispatch_thread_target,
    trim_recent_entries, write_runtime_command_json, ImReplyDeliveryState, ReplyDeliveryTrace,
    ImRuntimeLifecycleEventCommandPayload, ImRuntimeProcessingStopCommandPayload,
    ImRuntimeTextCommandPayload,
};
use crate::im::types::{ImEvent, ImEventType};
use crate::windows_process::hide_console_window;
use sqlx::SqlitePool;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::AppHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FeishuRuntimeOutboundFailureKind {
    Timeout,
    Disconnected,
    CommandError,
    WriteFailed,
    Other,
}

pub type FeishuRuntimeProcessingStopHook = dyn Fn(
        &OpenClawPluginFeishuProcessingStopRequest,
    ) -> Result<(), String>
    + Send
    + Sync;

pub type FeishuRuntimeLifecycleEventHook = dyn Fn(
        &OpenClawPluginFeishuLifecycleEventRequest,
    ) -> Result<(), String>
    + Send
    + Sync;

fn feishu_runtime_processing_stop_hook_slot(
) -> &'static Mutex<Option<Arc<FeishuRuntimeProcessingStopHook>>> {
    static SLOT: OnceLock<Mutex<Option<Arc<FeishuRuntimeProcessingStopHook>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn feishu_runtime_lifecycle_event_hook_slot(
) -> &'static Mutex<Option<Arc<FeishuRuntimeLifecycleEventHook>>> {
    static SLOT: OnceLock<Mutex<Option<Arc<FeishuRuntimeLifecycleEventHook>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
#[doc(hidden)]
pub fn set_feishu_runtime_processing_stop_hook_for_tests(
    hook: Option<Arc<FeishuRuntimeProcessingStopHook>>,
) {
    if let Ok(mut guard) = feishu_runtime_processing_stop_hook_slot().lock() {
        *guard = hook;
    }
}

#[cfg(test)]
#[doc(hidden)]
pub fn set_feishu_runtime_lifecycle_event_hook_for_tests(
    hook: Option<Arc<FeishuRuntimeLifecycleEventHook>>,
) {
    if let Ok(mut guard) = feishu_runtime_lifecycle_event_hook_slot().lock() {
        *guard = hook;
    }
}

pub(crate) fn infer_feishu_runtime_outbound_failure_kind(
    error: &str,
) -> FeishuRuntimeOutboundFailureKind {
    let normalized = error.trim().to_ascii_lowercase();
    if normalized.contains("timed out waiting for outbound send_result") {
        return FeishuRuntimeOutboundFailureKind::Timeout;
    }
    if normalized.contains("disconnected before outbound send_result") {
        return FeishuRuntimeOutboundFailureKind::Disconnected;
    }
    if normalized.contains("command error") {
        return FeishuRuntimeOutboundFailureKind::CommandError;
    }
    if normalized.contains("failed to write outbound command") {
        return FeishuRuntimeOutboundFailureKind::WriteFailed;
    }
    FeishuRuntimeOutboundFailureKind::Other
}

pub(crate) fn classify_feishu_runtime_outbound_failure(
    delivered_chunk_count: usize,
    _kind: FeishuRuntimeOutboundFailureKind,
) -> ImReplyDeliveryState {
    if delivered_chunk_count > 0 {
        ImReplyDeliveryState::FailedPartial
    } else {
        ImReplyDeliveryState::Failed
    }
}

pub(crate) fn merge_feishu_runtime_status_event(
    status: &mut OpenClawPluginFeishuRuntimeStatus,
    value: &serde_json::Value,
) {
    merge_runtime_status_event(
        value,
        &mut status.last_event_at,
        &mut status.last_error,
        &mut status.recent_logs,
        &mut status.account_id,
        &mut status.port,
        now_rfc3339(),
        40,
    );
}

pub(super) fn trim_recent_runtime_logs(status: &mut OpenClawPluginFeishuRuntimeStatus) {
    trim_recent_entries(&mut status.recent_logs, 40);
}

fn format_reply_trace_log_entry(trace: &ReplyDeliveryTrace) -> String {
    let final_state = trace
        .final_state
        .as_ref()
        .map(|value| format!("{value:?}"))
        .unwrap_or_else(|| "Unknown".to_string());
    let failed_chunks = if trace.failed_chunk_indexes.is_empty() {
        "-".to_string()
    } else {
        trace
            .failed_chunk_indexes
            .iter()
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join(",")
    };
    format!(
        "[reply_trace] id={} channel={} thread={} delivered={}/{} failed={} state={}",
        trace.logical_reply_id,
        trace.channel,
        trace.target_thread_id,
        trace.delivered_chunk_count,
        trace.planned_chunk_count,
        failed_chunks,
        final_state
    )
}

pub(crate) fn record_feishu_runtime_reply_trace(
    state: &OpenClawPluginFeishuRuntimeState,
    trace: &ReplyDeliveryTrace,
) {
    if let Ok(mut guard) = state.0.lock() {
        guard.status.last_event_at = Some(now_rfc3339());
        guard
            .status
            .recent_logs
            .push(format_reply_trace_log_entry(trace));
        trim_recent_runtime_logs(&mut guard.status);
    }
}

pub(crate) fn record_feishu_runtime_reply_trace_error(
    state: &OpenClawPluginFeishuRuntimeState,
    error: &str,
) {
    let normalized_error = error.trim();
    let (message, maybe_trace_json) = match normalized_error.split_once("\ntrace=") {
        Some((message, trace_json)) => (message.trim(), Some(trace_json.trim())),
        None => (normalized_error, None),
    };

    if let Some(trace_json) = maybe_trace_json {
        if let Ok(trace) = serde_json::from_str::<ReplyDeliveryTrace>(trace_json) {
            if let Ok(mut guard) = state.0.lock() {
                guard.status.last_event_at = Some(now_rfc3339());
                guard.status.last_error = Some(message.to_string());
                guard
                    .status
                    .recent_logs
                    .push(format!("[reply_trace] error={message}"));
                guard
                    .status
                    .recent_logs
                    .push(format_reply_trace_log_entry(&trace));
                trim_recent_runtime_logs(&mut guard.status);
            }
            return;
        }
    }

    if let Ok(mut guard) = state.0.lock() {
        guard.status.last_event_at = Some(now_rfc3339());
        guard.status.last_error = Some(message.to_string());
        guard
            .status
            .recent_logs
            .push(format!("[reply_trace] error={message}"));
        trim_recent_runtime_logs(&mut guard.status);
    }
}

pub(super) fn merge_feishu_runtime_reply_lifecycle_event(
    status: &mut OpenClawPluginFeishuRuntimeStatus,
    value: &serde_json::Value,
) -> Result<(), String> {
    let result = merge_runtime_reply_lifecycle_event(
        value,
        &mut status.last_event_at,
        &mut status.recent_logs,
        &mut status.recent_reply_lifecycle,
        now_rfc3339(),
        40,
        20,
    );
    if result.is_ok() {
        status.latest_reply_completion =
            project_latest_reply_completion(&status.recent_reply_lifecycle, status.last_event_at.clone());
    }
    result
}

fn map_phase_to_projected_state(
    phase: &crate::commands::im_host::ImReplyLifecyclePhase,
) -> Option<OpenClawPluginFeishuReplyCompletionState> {
    use crate::commands::im_host::ImReplyLifecyclePhase as Phase;

    match phase {
        Phase::DispatchIdle => {
            Some(OpenClawPluginFeishuReplyCompletionState::Completed)
        }
        Phase::Failed => Some(OpenClawPluginFeishuReplyCompletionState::Failed),
        Phase::Stopped => Some(OpenClawPluginFeishuReplyCompletionState::Stopped),
        Phase::AskUserRequested => Some(OpenClawPluginFeishuReplyCompletionState::AwaitingUser),
        Phase::ApprovalRequested => {
            Some(OpenClawPluginFeishuReplyCompletionState::AwaitingApproval)
        }
        Phase::InterruptRequested => Some(OpenClawPluginFeishuReplyCompletionState::Interrupted),
        Phase::WaitForIdle => Some(OpenClawPluginFeishuReplyCompletionState::WaitingForIdle),
        Phase::IdleReached => Some(OpenClawPluginFeishuReplyCompletionState::IdleReached),
        Phase::ReplyStarted
        | Phase::ProcessingStarted
        | Phase::AskUserAnswered
        | Phase::ApprovalResolved
        | Phase::Resumed
        | Phase::FullyComplete
        | Phase::ToolChunkQueued
        | Phase::BlockChunkQueued
        | Phase::FinalChunkQueued => Some(OpenClawPluginFeishuReplyCompletionState::Running),
        Phase::ProcessingStopped => None,
    }
}

fn project_latest_reply_completion(
    events: &[crate::commands::im_host::ImReplyLifecycleEvent],
    updated_at: Option<String>,
) -> Option<OpenClawPluginFeishuLatestReplyCompletion> {
    let latest = events.last()?;
    let logical_reply_id = latest.logical_reply_id.clone();

    for event in events.iter().rev() {
        if event.logical_reply_id != logical_reply_id {
            continue;
        }
        if let Some(state) = map_phase_to_projected_state(&event.phase) {
            return Some(OpenClawPluginFeishuLatestReplyCompletion {
                logical_reply_id: logical_reply_id.clone(),
                phase: event.phase.clone(),
                state,
                updated_at,
            });
        }
    }

    Some(OpenClawPluginFeishuLatestReplyCompletion {
        logical_reply_id,
        phase: latest.phase.clone(),
        state: OpenClawPluginFeishuReplyCompletionState::Running,
        updated_at,
    })
}

fn build_feishu_runtime_outbound_send_command_payload(
    request: &OpenClawPluginFeishuOutboundSendRequest,
) -> Result<ImRuntimeTextCommandPayload, String> {
    let request_id = normalize_required(&request.request_id, "request_id")?;
    let account_id = app_setting_string_or_default(Some(request.account_id.clone()), "default");
    let target = normalize_required(&request.target, "target")?;
    let text = request.text.trim().to_string();
    let mode = app_setting_string_or_default(Some(request.mode.clone()), "text");

    Ok(build_runtime_text_command_payload(
        request_id,
        "send_message",
        account_id,
        target,
        request
            .thread_id
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        text,
        mode,
    ))
}

fn build_feishu_runtime_processing_stop_command_payload(
    request: &OpenClawPluginFeishuProcessingStopRequest,
) -> Result<ImRuntimeProcessingStopCommandPayload, String> {
    Ok(build_runtime_processing_stop_command_payload(
        normalize_required(&request.request_id, "request_id")?,
        "processing_stop",
        app_setting_string_or_default(Some(request.account_id.clone()), "default"),
        normalize_required(&request.message_id, "message_id")?,
        request
            .logical_reply_id
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        request
            .final_state
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    ))
}

fn build_feishu_runtime_lifecycle_event_command_payload(
    request: &OpenClawPluginFeishuLifecycleEventRequest,
) -> Result<ImRuntimeLifecycleEventCommandPayload, String> {
    Ok(build_runtime_lifecycle_event_command_payload(
        normalize_required(&request.request_id, "request_id")?,
        "lifecycle_event",
        app_setting_string_or_default(Some(request.account_id.clone()), "default"),
        request.phase.clone(),
        request
            .logical_reply_id
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        request
            .thread_id
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        request
            .message_id
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    ))
}

fn write_feishu_runtime_command_json_in_state(
    state: &OpenClawPluginFeishuRuntimeState,
    payload_json: &str,
) -> Result<(), String> {
    let stdin = {
        let guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock feishu runtime state".to_string())?;
        ensure_runtime_stdin_for_commands(
            guard.status.running,
            guard.stdin.clone(),
            "official feishu runtime",
        )?
    };

    write_runtime_command_json(&stdin, payload_json)
        .map_err(|error| error.replace("runtime stdin", "feishu runtime stdin"))
}

pub(crate) fn register_pending_feishu_runtime_outbound_send_waiter(
    state: &OpenClawPluginFeishuRuntimeState,
    request_id: &str,
) -> Result<std::sync::mpsc::Receiver<Result<OpenClawPluginFeishuOutboundSendResult, String>>, String>
{
    let mut guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock feishu runtime state".to_string())?;
    let store = &mut *guard;
    let pending_requests = &mut store.pending_outbound_send_results;
    let status = &mut store.status;
    register_pending_runtime_request_with_status(
        pending_requests,
        request_id,
        &mut status.last_event_at,
        &mut status.recent_logs,
        format!("[outbound] queued send_message requestId={request_id}"),
        now_rfc3339(),
        40,
    )
}

fn deliver_pending_feishu_runtime_outbound_send_result(
    state: &OpenClawPluginFeishuRuntimeState,
    result: OpenClawPluginFeishuOutboundSendResult,
) -> bool {
    let request_id = result.request_id.clone();
    {
        let mut guard = match state.0.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        let store = &mut *guard;
        let pending_requests = &mut store.pending_outbound_send_results;
        let status = &mut store.status;
        deliver_runtime_result_with_status(
            pending_requests,
            &request_id,
            result,
            &mut status.last_event_at,
            &mut status.recent_logs,
            format!("[outbound] send_result requestId={request_id}"),
            Some(format!(
                "[warn] runtime: unhandled outbound send_result requestId={request_id}"
            )),
            now_rfc3339(),
            40,
        )
    }
}

fn deliver_pending_feishu_runtime_outbound_command_error(
    state: &OpenClawPluginFeishuRuntimeState,
    error_event: OpenClawPluginFeishuOutboundCommandErrorEvent,
) -> bool {
    let error_message = error_event.error.trim().to_string();
    let normalized_request_id = error_event
        .request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let outcome = {
        let mut guard = match state.0.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        guard.status.last_event_at = Some(now_rfc3339());
        guard.status.last_error = Some(error_message.clone());
        if let Some(request_id) = normalized_request_id {
            guard.status.recent_logs.push(format!(
                "[outbound] command_error requestId={request_id}: {error_message}"
            ));
        } else {
            guard
                .status
                .recent_logs
                .push("[outbound] command_error without requestId".to_string());
        }
        trim_recent_runtime_logs(&mut guard.status);
        deliver_runtime_command_error(
            &mut guard.pending_outbound_send_results,
            normalized_request_id,
            &error_message,
            "official feishu runtime",
            "official feishu runtime reported an outbound command error",
        )
    };

    outcome.delivered || outcome.failed_count > 0
}

fn fail_pending_feishu_runtime_outbound_send_waiters(
    state: &OpenClawPluginFeishuRuntimeState,
    error: String,
) -> usize {
    {
        let mut guard = match state.0.lock() {
            Ok(guard) => guard,
            Err(_) => return 0,
        };
        let store = &mut *guard;
        let pending_requests = &mut store.pending_outbound_send_results;
        let status = &mut store.status;
        fail_pending_runtime_requests_with_status(
            pending_requests,
            error.clone(),
            &mut status.last_event_at,
            &mut status.recent_logs,
            format!("[outbound] {error}"),
            now_rfc3339(),
            40,
        )
    }
}

fn parse_openclaw_plugin_feishu_runtime_send_result_event(
    value: &serde_json::Value,
) -> Result<OpenClawPluginFeishuOutboundSendResult, String> {
    parse_runtime_event(value, "send_result")
        .map_err(|error| error.replace("unexpected runtime event", "unexpected outbound event"))
}

fn parse_openclaw_plugin_feishu_runtime_command_error_event(
    value: &serde_json::Value,
) -> Result<OpenClawPluginFeishuOutboundCommandErrorEvent, String> {
    parse_runtime_event(value, "command_error")
        .map_err(|error| error.replace("unexpected runtime event", "unexpected outbound event"))
}

pub(crate) fn handle_openclaw_plugin_feishu_runtime_send_result_event(
    state: &OpenClawPluginFeishuRuntimeState,
    value: &serde_json::Value,
) -> bool {
    match parse_openclaw_plugin_feishu_runtime_send_result_event(value) {
        Ok(result) => deliver_pending_feishu_runtime_outbound_send_result(state, result),
        Err(error) => {
            if let Ok(mut guard) = state.0.lock() {
                guard.status.last_error = Some(error.clone());
                guard
                    .status
                    .recent_logs
                    .push(format!("[error] runtime: {error}"));
                trim_recent_runtime_logs(&mut guard.status);
                guard.status.last_event_at = Some(now_rfc3339());
            }
            false
        }
    }
}

pub(crate) fn handle_openclaw_plugin_feishu_runtime_command_error_event(
    state: &OpenClawPluginFeishuRuntimeState,
    value: &serde_json::Value,
) -> bool {
    match parse_openclaw_plugin_feishu_runtime_command_error_event(value) {
        Ok(error_event) => {
            deliver_pending_feishu_runtime_outbound_command_error(state, error_event)
        }
        Err(error) => {
            if let Ok(mut guard) = state.0.lock() {
                guard.status.last_error = Some(error.clone());
                guard
                    .status
                    .recent_logs
                    .push(format!("[error] runtime: {error}"));
                trim_recent_runtime_logs(&mut guard.status);
                guard.status.last_event_at = Some(now_rfc3339());
            }
            false
        }
    }
}

pub(crate) fn send_openclaw_plugin_feishu_runtime_outbound_message_in_state(
    state: &OpenClawPluginFeishuRuntimeState,
    request: OpenClawPluginFeishuOutboundSendRequest,
) -> Result<OpenClawPluginFeishuOutboundSendResult, String> {
    let payload = build_feishu_runtime_outbound_send_command_payload(&request)?;
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| format!("failed to serialize outbound send command: {error}"))?;

    let receiver =
        register_pending_feishu_runtime_outbound_send_waiter(state, &payload.request_id)?;

    if let Err(error) = write_feishu_runtime_command_json_in_state(state, &payload_json) {
        let _ = fail_pending_feishu_runtime_outbound_send_waiters(
            state,
            format!("failed to write outbound command: {error}"),
        );
        return Err(format!("failed to write outbound command: {error}"));
    }

    match receiver.recv_timeout(Duration::from_secs(30)) {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(error)) => Err(error),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            if let Ok(mut guard) = state.0.lock() {
                let store = &mut *guard;
                let pending_requests = &mut store.pending_outbound_send_results;
                let status = &mut store.status;
                let _ = drop_pending_runtime_request_with_status(
                    pending_requests,
                    &payload.request_id,
                    &mut status.last_event_at,
                    &mut status.recent_logs,
                    format!(
                        "[warn] runtime: outbound send timed out requestId={}",
                        payload.request_id
                    ),
                    now_rfc3339(),
                    40,
                );
            }
            Err(format!(
                "timed out waiting for outbound send_result for requestId {}",
                payload.request_id
            ))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(format!(
            "official feishu runtime disconnected before outbound send_result for requestId {}",
            payload.request_id
        )),
    }
}

pub(crate) fn send_openclaw_plugin_feishu_runtime_processing_stop_in_state(
    state: &OpenClawPluginFeishuRuntimeState,
    request: OpenClawPluginFeishuProcessingStopRequest,
) -> Result<(), String> {
    if let Ok(guard) = feishu_runtime_processing_stop_hook_slot().lock() {
        if let Some(hook) = guard.as_ref() {
            return hook(&request);
        }
    }
    let payload = build_feishu_runtime_processing_stop_command_payload(&request)?;
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| format!("failed to serialize processing_stop command: {error}"))?;
    write_feishu_runtime_command_json_in_state(state, &payload_json)
}

pub(crate) fn send_openclaw_plugin_feishu_runtime_lifecycle_event_in_state(
    state: &OpenClawPluginFeishuRuntimeState,
    request: OpenClawPluginFeishuLifecycleEventRequest,
) -> Result<(), String> {
    if let Ok(guard) = feishu_runtime_lifecycle_event_hook_slot().lock() {
        if let Some(hook) = guard.as_ref() {
            return hook(&request);
        }
    }
    let payload = build_feishu_runtime_lifecycle_event_command_payload(&request)?;
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| format!("failed to serialize lifecycle_event command: {error}"))?;
    write_feishu_runtime_command_json_in_state(state, &payload_json)
}

pub(crate) fn current_feishu_runtime_status(
    state: &OpenClawPluginFeishuRuntimeState,
) -> OpenClawPluginFeishuRuntimeStatus {
    let mut status = state
        .0
        .lock()
        .expect("lock feishu runtime state")
        .status
        .clone();
    status.latest_reply_completion =
        project_latest_reply_completion(&status.recent_reply_lifecycle, status.last_event_at.clone());
    status
}

pub(crate) fn handle_feishu_runtime_pairing_request_event(
    pool: &SqlitePool,
    status: &mut OpenClawPluginFeishuRuntimeStatus,
    value: &serde_json::Value,
) {
    let sender_id = value
        .get("senderId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty());
    let account_id = value
        .get("accountId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .unwrap_or("default");
    let code = value
        .get("code")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .unwrap_or("PAIRING");

    let Some(sender_id) = sender_id else {
        return;
    };

    match tauri::async_runtime::block_on(upsert_feishu_pairing_request_with_pool(
        pool,
        account_id,
        sender_id,
        "",
        Some(code),
    )) {
        Ok((record, created)) => {
            status.last_event_at = Some(now_rfc3339());
            let entry = format!(
                "[pairing] feishu: {} request {} for {} code={}",
                if created { "created" } else { "reused" },
                record.id,
                record.sender_id,
                record.code
            );
            status.recent_logs.push(entry);
            trim_recent_runtime_logs(status);
            if record.code.trim().is_empty() || record.code == "PAIRING" {
                status.last_error = Some(format!(
                    "official runtime emitted placeholder pairing code for {} (raw={code})",
                    record.sender_id
                ));
            } else {
                status.last_error = None;
            }
        }
        Err(error) => {
            status.last_event_at = Some(now_rfc3339());
            status.last_error = Some(format!("failed to persist feishu pairing request: {error}"));
            status.recent_logs.push(format!(
                "[error] runtime: failed to persist feishu pairing request: {error}"
            ));
            trim_recent_runtime_logs(status);
        }
    }
}

async fn resolve_feishu_runtime_dispatch_thread_id_with_pool(
    pool: &SqlitePool,
    thread_id: &str,
    account_id: Option<&str>,
    sender_id: Option<&str>,
    chat_type: Option<&str>,
) -> Result<String, String> {
    let normalized_thread_id = thread_id.trim();
    if normalized_thread_id.is_empty() {
        return Err("dispatch_request missing threadId".to_string());
    }

    let normalized_chat_type = chat_type.map(str::trim).filter(|value| !value.is_empty());
    let needs_direct_mapping = matches!(normalized_chat_type, Some("direct") | None)
        && normalized_thread_id.starts_with("ou_");
    if !needs_direct_mapping {
        return Ok(resolve_dispatch_thread_target(
            normalized_thread_id,
            None,
            normalized_chat_type,
            "ou_",
            None,
        )?
        .thread_id);
    }

    let normalized_account_id = account_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("default");
    let normalized_sender_id = sender_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(normalized_thread_id);

    let row = sqlx::query_as::<_, (String,)>(
        "SELECT chat_id
         FROM feishu_pairing_requests
         WHERE channel = 'feishu'
           AND account_id = ?
           AND sender_id = ?
           AND chat_id <> ''
         ORDER BY
           CASE status WHEN 'approved' THEN 0 WHEN 'pending' THEN 1 ELSE 2 END,
           updated_at DESC,
           created_at DESC
         LIMIT 1",
    )
    .bind(normalized_account_id)
    .bind(normalized_sender_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("failed to resolve feishu chat_id from pairing requests: {e}"))?;

    Ok(resolve_dispatch_thread_target(
        normalized_thread_id,
        None,
        normalized_chat_type,
        "ou_",
        row.as_ref().map(|(chat_id,)| chat_id.as_str()),
    )?
    .thread_id)
}

pub(crate) async fn parse_feishu_runtime_dispatch_event_with_pool(
    pool: &SqlitePool,
    value: &serde_json::Value,
) -> Result<ImEvent, String> {
    let raw_thread_id = value
        .get("threadId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| "dispatch_request missing threadId".to_string())?;
    let text = value
        .get("text")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let account_id = value
        .get("accountId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let sender_id = value
        .get("senderId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let explicit_chat_id = value
        .get("chatId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let chat_type = value
        .get("chatType")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let message_id = value
        .get("messageId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let role_id = value
        .get("roleId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let mapped_thread_id = resolve_feishu_runtime_dispatch_thread_id_with_pool(
        pool,
        raw_thread_id,
        account_id.as_deref(),
        sender_id.as_deref(),
        chat_type.as_deref(),
    )
    .await?;
    let thread_id = resolve_dispatch_thread_target(
        raw_thread_id,
        explicit_chat_id.as_deref(),
        chat_type.as_deref(),
        "ou_",
        Some(mapped_thread_id.as_str()),
    )?
    .thread_id;

    Ok(ImEvent {
        channel: "feishu".to_string(),
        event_type: if role_id.is_some() {
            ImEventType::MentionRole
        } else {
            ImEventType::MessageCreated
        },
        thread_id,
        event_id: message_id.clone(),
        message_id,
        text,
        role_id,
        account_id: account_id.clone(),
        tenant_id: account_id,
        sender_id,
        chat_type,
    })
}

pub(crate) async fn start_openclaw_plugin_feishu_runtime_with_pool(
    pool: &SqlitePool,
    state: &OpenClawPluginFeishuRuntimeState,
    plugin_id: &str,
    account_id: Option<&str>,
    app: Option<AppHandle>,
) -> Result<OpenClawPluginFeishuRuntimeStatus, String> {
    let _node_version = ensure_supported_feishu_host_node_version()?;
    let normalized_plugin_id = normalize_required(plugin_id, "plugin_id")?;
    let normalized_account_id = normalize_required(account_id.unwrap_or("default"), "account_id")
        .unwrap_or_else(|_| "default".to_string());
    let current_pid = {
        let guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock feishu runtime state".to_string())?;
        guard.status.pid
    };
    let should_stop_existing = {
        let guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock feishu runtime state".to_string())?;
        guard.status.running
            && (guard.status.plugin_id != normalized_plugin_id
                || guard.status.account_id != normalized_account_id)
    };

    if should_stop_existing {
        let _ = stop_openclaw_plugin_feishu_runtime_in_state(state);
    }

    {
        let guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock feishu runtime state".to_string())?;
        if guard.status.running
            && guard.status.plugin_id == normalized_plugin_id
            && guard.status.account_id == normalized_account_id
        {
            return Ok(guard.status.clone());
        }
    }

    let install = get_openclaw_plugin_install_by_id_with_pool(pool, &normalized_plugin_id).await?;
    let stale_pids = cleanup_stale_feishu_runtime_processes(
        &install.install_path,
        &normalized_account_id,
        current_pid,
    )?;
    if !stale_pids.is_empty() {
        if let Ok(mut guard) = state.0.lock() {
            guard.status.last_event_at = Some(now_rfc3339());
            guard.status.recent_logs.push(format!(
                "[runtime] cleaned up stale feishu host pids: {}",
                stale_pids
                    .iter()
                    .map(|pid| pid.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            trim_recent_runtime_logs(&mut guard.status);
        }
    }
    let config_json = build_feishu_openclaw_config_with_pool(pool).await?;
    let script_path = resolve_plugin_host_run_feishu_script();
    if !script_path.exists() {
        return Err(format!(
            "plugin host feishu runtime script is missing: {}",
            script_path.display()
        ));
    }

    let plugin_host_dir = resolve_plugin_host_dir();
    let mut command = Command::new("node");
    command
        .current_dir(&plugin_host_dir)
        .arg(script_path)
        .arg("--plugin-root")
        .arg(&install.install_path)
        .arg("--fixture-name")
        .arg(format!("{}-runtime", install.plugin_id))
        .arg("--account-id")
        .arg(&normalized_account_id)
        .arg("--config-json")
        .arg(config_json.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(app) = app.as_ref() {
        command
            .arg("--fixture-root")
            .arg(resolve_plugin_host_fixture_root(app)?);
    }
    super::apply_command_search_path(&mut command, &[]);
    hide_console_window(&mut command);

    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to launch official feishu runtime: {e}"))?;
    let pid = child.id();
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "failed to capture official feishu runtime stdin".to_string())?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let child_slot = std::sync::Arc::new(std::sync::Mutex::new(Some(child)));
    let stdin_slot = std::sync::Arc::new(std::sync::Mutex::new(stdin));

    {
        let mut guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock feishu runtime state".to_string())?;
        guard.process = Some(child_slot.clone());
        guard.stdin = Some(stdin_slot.clone());
        guard.status = OpenClawPluginFeishuRuntimeStatus {
            plugin_id: normalized_plugin_id.clone(),
            account_id: normalized_account_id.clone(),
            running: true,
            started_at: Some(now_rfc3339()),
            last_stop_at: None,
            last_event_at: None,
            last_error: None,
            pid: Some(pid),
            port: None,
            recent_logs: Vec::new(),
            recent_reply_lifecycle: Vec::new(),
            latest_reply_completion: None,
        };
    }

    if let Some(stdout) = stdout {
        let state_clone = state.clone();
        let pool_clone = pool.clone();
        let app_clone = app.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                handle_openclaw_plugin_feishu_runtime_stdout_line(
                    &pool_clone,
                    &state_clone,
                    app_clone.as_ref(),
                    trimmed,
                );
            }
        });
    }

    if let Some(stderr) = stderr {
        let state_clone = state.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                eprintln!("[openclaw-feishu-runtime] {}", trimmed);
                if let Ok(mut guard) = state_clone.0.lock() {
                    guard.status.last_error = Some(trimmed.to_string());
                    guard
                        .status
                        .recent_logs
                        .push(format!("[stderr] runtime: {}", trimmed));
                    trim_recent_runtime_logs(&mut guard.status);
                    guard.status.last_event_at = Some(now_rfc3339());
                }
            }
        });
    }

    {
        let state_clone = state.clone();
        let child_slot_clone = child_slot.clone();
        thread::spawn(move || loop {
            let exit_status = {
                let mut child_guard = match child_slot_clone.lock() {
                    Ok(guard) => guard,
                    Err(_) => break,
                };
                if let Some(child) = child_guard.as_mut() {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            let success = status.success();
                            let code = status.code();
                            *child_guard = None;
                            Some((success, code, None::<String>))
                        }
                        Ok(None) => None,
                        Err(error) => {
                            *child_guard = None;
                            Some((false, Some(-1), Some(error.to_string())))
                        }
                    }
                } else {
                    break;
                }
            };

            match exit_status {
                Some((success, code, command_error)) => {
                    let failure_message = if let Some(error) = command_error.as_ref() {
                        format!("official feishu runtime wait failed: {error}")
                    } else {
                        match code {
                            Some(value) if value >= 0 => {
                                format!("official feishu runtime exited with code {value}")
                            }
                            _ => "official feishu runtime exited unexpectedly".to_string(),
                        }
                    };
                    let should_fail_waiters = if let Ok(mut guard) = state_clone.0.lock() {
                        guard.process = None;
                        guard.stdin = None;
                        let should_fail_waiters = !guard.pending_outbound_send_results.is_empty();
                        guard.status.running = false;
                        guard.status.pid = None;
                        guard.status.last_stop_at = Some(now_rfc3339());
                        if !success
                            && guard
                                .status
                                .last_error
                                .as_deref()
                                .unwrap_or("")
                                .trim()
                                .is_empty()
                        {
                            guard.status.last_error = Some(failure_message.clone());
                        }
                        should_fail_waiters
                    } else {
                        false
                    };
                    if should_fail_waiters {
                        let _ = fail_pending_feishu_runtime_outbound_send_waiters(
                            &state_clone,
                            "official feishu runtime exited before outbound result was delivered"
                                .to_string(),
                        );
                    }
                    break;
                }
                None => thread::sleep(Duration::from_millis(250)),
            }
        });
    }

    Ok(current_feishu_runtime_status(state))
}

#[cfg(test)]
mod tests {
    use super::{
        classify_feishu_runtime_outbound_failure, infer_feishu_runtime_outbound_failure_kind,
        merge_feishu_runtime_reply_lifecycle_event, project_latest_reply_completion,
        FeishuRuntimeOutboundFailureKind,
    };
    use crate::commands::openclaw_plugins::im_host_contract::{
        ImReplyDeliveryState, ImReplyLifecyclePhase,
    };
    use crate::commands::openclaw_plugins::{
        OpenClawPluginFeishuReplyCompletionState, OpenClawPluginFeishuRuntimeStatus,
    };

    #[test]
    fn infer_timeout_failure_kind() {
        let kind = infer_feishu_runtime_outbound_failure_kind(
            "timed out waiting for outbound send_result for requestId abc",
        );
        assert_eq!(kind, FeishuRuntimeOutboundFailureKind::Timeout);
    }

    #[test]
    fn classify_partial_failure_when_chunks_already_delivered() {
        let state = classify_feishu_runtime_outbound_failure(
            1,
            FeishuRuntimeOutboundFailureKind::Disconnected,
        );
        assert_eq!(state, ImReplyDeliveryState::FailedPartial);
    }

    #[test]
    fn merge_reply_lifecycle_event_tracks_recent_events() {
        let mut status = OpenClawPluginFeishuRuntimeStatus::default();
        let value = serde_json::json!({
            "event": "reply_lifecycle",
            "logicalReplyId": "reply-1",
            "phase": "processing_started",
            "channel": "feishu",
            "accountId": "default",
            "threadId": "oc_chat_1",
            "messageId": "om_1"
        });

        merge_feishu_runtime_reply_lifecycle_event(&mut status, &value)
            .expect("merge lifecycle event");

        assert_eq!(status.recent_reply_lifecycle.len(), 1);
        assert_eq!(
            status.recent_reply_lifecycle[0].phase,
            ImReplyLifecyclePhase::ProcessingStarted
        );
        assert_eq!(
            status.latest_reply_completion.as_ref().map(|entry| &entry.state),
            Some(&OpenClawPluginFeishuReplyCompletionState::Running)
        );
    }

    #[test]
    fn reply_completion_projection_keeps_fully_complete_running_until_dispatch_idle() {
        let events = vec![
            crate::commands::im_host::ImReplyLifecycleEvent {
                logical_reply_id: "reply-1".to_string(),
                phase: ImReplyLifecyclePhase::FullyComplete,
                channel: "feishu".to_string(),
                account_id: Some("default".to_string()),
                thread_id: Some("oc_chat_1".to_string()),
                chat_id: None,
                message_id: Some("om_1".to_string()),
                queued_counts: None,
            },
            crate::commands::im_host::ImReplyLifecycleEvent {
                logical_reply_id: "reply-1".to_string(),
                phase: ImReplyLifecyclePhase::ProcessingStopped,
                channel: "feishu".to_string(),
                account_id: Some("default".to_string()),
                thread_id: Some("oc_chat_1".to_string()),
                chat_id: None,
                message_id: Some("om_1".to_string()),
                queued_counts: None,
            },
        ];

        let projection =
            project_latest_reply_completion(&events, Some("2026-04-19T00:00:00Z".to_string()))
                .expect("projection");

        assert_eq!(projection.phase, ImReplyLifecyclePhase::FullyComplete);
        assert_eq!(
            projection.state,
            OpenClawPluginFeishuReplyCompletionState::Running
        );
    }

    #[test]
    fn reply_completion_projection_marks_dispatch_idle_as_completed() {
        let events = vec![
            crate::commands::im_host::ImReplyLifecycleEvent {
                logical_reply_id: "reply-1".to_string(),
                phase: ImReplyLifecyclePhase::FullyComplete,
                channel: "feishu".to_string(),
                account_id: Some("default".to_string()),
                thread_id: Some("oc_chat_1".to_string()),
                chat_id: None,
                message_id: Some("om_1".to_string()),
                queued_counts: None,
            },
            crate::commands::im_host::ImReplyLifecycleEvent {
                logical_reply_id: "reply-1".to_string(),
                phase: ImReplyLifecyclePhase::DispatchIdle,
                channel: "feishu".to_string(),
                account_id: Some("default".to_string()),
                thread_id: Some("oc_chat_1".to_string()),
                chat_id: None,
                message_id: Some("om_1".to_string()),
                queued_counts: None,
            },
        ];

        let projection =
            project_latest_reply_completion(&events, Some("2026-04-19T00:00:01Z".to_string()))
                .expect("projection");

        assert_eq!(projection.phase, ImReplyLifecyclePhase::DispatchIdle);
        assert_eq!(
            projection.state,
            OpenClawPluginFeishuReplyCompletionState::Completed
        );
    }

    #[test]
    fn reply_completion_projection_marks_ask_user_as_awaiting_user() {
        let events = vec![crate::commands::im_host::ImReplyLifecycleEvent {
            logical_reply_id: "reply-ask".to_string(),
            phase: ImReplyLifecyclePhase::AskUserRequested,
            channel: "feishu".to_string(),
            account_id: Some("default".to_string()),
            thread_id: Some("oc_chat_ask".to_string()),
            chat_id: None,
            message_id: Some("om_ask".to_string()),
            queued_counts: None,
        }];

        let projection =
            project_latest_reply_completion(&events, Some("2026-04-19T00:01:00Z".to_string()))
                .expect("projection");

        assert_eq!(projection.phase, ImReplyLifecyclePhase::AskUserRequested);
        assert_eq!(
            projection.state,
            OpenClawPluginFeishuReplyCompletionState::AwaitingUser
        );
    }
}

pub(crate) async fn maybe_restore_openclaw_plugin_feishu_runtime_with_pool(
    pool: &SqlitePool,
    state: &OpenClawPluginFeishuRuntimeState,
    app: AppHandle,
) -> Result<bool, String> {
    let progress = get_feishu_setup_progress_with_pool(pool, state).await?;
    if !should_auto_restore_feishu_runtime(&progress) {
        return Ok(false);
    }

    start_openclaw_plugin_feishu_runtime_with_pool(pool, state, "openclaw-lark", None, Some(app))
        .await
        .map(|_| true)
}

pub(crate) fn stop_openclaw_plugin_feishu_runtime_in_state(
    state: &OpenClawPluginFeishuRuntimeState,
) -> Result<OpenClawPluginFeishuRuntimeStatus, String> {
    let (process, _stdin, should_fail_waiters) = {
        let mut guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock feishu runtime state".to_string())?;
        (
            guard.process.take(),
            guard.stdin.take(),
            !guard.pending_outbound_send_results.is_empty(),
        )
    };

    if let Some(slot) = process {
        if let Ok(mut child_guard) = slot.lock() {
            if let Some(mut child) = child_guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    let mut guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock feishu runtime state".to_string())?;
    guard.status.running = false;
    guard.status.pid = None;
    guard.status.last_stop_at = Some(now_rfc3339());
    drop(guard);

    if should_fail_waiters {
        let _ = fail_pending_feishu_runtime_outbound_send_waiters(
            state,
            "official feishu runtime stopped before outbound result was delivered".to_string(),
        );
    }

    let guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock feishu runtime state".to_string())?;
    Ok(guard.status.clone())
}

fn quote_powershell_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn matches_feishu_runtime_command_line(
    command_line: &str,
    plugin_root: &str,
    account_id: &str,
) -> bool {
    command_line.contains("run-feishu-host.mjs")
        && command_line.contains(&format!("--plugin-root {}", plugin_root))
        && command_line.contains(&format!("--account-id {}", account_id))
}

#[cfg(target_os = "windows")]
fn list_matching_feishu_runtime_pids(
    plugin_root: &str,
    account_id: &str,
) -> Result<Vec<u32>, String> {
    let script = format!(
        "$pluginRoot = {plugin_root}; \
         $accountId = {account_id}; \
         Get-CimInstance Win32_Process | \
         Where-Object {{ \
           $_.Name -eq 'node.exe' -and \
           $_.CommandLine -ne $null -and \
           $_.CommandLine.Contains('run-feishu-host.mjs') -and \
           $_.CommandLine.Contains('--plugin-root') -and \
           $_.CommandLine.Contains($pluginRoot) -and \
           $_.CommandLine.Contains('--account-id') -and \
           $_.CommandLine.Contains($accountId) \
         }} | \
         Select-Object -ExpandProperty ProcessId",
        plugin_root = quote_powershell_literal(plugin_root),
        account_id = quote_powershell_literal(account_id),
    );

    let mut command = Command::new("powershell");
    command.args(["-NoProfile", "-Command", &script]);
    hide_console_window(&mut command);
    let output = command
        .output()
        .map_err(|error| format!("failed to inspect feishu runtime processes: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "failed to inspect feishu runtime processes: {}",
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pids = stdout
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect::<Vec<_>>();
    Ok(pids)
}

#[cfg(not(target_os = "windows"))]
fn list_matching_feishu_runtime_pids(
    _plugin_root: &str,
    _account_id: &str,
) -> Result<Vec<u32>, String> {
    Ok(Vec::new())
}

fn kill_process_tree_by_pid(pid: u32) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("taskkill");
        command.args(["/T", "/F", "/PID", &pid.to_string()]);
        hide_console_window(&mut command);
        command
            .output()
            .map_err(|error| format!("failed to terminate runtime pid {pid}: {error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output()
            .map_err(|error| format!("failed to terminate runtime pid {pid}: {error}"))?;
        Ok(())
    }
}

fn cleanup_stale_feishu_runtime_processes(
    plugin_root: &str,
    account_id: &str,
    keep_pid: Option<u32>,
) -> Result<Vec<u32>, String> {
    let matching = list_matching_feishu_runtime_pids(plugin_root, account_id)?;
    let stale = matching
        .into_iter()
        .filter(|pid| Some(*pid) != keep_pid)
        .collect::<Vec<_>>();

    for pid in &stale {
        let _ = kill_process_tree_by_pid(*pid);
    }

    Ok(stale)
}
