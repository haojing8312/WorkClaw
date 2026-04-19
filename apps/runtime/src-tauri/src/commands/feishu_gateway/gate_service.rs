use super::list_feishu_pairing_allow_from_with_pool;
use super::metadata_service::{
    resolve_feishu_host_metadata_from_registry_with_pool, FeishuChannelAccountMetadata,
    FeishuHostMetadata,
};
use super::types::{FeishuGateAccountConfig, FeishuGateGroupConfig, FeishuInboundGateDecision};
use crate::commands::channel_connectors::ChannelConnectorMonitorState;
use crate::commands::im_host::ImChannelHostRuntimeState;
use crate::commands::openclaw_plugins::OpenClawPluginFeishuRuntimeState;
use crate::im::types::ImEvent;
use sqlx::SqlitePool;
use tauri::AppHandle;

pub(crate) fn is_direct_feishu_chat(event: &ImEvent) -> bool {
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
    account_snapshot: &FeishuChannelAccountMetadata,
) -> FeishuGateAccountConfig {
    account_snapshot
        .account
        .get("config")
        .cloned()
        .and_then(|value| serde_json::from_value::<FeishuGateAccountConfig>(value).ok())
        .unwrap_or_default()
}

pub(crate) fn select_feishu_channel_account_snapshot<'a>(
    metadata: &'a FeishuHostMetadata,
    event: &ImEvent,
) -> Option<&'a FeishuChannelAccountMetadata> {
    metadata.select_account(event.account_id.as_deref())
}

pub(crate) fn evaluate_openclaw_feishu_gate(
    event: &ImEvent,
    metadata: &FeishuHostMetadata,
) -> FeishuInboundGateDecision {
    let Some(account_snapshot) = select_feishu_channel_account_snapshot(metadata, event) else {
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

pub(crate) async fn evaluate_openclaw_feishu_gate_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<FeishuInboundGateDecision, String> {
    let mut metadata =
        match super::metadata_service::resolve_feishu_host_metadata_with_pool(pool).await {
            Ok(metadata) => metadata,
            Err(_) => return Ok(FeishuInboundGateDecision::Allow),
        };
    if let Some(target_account_id) = select_feishu_channel_account_snapshot(&metadata, event)
        .map(|account_snapshot| account_snapshot.account_id.clone())
    {
        let pairing_allow_from =
            list_feishu_pairing_allow_from_with_pool(pool, &target_account_id).await?;
        if !pairing_allow_from.is_empty() {
            metadata.extend_account_allow_from(&target_account_id, &pairing_allow_from);
        }
    }
    Ok(evaluate_openclaw_feishu_gate(event, &metadata))
}

pub(crate) async fn evaluate_openclaw_feishu_gate_from_registry_with_pool(
    pool: &SqlitePool,
    runtime_state: &OpenClawPluginFeishuRuntimeState,
    channel_monitor_state: &ChannelConnectorMonitorState,
    host_runtime_state: &ImChannelHostRuntimeState,
    app: &AppHandle,
    event: &ImEvent,
) -> Result<FeishuInboundGateDecision, String> {
    let mut metadata = match resolve_feishu_host_metadata_from_registry_with_pool(
        pool,
        runtime_state,
        channel_monitor_state,
        host_runtime_state,
        app,
    )
    .await
    {
        Ok(metadata) => metadata,
        Err(_) => return Ok(FeishuInboundGateDecision::Allow),
    };
    if let Some(target_account_id) = select_feishu_channel_account_snapshot(&metadata, event)
        .map(|account_snapshot| account_snapshot.account_id.clone())
    {
        let pairing_allow_from =
            list_feishu_pairing_allow_from_with_pool(pool, &target_account_id).await?;
        if !pairing_allow_from.is_empty() {
            metadata.extend_account_allow_from(&target_account_id, &pairing_allow_from);
        }
    }
    Ok(evaluate_openclaw_feishu_gate(event, &metadata))
}
