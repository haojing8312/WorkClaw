use crate::approval_bus::{
    ApprovalDecision, ApprovalManager, ApprovalResolveResult, PendingApprovalRecord,
};
use crate::commands::approvals::load_approval_record_with_pool;
use crate::commands::chat::ApprovalManagerState;
use crate::commands::employee_agents::{
    ensure_employee_sessions_for_event_with_pool, link_inbound_event_to_session_with_pool,
    list_agent_employees_with_pool, AgentEmployee,
};
use crate::commands::im_config::get_thread_role_config_with_pool;
use crate::commands::im_gateway::{process_im_event, FeishuCallbackResult};
use crate::commands::openclaw_gateway::resolve_openclaw_route_with_pool;
use crate::commands::openclaw_plugins::get_openclaw_plugin_feishu_channel_snapshot_with_pool;
use crate::commands::openclaw_plugins::{
    send_openclaw_plugin_feishu_runtime_outbound_message_in_state,
    OpenClawPluginChannelAccountSnapshot, OpenClawPluginChannelSnapshotResult,
    OpenClawPluginFeishuOutboundSendRequest, OpenClawPluginFeishuRuntimeState,
};
use crate::commands::skills::DbState;
use crate::diagnostics::{self, ManagedDiagnosticsState};
use crate::im::feishu_adapter::build_feishu_markdown_message;
use crate::im::feishu_formatter::format_role_message;
use crate::im::runtime_bridge::{
    build_im_role_dispatch_request_for_channel, build_im_role_event_payload_for_channel,
    ImRoleDispatchRequest, ImRoleEventPayload,
};
use crate::im::types::{ImEvent, ImEventType};
use reqwest::Client;
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Emitter;
use tauri::Manager;
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
enum FeishuInboundGateDecision {
    Allow,
    Reject { reason: &'static str },
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct FeishuGateAccountConfig {
    dm_policy: Option<String>,
    group_policy: Option<String>,
    require_mention: Option<bool>,
    allow_from: Vec<String>,
    group_allow_from: Vec<String>,
    groups: std::collections::HashMap<String, FeishuGateGroupConfig>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct FeishuGateGroupConfig {
    enabled: Option<bool>,
    group_policy: Option<String>,
    require_mention: Option<bool>,
    allow_from: Vec<String>,
}

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, FromRow)]
pub struct FeishuPairingRequestRecord {
    pub id: String,
    pub channel: String,
    pub account_id: String,
    pub sender_id: String,
    pub chat_id: String,
    pub code: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub resolved_at: Option<String>,
    pub resolved_by_user: String,
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
    chat_type: Option<String>,
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
        sender_id: event
            .sender
            .as_ref()
            .and_then(|sender| sender.sender_id.as_ref())
            .and_then(|id| id.open_id.clone()),
        chat_type: message.chat_type,
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

fn normalize_optional_non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
}

fn resolve_fallback_default_feishu_account_id(
    has_default_credentials: bool,
    employee_account_ids: &[String],
) -> Option<String> {
    if has_default_credentials {
        return Some("default".to_string());
    }

    employee_account_ids
        .iter()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .map(str::to_string)
}

async fn resolve_default_feishu_account_id_with_pool(
    pool: &SqlitePool,
) -> Result<Option<String>, String> {
    if let Ok(snapshot) =
        get_openclaw_plugin_feishu_channel_snapshot_with_pool(pool, "openclaw-lark").await
    {
        if let Some(default_account_id) =
            normalize_optional_non_empty(snapshot.snapshot.default_account_id)
        {
            return Ok(Some(default_account_id));
        }
        let fallback = snapshot
            .snapshot
            .account_ids
            .into_iter()
            .map(|value| value.trim().to_string())
            .find(|value| !value.is_empty());
        if fallback.is_some() {
            return Ok(fallback);
        }
    }

    let app_id = get_app_setting(pool, "feishu_app_id")
        .await?
        .unwrap_or_default();
    let app_secret = get_app_setting(pool, "feishu_app_secret")
        .await?
        .unwrap_or_default();
    let employee_account_ids = list_enabled_employee_feishu_connections_with_pool(pool)
        .await?
        .into_iter()
        .map(|item| item.employee_id)
        .collect::<Vec<_>>();

    Ok(resolve_fallback_default_feishu_account_id(
        !app_id.trim().is_empty() && !app_secret.trim().is_empty(),
        &employee_account_ids,
    ))
}

fn apply_default_feishu_account_id(event: &mut ImEvent, default_account_id: Option<&str>) {
    let already_has_account = event
        .account_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    if already_has_account {
        return;
    }

    if let Some(default_account_id) = default_account_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        event.account_id = Some(default_account_id.to_string());
    }
}

fn is_direct_feishu_chat(event: &ImEvent) -> bool {
    matches!(
        event.chat_type.as_deref().map(str::trim),
        Some("p2p") | Some("direct")
    )
}

fn normalize_allow_entries(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect()
}

fn allowlist_matches_sender(sender_id: Option<&str>, allow_from: &[String]) -> bool {
    let normalized_allow_from = normalize_allow_entries(allow_from);
    if normalized_allow_from.iter().any(|entry| entry == "*") {
        return true;
    }

    let Some(sender_id) = sender_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    let normalized_sender = sender_id.to_ascii_lowercase();
    normalized_allow_from
        .iter()
        .any(|entry| entry == &normalized_sender)
}

fn split_legacy_group_allow_from(raw_group_allow_from: &[String]) -> (Vec<String>, Vec<String>) {
    let mut legacy_chat_ids = Vec::new();
    let mut sender_allow_from = Vec::new();
    for entry in raw_group_allow_from {
        let normalized = entry.trim().to_string();
        if normalized.is_empty() {
            continue;
        }
        if normalized.starts_with("oc_") {
            legacy_chat_ids.push(normalized);
        } else {
            sender_allow_from.push(normalized);
        }
    }
    (legacy_chat_ids, sender_allow_from)
}

fn resolve_feishu_group_config<'a>(
    groups: &'a std::collections::HashMap<String, FeishuGateGroupConfig>,
    group_id: &str,
) -> Option<&'a FeishuGateGroupConfig> {
    if let Some(exact) = groups.get(group_id) {
        return Some(exact);
    }
    let lowered = group_id.to_ascii_lowercase();
    groups
        .iter()
        .find(|(key, _)| key.to_ascii_lowercase() == lowered)
        .map(|(_, value)| value)
}

fn build_feishu_gate_account_config(
    account_snapshot: &OpenClawPluginChannelAccountSnapshot,
) -> FeishuGateAccountConfig {
    account_snapshot
        .account
        .get("config")
        .cloned()
        .and_then(|value| serde_json::from_value::<FeishuGateAccountConfig>(value).ok())
        .unwrap_or_default()
}

fn select_feishu_channel_account_snapshot<'a>(
    snapshot: &'a OpenClawPluginChannelSnapshotResult,
    event: &ImEvent,
) -> Option<&'a OpenClawPluginChannelAccountSnapshot> {
    let normalized_event_account = event
        .account_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);
    let default_account = snapshot
        .snapshot
        .default_account_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);

    if let Some(event_account_id) = normalized_event_account.as_deref() {
        if let Some(found) = snapshot.snapshot.accounts.iter().find(|account| {
            account
                .account_id
                .trim()
                .eq_ignore_ascii_case(event_account_id)
        }) {
            return Some(found);
        }
    }

    if let Some(default_account_id) = default_account.as_deref() {
        if let Some(found) = snapshot.snapshot.accounts.iter().find(|account| {
            account
                .account_id
                .trim()
                .eq_ignore_ascii_case(default_account_id)
        }) {
            return Some(found);
        }
    }

    snapshot.snapshot.accounts.first()
}

fn resolve_feishu_pairing_account_id(
    event: &ImEvent,
    snapshot: Option<&OpenClawPluginChannelSnapshotResult>,
) -> String {
    if let Some(account_id) = snapshot
        .and_then(|value| select_feishu_channel_account_snapshot(value, event))
        .map(|account| account.account_id.trim())
        .filter(|value| !value.is_empty())
    {
        return account_id.to_string();
    }

    event
        .account_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("default")
        .to_string()
}

fn evaluate_openclaw_feishu_gate(
    event: &ImEvent,
    snapshot: &OpenClawPluginChannelSnapshotResult,
) -> FeishuInboundGateDecision {
    let Some(account_snapshot) = select_feishu_channel_account_snapshot(snapshot, event) else {
        return FeishuInboundGateDecision::Allow;
    };
    let account_config = build_feishu_gate_account_config(account_snapshot);
    let sender_id = event.sender_id.as_deref();

    if is_direct_feishu_chat(event) {
        let dm_policy = account_config.dm_policy.as_deref().unwrap_or("pairing");
        return match dm_policy {
            "disabled" => FeishuInboundGateDecision::Reject {
                reason: "dm_disabled",
            },
            "open" => FeishuInboundGateDecision::Allow,
            "allowlist" => {
                if allowlist_matches_sender(sender_id, &account_snapshot.allow_from) {
                    FeishuInboundGateDecision::Allow
                } else {
                    FeishuInboundGateDecision::Reject {
                        reason: "dm_not_allowed",
                    }
                }
            }
            _ => {
                if allowlist_matches_sender(sender_id, &account_snapshot.allow_from) {
                    FeishuInboundGateDecision::Allow
                } else {
                    FeishuInboundGateDecision::Reject {
                        reason: "pairing_pending",
                    }
                }
            }
        };
    }

    let group_id = event.thread_id.trim();
    if group_id.is_empty() {
        return FeishuInboundGateDecision::Allow;
    }

    let groups = &account_config.groups;
    let group_config = resolve_feishu_group_config(groups, group_id);
    let default_group_config = groups.get("*");
    let (legacy_chat_ids, sender_group_allow_from) =
        split_legacy_group_allow_from(&account_config.group_allow_from);

    let groups_configured = groups.keys().any(|key| key.trim() != "*");
    let group_level_policy =
        account_config
            .group_policy
            .as_deref()
            .unwrap_or(if groups_configured {
                "allowlist"
            } else {
                "open"
            });

    let group_allowed = match group_level_policy {
        "disabled" => false,
        "open" if !groups_configured => true,
        _ => {
            group_config.is_some()
                || default_group_config.is_some()
                || legacy_chat_ids.iter().any(|chat_id| chat_id == group_id)
        }
    };
    if !group_allowed {
        return FeishuInboundGateDecision::Reject {
            reason: "group_not_allowed",
        };
    }

    if group_config.and_then(|item| item.enabled) == Some(false) {
        return FeishuInboundGateDecision::Reject {
            reason: "group_disabled",
        };
    }

    let sender_policy = group_config
        .and_then(|item| item.group_policy.as_deref())
        .or_else(|| default_group_config.and_then(|item| item.group_policy.as_deref()))
        .or(account_config.group_policy.as_deref())
        .unwrap_or("open");
    let mut sender_allow_from = sender_group_allow_from;
    if let Some(group_config) = group_config {
        sender_allow_from.extend(group_config.allow_from.iter().cloned());
    } else if let Some(default_group_config) = default_group_config {
        sender_allow_from.extend(default_group_config.allow_from.iter().cloned());
    }

    let sender_allowed = match sender_policy {
        "disabled" => false,
        "open" => true,
        _ => allowlist_matches_sender(sender_id, &sender_allow_from),
    };
    if !sender_allowed {
        return FeishuInboundGateDecision::Reject {
            reason: "sender_not_allowed",
        };
    }

    let require_mention = group_config
        .and_then(|item| item.require_mention)
        .or_else(|| default_group_config.and_then(|item| item.require_mention))
        .or(account_config.require_mention)
        .unwrap_or(true);
    if require_mention
        && event
            .role_id
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
    {
        return FeishuInboundGateDecision::Reject {
            reason: "no_mention",
        };
    }

    FeishuInboundGateDecision::Allow
}

async fn evaluate_openclaw_feishu_gate_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<FeishuInboundGateDecision, String> {
    let mut snapshot =
        match get_openclaw_plugin_feishu_channel_snapshot_with_pool(pool, "openclaw-lark").await {
            Ok(snapshot) => snapshot,
            Err(_) => return Ok(FeishuInboundGateDecision::Allow),
        };
    if let Some(account_snapshot) = select_feishu_channel_account_snapshot(&snapshot, event) {
        let pairing_allow_from =
            list_feishu_pairing_allow_from_with_pool(pool, &account_snapshot.account_id).await?;
        if !pairing_allow_from.is_empty() {
            let target_account_id = account_snapshot.account_id.clone();
            if let Some(account) = snapshot
                .snapshot
                .accounts
                .iter_mut()
                .find(|account| account.account_id == target_account_id)
            {
                for sender_id in pairing_allow_from {
                    if !account.allow_from.iter().any(|entry| entry == &sender_id) {
                        account.allow_from.push(sender_id);
                    }
                }
            }
        }
    }
    Ok(evaluate_openclaw_feishu_gate(event, &snapshot))
}

fn generate_feishu_pairing_code() -> String {
    Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect::<String>()
        .to_ascii_uppercase()
}

fn normalize_explicit_feishu_pairing_code(code: Option<&str>) -> Option<String> {
    let normalized = code
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| !value.eq_ignore_ascii_case("PAIRING"))?;
    Some(normalized.to_ascii_uppercase())
}

pub(crate) async fn upsert_feishu_pairing_request_with_pool(
    pool: &SqlitePool,
    account_id: &str,
    sender_id: &str,
    chat_id: &str,
    explicit_code: Option<&str>,
) -> Result<(FeishuPairingRequestRecord, bool), String> {
    let normalized_account_id = account_id.trim();
    let normalized_sender_id = sender_id.trim();
    let normalized_chat_id = chat_id.trim();
    let normalized_explicit_code = normalize_explicit_feishu_pairing_code(explicit_code);
    if normalized_account_id.is_empty() || normalized_sender_id.is_empty() {
        return Err("pairing request requires account_id and sender_id".to_string());
    }

    if let Some(existing) = sqlx::query_as::<_, FeishuPairingRequestRecord>(
        "SELECT id, channel, account_id, sender_id, chat_id, code, status, created_at, updated_at, resolved_at, resolved_by_user
         FROM feishu_pairing_requests
         WHERE channel = 'feishu' AND account_id = ? AND sender_id = ? AND status = 'pending'
         LIMIT 1",
    )
    .bind(normalized_account_id)
    .bind(normalized_sender_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    {
        let now = chrono::Utc::now().to_rfc3339();
        let next_chat_id = if normalized_chat_id.is_empty() {
            existing.chat_id.clone()
        } else {
            normalized_chat_id.to_string()
        };
        let next_code = normalized_explicit_code
            .clone()
            .unwrap_or_else(|| existing.code.clone());
        sqlx::query(
            "UPDATE feishu_pairing_requests
             SET chat_id = ?, code = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&next_chat_id)
        .bind(&next_code)
        .bind(&now)
        .bind(&existing.id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        return Ok((
            FeishuPairingRequestRecord {
                chat_id: next_chat_id,
                code: next_code,
                updated_at: now,
                ..existing
            },
            false,
        ));
    }

    let id = Uuid::new_v4().to_string();
    let code = normalized_explicit_code.unwrap_or_else(generate_feishu_pairing_code);
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO feishu_pairing_requests (
            id, channel, account_id, sender_id, chat_id, code, status, created_at, updated_at, resolved_at, resolved_by_user
        ) VALUES (?, 'feishu', ?, ?, ?, ?, 'pending', ?, ?, NULL, '')",
    )
    .bind(&id)
    .bind(normalized_account_id)
    .bind(normalized_sender_id)
    .bind(normalized_chat_id)
    .bind(&code)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok((
        FeishuPairingRequestRecord {
            id,
            channel: "feishu".to_string(),
            account_id: normalized_account_id.to_string(),
            sender_id: normalized_sender_id.to_string(),
            chat_id: normalized_chat_id.to_string(),
            code,
            status: "pending".to_string(),
            created_at: now.clone(),
            updated_at: now,
            resolved_at: None,
            resolved_by_user: String::new(),
        },
        true,
    ))
}

pub(crate) async fn list_feishu_pairing_allow_from_with_pool(
    pool: &SqlitePool,
    account_id: &str,
) -> Result<Vec<String>, String> {
    let normalized_account_id = account_id.trim();
    if normalized_account_id.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT sender_id
         FROM feishu_pairing_allow_from
         WHERE channel = 'feishu' AND account_id = ?
         ORDER BY approved_at DESC",
    )
    .bind(normalized_account_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(|(sender_id,)| sender_id).collect())
}

fn build_feishu_pairing_request_text(record: &FeishuPairingRequestRecord) -> String {
    format!(
        "已收到你的配对申请。\n配对码：{code}\n发送者：{sender}\n请在 WorkClaw 桌面端审核通过后继续私聊本机器人。",
        code = record.code,
        sender = record.sender_id
    )
}

fn build_feishu_pairing_resolution_text(record: &FeishuPairingRequestRecord) -> String {
    match record.status.as_str() {
        "approved" => format!(
            "配对已通过。你现在可以直接私聊本机器人。\n配对码：{code}",
            code = record.code
        ),
        "denied" => format!(
            "配对申请未通过。\n配对码：{code}\n如需继续使用，请联系管理员后重新发起配对。",
            code = record.code
        ),
        _ => format!("配对请求状态已更新：{}", record.status),
    }
}

async fn maybe_create_feishu_pairing_request_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Option<FeishuPairingRequestRecord>, String> {
    if !is_direct_feishu_chat(event) {
        return Ok(None);
    }
    let snapshot = get_openclaw_plugin_feishu_channel_snapshot_with_pool(pool, "openclaw-lark")
        .await
        .ok();
    let account_id = resolve_feishu_pairing_account_id(event, snapshot.as_ref());
    let Some(sender_id) = event
        .sender_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    let (record, created) = upsert_feishu_pairing_request_with_pool(
        pool,
        &account_id,
        sender_id,
        &event.thread_id,
        None,
    )
    .await?;
    if created && !record.chat_id.trim().is_empty() {
        let _ = send_feishu_text_message_with_pool(
            pool,
            &record.chat_id,
            &build_feishu_pairing_request_text(&record),
            None,
        )
        .await;
    }
    Ok(Some(record))
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
    let feishu_specific = get_app_setting(pool, "feishu_sidecar_base_url").await?;
    if feishu_specific
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
    {
        return Ok(feishu_specific);
    }
    Ok(get_app_setting(pool, "im_sidecar_base_url").await?)
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

pub(crate) async fn list_enabled_employee_feishu_connections_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<FeishuEmployeeConnectionInput>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT employee_id, role_id, name, feishu_app_id, feishu_app_secret
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
    for (employee_id_raw, role_id, name, app_id, app_secret) in rows {
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
            name: name.trim().to_string(),
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
            build_im_role_event_payload_for_channel(
                &session_id,
                &event.thread_id,
                &role_id,
                &role_id,
                "feishu",
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
            let mut req = build_im_role_dispatch_request_for_channel(
                &session_id,
                &event.thread_id,
                &role_id,
                &role_id,
                "feishu",
                &format!("场景={}。用户输入：{}", cfg.scenario_template, user_text),
                agent_type,
            );
            req.message_id = event.message_id.clone().unwrap_or_default();
            req
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FeishuApprovalCommand {
    approval_id: String,
    decision: ApprovalDecision,
}

#[derive(Debug, Clone, FromRow)]
struct ApprovalResolutionNotificationRow {
    id: String,
    session_id: String,
    summary: String,
    status: String,
    decision: String,
    resolved_by_surface: String,
    resolved_by_user: String,
}

fn parse_feishu_approval_command(text: Option<&str>) -> Option<FeishuApprovalCommand> {
    let raw = text?.trim();
    if raw.is_empty() {
        return None;
    }

    let parts = raw.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 2 || !parts[0].eq_ignore_ascii_case("/approve") {
        return None;
    }

    let approval_id = parts[1].trim();
    if approval_id.is_empty() {
        return None;
    }

    let decision = match parts
        .get(2)
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        None | Some("") | Some("allow_once") | Some("allow-once") | Some("approve") => {
            ApprovalDecision::AllowOnce
        }
        Some("allow_always") | Some("allow-always") => ApprovalDecision::AllowAlways,
        Some("deny") | Some("reject") => ApprovalDecision::Deny,
        Some(_) => return None,
    };

    Some(FeishuApprovalCommand {
        approval_id: approval_id.to_string(),
        decision,
    })
}

pub async fn send_feishu_text_message_with_pool(
    pool: &SqlitePool,
    chat_id: &str,
    text: &str,
    _sidecar_base_url: Option<String>,
) -> Result<String, String> {
    let runtime_state = resolve_registered_feishu_runtime_state_for_outbound()?;
    send_feishu_text_message_via_official_runtime_with_pool(
        pool,
        &runtime_state,
        chat_id,
        text,
        None,
    )
    .await
}

async fn lookup_feishu_thread_for_session_with_pool(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<String>, String> {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT ts.thread_id
         FROM im_thread_sessions ts
         WHERE ts.session_id = ?
           AND EXISTS (
             SELECT 1
             FROM im_inbox_events e
             WHERE e.thread_id = ts.thread_id AND e.source = 'feishu'
           )
         ORDER BY ts.updated_at DESC, ts.created_at DESC
         LIMIT 1",
    )
    .bind(session_id.trim())
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("查询飞书线程映射失败: {e}"))?;

    Ok(row.map(|(thread_id,)| thread_id))
}

fn build_feishu_approval_request_text(record: &PendingApprovalRecord) -> String {
    let impact = record
        .impact
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("此操作属于高风险动作，请确认后继续。");
    let irreversible = if record.irreversible {
        "不可逆"
    } else {
        "可恢复性未知"
    };

    format!(
        "待审批 #{approval_id}\n工具：{tool_name}\n摘要：{summary}\n影响：{impact}\n风险：{irreversible}\n回复命令：/approve {approval_id} allow_once | allow_always | deny",
        approval_id = record.approval_id,
        tool_name = record.tool_name,
        summary = record.summary,
        impact = impact,
        irreversible = irreversible,
    )
}

fn build_feishu_approval_resolution_text(
    approval_id: &str,
    result: &ApprovalResolveResult,
    summary: Option<&str>,
) -> String {
    match result {
        ApprovalResolveResult::Applied {
            status, decision, ..
        } => {
            let action = match decision {
                ApprovalDecision::AllowOnce => "allow_once",
                ApprovalDecision::AllowAlways => "allow_always",
                ApprovalDecision::Deny => "deny",
            };
            let suffix = if *decision == ApprovalDecision::Deny {
                "本次操作已取消。"
            } else {
                "任务将继续执行。"
            };
            format!(
                "审批 {approval_id} 已处理：{status}（{action}）。{summary_line}{suffix}",
                approval_id = approval_id,
                status = status,
                action = action,
                summary_line = summary
                    .map(|value| format!("摘要：{}。", value.trim()))
                    .unwrap_or_default(),
                suffix = suffix,
            )
        }
        ApprovalResolveResult::AlreadyResolved {
            status, decision, ..
        } => {
            let decision_label = decision
                .as_ref()
                .map(|value| match value {
                    ApprovalDecision::AllowOnce => "allow_once",
                    ApprovalDecision::AllowAlways => "allow_always",
                    ApprovalDecision::Deny => "deny",
                })
                .unwrap_or("unknown");
            format!(
                "审批 {approval_id} 已被处理，当前状态：{status}（{decision_label}）。",
                approval_id = approval_id,
                status = status,
                decision_label = decision_label,
            )
        }
        ApprovalResolveResult::NotFound { .. } => {
            format!(
                "未找到待审批项 {approval_id}，请确认审批编号是否正确。",
                approval_id = approval_id,
            )
        }
    }
}

fn feishu_runtime_outbound_state_slot() -> &'static Mutex<Option<OpenClawPluginFeishuRuntimeState>>
{
    static SLOT: OnceLock<Mutex<Option<OpenClawPluginFeishuRuntimeState>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

pub fn remember_feishu_runtime_state_for_outbound(
    runtime_state: &OpenClawPluginFeishuRuntimeState,
) {
    if let Ok(mut guard) = feishu_runtime_outbound_state_slot().lock() {
        *guard = Some(runtime_state.clone());
    }
}

pub fn clear_feishu_runtime_state_for_outbound() {
    if let Ok(mut guard) = feishu_runtime_outbound_state_slot().lock() {
        *guard = None;
    }
}

fn resolve_registered_feishu_runtime_state_for_outbound(
) -> Result<OpenClawPluginFeishuRuntimeState, String> {
    feishu_runtime_outbound_state_slot()
        .lock()
        .map_err(|_| "failed to lock feishu runtime registration".to_string())?
        .clone()
        .ok_or_else(|| "official feishu runtime is not registered for outbound sends".to_string())
}

async fn send_feishu_text_message_via_official_runtime_with_pool(
    pool: &SqlitePool,
    runtime_state: &OpenClawPluginFeishuRuntimeState,
    chat_id: &str,
    text: &str,
    account_id: Option<String>,
) -> Result<String, String> {
    let resolved_account_id = match account_id {
        Some(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                resolve_default_feishu_account_id_with_pool(pool)
                    .await?
                    .unwrap_or_else(|| "default".to_string())
            } else {
                trimmed
            }
        }
        None => resolve_default_feishu_account_id_with_pool(pool)
            .await?
            .unwrap_or_else(|| "default".to_string()),
    };

    let result = send_openclaw_plugin_feishu_runtime_outbound_message_in_state(
        runtime_state,
        OpenClawPluginFeishuOutboundSendRequest {
            request_id: Uuid::new_v4().to_string(),
            account_id: resolved_account_id,
            target: chat_id.trim().to_string(),
            thread_id: Some(chat_id.trim().to_string()),
            text: text.trim().to_string(),
            mode: "text".to_string(),
        },
    )?;

    serde_json::to_string(&result)
        .map_err(|error| format!("failed to serialize outbound send result: {error}"))
}

pub async fn notify_feishu_approval_requested_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    record: &PendingApprovalRecord,
    sidecar_base_url: Option<String>,
) -> Result<(), String> {
    let Some(thread_id) = lookup_feishu_thread_for_session_with_pool(pool, session_id).await?
    else {
        return Ok(());
    };

    send_feishu_text_message_with_pool(
        pool,
        &thread_id,
        &build_feishu_approval_request_text(record),
        sidecar_base_url,
    )
    .await?;
    Ok(())
}

pub(crate) async fn notify_feishu_approval_resolved_with_pool(
    pool: &SqlitePool,
    approval_id: &str,
    sidecar_base_url: Option<String>,
) -> Result<(), String> {
    let Some(row) = sqlx::query_as::<_, ApprovalResolutionNotificationRow>(
        "SELECT id, session_id, summary, status, decision, resolved_by_surface, resolved_by_user
         FROM approvals
         WHERE id = ?",
    )
    .bind(approval_id.trim())
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("读取审批结果通知数据失败: {e}"))?
    else {
        return Ok(());
    };

    let Some(thread_id) = lookup_feishu_thread_for_session_with_pool(pool, &row.session_id).await?
    else {
        return Ok(());
    };

    let decision = match row.decision.as_str() {
        "allow_once" => Some(ApprovalDecision::AllowOnce),
        "allow_always" => Some(ApprovalDecision::AllowAlways),
        "deny" => Some(ApprovalDecision::Deny),
        _ => None,
    };
    let result = ApprovalResolveResult::AlreadyResolved {
        approval_id: row.id.clone(),
        status: row.status.clone(),
        decision,
    };
    let resolved_by = if row.resolved_by_user.trim().is_empty() {
        row.resolved_by_surface.trim()
    } else {
        row.resolved_by_user.trim()
    };
    let text = format!(
        "{} 处理人：{}。",
        build_feishu_approval_resolution_text(&row.id, &result, Some(&row.summary)),
        if resolved_by.is_empty() {
            "unknown"
        } else {
            resolved_by
        }
    );
    send_feishu_text_message_with_pool(pool, &thread_id, &text, sidecar_base_url).await?;
    Ok(())
}

pub async fn maybe_handle_feishu_approval_command_with_pool(
    pool: &SqlitePool,
    approvals: &ApprovalManager,
    event: &ImEvent,
    sidecar_base_url: Option<String>,
) -> Result<Option<ApprovalResolveResult>, String> {
    let Some(command) = parse_feishu_approval_command(event.text.as_deref()) else {
        return Ok(None);
    };

    let resolved_by_user = event
        .account_id
        .as_deref()
        .or(event.tenant_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("feishu");

    let resolution = approvals
        .resolve_with_pool(
            pool,
            &command.approval_id,
            command.decision,
            "feishu",
            resolved_by_user,
        )
        .await?;

    let summary = load_approval_record_with_pool(pool, &command.approval_id)
        .await?
        .map(|record| record.summary);
    let message = build_feishu_approval_resolution_text(
        &command.approval_id,
        &resolution,
        summary.as_deref(),
    );
    send_feishu_text_message_with_pool(pool, &event.thread_id, &message, sidecar_base_url).await?;

    Ok(Some(resolution))
}

pub(crate) async fn list_feishu_pairing_requests_with_pool(
    pool: &SqlitePool,
    status: Option<String>,
) -> Result<Vec<FeishuPairingRequestRecord>, String> {
    let normalized_status = status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let records = if let Some(status) = normalized_status {
        sqlx::query_as::<_, FeishuPairingRequestRecord>(
            "SELECT id, channel, account_id, sender_id, chat_id, code, status, created_at, updated_at, resolved_at, resolved_by_user
             FROM feishu_pairing_requests
             WHERE channel = 'feishu' AND status = ?
             ORDER BY updated_at DESC, created_at DESC",
        )
        .bind(status)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as::<_, FeishuPairingRequestRecord>(
            "SELECT id, channel, account_id, sender_id, chat_id, code, status, created_at, updated_at, resolved_at, resolved_by_user
             FROM feishu_pairing_requests
             WHERE channel = 'feishu'
             ORDER BY updated_at DESC, created_at DESC",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?
    };

    Ok(records)
}

async fn resolve_feishu_pairing_request_with_pool(
    pool: &SqlitePool,
    request_id: &str,
    status: &str,
    resolved_by_user: Option<String>,
) -> Result<FeishuPairingRequestRecord, String> {
    let normalized_request_id = request_id.trim();
    if normalized_request_id.is_empty() {
        return Err("request_id is required".to_string());
    }
    if status != "approved" && status != "denied" {
        return Err("status must be approved or denied".to_string());
    }

    let mut record = sqlx::query_as::<_, FeishuPairingRequestRecord>(
        "SELECT id, channel, account_id, sender_id, chat_id, code, status, created_at, updated_at, resolved_at, resolved_by_user
         FROM feishu_pairing_requests
         WHERE id = ?
         LIMIT 1",
    )
    .bind(normalized_request_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("pairing request not found: {normalized_request_id}"))?;

    if record.status != "pending" {
        return Ok(record);
    }

    let now = chrono::Utc::now().to_rfc3339();
    let resolved_by_user = resolved_by_user.unwrap_or_default().trim().to_string();
    sqlx::query(
        "UPDATE feishu_pairing_requests
         SET status = ?, updated_at = ?, resolved_at = ?, resolved_by_user = ?
         WHERE id = ?",
    )
    .bind(status)
    .bind(&now)
    .bind(&now)
    .bind(&resolved_by_user)
    .bind(normalized_request_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    if status == "approved" {
        sqlx::query(
            "INSERT INTO feishu_pairing_allow_from (
                channel, account_id, sender_id, source_request_id, approved_at, approved_by_user
            ) VALUES ('feishu', ?, ?, ?, ?, ?)
            ON CONFLICT(channel, account_id, sender_id) DO UPDATE SET
                source_request_id = excluded.source_request_id,
                approved_at = excluded.approved_at,
                approved_by_user = excluded.approved_by_user",
        )
        .bind(&record.account_id)
        .bind(&record.sender_id)
        .bind(&record.id)
        .bind(&now)
        .bind(&resolved_by_user)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }

    record.status = status.to_string();
    record.updated_at = now.clone();
    record.resolved_at = Some(now);
    record.resolved_by_user = resolved_by_user;

    if !record.chat_id.trim().is_empty() {
        let _ = send_feishu_text_message_with_pool(
            pool,
            &record.chat_id,
            &build_feishu_pairing_resolution_text(&record),
            None,
        )
        .await;
    }

    Ok(record)
}

#[tauri::command]
pub async fn list_feishu_pairing_requests(
    status: Option<String>,
    db: State<'_, DbState>,
) -> Result<Vec<FeishuPairingRequestRecord>, String> {
    list_feishu_pairing_requests_with_pool(&db.0, status).await
}

#[tauri::command]
pub async fn approve_feishu_pairing_request(
    request_id: String,
    resolved_by_user: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuPairingRequestRecord, String> {
    resolve_feishu_pairing_request_with_pool(&db.0, &request_id, "approved", resolved_by_user).await
}

#[tauri::command]
pub async fn deny_feishu_pairing_request(
    request_id: String,
    resolved_by_user: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuPairingRequestRecord, String> {
    resolve_feishu_pairing_request_with_pool(&db.0, &request_id, "denied", resolved_by_user).await
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
    approvals: State<'_, ApprovalManagerState>,
) -> Result<FeishuGatewayResult, String> {
    validate_feishu_auth_with_pool(&db.0, auth_token).await?;
    validate_feishu_signature_with_pool(&db.0, &payload, timestamp, nonce, signature).await?;
    if let Some(runtime_state) = app.try_state::<OpenClawPluginFeishuRuntimeState>() {
        remember_feishu_runtime_state_for_outbound(runtime_state.inner());
    }
    match parse_feishu_payload(&payload)? {
        ParsedFeishuPayload::Challenge(challenge) => Ok(FeishuGatewayResult {
            accepted: true,
            deduped: false,
            challenge: Some(challenge),
        }),
        ParsedFeishuPayload::Event(mut event) => {
            let default_account_id = resolve_default_feishu_account_id_with_pool(&db.0).await?;
            apply_default_feishu_account_id(&mut event, default_account_id.as_deref());
            match evaluate_openclaw_feishu_gate_with_pool(&db.0, &event).await? {
                FeishuInboundGateDecision::Allow => {}
                FeishuInboundGateDecision::Reject { reason } => {
                    if reason == "pairing_pending" {
                        let _ =
                            maybe_create_feishu_pairing_request_with_pool(&db.0, &event).await?;
                    }
                    return Ok(FeishuGatewayResult {
                        accepted: false,
                        deduped: false,
                        challenge: None,
                    });
                }
            }
            let r = dispatch_feishu_inbound_to_workclaw_with_pool_and_app(
                &db.0,
                &app,
                &event,
                Some(approvals.inner().0.as_ref()),
            )
            .await?;
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
    app: tauri::AppHandle,
    chat_id: String,
    text: String,
    _app_id: Option<String>,
    _app_secret: Option<String>,
    _sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
    runtime_state: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<String, String> {
    remember_feishu_runtime_state_for_outbound(runtime_state.inner());
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
    let result = send_feishu_text_message_via_official_runtime_with_pool(
        &db.0,
        runtime_state.inner(),
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
            // WS 多机器人场景下：仅在消息明确 @ 某个员工时才定向路由。
            // 无 @ 时交给默认主员工，避免被“连接所属员工”错误抢占。
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
                            build_im_role_event_payload_for_channel(
                                &s.session_id,
                                &inbound.thread_id,
                                &s.role_id,
                                &s.employee_name,
                                "feishu",
                                "running",
                                "飞书消息已同步到桌面会话，正在执行",
                                None,
                            ),
                        );
                        let _ = app.emit("im-role-dispatch-request", {
                            let mut req = build_im_role_dispatch_request_for_channel(
                                &s.session_id,
                                &inbound.thread_id,
                                &s.role_id,
                                &s.employee_name,
                                "feishu",
                                &inbound
                                    .text
                                    .clone()
                                    .unwrap_or_else(|| "请继续基于当前上下文推进".to_string()),
                                "general-purpose",
                            );
                            req.message_id = inbound.message_id.clone().unwrap_or_default();
                            req
                        });
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
        apply_default_feishu_account_id, evaluate_openclaw_feishu_gate,
        generate_feishu_pairing_code, list_feishu_pairing_allow_from_with_pool,
        parse_feishu_payload, resolve_fallback_default_feishu_account_id,
        resolve_feishu_pairing_account_id, resolve_feishu_pairing_request_with_pool,
        resolve_ws_role_id, sanitize_ws_inbound_text, upsert_feishu_pairing_request_with_pool,
        FeishuInboundGateDecision, FeishuWsEventRecord, ParsedFeishuPayload,
    };
    use crate::commands::employee_agents::AgentEmployee;
    use crate::commands::openclaw_plugins::{
        OpenClawPluginChannelAccountSnapshot, OpenClawPluginChannelSnapshot,
        OpenClawPluginChannelSnapshotResult,
    };
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    fn sample_channel_snapshot(
        account_id: &str,
        allow_from: Vec<&str>,
        account_config: serde_json::Value,
    ) -> OpenClawPluginChannelSnapshotResult {
        OpenClawPluginChannelSnapshotResult {
            plugin_root: "plugin-root".to_string(),
            prepared_root: "prepared-root".to_string(),
            manifest: serde_json::json!({}),
            entry_path: "index.js".to_string(),
            snapshot: OpenClawPluginChannelSnapshot {
                channel_id: "feishu".to_string(),
                default_account_id: Some(account_id.to_string()),
                account_ids: vec![account_id.to_string()],
                accounts: vec![OpenClawPluginChannelAccountSnapshot {
                    account_id: account_id.to_string(),
                    account: serde_json::json!({
                        "accountId": account_id,
                        "config": account_config,
                    }),
                    described_account: serde_json::json!({
                        "accountId": account_id,
                    }),
                    allow_from: allow_from.into_iter().map(str::to_string).collect(),
                    warnings: Vec::new(),
                }],
                reload_config_prefixes: vec!["channels.feishu".to_string()],
                target_hint: None,
            },
            log_record_count: 0,
        }
    }

    async fn setup_pairing_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        sqlx::query(
            "CREATE TABLE feishu_pairing_requests (
                id TEXT PRIMARY KEY,
                channel TEXT NOT NULL DEFAULT 'feishu',
                account_id TEXT NOT NULL DEFAULT 'default',
                sender_id TEXT NOT NULL,
                chat_id TEXT NOT NULL DEFAULT '',
                code TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                resolved_at TEXT,
                resolved_by_user TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .expect("create feishu_pairing_requests");

        sqlx::query(
            "CREATE UNIQUE INDEX idx_feishu_pairing_requests_pending
             ON feishu_pairing_requests(channel, account_id, sender_id)
             WHERE status = 'pending'",
        )
        .execute(&pool)
        .await
        .expect("create feishu_pairing_requests index");

        sqlx::query(
            "CREATE TABLE feishu_pairing_allow_from (
                channel TEXT NOT NULL DEFAULT 'feishu',
                account_id TEXT NOT NULL DEFAULT 'default',
                sender_id TEXT NOT NULL,
                source_request_id TEXT NOT NULL DEFAULT '',
                approved_at TEXT NOT NULL,
                approved_by_user TEXT NOT NULL DEFAULT '',
                PRIMARY KEY(channel, account_id, sender_id)
            )",
        )
        .execute(&pool)
        .await
        .expect("create feishu_pairing_allow_from");

        pool
    }

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
                    "chat_type": "group",
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
                assert_eq!(event.sender_id.as_deref(), Some("ou_sender"));
                assert_eq!(event.chat_type.as_deref(), Some("group"));
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
            chat_type: "group".to_string(),
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

    #[test]
    fn resolve_fallback_default_feishu_account_id_prefers_default_credentials() {
        let resolved = resolve_fallback_default_feishu_account_id(
            true,
            &["employee-a".to_string(), "employee-b".to_string()],
        );
        assert_eq!(resolved.as_deref(), Some("default"));
    }

    #[test]
    fn resolve_fallback_default_feishu_account_id_uses_first_employee_when_needed() {
        let resolved = resolve_fallback_default_feishu_account_id(
            false,
            &["".to_string(), "employee-b".to_string()],
        );
        assert_eq!(resolved.as_deref(), Some("employee-b"));
    }

    #[test]
    fn apply_default_feishu_account_id_only_fills_missing_values() {
        let mut event = crate::im::types::ImEvent {
            channel: "feishu".to_string(),
            event_type: crate::im::types::ImEventType::MessageCreated,
            thread_id: "oc_1".to_string(),
            event_id: None,
            message_id: None,
            text: Some("hello".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant_1".to_string()),
            sender_id: Some("ou_sender".to_string()),
            chat_type: Some("group".to_string()),
        };
        apply_default_feishu_account_id(&mut event, Some("default"));
        assert_eq!(event.account_id.as_deref(), Some("default"));

        event.account_id = Some("tenant_key".to_string());
        apply_default_feishu_account_id(&mut event, Some("another"));
        assert_eq!(event.account_id.as_deref(), Some("tenant_key"));
    }

    #[test]
    fn resolve_feishu_pairing_account_id_prefers_selected_snapshot_account() {
        let snapshot = sample_channel_snapshot(
            "default",
            vec![],
            serde_json::json!({
                "dmPolicy": "pairing"
            }),
        );
        let event = crate::im::types::ImEvent {
            channel: "feishu".to_string(),
            event_type: crate::im::types::ImEventType::MessageCreated,
            thread_id: "oc_chat".to_string(),
            event_id: None,
            message_id: None,
            text: Some("你好".to_string()),
            role_id: None,
            account_id: Some("tenant_key".to_string()),
            sender_id: Some("ou_sender".to_string()),
            chat_type: Some("p2p".to_string()),
            tenant_id: Some("tenant_key".to_string()),
        };

        let resolved = resolve_feishu_pairing_account_id(&event, Some(&snapshot));
        assert_eq!(resolved, "default");
    }

    #[test]
    fn evaluate_openclaw_feishu_gate_allows_allowlisted_direct_sender() {
        let snapshot = sample_channel_snapshot(
            "default",
            vec!["ou_allowed"],
            serde_json::json!({
                "dmPolicy": "allowlist"
            }),
        );
        let event = crate::im::types::ImEvent {
            channel: "feishu".to_string(),
            event_type: crate::im::types::ImEventType::MessageCreated,
            thread_id: "ou_allowed".to_string(),
            event_id: None,
            message_id: None,
            text: Some("hello".to_string()),
            role_id: None,
            account_id: Some("default".to_string()),
            tenant_id: Some("tenant_1".to_string()),
            sender_id: Some("ou_allowed".to_string()),
            chat_type: Some("p2p".to_string()),
        };

        assert_eq!(
            evaluate_openclaw_feishu_gate(&event, &snapshot),
            FeishuInboundGateDecision::Allow
        );
    }

    #[test]
    fn evaluate_openclaw_feishu_gate_rejects_unpaired_direct_sender() {
        let snapshot = sample_channel_snapshot(
            "default",
            vec![],
            serde_json::json!({
                "dmPolicy": "pairing"
            }),
        );
        let event = crate::im::types::ImEvent {
            channel: "feishu".to_string(),
            event_type: crate::im::types::ImEventType::MessageCreated,
            thread_id: "ou_stranger".to_string(),
            event_id: None,
            message_id: None,
            text: Some("hello".to_string()),
            role_id: None,
            account_id: Some("default".to_string()),
            tenant_id: Some("tenant_1".to_string()),
            sender_id: Some("ou_stranger".to_string()),
            chat_type: Some("p2p".to_string()),
        };

        assert_eq!(
            evaluate_openclaw_feishu_gate(&event, &snapshot),
            FeishuInboundGateDecision::Reject {
                reason: "pairing_pending"
            }
        );
    }

    #[test]
    fn evaluate_openclaw_feishu_gate_rejects_group_without_required_mention() {
        let snapshot = sample_channel_snapshot(
            "default",
            vec![],
            serde_json::json!({
                "groupPolicy": "open",
                "requireMention": true
            }),
        );
        let event = crate::im::types::ImEvent {
            channel: "feishu".to_string(),
            event_type: crate::im::types::ImEventType::MessageCreated,
            thread_id: "oc_group_1".to_string(),
            event_id: None,
            message_id: None,
            text: Some("大家看一下".to_string()),
            role_id: None,
            account_id: Some("default".to_string()),
            tenant_id: Some("tenant_1".to_string()),
            sender_id: Some("ou_sender".to_string()),
            chat_type: Some("group".to_string()),
        };

        assert_eq!(
            evaluate_openclaw_feishu_gate(&event, &snapshot),
            FeishuInboundGateDecision::Reject {
                reason: "no_mention"
            }
        );
    }

    #[test]
    fn evaluate_openclaw_feishu_gate_rejects_group_outside_allowlist() {
        let snapshot = sample_channel_snapshot(
            "default",
            vec![],
            serde_json::json!({
                "groupPolicy": "allowlist",
                "groups": {
                    "oc_allowed": {
                        "enabled": true
                    }
                }
            }),
        );
        let event = crate::im::types::ImEvent {
            channel: "feishu".to_string(),
            event_type: crate::im::types::ImEventType::MessageCreated,
            thread_id: "oc_denied".to_string(),
            event_id: None,
            message_id: None,
            text: Some("hello".to_string()),
            role_id: Some("ou_role".to_string()),
            account_id: Some("default".to_string()),
            tenant_id: Some("tenant_1".to_string()),
            sender_id: Some("ou_sender".to_string()),
            chat_type: Some("group".to_string()),
        };

        assert_eq!(
            evaluate_openclaw_feishu_gate(&event, &snapshot),
            FeishuInboundGateDecision::Reject {
                reason: "group_not_allowed"
            }
        );
    }

    #[test]
    fn generate_feishu_pairing_code_returns_eight_chars() {
        let code = generate_feishu_pairing_code();
        assert_eq!(code.len(), 8);
        assert!(code.chars().all(|ch| ch.is_ascii_alphanumeric()));
    }

    #[tokio::test]
    async fn upsert_feishu_pairing_request_reuses_existing_pending_record() {
        let pool = setup_pairing_pool().await;

        let (first, created_first) =
            upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender", "oc_chat", None)
                .await
                .expect("create first request");
        let (second, created_second) = upsert_feishu_pairing_request_with_pool(
            &pool,
            "default",
            "ou_sender",
            "oc_chat_new",
            None,
        )
        .await
        .expect("reuse pending request");

        assert!(created_first);
        assert!(!created_second);
        assert_eq!(first.id, second.id);
        assert_eq!(second.chat_id, "oc_chat_new");
        assert_eq!(first.code, second.code);
    }

    #[tokio::test]
    async fn approve_feishu_pairing_request_persists_allow_from_entry() {
        let pool = setup_pairing_pool().await;

        let (request, _) =
            upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender", "", None)
                .await
                .expect("create request");
        let resolved = resolve_feishu_pairing_request_with_pool(
            &pool,
            &request.id,
            "approved",
            Some("tester".to_string()),
        )
        .await
        .expect("approve request");

        assert_eq!(resolved.status, "approved");
        assert_eq!(resolved.resolved_by_user, "tester");

        let allow_from = list_feishu_pairing_allow_from_with_pool(&pool, "default")
            .await
            .expect("list allow from");
        assert_eq!(allow_from, vec!["ou_sender".to_string()]);
    }

    #[tokio::test]
    async fn deny_feishu_pairing_request_does_not_persist_allow_from_entry() {
        let pool = setup_pairing_pool().await;

        let (request, _) =
            upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender", "", None)
                .await
                .expect("create request");
        let resolved = resolve_feishu_pairing_request_with_pool(
            &pool,
            &request.id,
            "denied",
            Some("tester".to_string()),
        )
        .await
        .expect("deny request");

        assert_eq!(resolved.status, "denied");

        let allow_from = list_feishu_pairing_allow_from_with_pool(&pool, "default")
            .await
            .expect("list allow from");
        assert!(allow_from.is_empty());
    }

    #[tokio::test]
    async fn list_feishu_pairing_requests_filters_by_status() {
        let pool = setup_pairing_pool().await;

        let (first, _) =
            upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender_a", "", None)
                .await
                .expect("create first request");
        let (_second, _) =
            upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender_b", "", None)
                .await
                .expect("create second request");
        let _ = resolve_feishu_pairing_request_with_pool(
            &pool,
            &first.id,
            "approved",
            Some("tester".to_string()),
        )
        .await
        .expect("approve request");

        let pending =
            super::list_feishu_pairing_requests_with_pool(&pool, Some("pending".to_string()))
                .await
                .expect("list pending requests");
        let approved =
            super::list_feishu_pairing_requests_with_pool(&pool, Some("approved".to_string()))
                .await
                .expect("list approved requests");

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].sender_id, "ou_sender_b");
        assert_eq!(approved.len(), 1);
        assert_eq!(approved[0].sender_id, "ou_sender_a");
    }

    #[tokio::test]
    async fn approve_new_pending_request_still_succeeds_when_sender_has_old_approved_record() {
        let pool = setup_pairing_pool().await;

        let (first, _) =
            upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender", "", None)
                .await
                .expect("create first request");
        let _ = resolve_feishu_pairing_request_with_pool(
            &pool,
            &first.id,
            "approved",
            Some("tester".to_string()),
        )
        .await
        .expect("approve first request");

        let (second, _) =
            upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender", "", None)
                .await
                .expect("create second pending request");
        let resolved = resolve_feishu_pairing_request_with_pool(
            &pool,
            &second.id,
            "approved",
            Some("tester-2".to_string()),
        )
        .await
        .expect("approve second request");

        assert_eq!(resolved.status, "approved");

        let approved =
            super::list_feishu_pairing_requests_with_pool(&pool, Some("approved".to_string()))
                .await
                .expect("list approved requests");
        assert_eq!(approved.len(), 2);
    }

    #[tokio::test]
    async fn upsert_feishu_pairing_request_persists_explicit_runtime_code() {
        let pool = setup_pairing_pool().await;

        let (request, created) = upsert_feishu_pairing_request_with_pool(
            &pool,
            "default",
            "ou_sender",
            "",
            Some("dl1m1d25"),
        )
        .await
        .expect("create request with runtime code");

        assert!(created);
        assert_eq!(request.code, "DL1M1D25");
    }

    #[tokio::test]
    async fn upsert_feishu_pairing_request_updates_pending_code_from_runtime_event() {
        let pool = setup_pairing_pool().await;

        let (first, _) =
            upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender", "", None)
                .await
                .expect("create initial request");
        let (second, created) = upsert_feishu_pairing_request_with_pool(
            &pool,
            "default",
            "ou_sender",
            "",
            Some("4965d3b0"),
        )
        .await
        .expect("reuse pending request with runtime code");

        assert!(!created);
        assert_eq!(first.id, second.id);
        assert_eq!(second.code, "4965D3B0");
    }
}

pub(crate) async fn dispatch_feishu_inbound_to_workclaw_with_pool_and_app(
    pool: &SqlitePool,
    app: &tauri::AppHandle,
    event: &ImEvent,
    approval_manager: Option<&ApprovalManager>,
) -> Result<FeishuCallbackResult, String> {
    if let Some(runtime_state) = app.try_state::<OpenClawPluginFeishuRuntimeState>() {
        remember_feishu_runtime_state_for_outbound(runtime_state.inner());
    }
    let result = process_im_event(pool, event.clone()).await?;
    if result.deduped {
        return Ok(result);
    }

    if let Some(approval_manager) = approval_manager {
        let approval_command = parse_feishu_approval_command(event.text.as_deref());
        if let Some(command) = approval_command {
            if maybe_handle_feishu_approval_command_with_pool(pool, approval_manager, event, None)
                .await?
                .is_some()
            {
                if let Some(record) =
                    load_approval_record_with_pool(pool, &command.approval_id).await?
                {
                    let _ = app.emit("approval-resolved", &record);
                }
                return Ok(result);
            }
        }
    }

    let route_decision = resolve_openclaw_route_with_pool(pool, event).await.ok();
    let employee_sessions = ensure_employee_sessions_for_event_with_pool(pool, event).await?;
    for session in &employee_sessions {
        let _ = link_inbound_event_to_session_with_pool(
            pool,
            event,
            &session.employee_id,
            &session.session_id,
        )
        .await;
        let route_agent_id = route_decision
            .as_ref()
            .and_then(|value| value.get("agentId"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&session.role_id)
            .to_string();
        let route_session_key = route_decision
            .as_ref()
            .and_then(|value| value.get("sessionKey"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&session.session_id)
            .to_string();
        let matched_by = route_decision
            .as_ref()
            .and_then(|value| value.get("matchedBy"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("default")
            .to_string();
        let _ = app.emit(
            "im-route-decision",
            ImRouteDecisionEvent {
                session_id: session.session_id.clone(),
                thread_id: event.thread_id.clone(),
                agent_id: route_agent_id,
                session_key: route_session_key,
                matched_by,
            },
        );

        let _ = app.emit(
            "im-role-event",
            build_im_role_event_payload_for_channel(
                &session.session_id,
                &event.thread_id,
                &session.role_id,
                &session.employee_name,
                "feishu",
                "running",
                "飞书消息已同步到桌面会话，正在执行",
                None,
            ),
        );
        let prompt = event
            .text
            .clone()
            .unwrap_or_else(|| "请继续基于当前上下文推进".to_string());
        let _ = app.emit("im-role-dispatch-request", {
            let mut req = build_im_role_dispatch_request_for_channel(
                &session.session_id,
                &event.thread_id,
                &session.role_id,
                &session.employee_name,
                "feishu",
                &prompt,
                "general-purpose",
            );
            req.message_id = event.message_id.clone().unwrap_or_default();
            req
        });
    }

    if employee_sessions.is_empty() {
        let planned = plan_role_events_for_feishu(pool, event).await?;
        for evt in planned {
            let _ = app.emit("im-role-event", evt);
        }
        let dispatches = plan_role_dispatch_requests_for_feishu(pool, event).await?;
        for req in dispatches {
            let _ = app.emit("im-role-dispatch-request", req);
        }
    }

    Ok(result)
}
