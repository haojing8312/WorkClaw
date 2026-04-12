use crate::commands::feishu_gateway::{
    dispatch_feishu_inbound_to_workclaw_with_pool_and_app, upsert_feishu_pairing_request_with_pool,
};
use crate::im::types::{ImEvent, ImEventType};
use crate::windows_process::hide_console_window;
use sqlx::SqlitePool;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use tauri::AppHandle;

use super::{
    app_setting_string_or_default, build_feishu_openclaw_config_with_pool,
    ensure_supported_feishu_host_node_version, get_feishu_setup_progress_with_pool,
    get_openclaw_plugin_install_by_id_with_pool, normalize_required, now_rfc3339,
    resolve_plugin_host_dir, resolve_plugin_host_fixture_root,
    resolve_plugin_host_run_feishu_script, should_auto_restore_feishu_runtime,
    OpenClawPluginFeishuOutboundCommandErrorEvent, OpenClawPluginFeishuOutboundSendCommandPayload,
    OpenClawPluginFeishuOutboundSendRequest, OpenClawPluginFeishuOutboundSendResult,
    OpenClawPluginFeishuRuntimeState, OpenClawPluginFeishuRuntimeStatus,
};

pub(crate) fn merge_feishu_runtime_status_event(
    status: &mut OpenClawPluginFeishuRuntimeStatus,
    value: &serde_json::Value,
) {
    let Some(event) = value.get("event").and_then(|entry| entry.as_str()) else {
        return;
    };

    match event {
        "status" => {
            status.last_event_at = Some(now_rfc3339());
            if let Some(patch) = value.get("patch").and_then(|entry| entry.as_object()) {
                if let Some(account_id) = patch.get("accountId").and_then(|entry| entry.as_str()) {
                    status.account_id = account_id.to_string();
                }
                if let Some(port) = patch.get("port").and_then(|entry| entry.as_u64()) {
                    status.port = Some(port as u16);
                }
                if let Some(last_error) = patch.get("lastError").and_then(|entry| entry.as_str()) {
                    let normalized = last_error.trim();
                    status.last_error = if normalized.is_empty() {
                        None
                    } else {
                        Some(normalized.to_string())
                    };
                } else if !patch.is_empty() {
                    status.last_error = None;
                }
            }
        }
        "log" => {
            status.last_event_at = Some(now_rfc3339());
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
                status.recent_logs.push(entry.clone());
                trim_recent_runtime_logs(status);
                if level == "error" {
                    status.last_error = Some(entry);
                }
            }
        }
        "fatal" => {
            status.last_event_at = Some(now_rfc3339());
            if let Some(error) = value.get("error").and_then(|entry| entry.as_str()) {
                let normalized = error.trim();
                if !normalized.is_empty() {
                    status.last_error = Some(normalized.to_string());
                    status
                        .recent_logs
                        .push(format!("[fatal] runtime: {normalized}"));
                    trim_recent_runtime_logs(status);
                }
            }
        }
        _ => {}
    }
}

fn trim_recent_runtime_logs(status: &mut OpenClawPluginFeishuRuntimeStatus) {
    if status.recent_logs.len() > 40 {
        let overflow = status.recent_logs.len() - 40;
        status.recent_logs.drain(0..overflow);
    }
}

fn build_feishu_runtime_outbound_send_command_payload(
    request: &OpenClawPluginFeishuOutboundSendRequest,
) -> Result<OpenClawPluginFeishuOutboundSendCommandPayload, String> {
    let request_id = normalize_required(&request.request_id, "request_id")?;
    let account_id = app_setting_string_or_default(Some(request.account_id.clone()), "default");
    let target = normalize_required(&request.target, "target")?;
    let text = request.text.trim().to_string();
    let mode = app_setting_string_or_default(Some(request.mode.clone()), "text");

    Ok(OpenClawPluginFeishuOutboundSendCommandPayload {
        request_id,
        command: "send_message".to_string(),
        account_id,
        target,
        thread_id: request
            .thread_id
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        text,
        mode,
    })
}

pub(crate) fn register_pending_feishu_runtime_outbound_send_waiter(
    state: &OpenClawPluginFeishuRuntimeState,
    request_id: &str,
) -> Result<std::sync::mpsc::Receiver<Result<OpenClawPluginFeishuOutboundSendResult, String>>, String>
{
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let mut guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock feishu runtime state".to_string())?;
    if guard.pending_outbound_send_results.contains_key(request_id) {
        return Err(format!("duplicate outbound requestId: {request_id}"));
    }
    guard
        .pending_outbound_send_results
        .insert(request_id.to_string(), sender);
    guard.status.last_event_at = Some(now_rfc3339());
    guard.status.recent_logs.push(format!(
        "[outbound] queued send_message requestId={request_id}"
    ));
    trim_recent_runtime_logs(&mut guard.status);
    Ok(receiver)
}

fn deliver_pending_feishu_runtime_outbound_send_result(
    state: &OpenClawPluginFeishuRuntimeState,
    result: OpenClawPluginFeishuOutboundSendResult,
) -> bool {
    let request_id = result.request_id.clone();
    let sender = {
        let mut guard = match state.0.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        guard.status.last_event_at = Some(now_rfc3339());
        guard
            .status
            .recent_logs
            .push(format!("[outbound] send_result requestId={request_id}"));
        trim_recent_runtime_logs(&mut guard.status);
        guard.pending_outbound_send_results.remove(&request_id)
    };

    match sender {
        Some(sender) => sender.send(Ok(result)).is_ok(),
        None => {
            if let Ok(mut guard) = state.0.lock() {
                guard.status.recent_logs.push(format!(
                    "[warn] runtime: unhandled outbound send_result requestId={request_id}"
                ));
                trim_recent_runtime_logs(&mut guard.status);
            }
            false
        }
    }
}

fn deliver_pending_feishu_runtime_outbound_command_error(
    state: &OpenClawPluginFeishuRuntimeState,
    error_event: OpenClawPluginFeishuOutboundCommandErrorEvent,
) -> bool {
    let error_message = error_event.error.trim().to_string();
    let Some(request_id) = error_event
        .request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        let failed = fail_pending_feishu_runtime_outbound_send_waiters(
            state,
            if error_message.is_empty() {
                "official feishu runtime reported an outbound command error".to_string()
            } else {
                format!("official feishu runtime command error: {error_message}")
            },
        );
        return failed > 0;
    };

    let sender = {
        let mut guard = match state.0.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        guard.status.last_event_at = Some(now_rfc3339());
        guard.status.last_error = Some(error_message.clone());
        guard.status.recent_logs.push(format!(
            "[outbound] command_error requestId={request_id}: {error_message}"
        ));
        trim_recent_runtime_logs(&mut guard.status);
        guard.pending_outbound_send_results.remove(request_id)
    };

    match sender {
        Some(sender) => sender
            .send(Err(format!(
                "official feishu runtime command error: {error_message}"
            )))
            .is_ok(),
        None => false,
    }
}

fn fail_pending_feishu_runtime_outbound_send_waiters(
    state: &OpenClawPluginFeishuRuntimeState,
    error: String,
) -> usize {
    let senders = {
        let mut guard = match state.0.lock() {
            Ok(guard) => guard,
            Err(_) => return 0,
        };
        guard.status.last_event_at = Some(now_rfc3339());
        guard.status.recent_logs.push(format!("[outbound] {error}"));
        trim_recent_runtime_logs(&mut guard.status);
        std::mem::take(&mut guard.pending_outbound_send_results)
    };

    let mut count = 0;
    for (_request_id, sender) in senders {
        let _ = sender.send(Err(error.clone()));
        count += 1;
    }
    count
}

fn parse_openclaw_plugin_feishu_runtime_send_result_event(
    value: &serde_json::Value,
) -> Result<OpenClawPluginFeishuOutboundSendResult, String> {
    let event = value
        .get("event")
        .and_then(|entry| entry.as_str())
        .unwrap_or_default();
    if event != "send_result" {
        return Err(format!("unexpected outbound event: {event}"));
    }
    serde_json::from_value::<OpenClawPluginFeishuOutboundSendResult>(value.clone())
        .map_err(|error| format!("invalid send_result event: {error}"))
}

fn parse_openclaw_plugin_feishu_runtime_command_error_event(
    value: &serde_json::Value,
) -> Result<OpenClawPluginFeishuOutboundCommandErrorEvent, String> {
    let event = value
        .get("event")
        .and_then(|entry| entry.as_str())
        .unwrap_or_default();
    if event != "command_error" {
        return Err(format!("unexpected outbound event: {event}"));
    }
    serde_json::from_value::<OpenClawPluginFeishuOutboundCommandErrorEvent>(value.clone())
        .map_err(|error| format!("invalid command_error event: {error}"))
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

    let stdin = {
        let guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock feishu runtime state".to_string())?;
        if !guard.status.running {
            return Err("official feishu runtime is not running".to_string());
        }
        guard.stdin.clone().ok_or_else(|| {
            "official feishu runtime is not accepting outbound commands".to_string()
        })?
    };

    let receiver =
        register_pending_feishu_runtime_outbound_send_waiter(state, &payload.request_id)?;

    if let Err(error) = {
        let mut stdin_guard = stdin
            .lock()
            .map_err(|_| "failed to lock feishu runtime stdin".to_string())?;
        stdin_guard
            .write_all(payload_json.as_bytes())
            .and_then(|_| stdin_guard.write_all(b"\n"))
            .and_then(|_| stdin_guard.flush())
    } {
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
            let _ = {
                if let Ok(mut guard) = state.0.lock() {
                    guard
                        .pending_outbound_send_results
                        .remove(&payload.request_id);
                    guard.status.recent_logs.push(format!(
                        "[warn] runtime: outbound send timed out requestId={}",
                        payload.request_id
                    ));
                    trim_recent_runtime_logs(&mut guard.status);
                    guard.status.last_event_at = Some(now_rfc3339());
                }
                Ok::<(), ()>(())
            };
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

pub(crate) fn current_feishu_runtime_status(
    state: &OpenClawPluginFeishuRuntimeState,
) -> OpenClawPluginFeishuRuntimeStatus {
    state
        .0
        .lock()
        .expect("lock feishu runtime state")
        .status
        .clone()
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

pub(crate) fn handle_openclaw_plugin_feishu_runtime_stdout_line(
    pool: &SqlitePool,
    state: &OpenClawPluginFeishuRuntimeState,
    app: Option<&AppHandle>,
    trimmed: &str,
) {
    if trimmed.is_empty() {
        return;
    }

    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return;
    };

    let event = value
        .get("event")
        .and_then(|entry| entry.as_str())
        .unwrap_or_default();
    if event == "send_result" {
        let handled = handle_openclaw_plugin_feishu_runtime_send_result_event(state, &value);
        if !handled {
            let request_id = value
                .get("requestId")
                .and_then(|entry| entry.as_str())
                .unwrap_or("unknown");
            if let Ok(mut guard) = state.0.lock() {
                guard.status.recent_logs.push(format!(
                    "[warn] runtime: dropped send_result requestId={request_id}"
                ));
                trim_recent_runtime_logs(&mut guard.status);
                guard.status.last_event_at = Some(now_rfc3339());
            }
        }
        return;
    }
    if event == "command_error" {
        let handled = handle_openclaw_plugin_feishu_runtime_command_error_event(state, &value);
        if !handled {
            let request_id = value
                .get("requestId")
                .and_then(|entry| entry.as_str())
                .unwrap_or("unknown");
            if let Ok(mut guard) = state.0.lock() {
                guard.status.recent_logs.push(format!(
                    "[warn] runtime: dropped command_error requestId={request_id}"
                ));
                trim_recent_runtime_logs(&mut guard.status);
                guard.status.last_event_at = Some(now_rfc3339());
            }
        }
        return;
    }

    if let Ok(mut guard) = state.0.lock() {
        if event == "pairing_request" {
            handle_feishu_runtime_pairing_request_event(pool, &mut guard.status, &value);
        } else if event == "dispatch_request" {
            match tauri::async_runtime::block_on(parse_feishu_runtime_dispatch_event_with_pool(
                pool, &value,
            )) {
                Ok(inbound) => {
                    if let Some(app_handle) = app.as_ref() {
                        match tauri::async_runtime::block_on(
                            dispatch_feishu_inbound_to_workclaw_with_pool_and_app(
                                pool, app_handle, &inbound, None,
                            ),
                        ) {
                            Ok(result) => {
                                guard.status.last_error = None;
                                guard.status.recent_logs.push(format!(
                                    "[dispatch] feishu: accepted={} deduped={} thread={}",
                                    result.accepted, result.deduped, inbound.thread_id
                                ));
                            }
                            Err(error) => {
                                guard.status.last_error = Some(format!(
                                    "failed to bridge official feishu dispatch: {error}"
                                ));
                                guard.status.recent_logs.push(format!(
                                    "[error] runtime: failed to bridge official feishu dispatch: {error}"
                                ));
                            }
                        }
                    } else {
                        guard.status.recent_logs.push(
                            "[warn] runtime: dispatch_request ignored because no app handle was available"
                                .to_string(),
                        );
                    }
                }
                Err(error) => {
                    guard.status.last_error =
                        Some(format!("invalid official feishu dispatch event: {error}"));
                    guard.status.recent_logs.push(format!(
                        "[error] runtime: invalid official feishu dispatch event: {error}"
                    ));
                }
            }
            trim_recent_runtime_logs(&mut guard.status);
        } else {
            merge_feishu_runtime_status_event(&mut guard.status, &value);
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

    let is_direct = matches!(chat_type.map(str::trim), Some("direct") | None);
    let looks_like_sender_open_id = normalized_thread_id.starts_with("ou_");
    if !is_direct || !looks_like_sender_open_id {
        return Ok(normalized_thread_id.to_string());
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

    Ok(row
        .map(|(chat_id,)| chat_id)
        .filter(|chat_id| !chat_id.trim().is_empty())
        .unwrap_or_else(|| normalized_thread_id.to_string()))
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
    let thread_id = if matches!(chat_type.as_deref(), Some("direct") | None) {
        if let Some(chat_id) = explicit_chat_id.clone() {
            chat_id
        } else {
            resolve_feishu_runtime_dispatch_thread_id_with_pool(
                pool,
                raw_thread_id,
                account_id.as_deref(),
                sender_id.as_deref(),
                chat_type.as_deref(),
            )
            .await?
        }
    } else {
        resolve_feishu_runtime_dispatch_thread_id_with_pool(
            pool,
            raw_thread_id,
            account_id.as_deref(),
            sender_id.as_deref(),
            chat_type.as_deref(),
        )
        .await?
    };

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
