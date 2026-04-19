use crate::commands::channel_connectors::{
    get_channel_connector_diagnostics_with_pool, get_channel_connector_monitor_status_in_state,
    list_channel_connectors_with_pool, start_channel_connector_monitor_with_pool_and_app,
    stop_channel_connector_monitor_in_state, ChannelConnectorDiagnostics,
    ChannelConnectorMonitorState, ChannelConnectorMonitorStatus,
};
use crate::commands::im_host::{
    get_im_channel_host_runtime_snapshot_in_state, get_im_channel_runtime_status_in_state,
    record_im_channel_host_action, record_im_channel_runtime_status, ImChannelHostRuntimeSnapshot,
    ImChannelHostRuntimeState,
};
use crate::commands::openclaw_plugins::{
    build_wecom_runtime_status_value, current_feishu_runtime_status,
    get_openclaw_plugin_feishu_channel_snapshot_with_pool_and_app,
    list_openclaw_plugin_channel_hosts_with_pool_and_app,
    start_openclaw_plugin_feishu_runtime_with_pool, stop_openclaw_plugin_feishu_runtime_in_state,
    OpenClawPluginChannelHost, OpenClawPluginChannelSnapshotResult,
    OpenClawPluginFeishuRuntimeState, OpenClawPluginFeishuRuntimeStatus, WecomRuntimeAdapterStatus,
};
use crate::commands::skills::DbState;
use crate::commands::wecom_gateway::{
    get_wecom_connector_status_with_pool, get_wecom_gateway_settings_with_pool,
    start_wecom_connector_with_pool, stop_wecom_connector_with_pool, WecomConnectorStatus,
    WecomGatewaySettings,
};
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::{AppHandle, State};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ImChannelRegistryEntry {
    pub channel: String,
    pub display_name: String,
    pub host_kind: String,
    pub status: String,
    pub summary: String,
    pub detail: String,
    pub capabilities: Vec<String>,
    pub instance_id: Option<String>,
    pub last_error: Option<String>,
    pub plugin_host: Option<OpenClawPluginChannelHost>,
    pub runtime_status: Option<Value>,
    pub diagnostics: Option<ChannelConnectorDiagnostics>,
    pub monitor_status: Option<ChannelConnectorMonitorStatus>,
    pub connector_settings: Option<std::collections::HashMap<String, String>>,
    pub automation_status: Option<Value>,
    pub recent_action: Option<Value>,
}

fn has_wecom_credentials(settings: &WecomGatewaySettings) -> bool {
    !settings.corp_id.trim().is_empty()
        && !settings.agent_id.trim().is_empty()
        && !settings.agent_secret.trim().is_empty()
}

fn summarize_feishu_entry(
    host: Option<OpenClawPluginChannelHost>,
    runtime_status: OpenClawPluginFeishuRuntimeStatus,
) -> ImChannelRegistryEntry {
    let status = if runtime_status.running {
        "running"
    } else if host.as_ref().map(|item| item.status.as_str()) == Some("ready") {
        "ready"
    } else if host.as_ref().and_then(|item| item.error.as_ref()).is_some()
        || runtime_status.last_error.is_some()
    {
        "degraded"
    } else {
        "stopped"
    };

    let mut detail_parts = Vec::new();
    if let Some(version) = host
        .as_ref()
        .map(|item| item.version.trim())
        .filter(|value| !value.is_empty())
    {
        detail_parts.push(format!("插件版本 {version}"));
    }
    if !runtime_status.account_id.trim().is_empty() {
        detail_parts.push(format!("账号 {}", runtime_status.account_id));
    }
    detail_parts.push(if runtime_status.running {
        "运行时已启动".to_string()
    } else {
        "运行时未启动".to_string()
    });

    ImChannelRegistryEntry {
        channel: "feishu".to_string(),
        display_name: host
            .as_ref()
            .map(|item| item.display_name.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "飞书".to_string()),
        host_kind: "openclaw_plugin".to_string(),
        status: status.to_string(),
        summary: if runtime_status.running {
            "通过 OpenClaw 官方飞书插件接收与回复消息。".to_string()
        } else {
            "飞书渠道由 OpenClaw 官方插件宿主提供，WorkClaw 只负责路由、会话与回复生命周期。"
                .to_string()
        },
        detail: detail_parts.join(" · "),
        capabilities: host
            .as_ref()
            .map(|item| item.capabilities.clone())
            .unwrap_or_default(),
        instance_id: Some(runtime_status.account_id.clone())
            .filter(|value| !value.trim().is_empty()),
        last_error: runtime_status
            .last_error
            .clone()
            .or_else(|| host.as_ref().and_then(|item| item.error.clone())),
        plugin_host: host,
        runtime_status: serde_json::to_value(runtime_status).ok(),
        diagnostics: None,
        monitor_status: None,
        connector_settings: None,
        automation_status: None,
        recent_action: None,
    }
}

fn summarize_wecom_entry(
    display_name: Option<String>,
    capabilities: Vec<String>,
    settings: WecomGatewaySettings,
    connector_status: Option<WecomConnectorStatus>,
    host_runtime_status: Option<Value>,
    diagnostics: Option<ChannelConnectorDiagnostics>,
    monitor_status: Option<ChannelConnectorMonitorStatus>,
) -> ImChannelRegistryEntry {
    let configured = has_wecom_credentials(&settings);
    let status = if !configured {
        "not_configured"
    } else if connector_status
        .as_ref()
        .map(|item| item.running)
        .unwrap_or(false)
    {
        "running"
    } else if connector_status
        .as_ref()
        .and_then(|item| item.last_error.as_ref())
        .is_some()
        || monitor_status
            .as_ref()
            .and_then(|item| item.last_error.as_ref())
            .is_some()
    {
        "degraded"
    } else if connector_status
        .as_ref()
        .map(|item| item.state.as_str() == "ready")
        .unwrap_or(false)
    {
        "ready"
    } else {
        "stopped"
    };

    let mut detail_parts = Vec::new();
    detail_parts.push(if configured {
        "凭据已配置".to_string()
    } else {
        "未配置凭据".to_string()
    });
    if let Some(instance_id) = connector_status
        .as_ref()
        .map(|item| item.instance_id.trim())
        .filter(|value| !value.is_empty())
    {
        detail_parts.push(instance_id.to_string());
    }
    if let Some(total_synced) = monitor_status
        .as_ref()
        .filter(|item| item.running)
        .map(|item| item.total_synced)
    {
        detail_parts.push(format!("后台同步 {total_synced} 条"));
    }

    let mut connector_settings = std::collections::HashMap::new();
    connector_settings.insert("corp_id".to_string(), settings.corp_id.clone());
    connector_settings.insert("agent_id".to_string(), settings.agent_id.clone());
    connector_settings.insert("agent_secret".to_string(), settings.agent_secret.clone());
    connector_settings.insert(
        "sidecar_base_url".to_string(),
        settings.sidecar_base_url.clone(),
    );

    ImChannelRegistryEntry {
        channel: "wecom".to_string(),
        display_name: display_name.unwrap_or_else(|| "企业微信".to_string()),
        host_kind: "connector".to_string(),
        status: status.to_string(),
        summary: if configured {
            "通过 sidecar channel connector 接入企业微信，再由 WorkClaw 统一路由与回复。"
                .to_string()
        } else {
            "企业微信渠道将复用与 OpenClaw 兼容的 connector host 形态接入。".to_string()
        },
        detail: detail_parts.join(" · "),
        capabilities,
        instance_id: connector_status
            .as_ref()
            .map(|item| item.instance_id.clone())
            .or_else(|| {
                diagnostics
                    .as_ref()
                    .map(|item| item.health.instance_id.clone())
            }),
        last_error: connector_status
            .as_ref()
            .and_then(|item| item.last_error.clone())
            .or_else(|| {
                host_runtime_status.as_ref().and_then(|value| {
                    value
                        .get("last_error")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
            })
            .or_else(|| {
                monitor_status
                    .as_ref()
                    .and_then(|item| item.last_error.clone())
            })
            .or_else(|| {
                diagnostics
                    .as_ref()
                    .and_then(|item| item.health.last_error.clone())
            }),
        plugin_host: None,
        runtime_status: host_runtime_status
            .or_else(|| connector_status.and_then(|item| serde_json::to_value(item).ok())),
        diagnostics,
        monitor_status,
        connector_settings: Some(connector_settings),
        automation_status: None,
        recent_action: None,
    }
}

fn merge_runtime_snapshot_into_entries(
    mut entries: Vec<ImChannelRegistryEntry>,
    snapshot: &ImChannelHostRuntimeSnapshot,
) -> Vec<ImChannelRegistryEntry> {
    for entry in &mut entries {
        entry.automation_status = snapshot
            .last_restore_report
            .as_ref()
            .and_then(|report| {
                report
                    .entries
                    .iter()
                    .find(|item| item.channel == entry.channel)
            })
            .and_then(|item| serde_json::to_value(item).ok());
        entry.recent_action = snapshot
            .recent_actions
            .iter()
            .find(|item| item.channel == entry.channel)
            .and_then(|item| serde_json::to_value(item).ok());
    }
    entries
}

pub(crate) async fn list_im_channel_registry_with_pool(
    pool: &SqlitePool,
    feishu_runtime_state: &OpenClawPluginFeishuRuntimeState,
    channel_monitor_state: &ChannelConnectorMonitorState,
    host_runtime_state: &ImChannelHostRuntimeState,
    app: &AppHandle,
) -> Result<Vec<ImChannelRegistryEntry>, String> {
    let plugin_hosts = list_openclaw_plugin_channel_hosts_with_pool_and_app(pool, Some(app))
        .await
        .unwrap_or_default();
    let connector_catalog = list_channel_connectors_with_pool(pool, None)
        .await
        .unwrap_or_default();
    let feishu_runtime_status = current_feishu_runtime_status(feishu_runtime_state);
    let feishu_host = plugin_hosts
        .iter()
        .find(|item| item.channel.eq_ignore_ascii_case("feishu"))
        .cloned();

    let wecom_settings = get_wecom_gateway_settings_with_pool(pool).await?;
    let wecom_connector_status = get_wecom_connector_status_with_pool(pool, None).await.ok();
    let wecom_host_runtime_status =
        get_im_channel_runtime_status_in_state(host_runtime_state, "wecom")?;
    let wecom_monitor_status = Some(get_channel_connector_monitor_status_in_state(
        channel_monitor_state,
    ));
    let wecom_instance_id = wecom_connector_status
        .as_ref()
        .map(|item| item.instance_id.clone())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            wecom_monitor_status
                .as_ref()
                .and_then(|item| item.monitored_instance_id.clone())
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "wecom:wecom-main".to_string());
    let wecom_diagnostics =
        get_channel_connector_diagnostics_with_pool(pool, wecom_instance_id, None)
            .await
            .ok();
    let wecom_descriptor = connector_catalog
        .iter()
        .find(|item| item.channel.eq_ignore_ascii_case("wecom"));

    let entries = vec![
        summarize_feishu_entry(feishu_host, feishu_runtime_status),
        summarize_wecom_entry(
            wecom_descriptor.map(|item| item.display_name.clone()),
            wecom_descriptor
                .map(|item| item.capabilities.clone())
                .unwrap_or_default(),
            wecom_settings,
            wecom_connector_status,
            wecom_host_runtime_status,
            wecom_diagnostics,
            wecom_monitor_status,
        ),
    ];
    let snapshot = get_im_channel_host_runtime_snapshot_in_state(host_runtime_state)?;
    Ok(merge_runtime_snapshot_into_entries(entries, &snapshot))
}

pub(crate) async fn get_im_channel_registry_entry_with_pool(
    pool: &SqlitePool,
    feishu_runtime_state: &OpenClawPluginFeishuRuntimeState,
    channel_monitor_state: &ChannelConnectorMonitorState,
    host_runtime_state: &ImChannelHostRuntimeState,
    app: &AppHandle,
    channel: &str,
) -> Result<Option<ImChannelRegistryEntry>, String> {
    let normalized_channel = channel.trim().to_ascii_lowercase();
    let entries = list_im_channel_registry_with_pool(
        pool,
        feishu_runtime_state,
        channel_monitor_state,
        host_runtime_state,
        app,
    )
    .await?;
    Ok(entries
        .into_iter()
        .find(|entry| entry.channel.eq_ignore_ascii_case(&normalized_channel)))
}

pub(crate) async fn get_feishu_runtime_status_from_registry_with_pool(
    pool: &SqlitePool,
    feishu_runtime_state: &OpenClawPluginFeishuRuntimeState,
    channel_monitor_state: &ChannelConnectorMonitorState,
    host_runtime_state: &ImChannelHostRuntimeState,
    app: &AppHandle,
) -> Result<OpenClawPluginFeishuRuntimeStatus, String> {
    let entry = get_im_channel_registry_entry_with_pool(
        pool,
        feishu_runtime_state,
        channel_monitor_state,
        host_runtime_state,
        app,
        "feishu",
    )
    .await?;
    entry
        .and_then(|item| item.runtime_status)
        .and_then(|value| serde_json::from_value::<OpenClawPluginFeishuRuntimeStatus>(value).ok())
        .ok_or_else(|| "feishu runtime status is unavailable from im channel registry".to_string())
}

pub(crate) async fn get_plugin_channel_hosts_from_registry_with_pool(
    pool: &SqlitePool,
    feishu_runtime_state: &OpenClawPluginFeishuRuntimeState,
    channel_monitor_state: &ChannelConnectorMonitorState,
    host_runtime_state: &ImChannelHostRuntimeState,
    app: &AppHandle,
) -> Result<Vec<OpenClawPluginChannelHost>, String> {
    let entries = list_im_channel_registry_with_pool(
        pool,
        feishu_runtime_state,
        channel_monitor_state,
        host_runtime_state,
        app,
    )
    .await?;
    Ok(entries
        .into_iter()
        .filter_map(|entry| entry.plugin_host)
        .collect())
}

pub(crate) async fn get_feishu_channel_snapshot_from_registry_with_pool(
    pool: &SqlitePool,
    feishu_runtime_state: &OpenClawPluginFeishuRuntimeState,
    channel_monitor_state: &ChannelConnectorMonitorState,
    host_runtime_state: &ImChannelHostRuntimeState,
    app: &AppHandle,
    plugin_id: Option<&str>,
) -> Result<OpenClawPluginChannelSnapshotResult, String> {
    let entry = get_im_channel_registry_entry_with_pool(
        pool,
        feishu_runtime_state,
        channel_monitor_state,
        host_runtime_state,
        app,
        "feishu",
    )
    .await?;
    let resolved_plugin_id = plugin_id
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| entry.and_then(|item| item.plugin_host.map(|host| host.plugin_id)))
        .unwrap_or_else(|| "openclaw-lark".to_string());
    get_openclaw_plugin_feishu_channel_snapshot_with_pool_and_app(
        pool,
        &resolved_plugin_id,
        Some(app),
    )
    .await
}

#[tauri::command]
pub async fn list_im_channel_registry(
    app: AppHandle,
    db: State<'_, DbState>,
    runtime: State<'_, OpenClawPluginFeishuRuntimeState>,
    monitor: State<'_, ChannelConnectorMonitorState>,
    host_runtime_state: State<'_, ImChannelHostRuntimeState>,
) -> Result<Vec<ImChannelRegistryEntry>, String> {
    list_im_channel_registry_with_pool(
        &db.0,
        runtime.inner(),
        monitor.inner(),
        host_runtime_state.inner(),
        &app,
    )
    .await
}

#[tauri::command]
pub async fn get_im_channel_host_runtime_snapshot(
    state: State<'_, ImChannelHostRuntimeState>,
) -> Result<ImChannelHostRuntimeSnapshot, String> {
    get_im_channel_host_runtime_snapshot_in_state(state.inner())
}

#[tauri::command]
pub async fn set_im_channel_host_running(
    channel: String,
    desired_running: bool,
    app: AppHandle,
    db: State<'_, DbState>,
    runtime: State<'_, OpenClawPluginFeishuRuntimeState>,
    monitor: State<'_, ChannelConnectorMonitorState>,
    host_runtime_state: State<'_, ImChannelHostRuntimeState>,
) -> Result<ImChannelRegistryEntry, String> {
    let normalized_channel = channel.trim().to_ascii_lowercase();
    let operation_result: Result<(), String> = match normalized_channel.as_str() {
        "feishu" => {
            if desired_running {
                start_openclaw_plugin_feishu_runtime_with_pool(
                    &db.0,
                    runtime.inner(),
                    "openclaw-lark",
                    None,
                    Some(app.clone()),
                )
                .await
                .map(|_| ())
            } else {
                stop_openclaw_plugin_feishu_runtime_in_state(runtime.inner()).map(|_| ())
            }
        }
        "wecom" => {
            if desired_running {
                let instance_id =
                    start_wecom_connector_with_pool(&db.0, None, None, None, None).await?;
                start_channel_connector_monitor_with_pool_and_app(
                    &db.0,
                    monitor.inner().clone(),
                    Some(host_runtime_state.inner().clone()),
                    app.clone(),
                    instance_id,
                    Some(1500),
                    Some(50),
                    Some("processed".to_string()),
                    None,
                )
                .await
                .map(|_| ())
            } else {
                let _ = stop_channel_connector_monitor_in_state(monitor.inner().clone());
                stop_wecom_connector_with_pool(&db.0, None).await
            }
        }
        _ => Err(format!("unsupported im channel host: {}", channel)),
    };

    let action_label = if desired_running {
        "set_running"
    } else {
        "set_stopped"
    };
    match &operation_result {
        Ok(_) => {
            if normalized_channel == "wecom" {
                let monitor_status = get_channel_connector_monitor_status_in_state(monitor.inner());
                let connector_status = get_wecom_connector_status_with_pool(&db.0, None).await.ok();
                let runtime_status = WecomRuntimeAdapterStatus {
                    running: connector_status
                        .as_ref()
                        .map(|item| item.running)
                        .unwrap_or(false),
                    instance_id: connector_status
                        .as_ref()
                        .map(|item| item.instance_id.clone())
                        .filter(|value| !value.trim().is_empty())
                        .or(monitor_status.monitored_instance_id.clone()),
                    started_at: connector_status
                        .as_ref()
                        .and_then(|item| item.started_at.clone()),
                    last_event_at: monitor_status.last_synced_at.clone(),
                    last_error: connector_status
                        .as_ref()
                        .and_then(|item| item.last_error.clone())
                        .or(monitor_status.last_error.clone()),
                    reconnect_attempts: connector_status
                        .as_ref()
                        .map(|item| item.reconnect_attempts)
                        .unwrap_or_default(),
                    queue_depth: connector_status
                        .as_ref()
                        .map(|item| item.queue_depth)
                        .unwrap_or_default(),
                    recent_logs: vec![format!(
                        "[wecom] host {} via registry toggle",
                        if desired_running {
                            "started"
                        } else {
                            "stopped"
                        }
                    )],
                };
                let _ = record_im_channel_runtime_status(
                    host_runtime_state.inner(),
                    "wecom",
                    build_wecom_runtime_status_value(&runtime_status),
                );
            }
            let _ = record_im_channel_host_action(
                host_runtime_state.inner(),
                &normalized_channel,
                action_label,
                desired_running,
                true,
                format!("host state updated for {}", normalized_channel),
                None,
                "settings-ui",
            );
        }
        Err(error) => {
            let _ = record_im_channel_host_action(
                host_runtime_state.inner(),
                &normalized_channel,
                action_label,
                desired_running,
                false,
                format!("host state update failed for {}", normalized_channel),
                Some(error.clone()),
                "settings-ui",
            );
        }
    }
    operation_result?;

    list_im_channel_registry_with_pool(
        &db.0,
        runtime.inner(),
        monitor.inner(),
        host_runtime_state.inner(),
        &app,
    )
    .await?
    .into_iter()
    .find(|entry| entry.channel.eq_ignore_ascii_case(&normalized_channel))
    .ok_or_else(|| {
        format!(
            "im channel host registry entry not found after update: {}",
            channel
        )
    })
}
