use super::{
    emit_employee_inbound_dispatch_sessions, evaluate_openclaw_feishu_gate_with_pool,
    list_enabled_employee_feishu_connections_with_pool,
    maybe_handle_feishu_approval_command_with_pool, parse_feishu_approval_command,
    resolve_default_feishu_account_id_with_pool, resolve_feishu_app_credentials,
    resolve_feishu_sidecar_base_url,
};
use crate::commands::approvals::load_approval_record_with_pool;
use crate::commands::chat::ApprovalManagerState;
use crate::commands::employee_agents::{
    bridge_inbound_event_to_employee_sessions_with_pool, list_agent_employees_with_pool,
    AgentEmployee,
};
use crate::commands::im_gateway::process_im_event;
use crate::commands::openclaw_gateway::resolve_openclaw_route_with_pool;
use crate::commands::feishu_gateway::pairing_service::maybe_create_feishu_pairing_request_with_pool;
use crate::commands::skills::DbState;
use crate::diagnostics::{self, ManagedDiagnosticsState};
use crate::im::feishu_adapter::build_feishu_markdown_message;
use crate::im::feishu_formatter::format_role_message;
use crate::im::types::{ImEvent, ImEventType};
use crate::commands::feishu_gateway::types::FeishuInboundGateDecision;
use reqwest::Client;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Emitter;
use tauri::Manager;
use tauri::State;
use super::payload_parser::strip_placeholder_mentions;

#[derive(Debug, serde::Deserialize)]
struct SidecarResponse {
    output: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuWsStatus {
    pub running: bool,
    pub started_at: Option<String>,
    pub queued_events: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuEmployeeConnectionInput {
    pub employee_id: String,
    pub name: String,
    pub app_id: String,
    pub app_secret: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuEmployeeWsStatus {
    pub employee_id: String,
    pub running: bool,
    pub started_at: Option<String>,
    pub queued_events: usize,
    pub last_event_at: Option<String>,
    pub last_error: Option<String>,
    pub reconnect_attempts: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuWsStatusSummary {
    pub running: bool,
    pub started_at: Option<String>,
    pub queued_events: usize,
    pub running_count: usize,
    pub items: Vec<FeishuEmployeeWsStatus>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuEmployeeConnectionStatuses {
    pub relay: FeishuEventRelayStatus,
    pub sidecar: FeishuWsStatusSummary,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuWsEventRecord {
    #[serde(default)]
    pub employee_id: String,
    #[serde(default)]
    pub source_employee_ids: Vec<String>,
    pub id: String,
    pub event_type: String,
    pub chat_id: String,
    pub message_id: String,
    pub text: String,
    #[serde(default)]
    pub mention_open_id: String,
    #[serde(default)]
    pub mention_open_ids: Vec<String>,
    pub sender_open_id: String,
    #[serde(default)]
    pub chat_type: String,
    pub received_at: String,
}

pub(crate) fn sanitize_ws_inbound_text(raw: &str) -> Option<String> {
    let stripped = strip_placeholder_mentions(raw.to_string());
    if stripped.trim().is_empty() {
        None
    } else {
        Some(stripped)
    }
}

fn collect_ws_mention_candidates(event: &FeishuWsEventRecord) -> Vec<String> {
    let mut out = Vec::new();
    for raw in &event.mention_open_ids {
        let normalized = raw.trim().to_string();
        if normalized.is_empty() || out.iter().any(|v| v == &normalized) {
            continue;
        }
        out.push(normalized);
    }
    let fallback = event.mention_open_id.trim().to_string();
    if !fallback.is_empty() && !out.iter().any(|v| v == &fallback) {
        out.push(fallback);
    }
    out
}

fn extract_mention_labels(text: &str) -> Vec<String> {
    let mut labels = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '@' {
            continue;
        }
        let mut label = String::new();
        while let Some(next) = chars.peek() {
            if next.is_whitespace() {
                break;
            }
            label.push(*next);
            chars.next();
        }
        let normalized = label
            .trim()
            .trim_matches(|c: char| c == ',' || c == '，' || c == ':' || c == '：');
        if normalized.is_empty() {
            continue;
        }
        labels.push(normalized.to_string());
    }
    labels
}

pub(crate) fn resolve_ws_role_id(
    candidates: &[String],
    text: Option<&str>,
    source_employee_ids: &[String],
    employees: &[AgentEmployee],
) -> Option<String> {
    for candidate in candidates {
        if employees.iter().any(|e| {
            e.feishu_open_id == *candidate || e.role_id == *candidate || e.employee_id == *candidate
        }) {
            return Some(candidate.clone());
        }
    }

    if let Some(text) = text {
        for label in extract_mention_labels(text) {
            for employee in employees {
                let name = employee.name.trim();
                if name.is_empty() {
                    continue;
                }
                if label.contains(name) || name.contains(&label) {
                    return Some(employee.employee_id.clone());
                }
            }
        }
    }

    let mut matched_sources = Vec::new();
    for source in source_employee_ids {
        let normalized = source.trim();
        if normalized.is_empty() {
            continue;
        }
        if let Some(employee) = employees
            .iter()
            .find(|e| e.id == normalized || e.employee_id == normalized || e.role_id == normalized)
        {
            let employee_id = employee.employee_id.clone();
            if !matched_sources.iter().any(|v| v == &employee_id) {
                matched_sources.push(employee_id);
            }
        }
    }
    if matched_sources.len() == 1 {
        return Some(matched_sources[0].clone());
    }

    candidates.first().cloned()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuChatInfo {
    pub chat_id: String,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuChatListResult {
    pub items: Vec<FeishuChatInfo>,
    pub has_more: bool,
    pub page_token: Option<String>,
}

#[derive(Clone, Default)]
pub struct FeishuEventRelayState {
    running: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    interval_ms: Arc<AtomicU64>,
    total_accepted: Arc<AtomicUsize>,
    last_error: Arc<Mutex<Option<String>>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuEventRelayStatus {
    pub running: bool,
    pub generation: u64,
    pub interval_ms: u64,
    pub total_accepted: usize,
    pub last_error: Option<String>,
}

fn feishu_event_relay_status(state: &FeishuEventRelayState) -> FeishuEventRelayStatus {
    FeishuEventRelayStatus {
        running: state.running.load(Ordering::SeqCst),
        generation: state.generation.load(Ordering::SeqCst),
        interval_ms: state.interval_ms.load(Ordering::SeqCst),
        total_accepted: state.total_accepted.load(Ordering::SeqCst),
        last_error: state.last_error.lock().ok().and_then(|guard| guard.clone()),
    }
}

pub async fn call_sidecar_json(
    path: &str,
    body: serde_json::Value,
    sidecar_base_url: Option<String>,
) -> Result<serde_json::Value, String> {
    let base = sidecar_base_url.unwrap_or_else(|| "http://localhost:8765".to_string());
    let url = format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    let client = Client::new();
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("sidecar request failed: {}", e))?;
    let payload: SidecarResponse = resp
        .json()
        .await
        .map_err(|e| format!("sidecar json decode failed: {}", e))?;
    if let Some(err) = payload.error {
        return Err(format!("sidecar error: {}", err));
    }
    let output = payload.output.unwrap_or_else(|| "null".to_string());
    serde_json::from_str::<serde_json::Value>(&output)
        .map_err(|e| format!("sidecar output parse failed: {}", e))
}

pub async fn send_feishu_via_sidecar(
    payload: serde_json::Value,
    sidecar_base_url: Option<String>,
) -> Result<String, String> {
    let v = call_sidecar_json("/api/feishu/send-message", payload, sidecar_base_url).await?;
    Ok(v.to_string())
}

pub async fn list_feishu_chats_with_pool(
    pool: &SqlitePool,
    page_size: Option<usize>,
    page_token: Option<String>,
    user_id_type: Option<String>,
    app_id: Option<String>,
    app_secret: Option<String>,
    sidecar_base_url: Option<String>,
) -> Result<FeishuChatListResult, String> {
    let (resolved_app_id, resolved_app_secret) =
        resolve_feishu_app_credentials(pool, app_id, app_secret).await?;
    let resolved_sidecar_base_url = resolve_feishu_sidecar_base_url(pool, sidecar_base_url).await?;

    let mut payload = serde_json::json!({
        "page_size": page_size.unwrap_or(20).clamp(1, 100),
    });
    if let Some(v) = page_token {
        payload["page_token"] = serde_json::Value::String(v);
    }
    if let Some(v) = user_id_type {
        payload["user_id_type"] = serde_json::Value::String(v);
    }
    if let Some(v) = resolved_app_id {
        payload["app_id"] = serde_json::Value::String(v);
    }
    if let Some(v) = resolved_app_secret {
        payload["app_secret"] = serde_json::Value::String(v);
    }

    let v = call_sidecar_json("/api/feishu/list-chats", payload, resolved_sidecar_base_url).await?;
    serde_json::from_value(v).map_err(|e| format!("parse chat list failed: {}", e))
}

pub async fn push_role_summary_to_feishu_with_pool(
    pool: &SqlitePool,
    chat_id: String,
    role_id: String,
    role_name: String,
    conclusion: String,
    evidence: String,
    uncertainty: String,
    next_step: String,
    app_id: Option<String>,
    app_secret: Option<String>,
    sidecar_base_url: Option<String>,
) -> Result<String, String> {
    let (resolved_app_id, resolved_app_secret) =
        resolve_feishu_app_credentials(pool, app_id, app_secret).await?;
    let resolved_sidecar_base_url = resolve_feishu_sidecar_base_url(pool, sidecar_base_url).await?;

    let formatted = format_role_message(&conclusion, &evidence, &uncertainty, &next_step);
    let message = format!("**{}({})**\n\n{}", role_name, role_id, formatted);
    let mut payload = build_feishu_markdown_message(&chat_id, &message);
    if let Some(v) = resolved_app_id {
        payload["app_id"] = serde_json::Value::String(v);
    }
    if let Some(v) = resolved_app_secret {
        payload["app_secret"] = serde_json::Value::String(v);
    }
    send_feishu_via_sidecar(payload, resolved_sidecar_base_url).await
}

pub(crate) async fn start_feishu_long_connection_with_pool(
    pool: &SqlitePool,
    sidecar_base_url: Option<String>,
    app_id: Option<String>,
    app_secret: Option<String>,
) -> Result<FeishuWsStatus, String> {
    let (resolved_app_id, resolved_app_secret) =
        resolve_feishu_app_credentials(pool, app_id, app_secret).await?;
    let base = resolve_feishu_sidecar_base_url(pool, sidecar_base_url).await?;
    let body = serde_json::json!({
        "app_id": resolved_app_id.unwrap_or_default(),
        "app_secret": resolved_app_secret.unwrap_or_default(),
    });
    let v = call_sidecar_json("/api/feishu/ws/start", body, base).await?;
    serde_json::from_value(v).map_err(|e| format!("parse ws status failed: {}", e))
}

pub(crate) async fn reconcile_feishu_employee_connections_with_pool(
    pool: &SqlitePool,
    sidecar_base_url: Option<String>,
) -> Result<FeishuWsStatusSummary, String> {
    let base = resolve_feishu_sidecar_base_url(pool, sidecar_base_url).await?;
    let employees = list_enabled_employee_feishu_connections_with_pool(pool).await?;
    let body = serde_json::json!({
        "employees": employees,
    });
    let v = call_sidecar_json("/api/feishu/ws/reconcile", body, base).await?;
    serde_json::from_value(v).map_err(|e| format!("parse ws reconcile status failed: {}", e))
}

pub(crate) async fn get_feishu_long_connection_status_summary_with_pool(
    pool: &SqlitePool,
    sidecar_base_url: Option<String>,
) -> Result<FeishuWsStatusSummary, String> {
    let base = resolve_feishu_sidecar_base_url(pool, sidecar_base_url).await?;
    let v = call_sidecar_json("/api/feishu/ws/status", serde_json::json!({}), base).await?;
    serde_json::from_value(v).map_err(|e| format!("parse ws summary status failed: {}", e))
}

pub(crate) async fn stop_feishu_long_connection_with_pool(
    pool: &SqlitePool,
    sidecar_base_url: Option<String>,
) -> Result<FeishuWsStatus, String> {
    let base = resolve_feishu_sidecar_base_url(pool, sidecar_base_url).await?;
    let v = call_sidecar_json("/api/feishu/ws/stop", serde_json::json!({}), base).await?;
    serde_json::from_value(v).map_err(|e| format!("parse ws status failed: {}", e))
}

pub(crate) async fn get_feishu_long_connection_status_with_pool(
    pool: &SqlitePool,
    sidecar_base_url: Option<String>,
) -> Result<FeishuWsStatus, String> {
    let base = resolve_feishu_sidecar_base_url(pool, sidecar_base_url).await?;
    let v = call_sidecar_json("/api/feishu/ws/status", serde_json::json!({}), base).await?;
    serde_json::from_value(v).map_err(|e| format!("parse ws status failed: {}", e))
}

pub(crate) async fn get_feishu_employee_connection_statuses_with_pool(
    pool: &SqlitePool,
    sidecar_base_url: Option<String>,
    relay: &FeishuEventRelayState,
) -> Result<FeishuEmployeeConnectionStatuses, String> {
    let sidecar = get_feishu_long_connection_status_summary_with_pool(pool, sidecar_base_url).await?;
    Ok(FeishuEmployeeConnectionStatuses {
        relay: feishu_event_relay_status(relay),
        sidecar,
    })
}

pub(crate) async fn sync_feishu_ws_events_core(
    pool: &SqlitePool,
    sidecar_base_url: Option<String>,
    limit: Option<usize>,
    app: Option<&tauri::AppHandle>,
) -> Result<usize, String> {
    let base = resolve_feishu_sidecar_base_url(pool, sidecar_base_url).await?;
    let lim = limit.unwrap_or(50).clamp(1, 500);
    let v = call_sidecar_json(
        "/api/feishu/ws/drain-events",
        serde_json::json!({ "limit": lim }),
        base,
    )
    .await?;
    let events: Vec<FeishuWsEventRecord> =
        serde_json::from_value(v).map_err(|e| format!("parse ws events failed: {}", e))?;
    let default_account_id = resolve_default_feishu_account_id_with_pool(pool).await?;
    let enabled_employees = list_agent_employees_with_pool(pool)
        .await?
        .into_iter()
        .filter(|e| e.enabled)
        .collect::<Vec<_>>();

    let mut accepted = 0usize;
    for e in events {
        if e.chat_id.trim().is_empty() {
            continue;
        }
        let role_candidates = collect_ws_mention_candidates(&e);
        let mut source_employee_ids = e.source_employee_ids.clone();
        if source_employee_ids.is_empty() && !e.employee_id.trim().is_empty() {
            source_employee_ids.push(e.employee_id.trim().to_string());
        }
        let inbound = ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: e.chat_id.clone(),
            event_id: Some(e.id.clone()),
            message_id: if e.message_id.trim().is_empty() {
                None
            } else {
                Some(e.message_id.clone())
            },
            text: sanitize_ws_inbound_text(&e.text),
            role_id: resolve_ws_role_id(
                &role_candidates,
                Some(&e.text),
                &source_employee_ids,
                &enabled_employees,
            ),
            account_id: default_account_id.clone(),
            tenant_id: if e.sender_open_id.trim().is_empty() {
                None
            } else {
                Some(e.sender_open_id.clone())
            },
            sender_id: if e.sender_open_id.trim().is_empty() {
                None
            } else {
                Some(e.sender_open_id.clone())
            },
            chat_type: if e.chat_type.trim().is_empty() {
                None
            } else {
                Some(e.chat_type.trim().to_string())
            },
        };
        match evaluate_openclaw_feishu_gate_with_pool(pool, &inbound).await? {
            FeishuInboundGateDecision::Allow => {}
            FeishuInboundGateDecision::Reject { reason } => {
                if reason == "pairing_pending" {
                    let _ = maybe_create_feishu_pairing_request_with_pool(pool, &inbound).await?;
                }
                continue;
            }
        }
        let r = process_im_event(pool, inbound.clone()).await?;
        if r.accepted && !r.deduped {
            if let Some(app) = app {
                if let Some(approval_state) = app.try_state::<ApprovalManagerState>() {
                    let approval_command = parse_feishu_approval_command(inbound.text.as_deref());
                    if let Some(command) = approval_command {
                        if maybe_handle_feishu_approval_command_with_pool(
                            pool,
                            approval_state.0.as_ref(),
                            &inbound,
                            None,
                        )
                        .await?
                        .is_some()
                        {
                            if let Some(record) =
                                load_approval_record_with_pool(pool, &command.approval_id).await?
                            {
                                let _ = app.emit("approval-resolved", &record);
                            }
                            accepted += 1;
                            continue;
                        }
                    }
                }
            }
            let route_decision = resolve_openclaw_route_with_pool(pool, &inbound).await.ok();
            if let Ok(dispatches) = bridge_inbound_event_to_employee_sessions_with_pool(
                pool,
                &inbound,
                route_decision.as_ref(),
            )
            .await
            {
                if let Some(app) = app {
                    emit_employee_inbound_dispatch_sessions(app, "feishu", &dispatches);
                }
            }
            accepted += 1;
        }
    }
    Ok(accepted)
}

pub(crate) async fn start_feishu_event_relay_with_pool_and_app(
    pool: &SqlitePool,
    relay_state: FeishuEventRelayState,
    app: Option<tauri::AppHandle>,
    sidecar_base_url: Option<String>,
    interval_ms: Option<u64>,
    limit: Option<usize>,
) -> Result<FeishuEventRelayStatus, String> {
    if relay_state.running.load(Ordering::SeqCst) {
        return Ok(feishu_event_relay_status(&relay_state));
    }

    let base = resolve_feishu_sidecar_base_url(pool, sidecar_base_url).await?;
    let lim = limit.unwrap_or(50).clamp(1, 500);
    let tick_ms = interval_ms.unwrap_or(1500).clamp(200, 30_000);
    relay_state.interval_ms.store(tick_ms, Ordering::SeqCst);
    if let Ok(mut guard) = relay_state.last_error.lock() {
        *guard = None;
    }

    let generation = relay_state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    relay_state.running.store(true, Ordering::SeqCst);
    let pool = pool.clone();
    let relay_worker_state = relay_state.clone();
    let app_for_worker = app.clone();

    tauri::async_runtime::spawn(async move {
        loop {
            if relay_worker_state.generation.load(Ordering::SeqCst) != generation {
                break;
            }
            match sync_feishu_ws_events_core(
                &pool,
                base.clone(),
                Some(lim),
                app_for_worker.as_ref(),
            )
            .await
            {
                Ok(n) => {
                    if n > 0 {
                        relay_worker_state
                            .total_accepted
                            .fetch_add(n, Ordering::SeqCst);
                    }
                    if let Ok(mut guard) = relay_worker_state.last_error.lock() {
                        *guard = None;
                    }
                }
                Err(e) => {
                    if let Ok(mut guard) = relay_worker_state.last_error.lock() {
                        *guard = Some(e);
                    }
                }
            }
            let ms = relay_worker_state
                .interval_ms
                .load(Ordering::SeqCst)
                .clamp(200, 30_000);
            tokio::time::sleep(Duration::from_millis(ms)).await;
        }
        if relay_worker_state.generation.load(Ordering::SeqCst) == generation {
            relay_worker_state.running.store(false, Ordering::SeqCst);
        }
    });

    Ok(feishu_event_relay_status(&relay_state))
}

pub(crate) fn stop_feishu_event_relay_in_state(
    relay_state: FeishuEventRelayState,
) -> FeishuEventRelayStatus {
    relay_state.generation.fetch_add(1, Ordering::SeqCst);
    relay_state.running.store(false, Ordering::SeqCst);
    feishu_event_relay_status(&relay_state)
}

pub(crate) fn get_feishu_event_relay_status_in_state(
    relay_state: &FeishuEventRelayState,
) -> FeishuEventRelayStatus {
    feishu_event_relay_status(relay_state)
}

pub async fn send_feishu_text_message(
    app: tauri::AppHandle,
    chat_id: String,
    text: String,
    db: State<'_, DbState>,
    runtime_state: &crate::commands::openclaw_plugins::OpenClawPluginFeishuRuntimeState,
) -> Result<String, String> {
    super::remember_feishu_runtime_state_for_outbound(runtime_state);
    let account_id = resolve_default_feishu_account_id_with_pool(&db.0)
        .await?
        .unwrap_or_else(|| "default".to_string());
    let runtime_label = "official-plugin-runtime";
    let diagnostics_state = app.try_state::<ManagedDiagnosticsState>();

    if let Some(diagnostics_state) = diagnostics_state.as_ref() {
        let _ = diagnostics::write_log_record(
            &diagnostics_state.0.paths,
            diagnostics::LogLevel::Info,
            "feishu",
            "send_message_dispatch_start",
            "feishu outbound dispatch started",
            Some(serde_json::json!({
                "chat_id": chat_id.trim(),
                "account_id": account_id.trim(),
                "runtime": runtime_label,
                "text_preview": text.trim().chars().take(80).collect::<String>(),
            })),
        );
    }
    let result = super::send_feishu_text_message_via_official_runtime_with_pool(
        &db.0,
        runtime_state,
        &chat_id,
        &text,
        Some(account_id.clone()),
    )
    .await;
    match &result {
        Ok(response) => {
            if let Some(diagnostics_state) = diagnostics_state.as_ref() {
                let _ = diagnostics::write_log_record(
                    &diagnostics_state.0.paths,
                    diagnostics::LogLevel::Info,
                    "feishu",
                    "send_message_dispatch_succeeded",
                    "feishu outbound dispatch succeeded",
                    Some(serde_json::json!({
                        "chat_id": chat_id.trim(),
                        "account_id": account_id.trim(),
                        "runtime": runtime_label,
                        "response_preview": response.chars().take(160).collect::<String>(),
                    })),
                );
            }
        }
        Err(error) => {
            if let Some(diagnostics_state) = diagnostics_state.as_ref() {
                let _ = diagnostics::write_log_record(
                    &diagnostics_state.0.paths,
                    diagnostics::LogLevel::Error,
                    "feishu",
                    "send_message_dispatch_failed",
                    "feishu outbound dispatch failed",
                    Some(serde_json::json!({
                        "chat_id": chat_id.trim(),
                        "account_id": account_id.trim(),
                        "runtime": runtime_label,
                        "error": error.to_string(),
                    })),
                );
            }
        }
    }
    result
}
