use crate::commands::feishu_gateway::{call_sidecar_json, get_app_setting};
use crate::commands::skills::DbState;
use crate::im::types::ImEvent;
use sqlx::SqlitePool;
use tauri::State;

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
    serde_json::from_value(response).map_err(|e| format!("parse channel connector catalog failed: {}", e))
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
        .map(|event| ImEvent {
            channel: event
                .get("channel")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("app")
                .to_string(),
            event_type: crate::im::types::ImEventType::MessageCreated,
            thread_id: event
                .get("thread_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            event_id: None,
            message_id: event
                .get("message_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            text: event
                .get("text")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            role_id: None,
            account_id: event
                .get("account_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            tenant_id: event
                .get("workspace_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
        })
        .collect::<Vec<_>>();
    Ok(events)
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
