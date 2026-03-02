use crate::commands::im_gateway::{process_im_event, FeishuCallbackResult};
use crate::commands::im_config::get_thread_role_config_with_pool;
use crate::commands::skills::DbState;
use crate::im::runtime_bridge::{build_im_role_event_payload, ImRoleEventPayload};
use crate::im::types::{ImEvent, ImEventType};
use sqlx::SqlitePool;
use tauri::Emitter;
use tauri::State;

#[derive(Debug, Clone, serde::Deserialize)]
struct OpenClawEnvelope {
    event: serde_json::Value,
}

pub fn parse_openclaw_payload(payload: &str) -> Result<ImEvent, String> {
    let raw_value: serde_json::Value = serde_json::from_str(payload)
        .map_err(|e| format!("invalid openclaw payload: {}", e))?;

    let candidate = if raw_value.get("event").is_some() {
        let wrapped: OpenClawEnvelope = serde_json::from_value(raw_value)
            .map_err(|e| format!("invalid openclaw envelope: {}", e))?;
        wrapped.event
    } else {
        raw_value
    };

    if let Ok(raw) = serde_json::from_value::<OpenClawRawEvent>(candidate.clone()) {
        return map_raw_event(raw);
    }

    serde_json::from_value::<ImEvent>(candidate)
        .map_err(|e| format!("invalid openclaw event shape: {}", e))
}

#[derive(Debug, Clone, serde::Deserialize)]
struct OpenClawRawEvent {
    event_type: Option<String>,
    event_id: Option<String>,
    thread_id: Option<String>,
    message_id: Option<String>,
    text: Option<String>,
    tenant_id: Option<String>,
    mention_role: Option<String>,
    message: Option<OpenClawMessage>,
    chat: Option<OpenClawChat>,
    sender: Option<OpenClawSender>,
    mentions: Option<Vec<OpenClawMention>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct OpenClawMessage {
    id: Option<String>,
    text: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct OpenClawChat {
    id: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct OpenClawSender {
    id: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct OpenClawMention {
    mention_type: Option<String>,
    #[serde(rename = "type")]
    mention_type_alt: Option<String>,
    id: Option<String>,
}

fn map_raw_event(raw: OpenClawRawEvent) -> Result<ImEvent, String> {
    let event_type = parse_event_type(raw.event_type.as_deref().unwrap_or("message.created"))?;
    let thread_id = raw
        .thread_id
        .or_else(|| raw.chat.and_then(|c| c.id))
        .ok_or_else(|| "openclaw event missing thread/chat id".to_string())?;

    let message_id = raw
        .message_id
        .or_else(|| raw.message.as_ref().and_then(|m| m.id.clone()));
    let text = raw
        .text
        .or_else(|| raw.message.as_ref().and_then(|m| m.text.clone()))
        .or_else(|| raw.message.as_ref().and_then(|m| m.content.clone()));

    let role_id = raw.mention_role.or_else(|| {
        raw.mentions
            .unwrap_or_default()
            .into_iter()
            .find(|m| {
                let t = m.mention_type.as_deref().or(m.mention_type_alt.as_deref());
                t == Some("role")
            })
            .and_then(|m| m.id)
    });

    Ok(ImEvent {
        event_type,
        thread_id,
        event_id: raw.event_id,
        message_id,
        text,
        role_id,
        tenant_id: raw.tenant_id.or_else(|| raw.sender.and_then(|s| s.id)),
    })
}

fn parse_event_type(raw: &str) -> Result<ImEventType, String> {
    match raw {
        "message.created" => Ok(ImEventType::MessageCreated),
        "mention.role" => Ok(ImEventType::MentionRole),
        "command.pause" => Ok(ImEventType::CommandPause),
        "command.resume" => Ok(ImEventType::CommandResume),
        "human.override" => Ok(ImEventType::HumanOverride),
        _ => Err(format!("unsupported event_type: {}", raw)),
    }
}

pub async fn validate_openclaw_auth_with_pool(
    pool: &SqlitePool,
    auth_token: Option<String>,
) -> Result<(), String> {
    let configured: Option<(String,)> =
        sqlx::query_as("SELECT value FROM app_settings WHERE key = 'openclaw_ingress_token' LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

    let expected = configured.map(|(v,)| v).unwrap_or_default();
    if expected.trim().is_empty() {
        return Ok(());
    }

    let actual = auth_token.unwrap_or_default();
    if actual == expected {
        Ok(())
    } else {
        Err("openclaw auth token invalid".to_string())
    }
}

pub async fn plan_role_events_for_openclaw(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<ImRoleEventPayload>, String> {
    if event.event_type != ImEventType::MessageCreated && event.event_type != ImEventType::MentionRole {
        return Ok(Vec::new());
    }

    let cfg = match get_thread_role_config_with_pool(pool, &event.thread_id).await {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()),
    };

    let roles: Vec<String> = if let Some(role_id) = event.role_id.clone() {
        if cfg.roles.iter().any(|r| r == &role_id) {
            vec![role_id]
        } else {
            Vec::new()
        }
    } else {
        cfg.roles
    };

    let text = event.text.clone().unwrap_or_default();
    let session_id = format!("im-{}", event.thread_id);
    Ok(roles
        .into_iter()
        .map(|role_id| {
            build_im_role_event_payload(
                &session_id,
                &event.thread_id,
                &role_id,
                &role_id,
                "running",
                &format!("OpenClaw事件触发：{}", text),
                None,
            )
        })
        .collect())
}

#[tauri::command]
pub async fn handle_openclaw_event(
    payload: String,
    auth_token: Option<String>,
    app: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<FeishuCallbackResult, String> {
    let event = parse_openclaw_payload(&payload)?;
    validate_openclaw_auth_with_pool(&db.0, auth_token).await?;
    let result = process_im_event(&db.0, event.clone()).await?;
    if !result.deduped {
        let planned = plan_role_events_for_openclaw(&db.0, &event).await?;
        for evt in planned {
            let _ = app.emit("im-role-event", evt);
        }
    }
    Ok(result)
}
