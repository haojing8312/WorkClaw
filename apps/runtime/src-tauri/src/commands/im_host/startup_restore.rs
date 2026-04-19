use crate::commands::channel_connectors::{
    start_channel_connector_monitor_with_pool_and_app, ChannelConnectorMonitorState,
};
use crate::commands::im_host::ImChannelHostRuntimeState;
use crate::commands::openclaw_plugins::{
    maybe_restore_openclaw_plugin_feishu_runtime_with_pool, OpenClawPluginFeishuRuntimeState,
};
use crate::commands::wecom_gateway::{
    get_wecom_connector_status_with_pool, resolve_wecom_credentials,
    start_wecom_connector_with_pool, WecomConnectorStatus,
};
use sqlx::SqlitePool;
use tauri::AppHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImChannelRestoreKind {
    Feishu,
    Wecom,
}

impl ImChannelRestoreKind {
    fn channel(self) -> &'static str {
        match self {
            Self::Feishu => "feishu",
            Self::Wecom => "wecom",
        }
    }

    fn host_kind(self) -> &'static str {
        match self {
            Self::Feishu => "openclaw_plugin",
            Self::Wecom => "connector",
        }
    }

    fn all() -> [Self; 2] {
        [Self::Feishu, Self::Wecom]
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ImChannelRestoreEntry {
    pub channel: String,
    pub host_kind: String,
    pub should_restore: bool,
    pub restored: bool,
    pub monitor_restored: bool,
    pub detail: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ImChannelRestoreReport {
    pub feishu_runtime_restored: bool,
    pub wecom_connector_restored: bool,
    pub wecom_monitor_restored: bool,
    pub entries: Vec<ImChannelRestoreEntry>,
}

struct ImChannelRestoreContext<'a> {
    pool: &'a SqlitePool,
    feishu_runtime_state: &'a OpenClawPluginFeishuRuntimeState,
    channel_monitor_state: ChannelConnectorMonitorState,
    host_runtime_state: ImChannelHostRuntimeState,
    app: AppHandle,
}

async fn count_enabled_channel_bindings_with_pool(
    pool: &SqlitePool,
    channel: &str,
) -> Result<i64, String> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM im_routing_bindings WHERE enabled = 1 AND lower(channel) = lower(?)",
    )
    .bind(channel.trim())
    .fetch_one(pool)
    .await
    .map_err(|error| error.to_string())?;
    Ok(row.0.max(0))
}

async fn should_auto_restore_wecom_connector_with_pool(pool: &SqlitePool) -> Result<bool, String> {
    let has_credentials = resolve_wecom_credentials(pool, None, None, None)
        .await
        .is_ok();
    if !has_credentials {
        return Ok(false);
    }

    Ok(count_enabled_channel_bindings_with_pool(pool, "wecom").await? > 0)
}

async fn ensure_wecom_connector_running_with_pool(
    pool: &SqlitePool,
) -> Result<WecomConnectorStatus, String> {
    let status = get_wecom_connector_status_with_pool(pool, None).await?;
    if status.running {
        return Ok(status);
    }

    let instance_id = start_wecom_connector_with_pool(pool, None, None, None, None).await?;
    get_wecom_connector_status_with_pool(pool, None)
        .await
        .or_else(|_| {
            Ok(WecomConnectorStatus {
                running: true,
                state: "starting".to_string(),
                started_at: Some(chrono::Utc::now().to_rfc3339()),
                last_error: None,
                reconnect_attempts: 0,
                queue_depth: 0,
                instance_id,
            })
        })
}

async fn restore_feishu_channel(
    context: &ImChannelRestoreContext<'_>,
) -> Result<ImChannelRestoreEntry, String> {
    let restored = maybe_restore_openclaw_plugin_feishu_runtime_with_pool(
        context.pool,
        context.feishu_runtime_state,
        context.app.clone(),
    )
    .await
    .unwrap_or(false);

    Ok(ImChannelRestoreEntry {
        channel: ImChannelRestoreKind::Feishu.channel().to_string(),
        host_kind: ImChannelRestoreKind::Feishu.host_kind().to_string(),
        should_restore: restored,
        restored,
        monitor_restored: false,
        detail: if restored {
            "detected enabled Feishu runtime and attempted auto-restore".to_string()
        } else {
            "Feishu runtime did not meet auto-restore conditions".to_string()
        },
        error: None,
    })
}

async fn restore_wecom_channel(
    context: &ImChannelRestoreContext<'_>,
) -> Result<ImChannelRestoreEntry, String> {
    let should_restore = should_auto_restore_wecom_connector_with_pool(context.pool).await?;
    if !should_restore {
        return Ok(ImChannelRestoreEntry {
            channel: ImChannelRestoreKind::Wecom.channel().to_string(),
            host_kind: ImChannelRestoreKind::Wecom.host_kind().to_string(),
            should_restore: false,
            restored: false,
            monitor_restored: false,
            detail: "WeCom connector skipped auto-restore because credentials or enabled bindings are missing".to_string(),
            error: None,
        });
    }

    let status = ensure_wecom_connector_running_with_pool(context.pool).await?;
    let monitor_status = start_channel_connector_monitor_with_pool_and_app(
        context.pool,
        context.channel_monitor_state.clone(),
        Some(context.host_runtime_state.clone()),
        context.app.clone(),
        status.instance_id.clone(),
        Some(1500),
        Some(50),
        Some("processed".to_string()),
        None,
    )
    .await?;

    Ok(ImChannelRestoreEntry {
        channel: ImChannelRestoreKind::Wecom.channel().to_string(),
        host_kind: ImChannelRestoreKind::Wecom.host_kind().to_string(),
        should_restore: true,
        restored: status.running,
        monitor_restored: monitor_status.running,
        detail: format!(
            "WeCom connector auto-restore attempted for {}",
            status.instance_id
        ),
        error: status.last_error.or(monitor_status.last_error),
    })
}

async fn restore_channel_entry(
    kind: ImChannelRestoreKind,
    context: &ImChannelRestoreContext<'_>,
) -> Result<ImChannelRestoreEntry, String> {
    match kind {
        ImChannelRestoreKind::Feishu => restore_feishu_channel(context).await,
        ImChannelRestoreKind::Wecom => restore_wecom_channel(context).await,
    }
}

pub(crate) async fn restore_im_channels_with_pool(
    pool: &SqlitePool,
    feishu_runtime_state: &OpenClawPluginFeishuRuntimeState,
    channel_monitor_state: ChannelConnectorMonitorState,
    host_runtime_state: ImChannelHostRuntimeState,
    app: AppHandle,
) -> Result<ImChannelRestoreReport, String> {
    let context = ImChannelRestoreContext {
        pool,
        feishu_runtime_state,
        channel_monitor_state,
        host_runtime_state,
        app,
    };
    let mut report = ImChannelRestoreReport::default();

    for kind in ImChannelRestoreKind::all() {
        let entry = restore_channel_entry(kind, &context).await?;
        match kind {
            ImChannelRestoreKind::Feishu => {
                report.feishu_runtime_restored = entry.restored;
            }
            ImChannelRestoreKind::Wecom => {
                report.wecom_connector_restored = entry.restored;
                report.wecom_monitor_restored = entry.monitor_restored;
            }
        }
        report.entries.push(entry);
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::{
        should_auto_restore_wecom_connector_with_pool, ImChannelRestoreKind, ImChannelRestoreReport,
    };
    use sqlx::SqlitePool;

    #[test]
    fn restore_registry_lists_supported_channels() {
        assert_eq!(
            ImChannelRestoreKind::all()
                .into_iter()
                .map(ImChannelRestoreKind::channel)
                .collect::<Vec<_>>(),
            vec!["feishu", "wecom"]
        );
    }

    #[test]
    fn restore_report_defaults_to_empty_entries() {
        let report = ImChannelRestoreReport::default();
        assert!(report.entries.is_empty());
        assert!(!report.feishu_runtime_restored);
        assert!(!report.wecom_connector_restored);
        assert!(!report.wecom_monitor_restored);
    }

    #[tokio::test]
    async fn wecom_restore_requires_credentials_and_enabled_bindings() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .expect("create app_settings");
        sqlx::query(
            "CREATE TABLE im_routing_bindings (
                id TEXT PRIMARY KEY NOT NULL,
                agent_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                account_id TEXT NOT NULL,
                peer_kind TEXT NOT NULL,
                peer_id TEXT NOT NULL,
                guild_id TEXT NOT NULL,
                team_id TEXT NOT NULL,
                role_ids_json TEXT NOT NULL,
                connector_meta_json TEXT NOT NULL,
                priority INTEGER NOT NULL,
                enabled INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create im_routing_bindings");

        assert!(!should_auto_restore_wecom_connector_with_pool(&pool)
            .await
            .expect("without config"));

        for (key, value) in [
            ("wecom_corp_id", "corp-1"),
            ("wecom_agent_id", "agent-1"),
            ("wecom_agent_secret", "secret-1"),
        ] {
            sqlx::query("INSERT INTO app_settings (key, value) VALUES (?, ?)")
                .bind(key)
                .bind(value)
                .execute(&pool)
                .await
                .expect("insert app setting");
        }

        assert!(!should_auto_restore_wecom_connector_with_pool(&pool)
            .await
            .expect("without binding"));

        sqlx::query(
            "INSERT INTO im_routing_bindings (
                id, agent_id, channel, account_id, peer_kind, peer_id, guild_id, team_id,
                role_ids_json, connector_meta_json, priority, enabled, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("binding-1")
        .bind("main")
        .bind("wecom")
        .bind("tenant-wecom")
        .bind("group")
        .bind("room-1")
        .bind("")
        .bind("")
        .bind("[]")
        .bind("{}")
        .bind(100_i64)
        .bind(1_i64)
        .bind("2026-04-14T00:00:00Z")
        .bind("2026-04-14T00:00:00Z")
        .execute(&pool)
        .await
        .expect("insert binding");

        assert!(should_auto_restore_wecom_connector_with_pool(&pool)
            .await
            .expect("with credentials and binding"));
    }
}
