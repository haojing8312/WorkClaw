use crate::commands::approvals::load_approval_record_with_pool;
use crate::commands::chat::ApprovalManagerState;
use crate::commands::employee_agents::bridge_inbound_event_to_employee_sessions_with_pool;
use crate::commands::employee_agents::EmployeeInboundDispatchSession;
use crate::commands::feishu_gateway::{
    maybe_handle_feishu_approval_command_with_pool, parse_feishu_approval_command,
};
use crate::commands::im_gateway::{process_im_event, FeishuCallbackResult};
use crate::commands::openclaw_gateway::{
    plan_role_dispatch_requests_for_openclaw, plan_role_events_for_openclaw,
    resolve_openclaw_route_with_pool,
};
use crate::im::runtime_bridge::{
    build_im_role_dispatch_request_for_channel, build_im_role_event_payload_for_channel,
};
use crate::im::types::{ImEvent, ImEventType};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};

fn optional_non_empty_string(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn parse_normalized_im_event_value(
    value: &serde_json::Value,
) -> Result<ImEvent, String> {
    let channel = value
        .get("channel")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .unwrap_or("app")
        .to_string();
    let thread_id = value
        .get("thread_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| "normalized im event missing thread_id".to_string())?
        .to_string();
    let role_id = optional_non_empty_string(value.get("role_id"));
    let chat_type = optional_non_empty_string(value.get("chat_type")).or_else(|| {
        value
            .get("routing_context")
            .and_then(|entry| entry.get("peer"))
            .and_then(|entry| entry.get("kind"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_string)
    });

    Ok(ImEvent {
        channel,
        event_type: if role_id.is_some() {
            ImEventType::MentionRole
        } else {
            ImEventType::MessageCreated
        },
        thread_id,
        event_id: optional_non_empty_string(value.get("event_id"))
            .or_else(|| optional_non_empty_string(value.get("message_id"))),
        message_id: optional_non_empty_string(value.get("message_id")),
        text: optional_non_empty_string(value.get("text")),
        role_id,
        account_id: optional_non_empty_string(value.get("account_id")),
        tenant_id: optional_non_empty_string(value.get("workspace_id"))
            .or_else(|| optional_non_empty_string(value.get("tenant_id"))),
        sender_id: optional_non_empty_string(value.get("sender_id")),
        chat_type,
    })
}

pub(crate) fn emit_inbound_dispatch_sessions(
    app: &AppHandle,
    channel: &str,
    dispatches: &[EmployeeInboundDispatchSession],
) {
    for dispatch in dispatches {
        let _ = app.emit(
            "im-route-decision",
            serde_json::json!({
                "session_id": dispatch.session_id,
                "thread_id": dispatch.thread_id,
                "agent_id": dispatch.route_agent_id,
                "session_key": dispatch.route_session_key,
                "matched_by": dispatch.matched_by,
            }),
        );

        let _ = app.emit(
            "im-role-event",
            build_im_role_event_payload_for_channel(
                &dispatch.session_id,
                &dispatch.thread_id,
                &dispatch.role_id,
                &dispatch.employee_name,
                channel,
                "running",
                "IM 消息已同步到桌面会话，正在执行",
                None,
            ),
        );

        let _ = app.emit("im-role-dispatch-request", {
            let mut req = build_im_role_dispatch_request_for_channel(
                &dispatch.session_id,
                &dispatch.thread_id,
                &dispatch.role_id,
                &dispatch.employee_name,
                channel,
                &dispatch.prompt,
                "general-purpose",
            );
            req.message_id = dispatch.message_id.clone();
            req
        });
    }
}

pub(crate) async fn maybe_handle_registered_approval_command_with_pool_and_app(
    pool: &SqlitePool,
    app: &AppHandle,
    event: &ImEvent,
) -> Result<bool, String> {
    let Some(approval_state) = app.try_state::<ApprovalManagerState>() else {
        return Ok(false);
    };
    let Some(command) = parse_feishu_approval_command(event.text.as_deref()) else {
        return Ok(false);
    };

    if maybe_handle_feishu_approval_command_with_pool(pool, approval_state.0.as_ref(), event, None)
        .await?
        .is_none()
    {
        return Ok(false);
    }

    if let Some(record) = load_approval_record_with_pool(pool, &command.approval_id).await? {
        let _ = app.emit("approval-resolved", &record);
    }

    let _ = super::interactive_dispatch::maybe_notify_registered_approval_resolved_with_pool(
        pool,
        &command.approval_id,
        None,
    )
    .await;

    Ok(true)
}

pub(crate) async fn dispatch_im_inbound_to_workclaw_with_pool_and_app(
    pool: &SqlitePool,
    app: &AppHandle,
    event: &ImEvent,
) -> Result<FeishuCallbackResult, String> {
    let result = process_im_event(pool, event.clone()).await?;
    if result.deduped {
        return Ok(result);
    }

    if maybe_handle_registered_approval_command_with_pool_and_app(pool, app, event).await? {
        return Ok(result);
    }

    let route_decision = resolve_openclaw_route_with_pool(pool, event).await.ok();
    let dispatches =
        bridge_inbound_event_to_employee_sessions_with_pool(pool, event, route_decision.as_ref())
            .await?;
    emit_inbound_dispatch_sessions(app, &event.channel, &dispatches);

    if dispatches.is_empty() {
        let planned = plan_role_events_for_openclaw(pool, event).await?;
        for evt in planned {
            let _ = app.emit("im-role-event", evt);
        }
        let dispatch_requests = plan_role_dispatch_requests_for_openclaw(pool, event).await?;
        for req in dispatch_requests {
            let _ = app.emit("im-role-dispatch-request", req);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::parse_normalized_im_event_value;
    use crate::im::types::ImEventType;

    #[test]
    fn parse_normalized_event_maps_standard_fields() {
        let event = parse_normalized_im_event_value(&serde_json::json!({
            "channel": "wecom",
            "workspace_id": "corp-1",
            "account_id": "agent-1",
            "thread_id": "room-1",
            "message_id": "msg-1",
            "sender_id": "user-1",
            "text": "hello",
            "routing_context": {
                "peer": {
                    "kind": "group",
                    "id": "room-1"
                }
            }
        }))
        .expect("parse normalized event");

        assert_eq!(event.channel, "wecom");
        assert_eq!(event.event_type, ImEventType::MessageCreated);
        assert_eq!(event.thread_id, "room-1");
        assert_eq!(event.message_id.as_deref(), Some("msg-1"));
        assert_eq!(event.event_id.as_deref(), Some("msg-1"));
        assert_eq!(event.account_id.as_deref(), Some("agent-1"));
        assert_eq!(event.tenant_id.as_deref(), Some("corp-1"));
        assert_eq!(event.chat_type.as_deref(), Some("group"));
    }

    #[test]
    fn parse_normalized_event_uses_role_for_mention_type() {
        let event = parse_normalized_im_event_value(&serde_json::json!({
            "channel": "feishu",
            "thread_id": "chat-1",
            "role_id": "pm"
        }))
        .expect("parse mention event");

        assert_eq!(event.event_type, ImEventType::MentionRole);
        assert_eq!(event.role_id.as_deref(), Some("pm"));
    }
}
