use crate::commands::channel_connectors::ChannelConnectorMonitorState;
use crate::commands::im_host::{
    resolve_im_host_metadata_with_pool, ImChannelAccountMetadata, ImChannelHostRuntimeState,
    ImHostMetadata,
};
use crate::commands::openclaw_plugins::{
    get_openclaw_plugin_feishu_channel_snapshot_with_pool,
    resolve_primary_feishu_plugin_id_with_pool, OpenClawPluginFeishuRuntimeState,
};
use sqlx::SqlitePool;
use tauri::AppHandle;

pub(crate) type FeishuHostMetadata = ImHostMetadata;
pub(crate) type FeishuChannelAccountMetadata = ImChannelAccountMetadata;

pub(crate) async fn resolve_feishu_host_metadata_with_pool(
    pool: &SqlitePool,
) -> Result<FeishuHostMetadata, String> {
    let plugin_id = resolve_primary_feishu_plugin_id_with_pool(pool).await?;
    let snapshot = get_openclaw_plugin_feishu_channel_snapshot_with_pool(pool, &plugin_id).await?;
    Ok(ImHostMetadata {
        channel: "feishu".to_string(),
        host_kind: "openclaw_plugin".to_string(),
        status: "unknown".to_string(),
        instance_id: snapshot
            .snapshot
            .default_account_id
            .clone()
            .filter(|value| !value.trim().is_empty()),
        default_account_id: snapshot.snapshot.default_account_id,
        account_ids: snapshot.snapshot.account_ids,
        accounts: snapshot
            .snapshot
            .accounts
            .into_iter()
            .map(|account| ImChannelAccountMetadata {
                account_id: account.account_id,
                account: account.account,
                described_account: account.described_account,
                allow_from: account.allow_from,
                warnings: account.warnings,
            })
            .collect(),
        runtime_status: None,
        plugin_host: None,
    })
}

pub(crate) async fn resolve_feishu_host_metadata_from_registry_with_pool(
    pool: &SqlitePool,
    feishu_runtime_state: &OpenClawPluginFeishuRuntimeState,
    channel_monitor_state: &ChannelConnectorMonitorState,
    host_runtime_state: &ImChannelHostRuntimeState,
    app: &AppHandle,
) -> Result<FeishuHostMetadata, String> {
    resolve_im_host_metadata_with_pool(
        pool,
        feishu_runtime_state,
        channel_monitor_state,
        host_runtime_state,
        app,
        "feishu",
    )
    .await?
    .ok_or_else(|| "feishu host metadata is unavailable from im host registry".to_string())
}
