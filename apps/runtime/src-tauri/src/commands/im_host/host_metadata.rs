use super::channel_registry::{
    get_feishu_channel_snapshot_from_registry_with_pool, get_im_channel_registry_entry_with_pool,
    ImChannelRegistryEntry,
};
use crate::commands::channel_connectors::ChannelConnectorMonitorState;
use crate::commands::openclaw_plugins::{
    OpenClawPluginChannelAccountSnapshot, OpenClawPluginChannelHost,
    OpenClawPluginFeishuRuntimeState,
};
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::AppHandle;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ImChannelAccountMetadata {
    pub account_id: String,
    pub account: Value,
    pub described_account: Value,
    pub allow_from: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ImHostMetadata {
    pub channel: String,
    pub host_kind: String,
    pub status: String,
    pub instance_id: Option<String>,
    pub default_account_id: Option<String>,
    pub account_ids: Vec<String>,
    pub accounts: Vec<ImChannelAccountMetadata>,
    pub runtime_status: Option<Value>,
    pub plugin_host: Option<OpenClawPluginChannelHost>,
}

impl ImHostMetadata {
    pub(crate) fn default_account_id(&self) -> Option<&str> {
        self.default_account_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub(crate) fn account_ids(&self) -> impl Iterator<Item = &str> {
        self.account_ids
            .iter()
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub(crate) fn select_account<'a>(
        &'a self,
        event_account_id: Option<&str>,
    ) -> Option<&'a ImChannelAccountMetadata> {
        let normalized_event_account = event_account_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase);
        let default_account = self.default_account_id().map(str::to_ascii_lowercase);

        if let Some(event_account_id) = normalized_event_account.as_deref() {
            if let Some(found) = self.accounts.iter().find(|account| {
                account
                    .account_id
                    .trim()
                    .eq_ignore_ascii_case(event_account_id)
            }) {
                return Some(found);
            }
        }

        if let Some(default_account_id) = default_account.as_deref() {
            if let Some(found) = self.accounts.iter().find(|account| {
                account
                    .account_id
                    .trim()
                    .eq_ignore_ascii_case(default_account_id)
            }) {
                return Some(found);
            }
        }

        self.accounts.first()
    }

    pub(crate) fn extend_account_allow_from(&mut self, account_id: &str, sender_ids: &[String]) {
        let normalized_account_id = account_id.trim();
        if normalized_account_id.is_empty() || sender_ids.is_empty() {
            return;
        }

        if let Some(account) = self
            .accounts
            .iter_mut()
            .find(|account| account.account_id.trim() == normalized_account_id)
        {
            for sender_id in sender_ids {
                if !account.allow_from.iter().any(|entry| entry == sender_id) {
                    account.allow_from.push(sender_id.clone());
                }
            }
        }
    }
}

fn map_account_snapshot(account: OpenClawPluginChannelAccountSnapshot) -> ImChannelAccountMetadata {
    ImChannelAccountMetadata {
        account_id: account.account_id,
        account: account.account,
        described_account: account.described_account,
        allow_from: account.allow_from,
        warnings: account.warnings,
    }
}

fn build_registry_only_metadata(entry: ImChannelRegistryEntry) -> ImHostMetadata {
    ImHostMetadata {
        channel: entry.channel,
        host_kind: entry.host_kind,
        status: entry.status,
        instance_id: entry.instance_id,
        default_account_id: None,
        account_ids: Vec::new(),
        accounts: Vec::new(),
        runtime_status: entry.runtime_status,
        plugin_host: entry.plugin_host,
    }
}

pub(crate) async fn resolve_im_host_metadata_with_pool(
    pool: &SqlitePool,
    feishu_runtime_state: &OpenClawPluginFeishuRuntimeState,
    channel_monitor_state: &ChannelConnectorMonitorState,
    host_runtime_state: &super::ImChannelHostRuntimeState,
    app: &AppHandle,
    channel: &str,
) -> Result<Option<ImHostMetadata>, String> {
    let normalized_channel = channel.trim().to_ascii_lowercase();
    let Some(entry) = get_im_channel_registry_entry_with_pool(
        pool,
        feishu_runtime_state,
        channel_monitor_state,
        host_runtime_state,
        app,
        &normalized_channel,
    )
    .await?
    else {
        return Ok(None);
    };

    if normalized_channel == "feishu" {
        let snapshot = get_feishu_channel_snapshot_from_registry_with_pool(
            pool,
            feishu_runtime_state,
            channel_monitor_state,
            host_runtime_state,
            app,
            entry
                .plugin_host
                .as_ref()
                .map(|host| host.plugin_id.as_str()),
        )
        .await?;
        return Ok(Some(ImHostMetadata {
            channel: entry.channel,
            host_kind: entry.host_kind,
            status: entry.status,
            instance_id: entry.instance_id,
            default_account_id: snapshot.snapshot.default_account_id,
            account_ids: snapshot.snapshot.account_ids,
            accounts: snapshot
                .snapshot
                .accounts
                .into_iter()
                .map(map_account_snapshot)
                .collect(),
            runtime_status: entry.runtime_status,
            plugin_host: entry.plugin_host,
        }));
    }

    Ok(Some(build_registry_only_metadata(entry)))
}

#[cfg(test)]
mod tests {
    use super::{ImChannelAccountMetadata, ImHostMetadata};

    fn sample_metadata() -> ImHostMetadata {
        ImHostMetadata {
            channel: "feishu".to_string(),
            host_kind: "openclaw_plugin".to_string(),
            status: "ready".to_string(),
            instance_id: Some("default".to_string()),
            default_account_id: Some("default".to_string()),
            account_ids: vec!["default".to_string(), "workspace".to_string()],
            accounts: vec![
                ImChannelAccountMetadata {
                    account_id: "default".to_string(),
                    account: serde_json::json!({}),
                    described_account: serde_json::json!({}),
                    allow_from: vec!["ou_a".to_string()],
                    warnings: Vec::new(),
                },
                ImChannelAccountMetadata {
                    account_id: "workspace".to_string(),
                    account: serde_json::json!({}),
                    described_account: serde_json::json!({}),
                    allow_from: Vec::new(),
                    warnings: Vec::new(),
                },
            ],
            runtime_status: None,
            plugin_host: None,
        }
    }

    #[test]
    fn selects_event_account_before_default() {
        let metadata = sample_metadata();
        let selected = metadata
            .select_account(Some("workspace"))
            .expect("select workspace account");
        assert_eq!(selected.account_id, "workspace");
    }

    #[test]
    fn extend_account_allow_from_dedupes_entries() {
        let mut metadata = sample_metadata();
        metadata.extend_account_allow_from("default", &["ou_a".to_string(), "ou_b".to_string()]);
        let selected = metadata
            .select_account(Some("default"))
            .expect("select default account");
        assert_eq!(
            selected.allow_from,
            vec!["ou_a".to_string(), "ou_b".to_string()]
        );
    }
}
