use crate::commands::employee_agents::{
    ensure_employee_sessions_for_event_with_pool, link_inbound_event_to_session_with_pool,
    list_agent_employees_with_pool, AgentEmployee,
};
use crate::commands::im_config::get_thread_role_config_with_pool;
use crate::commands::im_gateway::process_im_event;
use crate::commands::openclaw_gateway::resolve_openclaw_route_with_pool;
use crate::commands::skills::DbState;
use crate::im::feishu_adapter::{build_feishu_markdown_message, build_feishu_text_message};
use crate::im::feishu_formatter::format_role_message;
use crate::im::runtime_bridge::{
    build_im_role_dispatch_request, build_im_role_event_payload, ImRoleDispatchRequest,
    ImRoleEventPayload,
};
use crate::im::types::{ImEvent, ImEventType};
use reqwest::Client;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Emitter;
use tauri::State;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuGatewayResult {
    pub accepted: bool,
    pub deduped: bool,
    pub challenge: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ImRouteDecisionEvent {
    pub session_id: String,
    pub thread_id: String,
    pub agent_id: String,
    pub session_key: String,
    pub matched_by: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuGatewaySettings {
    pub app_id: String,
    pub app_secret: String,
    pub ingress_token: String,
    pub encrypt_key: String,
    pub sidecar_base_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedFeishuPayload {
    Challenge(String),
    Event(ImEvent),
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FeishuEnvelope {
    challenge: Option<String>,
    header: Option<FeishuHeader>,
    event: Option<FeishuEvent>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FeishuHeader {
    event_id: Option<String>,
    event_type: Option<String>,
    tenant_key: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FeishuEvent {
    message: Option<FeishuMessage>,
    sender: Option<FeishuSender>,
    mentions: Option<Vec<FeishuMention>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FeishuMessage {
    message_id: Option<String>,
    chat_id: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FeishuSender {
    sender_id: Option<FeishuSenderId>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FeishuSenderId {
    open_id: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FeishuMention {
    key: Option<String>,
    #[serde(rename = "id")]
    mention_id: Option<FeishuMentionId>,
    #[serde(default)]
    open_id: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FeishuMentionId {
    open_id: Option<String>,
}

fn mention_open_id(mention: &FeishuMention) -> Option<String> {
    mention
        .mention_id
        .as_ref()
        .and_then(|id| id.open_id.clone())
        .or_else(|| mention.open_id.clone())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub fn parse_feishu_payload(payload: &str) -> Result<ParsedFeishuPayload, String> {
    if let Ok(event) = serde_json::from_str::<ImEvent>(payload) {
        return Ok(ParsedFeishuPayload::Event(event));
    }

    let env: FeishuEnvelope =
        serde_json::from_str(payload).map_err(|e| format!("invalid feishu payload: {}", e))?;
    if let Some(challenge) = env.challenge {
        return Ok(ParsedFeishuPayload::Challenge(challenge));
    }

    let header = env
        .header
        .ok_or_else(|| "feishu payload missing header".to_string())?;
    let event = env
        .event
        .ok_or_else(|| "feishu payload missing event".to_string())?;
    let message = event
        .message
        .ok_or_else(|| "feishu payload missing message".to_string())?;

    let mentions = event.mentions.unwrap_or_default();
    let mention_keys = mentions
        .iter()
        .filter_map(|m| m.key.as_ref().map(|v| v.trim().to_string()))
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>();
    let content_text = parse_message_text(message.content.as_deref().unwrap_or(""), &mention_keys);
    let role_id = mentions.iter().find_map(mention_open_id);

    let event_type = match header
        .event_type
        .as_deref()
        .unwrap_or("im.message.receive_v1")
    {
        "im.message.receive_v1" => ImEventType::MessageCreated,
        "im.message.reaction.created_v1" => ImEventType::MessageCreated,
        other => {
            if other.contains("mention") {
                ImEventType::MentionRole
            } else {
                return Err(format!("unsupported feishu event_type: {}", other));
            }
        }
    };

    Ok(ParsedFeishuPayload::Event(ImEvent {
        channel: "feishu".to_string(),
        event_type,
        thread_id: message
            .chat_id
            .ok_or_else(|| "feishu payload missing chat_id".to_string())?,
        event_id: header.event_id,
        message_id: message.message_id,
        text: content_text,
        role_id,
        account_id: header.tenant_key.clone(),
        tenant_id: header.tenant_key.or_else(|| {
            event
                .sender
                .and_then(|s| s.sender_id.and_then(|id| id.open_id))
        }),
    }))
}

fn strip_placeholder_mentions(mut text: String) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    let mut cleaned = String::with_capacity(chars.len());
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '@' && i + 1 < chars.len() && chars[i + 1] == '_' {
            i += 2;
            while i < chars.len() {
                let c = chars[i];
                if c.is_ascii_alphanumeric() || c == '_' {
                    i += 1;
                    continue;
                }
                break;
            }
            continue;
        }
        cleaned.push(chars[i]);
        i += 1;
    }
    text.clear();
    text.push_str(
        cleaned
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .as_str(),
    );
    text
}

fn parse_message_text(raw: &str, mention_keys: &[String]) -> Option<String> {
    if raw.trim().is_empty() {
        return None;
    }
    let base = if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
        v.get("text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(raw)
            .to_string()
    } else {
        raw.to_string()
    };
    let mut stripped = base;
    for key in mention_keys {
        stripped = stripped.replace(key, " ");
    }
    let stripped = strip_placeholder_mentions(stripped);
    if stripped.trim().is_empty() {
        None
    } else {
        Some(stripped)
    }
}

pub async fn validate_feishu_auth_with_pool(
    pool: &SqlitePool,
    auth_token: Option<String>,
) -> Result<(), String> {
    let configured: Option<(String,)> =
        sqlx::query_as("SELECT value FROM app_settings WHERE key = 'feishu_ingress_token' LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    let expected = configured.map(|(v,)| v).unwrap_or_default();
    if expected.trim().is_empty() {
        return Ok(());
    }
    if auth_token.unwrap_or_default() == expected {
        Ok(())
    } else {
        Err("feishu auth token invalid".to_string())
    }
}

pub async fn get_app_setting(pool: &SqlitePool, key: &str) -> Result<Option<String>, String> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value FROM app_settings WHERE key = ? LIMIT 1")
            .bind(key)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(row.map(|(v,)| v))
}

pub async fn set_app_setting(pool: &SqlitePool, key: &str, value: &str) -> Result<(), String> {
    sqlx::query("INSERT OR REPLACE INTO app_settings (key, value) VALUES (?, ?)")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn resolve_feishu_sidecar_base_url(
    pool: &SqlitePool,
    input: Option<String>,
) -> Result<Option<String>, String> {
    if let Some(v) = input {
        if !v.trim().is_empty() {
            return Ok(Some(v));
        }
    }
    Ok(get_app_setting(pool, "feishu_sidecar_base_url").await?)
}

pub async fn resolve_feishu_app_credentials(
    pool: &SqlitePool,
    app_id: Option<String>,
    app_secret: Option<String>,
) -> Result<(Option<String>, Option<String>), String> {
    if let (Some(id), Some(secret)) = (app_id.clone(), app_secret.clone()) {
        if !id.trim().is_empty() && !secret.trim().is_empty() {
            return Ok((Some(id), Some(secret)));
        }
    }

    let employee_creds = sqlx::query_as::<_, (String, String)>(
        "SELECT feishu_app_id, feishu_app_secret
         FROM agent_employees
         WHERE enabled = 1
           AND TRIM(feishu_app_id) <> ''
           AND TRIM(feishu_app_secret) <> ''
         ORDER BY is_default DESC, updated_at DESC
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Some((id, secret)) = employee_creds {
        return Ok((Some(id), Some(secret)));
    }

    // Backward compatibility: legacy global settings fallback.
    let resolved_app_id = get_app_setting(pool, "feishu_app_id").await?;
    let resolved_app_secret = get_app_setting(pool, "feishu_app_secret").await?;
    Ok((resolved_app_id, resolved_app_secret))
}

pub async fn list_enabled_employee_feishu_connections_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<FeishuEmployeeConnectionInput>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT employee_id, role_id, feishu_app_id, feishu_app_secret
         FROM agent_employees
         WHERE enabled = 1
           AND TRIM(feishu_app_id) <> ''
           AND TRIM(feishu_app_secret) <> ''
         ORDER BY is_default DESC, updated_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut result = Vec::with_capacity(rows.len());
    for (employee_id_raw, role_id, app_id, app_secret) in rows {
        let employee_id = if employee_id_raw.trim().is_empty() {
            role_id.trim().to_string()
        } else {
            employee_id_raw.trim().to_string()
        };
        if employee_id.is_empty() {
            continue;
        }
        result.push(FeishuEmployeeConnectionInput {
            employee_id,
            app_id: app_id.trim().to_string(),
            app_secret: app_secret.trim().to_string(),
        });
    }
    Ok(result)
}

pub fn calculate_feishu_signature(
    timestamp: &str,
    nonce: &str,
    encrypt_key: &str,
    body: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{}{}{}{}", timestamp, nonce, encrypt_key, body));
    let digest = hasher.finalize();
    format!("{:x}", digest)
}

pub async fn validate_feishu_signature_with_pool(
    pool: &SqlitePool,
    payload: &str,
    timestamp: Option<String>,
    nonce: Option<String>,
    signature: Option<String>,
) -> Result<(), String> {
    let encrypt_key = get_app_setting(pool, "feishu_encrypt_key")
        .await?
        .unwrap_or_default();
    if encrypt_key.trim().is_empty() {
        return Ok(());
    }

    let ts = timestamp.ok_or_else(|| "missing feishu timestamp".to_string())?;
    let nn = nonce.ok_or_else(|| "missing feishu nonce".to_string())?;
    let sig = signature.ok_or_else(|| "missing feishu signature".to_string())?;
    let expected = calculate_feishu_signature(&ts, &nn, &encrypt_key, payload);
    if expected == sig.to_ascii_lowercase() {
        Ok(())
    } else {
        Err("feishu signature invalid".to_string())
    }
}

pub async fn plan_role_events_for_feishu(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<ImRoleEventPayload>, String> {
    let cfg = match get_thread_role_config_with_pool(pool, &event.thread_id).await {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()),
    };
    let session_id = format!("im-{}", event.thread_id);
    let text = event.text.clone().unwrap_or_default();

    let roles: Vec<String> = event
        .role_id
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .and_then(|role_id| {
            if cfg.roles.iter().any(|r| r == role_id) {
                Some(vec![role_id.to_string()])
            } else {
                None
            }
        })
        .unwrap_or_else(|| cfg.roles.clone());

    Ok(roles
        .into_iter()
        .map(|role_id| {
            build_im_role_event_payload(
                &session_id,
                &event.thread_id,
                &role_id,
                &role_id,
                "running",
                &format!("飞书事件触发：{}", text),
                None,
            )
        })
        .collect())
}

pub async fn plan_role_dispatch_requests_for_feishu(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<ImRoleDispatchRequest>, String> {
    let cfg = match get_thread_role_config_with_pool(pool, &event.thread_id).await {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()),
    };
    let session_id = format!("im-{}", event.thread_id);
    let user_text = event
        .text
        .clone()
        .unwrap_or_else(|| "请基于当前上下文继续协作".to_string());

    let roles: Vec<String> = event
        .role_id
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .and_then(|role_id| {
            if cfg.roles.iter().any(|r| r == role_id) {
                Some(vec![role_id.to_string()])
            } else {
                None
            }
        })
        .unwrap_or_else(|| cfg.roles.clone());

    let agent_type = if cfg.scenario_template == "opportunity_review" {
        "plan"
    } else {
        "general-purpose"
    };

    Ok(roles
        .into_iter()
        .map(|role_id| {
            build_im_role_dispatch_request(
                &session_id,
                &event.thread_id,
                &role_id,
                &role_id,
                &format!("场景={}。用户输入：{}", cfg.scenario_template, user_text),
                agent_type,
            )
        })
        .collect())
}

#[tauri::command]
pub async fn handle_feishu_event(
    payload: String,
    auth_token: Option<String>,
    signature: Option<String>,
    timestamp: Option<String>,
    nonce: Option<String>,
    app: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<FeishuGatewayResult, String> {
    validate_feishu_auth_with_pool(&db.0, auth_token).await?;
    validate_feishu_signature_with_pool(&db.0, &payload, timestamp, nonce, signature).await?;
    match parse_feishu_payload(&payload)? {
        ParsedFeishuPayload::Challenge(challenge) => Ok(FeishuGatewayResult {
            accepted: true,
            deduped: false,
            challenge: Some(challenge),
        }),
        ParsedFeishuPayload::Event(event) => {
            let r = process_im_event(&db.0, event.clone()).await?;
            if !r.deduped {
                let route_decision = resolve_openclaw_route_with_pool(&db.0, &event).await.ok();
                let employee_sessions =
                    ensure_employee_sessions_for_event_with_pool(&db.0, &event).await?;
                for s in &employee_sessions {
                    let _ = link_inbound_event_to_session_with_pool(
                        &db.0,
                        &event,
                        &s.employee_id,
                        &s.session_id,
                    )
                    .await;
                    let route_agent_id = route_decision
                        .as_ref()
                        .and_then(|v| v.get("agentId"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or(&s.role_id)
                        .to_string();
                    let route_session_key = route_decision
                        .as_ref()
                        .and_then(|v| v.get("sessionKey"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or(&s.session_id)
                        .to_string();
                    let matched_by = route_decision
                        .as_ref()
                        .and_then(|v| v.get("matchedBy"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("default")
                        .to_string();
                    let _ = app.emit(
                        "im-route-decision",
                        ImRouteDecisionEvent {
                            session_id: s.session_id.clone(),
                            thread_id: event.thread_id.clone(),
                            agent_id: route_agent_id,
                            session_key: route_session_key,
                            matched_by,
                        },
                    );

                    let _ = app.emit(
                        "im-role-event",
                        build_im_role_event_payload(
                            &s.session_id,
                            &event.thread_id,
                            &s.role_id,
                            &s.employee_name,
                            "running",
                            "飞书消息已同步到桌面会话，正在执行",
                            None,
                        ),
                    );
                    let prompt = event
                        .text
                        .clone()
                        .unwrap_or_else(|| "请继续基于当前上下文推进".to_string());
                    let _ = app.emit(
                        "im-role-dispatch-request",
                        build_im_role_dispatch_request(
                            &s.session_id,
                            &event.thread_id,
                            &s.role_id,
                            &s.employee_name,
                            &prompt,
                            "general-purpose",
                        ),
                    );
                }

                if employee_sessions.is_empty() {
                    let planned = plan_role_events_for_feishu(&db.0, &event).await?;
                    for evt in planned {
                        let _ = app.emit("im-role-event", evt);
                    }
                    let dispatches = plan_role_dispatch_requests_for_feishu(&db.0, &event).await?;
                    for req in dispatches {
                        let _ = app.emit("im-role-dispatch-request", req);
                    }
                }
            }
            Ok(FeishuGatewayResult {
                accepted: r.accepted,
                deduped: r.deduped,
                challenge: None,
            })
        }
    }
}

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
    pub received_at: String,
}

fn sanitize_ws_inbound_text(raw: &str) -> Option<String> {
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

fn resolve_ws_role_id(
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

#[tauri::command]
pub async fn send_feishu_text_message(
    chat_id: String,
    text: String,
    app_id: Option<String>,
    app_secret: Option<String>,
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let (resolved_app_id, resolved_app_secret) =
        resolve_feishu_app_credentials(&db.0, app_id, app_secret).await?;
    let resolved_sidecar_base_url =
        resolve_feishu_sidecar_base_url(&db.0, sidecar_base_url).await?;

    let mut payload = build_feishu_text_message(&chat_id, &text);
    if let Some(v) = resolved_app_id {
        payload["app_id"] = serde_json::Value::String(v);
    }
    if let Some(v) = resolved_app_secret {
        payload["app_secret"] = serde_json::Value::String(v);
    }
    send_feishu_via_sidecar(payload, resolved_sidecar_base_url).await
}

#[tauri::command]
pub async fn list_feishu_chats(
    page_size: Option<usize>,
    page_token: Option<String>,
    user_id_type: Option<String>,
    app_id: Option<String>,
    app_secret: Option<String>,
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuChatListResult, String> {
    let (resolved_app_id, resolved_app_secret) =
        resolve_feishu_app_credentials(&db.0, app_id, app_secret).await?;
    let resolved_sidecar_base_url =
        resolve_feishu_sidecar_base_url(&db.0, sidecar_base_url).await?;

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

#[tauri::command]
pub async fn push_role_summary_to_feishu(
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
    db: State<'_, DbState>,
) -> Result<String, String> {
    let (resolved_app_id, resolved_app_secret) =
        resolve_feishu_app_credentials(&db.0, app_id, app_secret).await?;
    let resolved_sidecar_base_url =
        resolve_feishu_sidecar_base_url(&db.0, sidecar_base_url).await?;

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

#[tauri::command]
pub async fn set_feishu_gateway_settings(
    settings: FeishuGatewaySettings,
    db: State<'_, DbState>,
) -> Result<(), String> {
    set_app_setting(&db.0, "feishu_app_id", settings.app_id.as_str()).await?;
    set_app_setting(&db.0, "feishu_app_secret", settings.app_secret.as_str()).await?;
    set_app_setting(
        &db.0,
        "feishu_ingress_token",
        settings.ingress_token.as_str(),
    )
    .await?;
    set_app_setting(&db.0, "feishu_encrypt_key", settings.encrypt_key.as_str()).await?;
    set_app_setting(
        &db.0,
        "feishu_sidecar_base_url",
        settings.sidecar_base_url.as_str(),
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn get_feishu_gateway_settings(
    db: State<'_, DbState>,
) -> Result<FeishuGatewaySettings, String> {
    Ok(FeishuGatewaySettings {
        app_id: get_app_setting(&db.0, "feishu_app_id")
            .await?
            .unwrap_or_default(),
        app_secret: get_app_setting(&db.0, "feishu_app_secret")
            .await?
            .unwrap_or_default(),
        ingress_token: get_app_setting(&db.0, "feishu_ingress_token")
            .await?
            .unwrap_or_default(),
        encrypt_key: get_app_setting(&db.0, "feishu_encrypt_key")
            .await?
            .unwrap_or_default(),
        sidecar_base_url: get_app_setting(&db.0, "feishu_sidecar_base_url")
            .await?
            .unwrap_or_else(|| "http://localhost:8765".to_string()),
    })
}

#[tauri::command]
pub async fn start_feishu_long_connection(
    sidecar_base_url: Option<String>,
    app_id: Option<String>,
    app_secret: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuWsStatus, String> {
    start_feishu_long_connection_with_pool(&db.0, sidecar_base_url, app_id, app_secret).await
}

pub async fn start_feishu_long_connection_with_pool(
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

pub async fn reconcile_feishu_employee_connections_with_pool(
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

pub async fn get_feishu_long_connection_status_summary_with_pool(
    pool: &SqlitePool,
    sidecar_base_url: Option<String>,
) -> Result<FeishuWsStatusSummary, String> {
    let base = resolve_feishu_sidecar_base_url(pool, sidecar_base_url).await?;
    let v = call_sidecar_json("/api/feishu/ws/status", serde_json::json!({}), base).await?;
    serde_json::from_value(v).map_err(|e| format!("parse ws summary status failed: {}", e))
}

#[tauri::command]
pub async fn stop_feishu_long_connection(
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuWsStatus, String> {
    let base = resolve_feishu_sidecar_base_url(&db.0, sidecar_base_url).await?;
    let v = call_sidecar_json("/api/feishu/ws/stop", serde_json::json!({}), base).await?;
    serde_json::from_value(v).map_err(|e| format!("parse ws status failed: {}", e))
}

#[tauri::command]
pub async fn get_feishu_long_connection_status(
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuWsStatus, String> {
    let base = resolve_feishu_sidecar_base_url(&db.0, sidecar_base_url).await?;
    let v = call_sidecar_json("/api/feishu/ws/status", serde_json::json!({}), base).await?;
    serde_json::from_value(v).map_err(|e| format!("parse ws status failed: {}", e))
}

#[tauri::command]
pub async fn get_feishu_employee_connection_statuses(
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
    relay: State<'_, FeishuEventRelayState>,
) -> Result<FeishuEmployeeConnectionStatuses, String> {
    let sidecar =
        get_feishu_long_connection_status_summary_with_pool(&db.0, sidecar_base_url).await?;
    Ok(FeishuEmployeeConnectionStatuses {
        relay: feishu_event_relay_status(relay.inner()),
        sidecar,
    })
}

#[tauri::command]
pub async fn sync_feishu_ws_events(
    sidecar_base_url: Option<String>,
    limit: Option<usize>,
    app: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<usize, String> {
    sync_feishu_ws_events_core(&db.0, sidecar_base_url, limit, Some(&app)).await
}

pub async fn sync_feishu_ws_events_with_pool(
    pool: &SqlitePool,
    sidecar_base_url: Option<String>,
    limit: Option<usize>,
) -> Result<usize, String> {
    sync_feishu_ws_events_core(pool, sidecar_base_url, limit, None).await
}

async fn sync_feishu_ws_events_core(
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
            // WS 多机器人场景下：仅在消息明确 @ 某个员工时才定向路由。
            // 无 @ 时交给默认主员工，避免被“连接所属员工”错误抢占。
            role_id: resolve_ws_role_id(
                &role_candidates,
                Some(&e.text),
                &source_employee_ids,
                &enabled_employees,
            ),
            account_id: if e.sender_open_id.trim().is_empty() {
                None
            } else {
                Some(e.sender_open_id.clone())
            },
            tenant_id: if e.sender_open_id.trim().is_empty() {
                None
            } else {
                Some(e.sender_open_id.clone())
            },
        };
        let r = process_im_event(pool, inbound.clone()).await?;
        if r.accepted && !r.deduped {
            if let Ok(employee_sessions) =
                ensure_employee_sessions_for_event_with_pool(pool, &inbound).await
            {
                let route_decision = resolve_openclaw_route_with_pool(pool, &inbound).await.ok();
                for s in employee_sessions {
                    let _ = link_inbound_event_to_session_with_pool(
                        pool,
                        &inbound,
                        &s.employee_id,
                        &s.session_id,
                    )
                    .await;
                    if let Some(app) = app {
                        let route_agent_id = route_decision
                            .as_ref()
                            .and_then(|v| v.get("agentId"))
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or(&s.role_id)
                            .to_string();
                        let route_session_key = route_decision
                            .as_ref()
                            .and_then(|v| v.get("sessionKey"))
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or(&s.session_id)
                            .to_string();
                        let matched_by = route_decision
                            .as_ref()
                            .and_then(|v| v.get("matchedBy"))
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("default")
                            .to_string();
                        let _ = app.emit(
                            "im-route-decision",
                            ImRouteDecisionEvent {
                                session_id: s.session_id.clone(),
                                thread_id: inbound.thread_id.clone(),
                                agent_id: route_agent_id,
                                session_key: route_session_key,
                                matched_by,
                            },
                        );
                        let _ = app.emit(
                            "im-role-event",
                            build_im_role_event_payload(
                                &s.session_id,
                                &inbound.thread_id,
                                &s.role_id,
                                &s.employee_name,
                                "running",
                                "飞书消息已同步到桌面会话，正在执行",
                                None,
                            ),
                        );
                        let _ = app.emit(
                            "im-role-dispatch-request",
                            build_im_role_dispatch_request(
                                &s.session_id,
                                &inbound.thread_id,
                                &s.role_id,
                                &s.employee_name,
                                &inbound
                                    .text
                                    .clone()
                                    .unwrap_or_else(|| "请继续基于当前上下文推进".to_string()),
                                "general-purpose",
                            ),
                        );
                    }
                }
            }
            accepted += 1;
        }
    }
    Ok(accepted)
}

#[tauri::command]
pub async fn start_feishu_event_relay(
    sidecar_base_url: Option<String>,
    interval_ms: Option<u64>,
    limit: Option<usize>,
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    relay: State<'_, FeishuEventRelayState>,
) -> Result<FeishuEventRelayStatus, String> {
    start_feishu_event_relay_with_pool_and_app(
        &db.0,
        relay.inner().clone(),
        Some(app),
        sidecar_base_url,
        interval_ms,
        limit,
    )
    .await
}

pub async fn start_feishu_event_relay_with_pool(
    pool: &SqlitePool,
    relay_state: FeishuEventRelayState,
    sidecar_base_url: Option<String>,
    interval_ms: Option<u64>,
    limit: Option<usize>,
) -> Result<FeishuEventRelayStatus, String> {
    start_feishu_event_relay_with_pool_and_app(
        pool,
        relay_state,
        None,
        sidecar_base_url,
        interval_ms,
        limit,
    )
    .await
}

pub async fn start_feishu_event_relay_with_pool_and_app(
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

#[tauri::command]
pub async fn stop_feishu_event_relay(
    relay: State<'_, FeishuEventRelayState>,
) -> Result<FeishuEventRelayStatus, String> {
    let relay_state = relay.inner().clone();
    relay_state.generation.fetch_add(1, Ordering::SeqCst);
    relay_state.running.store(false, Ordering::SeqCst);
    Ok(feishu_event_relay_status(relay.inner()))
}

#[tauri::command]
pub async fn get_feishu_event_relay_status(
    relay: State<'_, FeishuEventRelayState>,
) -> Result<FeishuEventRelayStatus, String> {
    Ok(feishu_event_relay_status(relay.inner()))
}

#[cfg(test)]
mod tests {
    use super::{
        parse_feishu_payload, resolve_ws_role_id, sanitize_ws_inbound_text, FeishuWsEventRecord,
        ParsedFeishuPayload,
    };
    use crate::commands::employee_agents::AgentEmployee;

    #[test]
    fn parse_feishu_payload_extracts_mention_role_and_cleans_text() {
        let payload = serde_json::json!({
            "header": {
                "event_id": "evt_1",
                "event_type": "im.message.receive_v1",
                "tenant_key": "tenant_1"
            },
            "event": {
                "message": {
                    "message_id": "om_1",
                    "chat_id": "oc_1",
                    "content": "{\"text\":\"@_user_1 你细化一下技术方案\"}"
                },
                "sender": {
                    "sender_id": {
                        "open_id": "ou_sender"
                    }
                },
                "mentions": [
                    {
                        "key": "@_user_1",
                        "id": {
                            "open_id": "ou_dev_agent"
                        },
                        "name": "开发团队"
                    }
                ]
            }
        });

        let parsed = parse_feishu_payload(&payload.to_string()).expect("payload should parse");
        match parsed {
            ParsedFeishuPayload::Event(event) => {
                assert_eq!(event.thread_id, "oc_1");
                assert_eq!(event.role_id.as_deref(), Some("ou_dev_agent"));
                assert_eq!(event.text.as_deref(), Some("你细化一下技术方案"));
                assert_eq!(event.tenant_id.as_deref(), Some("tenant_1"));
            }
            ParsedFeishuPayload::Challenge(_) => panic!("should parse as event"),
        }
    }

    #[test]
    fn parse_feishu_payload_keeps_plain_text_when_no_mentions() {
        let payload = serde_json::json!({
            "header": {
                "event_id": "evt_2",
                "event_type": "im.message.receive_v1"
            },
            "event": {
                "message": {
                    "message_id": "om_2",
                    "chat_id": "oc_2",
                    "content": "{\"text\":\"请给出实施方案\"}"
                }
            }
        });

        let parsed = parse_feishu_payload(&payload.to_string()).expect("payload should parse");
        match parsed {
            ParsedFeishuPayload::Event(event) => {
                assert_eq!(event.role_id, None);
                assert_eq!(event.text.as_deref(), Some("请给出实施方案"));
            }
            ParsedFeishuPayload::Challenge(_) => panic!("should parse as event"),
        }
    }

    #[test]
    fn sanitize_ws_inbound_text_strips_placeholder_tokens() {
        let cleaned = sanitize_ws_inbound_text("@_user_1  你细化一下技术方案");
        assert_eq!(cleaned.as_deref(), Some("你细化一下技术方案"));
    }

    #[test]
    fn resolve_ws_role_id_prefers_candidate_matching_employee() {
        let employees = vec![
            AgentEmployee {
                id: "1".to_string(),
                employee_id: "project_manager".to_string(),
                name: "项目经理".to_string(),
                role_id: "project_manager".to_string(),
                persona: String::new(),
                feishu_open_id: "ou_pm".to_string(),
                feishu_app_id: String::new(),
                feishu_app_secret: String::new(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: String::new(),
                openclaw_agent_id: "project_manager".to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["feishu".to_string()],
                enabled: true,
                is_default: true,
                skill_ids: Vec::new(),
                created_at: "2026-03-05T00:00:00Z".to_string(),
                updated_at: "2026-03-05T00:00:00Z".to_string(),
            },
            AgentEmployee {
                id: "2".to_string(),
                employee_id: "dev_team".to_string(),
                name: "开发团队".to_string(),
                role_id: "dev_team".to_string(),
                persona: String::new(),
                feishu_open_id: "ou_dev_team".to_string(),
                feishu_app_id: String::new(),
                feishu_app_secret: String::new(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: String::new(),
                openclaw_agent_id: "dev_team".to_string(),
                routing_priority: 90,
                enabled_scopes: vec!["feishu".to_string()],
                enabled: true,
                is_default: false,
                skill_ids: Vec::new(),
                created_at: "2026-03-05T00:00:00Z".to_string(),
                updated_at: "2026-03-05T00:00:00Z".to_string(),
            },
        ];
        let event = FeishuWsEventRecord {
            employee_id: "project_manager".to_string(),
            source_employee_ids: vec!["project_manager".to_string(), "dev_team".to_string()],
            id: "oc_chat:om_1".to_string(),
            event_type: "im.message.receive_v1".to_string(),
            chat_id: "oc_chat".to_string(),
            message_id: "om_1".to_string(),
            text: "你细化一下技术方案".to_string(),
            mention_open_id: "ou_sender".to_string(),
            mention_open_ids: vec!["ou_sender".to_string(), "ou_dev_team".to_string()],
            sender_open_id: "ou_sender".to_string(),
            received_at: "2026-03-05T00:00:00Z".to_string(),
        };

        let selected = resolve_ws_role_id(
            &event.mention_open_ids,
            Some(&event.text),
            &event.source_employee_ids,
            &employees,
        );
        assert_eq!(selected.as_deref(), Some("ou_dev_team"));
    }

    #[test]
    fn resolve_ws_role_id_falls_back_to_single_source_employee() {
        let employees = vec![
            AgentEmployee {
                id: "1".to_string(),
                employee_id: "project_manager".to_string(),
                name: "项目经理".to_string(),
                role_id: "project_manager".to_string(),
                persona: String::new(),
                feishu_open_id: String::new(),
                feishu_app_id: String::new(),
                feishu_app_secret: String::new(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: String::new(),
                openclaw_agent_id: "project_manager".to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["feishu".to_string()],
                enabled: true,
                is_default: true,
                skill_ids: Vec::new(),
                created_at: "2026-03-05T00:00:00Z".to_string(),
                updated_at: "2026-03-05T00:00:00Z".to_string(),
            },
            AgentEmployee {
                id: "2".to_string(),
                employee_id: "tech_lead".to_string(),
                name: "开发人员".to_string(),
                role_id: "tech_lead".to_string(),
                persona: String::new(),
                feishu_open_id: String::new(),
                feishu_app_id: String::new(),
                feishu_app_secret: String::new(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: String::new(),
                openclaw_agent_id: "tech_lead".to_string(),
                routing_priority: 90,
                enabled_scopes: vec!["feishu".to_string()],
                enabled: true,
                is_default: false,
                skill_ids: Vec::new(),
                created_at: "2026-03-05T00:00:00Z".to_string(),
                updated_at: "2026-03-05T00:00:00Z".to_string(),
            },
        ];

        let selected = resolve_ws_role_id(
            &[],
            Some("请你继续处理"),
            &["tech_lead".to_string()],
            &employees,
        );
        assert_eq!(selected.as_deref(), Some("tech_lead"));
    }
}
