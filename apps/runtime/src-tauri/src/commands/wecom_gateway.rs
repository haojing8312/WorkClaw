use crate::commands::channel_connectors::{
    start_channel_connector_monitor_with_pool_and_app, stop_channel_connector_monitor_in_state,
    stop_channel_connector_with_pool, ChannelConnectorMonitorState,
};
use crate::commands::feishu_gateway::{call_sidecar_json, get_app_setting, set_app_setting};
use crate::commands::im_host::{
    get_im_channel_runtime_status_in_state, record_im_channel_runtime_status,
    ImChannelHostRuntimeState,
};
use crate::commands::im_host::lifecycle::{
    SessionLifecycleDispatch, SessionProcessingStopDispatch,
};
use crate::commands::openclaw_plugins::{
    build_wecom_runtime_status_value, handle_openclaw_plugin_wecom_runtime_stdout_line,
    parse_wecom_runtime_status_value,
};
use crate::commands::im_host::{
    build_sidecar_channel_instance_id, build_sidecar_text_message_request,
    parse_sidecar_channel_health,
};
use crate::commands::skills::DbState;
use sqlx::SqlitePool;
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

const DEFAULT_WECOM_ADAPTER_NAME: &str = "wecom";
const DEFAULT_WECOM_CONNECTOR_ID: &str = "wecom-main";

#[path = "wecom_gateway/outbound_service.rs"]
mod outbound_service;
#[path = "wecom_gateway/interactive_service.rs"]
mod interactive_service;
pub(crate) use outbound_service::{
    execute_registered_wecom_reply_plan_with_pool,
    maybe_emit_registered_wecom_lifecycle_phase_for_session_with_pool,
    maybe_stop_registered_wecom_processing_for_session_with_pool,
};
pub(crate) use interactive_service::{
    notify_wecom_approval_requested_with_pool, notify_wecom_approval_resolved_with_pool,
    notify_wecom_ask_user_requested_with_pool,
};

pub type WecomOutboundSendHook = dyn Fn(
        &str,
        &str,
    ) -> Result<serde_json::Value, String>
    + Send
    + Sync;

pub(crate) type WecomProcessingStopHook = dyn Fn(&SessionProcessingStopDispatch) -> Result<(), String>
    + Send
    + Sync;

pub(crate) type WecomLifecycleEventHook = dyn Fn(&SessionLifecycleDispatch) -> Result<(), String>
    + Send
    + Sync;

fn wecom_outbound_send_hook_slot() -> &'static Mutex<Option<Arc<WecomOutboundSendHook>>> {
    static SLOT: OnceLock<Mutex<Option<Arc<WecomOutboundSendHook>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn wecom_processing_stop_hook_slot() -> &'static Mutex<Option<Arc<WecomProcessingStopHook>>> {
    static SLOT: OnceLock<Mutex<Option<Arc<WecomProcessingStopHook>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn wecom_lifecycle_event_hook_slot() -> &'static Mutex<Option<Arc<WecomLifecycleEventHook>>> {
    static SLOT: OnceLock<Mutex<Option<Arc<WecomLifecycleEventHook>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
#[doc(hidden)]
pub(crate) fn set_wecom_outbound_send_hook_for_tests(hook: Option<Arc<WecomOutboundSendHook>>) {
    if let Ok(mut guard) = wecom_outbound_send_hook_slot().lock() {
        *guard = hook;
    }
}

#[cfg(test)]
#[doc(hidden)]
pub(crate) fn set_wecom_processing_stop_hook_for_tests(hook: Option<Arc<WecomProcessingStopHook>>) {
    if let Ok(mut guard) = wecom_processing_stop_hook_slot().lock() {
        *guard = hook;
    }
}

#[cfg(test)]
#[doc(hidden)]
pub(crate) fn set_wecom_lifecycle_event_hook_for_tests(hook: Option<Arc<WecomLifecycleEventHook>>) {
    if let Ok(mut guard) = wecom_lifecycle_event_hook_slot().lock() {
        *guard = hook;
    }
}

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
    pub state: String,
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

pub async fn get_wecom_gateway_settings_with_pool(
    pool: &SqlitePool,
) -> Result<WecomGatewaySettings, String> {
    Ok(WecomGatewaySettings {
        corp_id: get_app_setting(pool, "wecom_corp_id")
            .await?
            .unwrap_or_default(),
        agent_id: get_app_setting(pool, "wecom_agent_id")
            .await?
            .unwrap_or_default(),
        agent_secret: get_app_setting(pool, "wecom_agent_secret")
            .await?
            .unwrap_or_default(),
        sidecar_base_url: get_app_setting(pool, "wecom_sidecar_base_url")
            .await?
            .unwrap_or_default(),
    })
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
            "adapter_name": DEFAULT_WECOM_ADAPTER_NAME,
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
    let instance_id =
        build_sidecar_channel_instance_id(DEFAULT_WECOM_ADAPTER_NAME, DEFAULT_WECOM_CONNECTOR_ID);
    let response = call_sidecar_json(
        "/api/channels/health",
        serde_json::json!({
            "instance_id": instance_id,
        }),
        resolved_sidecar_base_url,
    )
    .await?;
    let snapshot = parse_sidecar_channel_health(&response, &instance_id);

    Ok(WecomConnectorStatus {
        running: snapshot.running,
        state: snapshot.state,
        started_at: snapshot.started_at,
        last_error: snapshot.last_error,
        reconnect_attempts: snapshot.reconnect_attempts,
        queue_depth: snapshot.queue_depth,
        instance_id: snapshot.instance_id,
    })
}

pub async fn send_wecom_text_message_with_pool(
    pool: &SqlitePool,
    conversation_id: String,
    text: String,
    host_runtime_state: Option<&ImChannelHostRuntimeState>,
    sidecar_base_url: Option<String>,
) -> Result<String, String> {
    if let Some(host_runtime_state) = host_runtime_state {
        let _ = record_wecom_runtime_event_in_state(
            host_runtime_state,
            &serde_json::json!({
                "event": "reply_lifecycle",
                "phase": "processing_started",
                "threadId": conversation_id,
            })
            .to_string(),
        );
    }

    let resolved_sidecar_base_url = resolve_wecom_sidecar_base_url(pool, sidecar_base_url).await?;
    let instance_id =
        build_sidecar_channel_instance_id(DEFAULT_WECOM_ADAPTER_NAME, DEFAULT_WECOM_CONNECTOR_ID);
    let request_payload = build_sidecar_text_message_request(&instance_id, &conversation_id, &text);
    let test_hook = wecom_outbound_send_hook_slot()
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    let response = if let Some(hook) = test_hook {
        hook(&conversation_id, &text)
    } else {
        call_sidecar_json(
            "/api/channels/send-message",
            request_payload,
            resolved_sidecar_base_url,
        )
        .await
    };
    match response {
        Ok(response) => {
            if let Some(host_runtime_state) = host_runtime_state {
                let message_id = response
                    .get("message_id")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| response.get("msgid").and_then(serde_json::Value::as_str))
                    .unwrap_or("");
                let _ = record_wecom_runtime_event_in_state(
                    host_runtime_state,
                    &serde_json::json!({
                        "event": "send_result",
                        "threadId": conversation_id,
                        "messageId": message_id,
                    })
                    .to_string(),
                );
                let _ = record_wecom_runtime_event_in_state(
                    host_runtime_state,
                    &serde_json::json!({
                        "event": "reply_lifecycle",
                        "phase": "fully_complete",
                        "threadId": conversation_id,
                        "messageId": message_id,
                    })
                    .to_string(),
                );
            }
            Ok(response.to_string())
        }
        Err(error) => {
            if let Some(host_runtime_state) = host_runtime_state {
                let _ = record_wecom_runtime_event_in_state(
                    host_runtime_state,
                    &serde_json::json!({
                        "event": "command_error",
                        "error": error,
                    })
                    .to_string(),
                );
                let _ = record_wecom_runtime_event_in_state(
                    host_runtime_state,
                    &serde_json::json!({
                        "event": "reply_lifecycle",
                        "phase": "failed",
                        "threadId": conversation_id,
                    })
                    .to_string(),
                );
            }
            Err(error)
        }
    }
}

fn record_wecom_runtime_event_in_state(
    host_runtime_state: &ImChannelHostRuntimeState,
    trimmed: &str,
) -> Result<(), String> {
    let mut merged = get_im_channel_runtime_status_in_state(host_runtime_state, "wecom")?
        .as_ref()
        .map(parse_wecom_runtime_status_value)
        .transpose()?
        .unwrap_or_default();
    handle_openclaw_plugin_wecom_runtime_stdout_line(&mut merged, trimmed, &|| {
        chrono::Utc::now().to_rfc3339()
    });
    record_im_channel_runtime_status(
        host_runtime_state,
        "wecom",
        build_wecom_runtime_status_value(&merged),
    )
}

pub async fn stop_wecom_connector_with_pool(
    pool: &SqlitePool,
    sidecar_base_url: Option<String>,
) -> Result<(), String> {
    let instance_id =
        build_sidecar_channel_instance_id(DEFAULT_WECOM_ADAPTER_NAME, DEFAULT_WECOM_CONNECTOR_ID);
    stop_channel_connector_with_pool(pool, instance_id, sidecar_base_url).await
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
    get_wecom_gateway_settings_with_pool(&db.0).await
}

#[tauri::command]
pub async fn start_wecom_connector(
    sidecar_base_url: Option<String>,
    corp_id: Option<String>,
    agent_id: Option<String>,
    agent_secret: Option<String>,
    app: AppHandle,
    db: State<'_, DbState>,
    monitor: State<'_, ChannelConnectorMonitorState>,
    host_runtime_state: State<'_, ImChannelHostRuntimeState>,
) -> Result<String, String> {
    let instance_id = start_wecom_connector_with_pool(
        &db.0,
        sidecar_base_url.clone(),
        corp_id,
        agent_id,
        agent_secret,
    )
    .await?;
    let _ = start_channel_connector_monitor_with_pool_and_app(
        &db.0,
        monitor.inner().clone(),
        Some(host_runtime_state.inner().clone()),
        app,
        instance_id.clone(),
        Some(1500),
        Some(50),
        Some("processed".to_string()),
        sidecar_base_url,
    )
    .await;
    Ok(instance_id)
}

#[tauri::command]
pub async fn stop_wecom_connector(
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
    monitor: State<'_, ChannelConnectorMonitorState>,
) -> Result<(), String> {
    let _ = stop_channel_connector_monitor_in_state(monitor.inner().clone());
    stop_wecom_connector_with_pool(&db.0, sidecar_base_url).await
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
    host_runtime_state: State<'_, ImChannelHostRuntimeState>,
) -> Result<String, String> {
    send_wecom_text_message_with_pool(
        &db.0,
        conversation_id,
        text,
        Some(host_runtime_state.inner()),
        sidecar_base_url,
    )
    .await
}
