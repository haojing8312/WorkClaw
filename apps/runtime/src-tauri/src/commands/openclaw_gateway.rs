use crate::commands::feishu_gateway::{call_sidecar_json, resolve_feishu_sidecar_base_url};
use crate::commands::im_config::get_thread_role_config_with_pool;
use crate::commands::im_gateway::FeishuCallbackResult;
use crate::commands::im_host::dispatch_im_inbound_to_workclaw_with_pool_and_app;
use crate::commands::im_routing::list_im_routing_bindings_with_pool;
use crate::commands::skills::DbState;
use crate::im::runtime_bridge::{
    build_im_role_dispatch_request_for_channel, build_im_role_event_payload, ImRoleDispatchRequest,
    ImRoleEventPayload,
};
use crate::im::types::{ImEvent, ImEventType};
use sqlx::SqlitePool;
use tauri::State;

#[derive(Debug, Clone, serde::Deserialize)]
struct OpenClawEnvelope {
    event: serde_json::Value,
}

pub fn parse_openclaw_payload(payload: &str) -> Result<ImEvent, String> {
    let raw_value: serde_json::Value =
        serde_json::from_str(payload).map_err(|e| format!("invalid openclaw payload: {}", e))?;

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
    channel: Option<String>,
    event_type: Option<String>,
    event_id: Option<String>,
    thread_id: Option<String>,
    chat_type: Option<String>,
    message_id: Option<String>,
    text: Option<String>,
    account_id: Option<String>,
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
    #[serde(rename = "type")]
    chat_type: Option<String>,
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
    let chat = raw.chat.clone();
    let thread_id = raw
        .thread_id
        .or_else(|| chat.as_ref().and_then(|c| c.id.clone()))
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
        channel: raw.channel.unwrap_or_else(|| "app".to_string()),
        event_type,
        thread_id,
        event_id: raw.event_id,
        message_id,
        text,
        role_id,
        account_id: raw.account_id.clone().or_else(|| raw.tenant_id.clone()),
        tenant_id: raw
            .tenant_id
            .clone()
            .or_else(|| raw.sender.as_ref().and_then(|s| s.id.clone())),
        sender_id: raw.sender.and_then(|s| s.id),
        chat_type: raw.chat_type.or_else(|| chat.and_then(|c| c.chat_type)),
    })
}

fn peer_kind_for_event(event: &ImEvent) -> &'static str {
    match event.chat_type.as_deref().map(str::trim) {
        Some("p2p") | Some("direct") => "direct",
        _ => "group",
    }
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
    let configured: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'openclaw_ingress_token' LIMIT 1",
    )
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
    if event.event_type != ImEventType::MessageCreated
        && event.event_type != ImEventType::MentionRole
    {
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
            let mut payload = build_im_role_event_payload(
                &session_id,
                &event.thread_id,
                &role_id,
                &role_id,
                "running",
                &format!("OpenClaw事件触发：{}", text),
                None,
            );
            payload.source_channel = if event.channel.trim().is_empty() {
                "app".to_string()
            } else {
                event.channel.trim().to_lowercase()
            };
            payload
        })
        .collect())
}

pub async fn plan_role_dispatch_requests_for_openclaw(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<ImRoleDispatchRequest>, String> {
    if event.event_type != ImEventType::MessageCreated
        && event.event_type != ImEventType::MentionRole
    {
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

    let session_id = format!("im-{}", event.thread_id);
    let user_text = event
        .text
        .clone()
        .unwrap_or_else(|| "请基于当前上下文继续协作".to_string());
    let agent_type = if cfg.scenario_template == "opportunity_review" {
        "plan"
    } else {
        "general-purpose"
    };
    let source_channel = if event.channel.trim().is_empty() {
        "app".to_string()
    } else {
        event.channel.trim().to_lowercase()
    };

    Ok(roles
        .into_iter()
        .map(|role_id| {
            let mut req = build_im_role_dispatch_request_for_channel(
                &session_id,
                &event.thread_id,
                &role_id,
                &role_id,
                &source_channel,
                &format!("场景={}。用户输入：{}", cfg.scenario_template, user_text),
                agent_type,
            );
            req.message_id = event.message_id.clone().unwrap_or_default();
            req
        })
        .collect())
}

fn bindings_to_openclaw_payload(
    bindings: Vec<crate::commands::im_routing::ImRoutingBinding>,
) -> Vec<serde_json::Value> {
    bindings
        .into_iter()
        .filter(|binding| binding.enabled)
        .map(|binding| {
            let mut match_obj = serde_json::json!({
                "channel": binding.channel,
                "accountId": if binding.account_id.trim().is_empty() { "*" } else { binding.account_id.trim() },
            });

            if !binding.peer_kind.trim().is_empty() && !binding.peer_id.trim().is_empty() {
                match_obj["peer"] = serde_json::json!({
                    "kind": binding.peer_kind.trim().to_lowercase(),
                    "id": binding.peer_id.trim(),
                });
            }
            if !binding.guild_id.trim().is_empty() {
                match_obj["guildId"] = serde_json::json!(binding.guild_id.trim());
            }
            if !binding.team_id.trim().is_empty() {
                match_obj["teamId"] = serde_json::json!(binding.team_id.trim());
            }
            if !binding.role_ids.is_empty() {
                match_obj["roles"] = serde_json::json!(binding.role_ids);
            }

            serde_json::json!({
                "agentId": binding.agent_id,
                "match": match_obj,
            })
        })
        .collect()
}

pub async fn resolve_openclaw_route_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<serde_json::Value, String> {
    let bindings = list_im_routing_bindings_with_pool(pool).await?;
    let default_agent_id = "main";
    let body = serde_json::json!({
        "channel": if event.channel.trim().is_empty() { "app" } else { event.channel.trim() },
        "account_id": event.account_id.clone().or_else(|| event.tenant_id.clone()).unwrap_or_default(),
        "peer": {
            "kind": peer_kind_for_event(event),
            "id": event.thread_id.clone(),
        },
        "default_agent_id": default_agent_id,
        "bindings": bindings_to_openclaw_payload(bindings),
    });
    let sidecar_base_url = resolve_feishu_sidecar_base_url(pool, None).await?;
    call_sidecar_json("/api/openclaw/resolve-route", body, sidecar_base_url).await
}

#[tauri::command]
pub async fn simulate_im_route(
    payload: serde_json::Value,
    db: State<'_, DbState>,
) -> Result<serde_json::Value, String> {
    let sidecar_base_url = resolve_feishu_sidecar_base_url(&db.0, None).await?;
    let channel = payload
        .get("channel")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("app");
    let account_id = payload
        .get("account_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let default_agent_id = payload
        .get("default_agent_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("main");
    let peer = payload.get("peer").cloned().unwrap_or_else(|| {
        serde_json::json!({
            "kind": "group",
            "id": "",
        })
    });
    let bindings = payload
        .get("bindings")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));

    let body = serde_json::json!({
        "channel": channel,
        "account_id": account_id,
        "peer": peer,
        "default_agent_id": default_agent_id,
        "bindings": bindings,
    });
    call_sidecar_json("/api/openclaw/resolve-route", body, sidecar_base_url).await
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
    dispatch_im_inbound_to_workclaw_with_pool_and_app(&db.0, &app, &event).await
}

#[cfg(test)]
mod tests {
    use super::{parse_openclaw_payload, peer_kind_for_event};
    use crate::im::types::{ImEvent, ImEventType};

    #[test]
    fn parse_openclaw_payload_preserves_sender_and_chat_context() {
        let payload = serde_json::json!({
            "channel": "feishu",
            "event_type": "message.created",
            "message_id": "om_1",
            "chat": {
                "id": "oc_1",
                "type": "group"
            },
            "sender": {
                "id": "ou_sender"
            },
            "message": {
                "id": "om_1",
                "text": "请继续推进"
            }
        });

        let parsed = parse_openclaw_payload(&payload.to_string()).expect("payload should parse");
        assert_eq!(parsed.sender_id.as_deref(), Some("ou_sender"));
        assert_eq!(parsed.chat_type.as_deref(), Some("group"));
    }

    #[test]
    fn peer_kind_for_event_uses_direct_for_p2p_messages() {
        let event = ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "ou_user_1".to_string(),
            event_id: None,
            message_id: None,
            text: Some("hello".to_string()),
            role_id: None,
            account_id: Some("default".to_string()),
            tenant_id: Some("tenant-1".to_string()),
            sender_id: Some("ou_sender".to_string()),
            chat_type: Some("p2p".to_string()),
        };

        assert_eq!(peer_kind_for_event(&event), "direct");
    }
}
