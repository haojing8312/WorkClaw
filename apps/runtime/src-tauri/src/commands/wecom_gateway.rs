use crate::commands::feishu_gateway::{call_sidecar_json, get_app_setting, set_app_setting};
use crate::commands::skills::DbState;
use sqlx::SqlitePool;
use tauri::State;

const DEFAULT_WECOM_CONNECTOR_ID: &str = "wecom-main";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct WecomGatewaySettings {
    pub corp_id: String,
    pub agent_id: String,
    pub agent_secret: String,
    pub sidecar_base_url: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct WecomConnectorStatus {
    pub running: bool,
    pub started_at: Option<String>,
    pub last_error: Option<String>,
    pub reconnect_attempts: i64,
    pub queue_depth: i64,
    pub instance_id: String,
}

pub async fn resolve_wecom_sidecar_base_url(
    pool: &SqlitePool,
    explicit: Option<String>,
) -> Result<Option<String>, String> {
    if let Some(value) = explicit {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed.to_string()));
        }
    }

    if let Some(value) = get_app_setting(pool, "wecom_sidecar_base_url").await? {
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

pub async fn resolve_wecom_credentials(
    pool: &SqlitePool,
    corp_id: Option<String>,
    agent_id: Option<String>,
    agent_secret: Option<String>,
) -> Result<(String, String, String), String> {
    let stored_corp_id = get_app_setting(pool, "wecom_corp_id")
        .await?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let stored_agent_id = get_app_setting(pool, "wecom_agent_id")
        .await?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let stored_agent_secret = get_app_setting(pool, "wecom_agent_secret")
        .await?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let resolved_corp_id = corp_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or(stored_corp_id)
        .ok_or_else(|| "missing wecom corp_id".to_string())?;

    let resolved_agent_id = agent_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or(stored_agent_id)
        .ok_or_else(|| "missing wecom agent_id".to_string())?;

    let resolved_agent_secret = agent_secret
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or(stored_agent_secret)
        .ok_or_else(|| "missing wecom agent_secret".to_string())?;

    Ok((resolved_corp_id, resolved_agent_id, resolved_agent_secret))
}

pub async fn start_wecom_connector_with_pool(
    pool: &SqlitePool,
    sidecar_base_url: Option<String>,
    corp_id: Option<String>,
    agent_id: Option<String>,
    agent_secret: Option<String>,
) -> Result<String, String> {
    let (resolved_corp_id, resolved_agent_id, resolved_agent_secret) =
        resolve_wecom_credentials(pool, corp_id, agent_id, agent_secret).await?;
    let resolved_sidecar_base_url = resolve_wecom_sidecar_base_url(pool, sidecar_base_url).await?;
    let response = call_sidecar_json(
        "/api/channels/start",
        serde_json::json!({
            "adapter_name": "wecom",
            "connector_id": DEFAULT_WECOM_CONNECTOR_ID,
            "settings": {
                "corp_id": resolved_corp_id,
                "agent_id": resolved_agent_id,
                "agent_secret": resolved_agent_secret,
            }
        }),
        resolved_sidecar_base_url,
    )
    .await?;

    response
        .get("instance_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "sidecar did not return wecom instance_id".to_string())
}

pub async fn get_wecom_connector_status_with_pool(
    pool: &SqlitePool,
    sidecar_base_url: Option<String>,
) -> Result<WecomConnectorStatus, String> {
    let resolved_sidecar_base_url = resolve_wecom_sidecar_base_url(pool, sidecar_base_url).await?;
    let instance_id = format!("wecom:{}", DEFAULT_WECOM_CONNECTOR_ID);
    let response = call_sidecar_json(
        "/api/channels/health",
        serde_json::json!({
            "instance_id": instance_id,
        }),
        resolved_sidecar_base_url,
    )
    .await?;

    Ok(WecomConnectorStatus {
        running: response
            .get("state")
            .and_then(serde_json::Value::as_str)
            .map(|value| value == "running" || value == "starting" || value == "degraded")
            .unwrap_or(false),
        started_at: response
            .get("last_ok_at")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        last_error: response
            .get("last_error")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty()),
        reconnect_attempts: response
            .get("reconnect_attempts")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0),
        queue_depth: response
            .get("queue_depth")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0),
        instance_id: response
            .get("instance_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("wecom:{}", DEFAULT_WECOM_CONNECTOR_ID)),
    })
}

pub async fn send_wecom_text_message_with_pool(
    pool: &SqlitePool,
    conversation_id: String,
    text: String,
    sidecar_base_url: Option<String>,
) -> Result<String, String> {
    let resolved_sidecar_base_url = resolve_wecom_sidecar_base_url(pool, sidecar_base_url).await?;
    let response = call_sidecar_json(
        "/api/channels/send-message",
        serde_json::json!({
            "instance_id": format!("wecom:{}", DEFAULT_WECOM_CONNECTOR_ID),
            "request": {
                "thread_id": conversation_id,
                "reply_target": conversation_id,
                "text": text,
            }
        }),
        resolved_sidecar_base_url,
    )
    .await?;
    Ok(response.to_string())
}

#[tauri::command]
pub async fn set_wecom_gateway_settings(
    settings: WecomGatewaySettings,
    db: State<'_, DbState>,
) -> Result<(), String> {
    set_app_setting(&db.0, "wecom_corp_id", settings.corp_id.as_str()).await?;
    set_app_setting(&db.0, "wecom_agent_id", settings.agent_id.as_str()).await?;
    set_app_setting(&db.0, "wecom_agent_secret", settings.agent_secret.as_str()).await?;
    set_app_setting(
        &db.0,
        "wecom_sidecar_base_url",
        settings.sidecar_base_url.as_str(),
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn get_wecom_gateway_settings(
    db: State<'_, DbState>,
) -> Result<WecomGatewaySettings, String> {
    Ok(WecomGatewaySettings {
        corp_id: get_app_setting(&db.0, "wecom_corp_id")
            .await?
            .unwrap_or_default(),
        agent_id: get_app_setting(&db.0, "wecom_agent_id")
            .await?
            .unwrap_or_default(),
        agent_secret: get_app_setting(&db.0, "wecom_agent_secret")
            .await?
            .unwrap_or_default(),
        sidecar_base_url: get_app_setting(&db.0, "wecom_sidecar_base_url")
            .await?
            .unwrap_or_default(),
    })
}

#[tauri::command]
pub async fn start_wecom_connector(
    sidecar_base_url: Option<String>,
    corp_id: Option<String>,
    agent_id: Option<String>,
    agent_secret: Option<String>,
    db: State<'_, DbState>,
) -> Result<String, String> {
    start_wecom_connector_with_pool(&db.0, sidecar_base_url, corp_id, agent_id, agent_secret).await
}

#[tauri::command]
pub async fn get_wecom_connector_status(
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<WecomConnectorStatus, String> {
    get_wecom_connector_status_with_pool(&db.0, sidecar_base_url).await
}

#[tauri::command]
pub async fn send_wecom_text_message(
    conversation_id: String,
    text: String,
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<String, String> {
    send_wecom_text_message_with_pool(&db.0, conversation_id, text, sidecar_base_url).await
}
