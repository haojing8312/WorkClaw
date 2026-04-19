use crate::commands::feishu_gateway::{call_sidecar_json, get_app_setting};
use crate::commands::im_host::{
    dispatch_im_inbound_to_workclaw_with_pool_and_app, get_im_channel_runtime_status_in_state,
    parse_normalized_im_event_value, record_im_channel_runtime_status, ImChannelHostRuntimeState,
};
use crate::commands::openclaw_plugins::{
    build_wecom_runtime_status_value, handle_openclaw_plugin_wecom_runtime_stdout_line_with_bridge,
    merge_wecom_runtime_status, parse_wecom_runtime_status_value, WecomRuntimeAdapterStatus,
};
use crate::commands::skills::DbState;
use crate::im::types::ImEvent;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, State};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ChannelConnectorIssue {
    pub code: String,
    pub category: String,
    pub user_message: String,
    pub technical_message: String,
    pub retryable: bool,
    pub occurred_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ChannelConnectorDescriptor {
    pub channel: String,
    pub display_name: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ChannelConnectorHealth {
    pub adapter_name: String,
    pub instance_id: String,
    pub state: String,
    pub last_ok_at: Option<String>,
    pub last_error: Option<String>,
    pub reconnect_attempts: i64,
    pub queue_depth: i64,
    pub issue: Option<ChannelConnectorIssue>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ChannelConnectorReplayStats {
    pub retained_events: i64,
    pub acked_events: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ChannelConnectorDiagnostics {
    pub connector: ChannelConnectorDescriptor,
    pub status: String,
    pub health: ChannelConnectorHealth,
    pub replay: ChannelConnectorReplayStats,
}

#[derive(Debug, Clone, Default)]
struct ChannelConnectorMonitorConfig {
    instance_id: Option<String>,
    ack_status: Option<String>,
    sidecar_base_url: Option<String>,
}

#[derive(Clone, Default)]
pub struct ChannelConnectorMonitorState {
    running: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    interval_ms: Arc<AtomicU64>,
    limit: Arc<AtomicU64>,
    total_synced: Arc<AtomicUsize>,
    last_error: Arc<Mutex<Option<String>>>,
    last_synced_at: Arc<Mutex<Option<String>>>,
    config: Arc<Mutex<ChannelConnectorMonitorConfig>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ChannelConnectorMonitorStatus {
    pub running: bool,
    pub generation: u64,
    pub interval_ms: u64,
    pub limit: u32,
    pub total_synced: usize,
    pub monitored_instance_id: Option<String>,
    pub ack_status: Option<String>,
    pub last_synced_at: Option<String>,
    pub last_error: Option<String>,
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn channel_connector_monitor_status(
    state: &ChannelConnectorMonitorState,
) -> ChannelConnectorMonitorStatus {
    let config = state
        .config
        .lock()
        .ok()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    ChannelConnectorMonitorStatus {
        running: state.running.load(Ordering::SeqCst),
        generation: state.generation.load(Ordering::SeqCst),
        interval_ms: state.interval_ms.load(Ordering::SeqCst),
        limit: state.limit.load(Ordering::SeqCst).clamp(1, 500) as u32,
        total_synced: state.total_synced.load(Ordering::SeqCst),
        monitored_instance_id: config.instance_id,
        ack_status: config.ack_status,
        last_synced_at: state
            .last_synced_at
            .lock()
            .ok()
            .and_then(|guard| guard.clone()),
        last_error: state.last_error.lock().ok().and_then(|guard| guard.clone()),
    }
}

async fn refresh_channel_host_runtime_status_with_pool(
    pool: &SqlitePool,
    host_runtime_state: &ImChannelHostRuntimeState,
    instance_id: &str,
    monitor_state: &ChannelConnectorMonitorState,
    sidecar_base_url: Option<String>,
) -> Result<(), String> {
    let normalized_instance_id = instance_id.trim();
    if !normalized_instance_id.starts_with("wecom:") {
        return Ok(());
    }

    let diagnostics = get_channel_connector_diagnostics_with_pool(
        pool,
        normalized_instance_id.to_string(),
        sidecar_base_url,
    )
    .await?;
    let monitor_status = get_channel_connector_monitor_status_in_state(monitor_state);
    let runtime_status = WecomRuntimeAdapterStatus {
        running: diagnostics.health.state.eq_ignore_ascii_case("running"),
        instance_id: Some(diagnostics.health.instance_id.clone())
            .filter(|value| !value.trim().is_empty()),
        started_at: diagnostics.health.last_ok_at.clone(),
        last_event_at: monitor_status.last_synced_at.clone(),
        last_error: diagnostics
            .health
            .last_error
            .clone()
            .or(monitor_status.last_error.clone()),
        reconnect_attempts: diagnostics.health.reconnect_attempts,
        queue_depth: diagnostics.health.queue_depth,
        recent_logs: vec![format!(
            "[wecom] monitor synced={} running={}",
            monitor_status.total_synced, diagnostics.health.state
        )],
    };
    merge_wecom_host_runtime_status_in_state(host_runtime_state, &runtime_status)
}

fn merge_wecom_host_runtime_status_in_state(
    host_runtime_state: &ImChannelHostRuntimeState,
    patch: &WecomRuntimeAdapterStatus,
) -> Result<(), String> {
    let mut merged = get_im_channel_runtime_status_in_state(host_runtime_state, "wecom")?
        .as_ref()
        .map(parse_wecom_runtime_status_value)
        .transpose()?
        .unwrap_or_default();
    merge_wecom_runtime_status(&mut merged, patch);
    record_im_channel_runtime_status(
        host_runtime_state,
        "wecom",
        build_wecom_runtime_status_value(&merged),
    )
}

pub async fn resolve_channel_connector_sidecar_base_url(
    pool: &SqlitePool,
    explicit: Option<String>,
) -> Result<Option<String>, String> {
    if let Some(value) = explicit {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed.to_string()));
        }
    }

    if let Some(value) = get_app_setting(pool, "im_sidecar_base_url").await? {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed.to_string()));
        }
    }

    Ok(None)
}

pub async fn list_channel_connectors_with_pool(
    pool: &SqlitePool,
    sidecar_base_url: Option<String>,
) -> Result<Vec<ChannelConnectorDescriptor>, String> {
    let resolved_sidecar_base_url =
        resolve_channel_connector_sidecar_base_url(pool, sidecar_base_url).await?;
    let response = call_sidecar_json(
        "/api/channels/catalog",
        serde_json::json!({}),
        resolved_sidecar_base_url,
    )
    .await?;
    serde_json::from_value(response)
        .map_err(|e| format!("parse channel connector catalog failed: {}", e))
}

pub async fn get_channel_connector_diagnostics_with_pool(
    pool: &SqlitePool,
    instance_id: String,
    sidecar_base_url: Option<String>,
) -> Result<ChannelConnectorDiagnostics, String> {
    let resolved_sidecar_base_url =
        resolve_channel_connector_sidecar_base_url(pool, sidecar_base_url).await?;
    let response = call_sidecar_json(
        "/api/channels/diagnostics",
        serde_json::json!({
            "instance_id": instance_id,
        }),
        resolved_sidecar_base_url,
    )
    .await?;
    serde_json::from_value(response)
        .map_err(|e| format!("parse channel connector diagnostics failed: {}", e))
}

pub async fn ack_channel_events_with_pool(
    pool: &SqlitePool,
    instance_id: String,
    message_ids: Vec<String>,
    status: Option<String>,
    sidecar_base_url: Option<String>,
) -> Result<(), String> {
    let resolved_sidecar_base_url =
        resolve_channel_connector_sidecar_base_url(pool, sidecar_base_url).await?;
    for message_id in message_ids {
        call_sidecar_json(
            "/api/channels/ack",
            serde_json::json!({
                "instance_id": instance_id,
                "message_id": message_id,
                "status": status,
            }),
            resolved_sidecar_base_url.clone(),
        )
        .await?;
    }
    Ok(())
}

pub async fn stop_channel_connector_with_pool(
    pool: &SqlitePool,
    instance_id: String,
    sidecar_base_url: Option<String>,
) -> Result<(), String> {
    let normalized_instance_id = instance_id.trim().to_string();
    if normalized_instance_id.is_empty() {
        return Err("instance_id is required".to_string());
    }
    let resolved_sidecar_base_url =
        resolve_channel_connector_sidecar_base_url(pool, sidecar_base_url).await?;
    let _ = call_sidecar_json(
        "/api/channels/stop",
        serde_json::json!({
            "instance_id": normalized_instance_id,
        }),
        resolved_sidecar_base_url,
    )
    .await?;
    Ok(())
}

pub async fn replay_channel_events_with_pool(
    pool: &SqlitePool,
    instance_id: String,
    limit: Option<u32>,
    sidecar_base_url: Option<String>,
) -> Result<Vec<ImEvent>, String> {
    let resolved_sidecar_base_url =
        resolve_channel_connector_sidecar_base_url(pool, sidecar_base_url).await?;
    let response = call_sidecar_json(
        "/api/channels/replay-events",
        serde_json::json!({
            "instance_id": instance_id,
            "limit": limit.unwrap_or(50),
        }),
        resolved_sidecar_base_url,
    )
    .await?;

    let normalized: Vec<serde_json::Value> = serde_json::from_value(response)
        .map_err(|e| format!("parse replayed channel events failed: {}", e))?;
    let events = normalized
        .into_iter()
        .map(|event| parse_normalized_im_event_value(&event))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(events)
}

async fn replay_normalized_channel_event_values_with_pool(
    pool: &SqlitePool,
    instance_id: String,
    limit: Option<u32>,
    sidecar_base_url: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let resolved_sidecar_base_url =
        resolve_channel_connector_sidecar_base_url(pool, sidecar_base_url).await?;
    let response = call_sidecar_json(
        "/api/channels/replay-events",
        serde_json::json!({
            "instance_id": instance_id,
            "limit": limit.unwrap_or(50),
        }),
        resolved_sidecar_base_url,
    )
    .await?;

    serde_json::from_value(response)
        .map_err(|e| format!("parse replayed channel events failed: {}", e))
}

pub async fn sync_channel_connector_events_with_pool_and_app(
    pool: &SqlitePool,
    app: &AppHandle,
    host_runtime_state: Option<&ImChannelHostRuntimeState>,
    instance_id: String,
    limit: Option<u32>,
    ack_status: Option<String>,
    sidecar_base_url: Option<String>,
) -> Result<usize, String> {
    let resolved_sidecar_base_url =
        resolve_channel_connector_sidecar_base_url(pool, sidecar_base_url).await?;
    let normalized_events = replay_normalized_channel_event_values_with_pool(
        pool,
        instance_id.clone(),
        limit,
        resolved_sidecar_base_url.clone(),
    )
    .await?;
    let mut synced = 0usize;

    for normalized_event in normalized_events {
        let event = parse_normalized_im_event_value(&normalized_event)?;
        let message_id = event
            .message_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if event.channel.eq_ignore_ascii_case("wecom") {
            let mut runtime_status = WecomRuntimeAdapterStatus::default();
            let mut runtime_event = normalized_event.clone();
            if let Some(object) = runtime_event.as_object_mut() {
                object.insert(
                    "event".to_string(),
                    serde_json::Value::String("dispatch_request".to_string()),
                );
            }
            handle_openclaw_plugin_wecom_runtime_stdout_line_with_bridge(
                pool,
                app,
                &mut runtime_status,
                &runtime_event.to_string(),
                &now_rfc3339,
            );
            if let Some(host_runtime_state) = host_runtime_state {
                let _ = merge_wecom_host_runtime_status_in_state(host_runtime_state, &runtime_status);
            }
        } else {
            dispatch_im_inbound_to_workclaw_with_pool_and_app(pool, app, &event).await?;
        }
        synced += 1;
        if let Some(message_id) = message_id {
            ack_channel_events_with_pool(
                pool,
                instance_id.clone(),
                vec![message_id],
                ack_status.clone(),
                resolved_sidecar_base_url.clone(),
            )
            .await?;
        }
    }

    Ok(synced)
}

pub async fn start_channel_connector_monitor_with_pool_and_app(
    pool: &SqlitePool,
    state: ChannelConnectorMonitorState,
    host_runtime_state: Option<ImChannelHostRuntimeState>,
    app: AppHandle,
    instance_id: String,
    interval_ms: Option<u64>,
    limit: Option<u32>,
    ack_status: Option<String>,
    sidecar_base_url: Option<String>,
) -> Result<ChannelConnectorMonitorStatus, String> {
    let normalized_instance_id = instance_id.trim().to_string();
    if normalized_instance_id.is_empty() {
        return Err("instance_id is required".to_string());
    }

    let tick_ms = interval_ms.unwrap_or(1500).clamp(200, 30_000);
    let batch_limit = limit.unwrap_or(50).clamp(1, 500);

    {
        let mut config = state
            .config
            .lock()
            .map_err(|_| "failed to lock channel connector monitor config".to_string())?;
        config.instance_id = Some(normalized_instance_id.clone());
        config.ack_status = ack_status.clone();
        config.sidecar_base_url = sidecar_base_url.clone();
    }
    state.interval_ms.store(tick_ms, Ordering::SeqCst);
    state.limit.store(batch_limit as u64, Ordering::SeqCst);
    if let Ok(mut guard) = state.last_error.lock() {
        *guard = None;
    }

    if state.running.swap(true, Ordering::SeqCst) {
        return Ok(channel_connector_monitor_status(&state));
    }

    let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let pool = pool.clone();
    let worker_state = state.clone();

    tauri::async_runtime::spawn(async move {
        loop {
            if worker_state.generation.load(Ordering::SeqCst) != generation {
                break;
            }

            let config = worker_state
                .config
                .lock()
                .ok()
                .map(|guard| guard.clone())
                .unwrap_or_default();
            let Some(instance_id) = config.instance_id.clone() else {
                if let Ok(mut guard) = worker_state.last_error.lock() {
                    *guard = Some("channel connector monitor missing instance_id".to_string());
                }
                break;
            };

            match sync_channel_connector_events_with_pool_and_app(
                &pool,
                &app,
                host_runtime_state.as_ref(),
                instance_id.clone(),
                Some(worker_state.limit.load(Ordering::SeqCst).clamp(1, 500) as u32),
                config.ack_status.clone(),
                config.sidecar_base_url.clone(),
            )
            .await
            {
                Ok(count) => {
                    if count > 0 {
                        worker_state.total_synced.fetch_add(count, Ordering::SeqCst);
                    }
                    if let Ok(mut guard) = worker_state.last_error.lock() {
                        *guard = None;
                    }
                    if let Ok(mut guard) = worker_state.last_synced_at.lock() {
                        *guard = Some(now_rfc3339());
                    }
                    if let Some(host_runtime_state) = host_runtime_state.as_ref() {
                        let _ = refresh_channel_host_runtime_status_with_pool(
                            &pool,
                            host_runtime_state,
                            &instance_id,
                            &worker_state,
                            config.sidecar_base_url.clone(),
                        )
                        .await;
                    }
                }
                Err(error) => {
                    if let Ok(mut guard) = worker_state.last_error.lock() {
                        *guard = Some(error);
                    }
                    if let Some(host_runtime_state) = host_runtime_state.as_ref() {
                        let _ = refresh_channel_host_runtime_status_with_pool(
                            &pool,
                            host_runtime_state,
                            &instance_id,
                            &worker_state,
                            config.sidecar_base_url.clone(),
                        )
                        .await;
                    }
                }
            }

            let sleep_ms = worker_state
                .interval_ms
                .load(Ordering::SeqCst)
                .clamp(200, 30_000);
            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        }

        if worker_state.generation.load(Ordering::SeqCst) == generation {
            worker_state.running.store(false, Ordering::SeqCst);
        }
    });

    Ok(channel_connector_monitor_status(&state))
}

pub fn stop_channel_connector_monitor_in_state(
    state: ChannelConnectorMonitorState,
) -> ChannelConnectorMonitorStatus {
    state.generation.fetch_add(1, Ordering::SeqCst);
    state.running.store(false, Ordering::SeqCst);
    channel_connector_monitor_status(&state)
}

pub fn get_channel_connector_monitor_status_in_state(
    state: &ChannelConnectorMonitorState,
) -> ChannelConnectorMonitorStatus {
    channel_connector_monitor_status(state)
}

#[tauri::command]
pub async fn list_channel_connectors(
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<Vec<ChannelConnectorDescriptor>, String> {
    list_channel_connectors_with_pool(&db.0, sidecar_base_url).await
}

#[tauri::command]
pub async fn get_channel_connector_diagnostics(
    instance_id: String,
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<ChannelConnectorDiagnostics, String> {
    get_channel_connector_diagnostics_with_pool(&db.0, instance_id, sidecar_base_url).await
}

#[tauri::command]
pub async fn ack_channel_events(
    instance_id: String,
    message_ids: Vec<String>,
    status: Option<String>,
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    ack_channel_events_with_pool(&db.0, instance_id, message_ids, status, sidecar_base_url).await
}

#[tauri::command]
pub async fn replay_channel_events(
    instance_id: String,
    limit: Option<u32>,
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<Vec<ImEvent>, String> {
    replay_channel_events_with_pool(&db.0, instance_id, limit, sidecar_base_url).await
}

#[tauri::command]
pub async fn sync_channel_connector_events(
    instance_id: String,
    limit: Option<u32>,
    ack_status: Option<String>,
    sidecar_base_url: Option<String>,
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<usize, String> {
    sync_channel_connector_events_with_pool_and_app(
        &db.0,
        &app,
        None,
        instance_id,
        limit,
        ack_status,
        sidecar_base_url,
    )
    .await
}

#[tauri::command]
pub async fn start_channel_connector_monitor(
    instance_id: String,
    interval_ms: Option<u64>,
    limit: Option<u32>,
    ack_status: Option<String>,
    sidecar_base_url: Option<String>,
    app: AppHandle,
    db: State<'_, DbState>,
    monitor: State<'_, ChannelConnectorMonitorState>,
) -> Result<ChannelConnectorMonitorStatus, String> {
    start_channel_connector_monitor_with_pool_and_app(
        &db.0,
        monitor.inner().clone(),
        None,
        app,
        instance_id,
        interval_ms,
        limit,
        ack_status,
        sidecar_base_url,
    )
    .await
}

#[tauri::command]
pub async fn stop_channel_connector_monitor(
    monitor: State<'_, ChannelConnectorMonitorState>,
) -> Result<ChannelConnectorMonitorStatus, String> {
    Ok(stop_channel_connector_monitor_in_state(
        monitor.inner().clone(),
    ))
}

#[tauri::command]
pub async fn get_channel_connector_monitor_status(
    monitor: State<'_, ChannelConnectorMonitorState>,
) -> Result<ChannelConnectorMonitorStatus, String> {
    Ok(get_channel_connector_monitor_status_in_state(
        monitor.inner(),
    ))
}
