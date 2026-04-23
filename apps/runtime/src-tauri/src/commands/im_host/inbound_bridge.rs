use crate::commands::approvals::load_approval_record_with_pool;
use crate::commands::chat::ApprovalManagerState;
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
use crate::im::{
    build_conversation_id, build_parent_conversation_candidates,
    resolve_agent_session_dispatches_with_pool, upsert_agent_conversation_binding,
    upsert_channel_delivery_route, AgentConversationBindingUpsert, AgentInboundDispatchSession,
    ChannelDeliveryRouteUpsert, ImConversationScope, ImConversationSurface, ImPeerKind,
};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};

fn optional_non_empty_string(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn normalized_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn normalized_tenant_id_for_channel(
    channel: &str,
    tenant_id: Option<&str>,
    sender_id: Option<&str>,
) -> Option<String> {
    let tenant_id = normalized_non_empty(tenant_id);
    if channel.trim() == "feishu" {
        tenant_id.or_else(|| normalized_non_empty(sender_id))
    } else {
        tenant_id
    }
}

fn normalized_account_id_for_channel(
    channel: &str,
    account_id: Option<&str>,
    tenant_id: Option<&str>,
    sender_id: Option<&str>,
) -> Option<String> {
    normalized_non_empty(account_id)
        .or_else(|| normalized_tenant_id_for_channel(channel, tenant_id, sender_id))
}

fn optional_string_list(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn infer_peer_kind(chat_type: Option<&str>) -> ImPeerKind {
    match chat_type.map(str::trim) {
        Some("p2p") | Some("direct") => ImPeerKind::Direct,
        _ => ImPeerKind::Group,
    }
}

fn inferred_scope_and_parts_from_conversation_id(
    conversation_id: Option<&str>,
) -> (Option<ImConversationScope>, Option<String>, Option<String>) {
    let Some(conversation_id) = conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return (None, None, None);
    };

    let sender_id = conversation_id
        .rsplit_once(":sender:")
        .map(|(_, sender)| sender.trim())
        .filter(|sender| !sender.is_empty())
        .map(str::to_string);
    let topic_id = conversation_id
        .split_once(":topic:")
        .map(|(_, topic)| topic)
        .map(|topic| topic.split(":sender:").next().unwrap_or(""))
        .map(str::trim)
        .filter(|topic| !topic.is_empty())
        .map(str::to_string);
    let scope = match (topic_id.is_some(), sender_id.is_some()) {
        (true, true) => Some(ImConversationScope::TopicSender),
        (true, false) => Some(ImConversationScope::Topic),
        (false, true) => Some(ImConversationScope::PeerSender),
        (false, false) => Some(ImConversationScope::Peer),
    };

    (scope, topic_id, sender_id)
}

fn parse_conversation_scope(scope: Option<&str>) -> Option<ImConversationScope> {
    match scope.map(str::trim).filter(|value| !value.is_empty()) {
        Some("peer") => Some(ImConversationScope::Peer),
        Some("peer_sender") => Some(ImConversationScope::PeerSender),
        Some("topic") => Some(ImConversationScope::Topic),
        Some("topic_sender") => Some(ImConversationScope::TopicSender),
        _ => None,
    }
}

fn build_event_conversation_metadata(
    channel: &str,
    thread_id: &str,
    account_id: Option<&str>,
    tenant_id: Option<&str>,
    sender_id: Option<&str>,
    chat_type: Option<&str>,
    message_id: Option<&str>,
    topic_id: Option<&str>,
    conversation_id: Option<&str>,
    conversation_scope: Option<&str>,
) -> (String, String, Vec<String>, String) {
    let normalized_tenant_id = normalized_tenant_id_for_channel(channel, tenant_id, sender_id);
    let normalized_account_id =
        normalized_account_id_for_channel(channel, account_id, tenant_id, sender_id);
    let (inferred_scope, inferred_topic_id, inferred_sender_id) =
        inferred_scope_and_parts_from_conversation_id(conversation_id);
    let scope = parse_conversation_scope(conversation_scope)
        .or(inferred_scope)
        .unwrap_or_else(|| {
            if topic_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some()
                || inferred_topic_id.is_some()
            {
                ImConversationScope::Topic
            } else {
                ImConversationScope::Peer
            }
        });
    let peer_kind = infer_peer_kind(chat_type);
    let surface = ImConversationSurface {
        channel: channel.trim().to_string(),
        account_id: normalized_account_id.unwrap_or_else(|| "default".to_string()),
        tenant_id: normalized_tenant_id,
        peer_kind,
        peer_id: thread_id.trim().to_string(),
        topic_id: topic_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or(inferred_topic_id),
        sender_id: sender_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or(inferred_sender_id),
        scope,
        message_id: message_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        raw_thread_id: None,
        raw_root_id: None,
    };
    let conversation_id = conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| build_conversation_id(&surface));
    let base_surface = ImConversationSurface {
        channel: surface.channel.clone(),
        account_id: surface.account_id.clone(),
        tenant_id: surface.tenant_id.clone(),
        peer_kind: surface.peer_kind,
        peer_id: surface.peer_id.clone(),
        topic_id: None,
        sender_id: None,
        scope: ImConversationScope::Peer,
        message_id: surface.message_id.clone(),
        raw_thread_id: None,
        raw_root_id: None,
    };
    (
        conversation_id,
        build_conversation_id(&base_surface),
        build_parent_conversation_candidates(&surface),
        surface.scope.as_str().to_string(),
    )
}

fn build_normalized_event_conversation_metadata(
    value: &serde_json::Value,
    channel: &str,
    thread_id: &str,
    account_id: Option<&str>,
    tenant_id: Option<&str>,
    sender_id: Option<&str>,
    chat_type: Option<&str>,
    message_id: Option<&str>,
    conversation_id: Option<&str>,
    conversation_scope: Option<&str>,
) -> (String, String, Vec<String>, String) {
    let topic_id = optional_non_empty_string(value.get("topic_id"))
        .or_else(|| optional_non_empty_string(value.get("root_id")))
        .or_else(|| {
            value
                .get("routing_context")
                .and_then(|entry| entry.get("topic"))
                .and_then(|entry| entry.get("id"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(str::to_string)
        });
    build_event_conversation_metadata(
        channel,
        thread_id,
        account_id,
        tenant_id,
        sender_id,
        chat_type,
        message_id,
        topic_id.as_deref(),
        conversation_id,
        conversation_scope,
    )
}

pub(crate) fn project_im_event_conversation_metadata(event: &ImEvent) -> ImEvent {
    let mut projected = event.clone();
    projected.account_id = normalized_non_empty(event.account_id.as_deref());
    projected.tenant_id = normalized_tenant_id_for_channel(
        &event.channel,
        event.tenant_id.as_deref(),
        event.sender_id.as_deref(),
    );

    let needs_projection = event
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
        || event
            .base_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        || event.parent_conversation_candidates.is_empty()
        || event
            .conversation_scope
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none();
    if !needs_projection {
        return projected;
    }

    let (conversation_id, base_conversation_id, parent_conversation_candidates, conversation_scope) =
        build_event_conversation_metadata(
            &event.channel,
            &event.thread_id,
            projected.account_id.as_deref(),
            projected.tenant_id.as_deref(),
            event.sender_id.as_deref(),
            event.chat_type.as_deref(),
            event.message_id.as_deref(),
            None,
            event.conversation_id.as_deref(),
            event.conversation_scope.as_deref(),
        );

    projected.conversation_id.get_or_insert(conversation_id);
    projected
        .base_conversation_id
        .get_or_insert(base_conversation_id);
    if projected.parent_conversation_candidates.is_empty() {
        projected.parent_conversation_candidates = parent_conversation_candidates;
    }
    projected
        .conversation_scope
        .get_or_insert(conversation_scope);
    projected
}

pub fn parse_normalized_im_event_value(value: &serde_json::Value) -> Result<ImEvent, String> {
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
    let account_id = optional_non_empty_string(value.get("account_id"));
    let sender_id = optional_non_empty_string(value.get("sender_id"));
    let raw_tenant_id = optional_non_empty_string(value.get("workspace_id"))
        .or_else(|| optional_non_empty_string(value.get("tenant_id")));
    let tenant_id =
        normalized_tenant_id_for_channel(&channel, raw_tenant_id.as_deref(), sender_id.as_deref());
    let message_id = optional_non_empty_string(value.get("message_id"));
    let event_id = optional_non_empty_string(value.get("event_id")).or_else(|| message_id.clone());
    let mut conversation_id = optional_non_empty_string(value.get("conversation_id"));
    let mut base_conversation_id = optional_non_empty_string(value.get("base_conversation_id"));
    let mut parent_conversation_candidates =
        optional_string_list(value.get("parent_conversation_candidates"));
    let mut conversation_scope = optional_non_empty_string(value.get("conversation_scope"));

    if conversation_id.is_none()
        || base_conversation_id.is_none()
        || parent_conversation_candidates.is_empty()
        || conversation_scope.is_none()
    {
        let (derived_conversation_id, derived_base_conversation_id, derived_parents, derived_scope) =
            build_normalized_event_conversation_metadata(
                value,
                &channel,
                &thread_id,
                account_id.as_deref(),
                tenant_id.as_deref(),
                sender_id.as_deref(),
                chat_type.as_deref(),
                message_id.as_deref(),
                conversation_id.as_deref(),
                conversation_scope.as_deref(),
            );
        conversation_id.get_or_insert(derived_conversation_id);
        base_conversation_id.get_or_insert(derived_base_conversation_id);
        if parent_conversation_candidates.is_empty() {
            parent_conversation_candidates = derived_parents;
        }
        conversation_scope.get_or_insert(derived_scope);
    }

    Ok(ImEvent {
        channel,
        event_type: if role_id.is_some() {
            ImEventType::MentionRole
        } else {
            ImEventType::MessageCreated
        },
        thread_id,
        event_id,
        message_id,
        text: optional_non_empty_string(value.get("text")),
        role_id,
        account_id,
        tenant_id,
        sender_id,
        chat_type,
        conversation_id,
        base_conversation_id,
        parent_conversation_candidates,
        conversation_scope,
    })
}

pub(crate) fn emit_inbound_dispatch_sessions(
    app: &AppHandle,
    channel: &str,
    dispatches: &[AgentInboundDispatchSession],
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
                &dispatch.agent_name,
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
                &dispatch.agent_name,
                channel,
                &dispatch.prompt,
                "general-purpose",
            );
            req.message_id = dispatch.message_id.clone();
            req
        });
    }
}

async fn record_openclaw_binding_projection_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
    dispatches: &[AgentInboundDispatchSession],
) -> Result<(), String> {
    let conversation_id = event
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| event.thread_id.trim());
    let base_conversation_id = event
        .base_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(conversation_id);
    let channel = if event.channel.trim().is_empty() {
        "app"
    } else {
        event.channel.trim()
    };
    let account_id = event
        .account_id
        .as_deref()
        .or(event.tenant_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("default");
    let scope = event.conversation_scope.as_deref().unwrap_or("peer");
    let peer_kind = match event.chat_type.as_deref().map(str::trim) {
        Some("p2p") | Some("direct") => "direct",
        _ => "group",
    };
    let topic_id = if matches!(scope, "topic" | "topic_sender") {
        event
            .conversation_id
            .as_deref()
            .and_then(|value| value.split_once(":topic:"))
            .map(|(_, topic)| topic.split(":sender:").next().unwrap_or(""))
            .unwrap_or("")
    } else {
        ""
    };
    let parent_candidates = event.parent_conversation_candidates.clone();
    let now = chrono::Utc::now().to_rfc3339();

    for dispatch in dispatches {
        upsert_agent_conversation_binding(
            pool,
            &AgentConversationBindingUpsert {
                conversation_id,
                channel,
                account_id,
                agent_id: dispatch.route_agent_id.trim(),
                session_key: dispatch.route_session_key.trim(),
                session_id: dispatch.session_id.trim(),
                base_conversation_id,
                parent_conversation_candidates: &parent_candidates,
                scope,
                peer_kind,
                peer_id: event.thread_id.trim(),
                topic_id,
                sender_id: event.sender_id.as_deref().unwrap_or_default(),
                created_at: &now,
                updated_at: &now,
            },
        )
        .await?;

        upsert_channel_delivery_route(
            pool,
            &ChannelDeliveryRouteUpsert {
                session_key: dispatch.route_session_key.trim(),
                channel,
                account_id,
                conversation_id,
                reply_target: event.thread_id.trim(),
                updated_at: &now,
            },
        )
        .await?;
    }

    Ok(())
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
    let projected_event = project_im_event_conversation_metadata(event);
    let result = process_im_event(pool, projected_event.clone()).await?;
    if result.deduped {
        return Ok(result);
    }

    if maybe_handle_registered_approval_command_with_pool_and_app(pool, app, &projected_event)
        .await?
    {
        return Ok(result);
    }

    let route_decision = resolve_openclaw_route_with_pool(pool, &projected_event)
        .await
        .ok();
    let dispatches =
        resolve_agent_session_dispatches_with_pool(pool, &projected_event, route_decision.as_ref())
            .await?;
    record_openclaw_binding_projection_with_pool(pool, &projected_event, &dispatches).await?;
    emit_inbound_dispatch_sessions(app, &projected_event.channel, &dispatches);

    if dispatches.is_empty() {
        let planned = plan_role_events_for_openclaw(pool, &projected_event).await?;
        for evt in planned {
            let _ = app.emit("im-role-event", evt);
        }
        let dispatch_requests =
            plan_role_dispatch_requests_for_openclaw(pool, &projected_event).await?;
        for req in dispatch_requests {
            let _ = app.emit("im-role-dispatch-request", req);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{parse_normalized_im_event_value, record_openclaw_binding_projection_with_pool};
    use crate::im::types::ImEvent;
    use crate::im::types::ImEventType;
    use crate::im::{
        find_agent_conversation_binding, find_channel_delivery_route_by_session_id,
        AgentInboundDispatchSession,
    };
    use sqlx::SqlitePool;

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
        assert_eq!(
            event.conversation_id.as_deref(),
            Some("wecom:agent-1:group:room-1")
        );
        assert_eq!(
            event.base_conversation_id.as_deref(),
            Some("wecom:agent-1:group:room-1")
        );
        assert_eq!(event.conversation_scope.as_deref(), Some("peer"));
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
        assert_eq!(
            event.conversation_id.as_deref(),
            Some("feishu:default:group:chat-1")
        );
    }

    #[test]
    fn parse_normalized_event_derives_topic_conversation_from_topic_id() {
        let event = parse_normalized_im_event_value(&serde_json::json!({
            "channel": "wecom",
            "workspace_id": "corp-1",
            "account_id": "agent-1",
            "thread_id": "room-1",
            "message_id": "msg-2",
            "sender_id": "user-1",
            "text": "hello topic",
            "chat_type": "group",
            "topic_id": "topic-42"
        }))
        .expect("parse topic event");

        assert_eq!(
            event.conversation_id.as_deref(),
            Some("wecom:agent-1:group:room-1:topic:topic-42")
        );
        assert_eq!(
            event.base_conversation_id.as_deref(),
            Some("wecom:agent-1:group:room-1")
        );
        assert_eq!(
            event.parent_conversation_candidates,
            vec!["wecom:agent-1:group:room-1".to_string()]
        );
        assert_eq!(event.conversation_scope.as_deref(), Some("topic"));
    }

    #[test]
    fn parse_normalized_event_backfills_missing_scope_metadata_from_conversation_id() {
        let event = parse_normalized_im_event_value(&serde_json::json!({
            "channel": "wecom",
            "workspace_id": "corp-1",
            "account_id": "agent-1",
            "thread_id": "room-1",
            "message_id": "msg-3",
            "sender_id": "user-1",
            "chat_type": "group",
            "conversation_id": "wecom:agent-1:group:room-1:topic:topic-99"
        }))
        .expect("parse topic event with partial metadata");

        assert_eq!(
            event.conversation_id.as_deref(),
            Some("wecom:agent-1:group:room-1:topic:topic-99")
        );
        assert_eq!(
            event.base_conversation_id.as_deref(),
            Some("wecom:agent-1:group:room-1")
        );
        assert_eq!(
            event.parent_conversation_candidates,
            vec!["wecom:agent-1:group:room-1".to_string()]
        );
        assert_eq!(event.conversation_scope.as_deref(), Some("topic"));
    }

    #[test]
    fn project_im_event_conversation_metadata_falls_back_from_blank_account_id_to_tenant_id() {
        let event = super::project_im_event_conversation_metadata(&ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "oc_blank_account_fast_path".to_string(),
            event_id: Some("evt_blank_account_fast_path".to_string()),
            message_id: Some("om_blank_account_fast_path".to_string()),
            text: Some("继续这个桥接边界情况".to_string()),
            role_id: None,
            account_id: Some("   ".to_string()),
            tenant_id: Some("tenant-fast-path".to_string()),
            sender_id: Some("ou_sender_fast_path".to_string()),
            chat_type: Some("group".to_string()),
            conversation_id: None,
            base_conversation_id: None,
            parent_conversation_candidates: Vec::new(),
            conversation_scope: None,
        });

        assert_eq!(
            event.conversation_id.as_deref(),
            Some("feishu:tenant-fast-path:group:oc_blank_account_fast_path")
        );
        assert_eq!(
            event.base_conversation_id.as_deref(),
            Some("feishu:tenant-fast-path:group:oc_blank_account_fast_path")
        );
        assert!(event.parent_conversation_candidates.is_empty());
        assert_eq!(event.conversation_scope.as_deref(), Some("peer"));
    }

    #[test]
    fn project_im_event_conversation_metadata_falls_back_to_sender_id_for_feishu_sparse_account() {
        let event = super::project_im_event_conversation_metadata(&ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "oc_sender_fallback_fast_path".to_string(),
            event_id: Some("evt_sender_fallback_fast_path".to_string()),
            message_id: Some("om_sender_fallback_fast_path".to_string()),
            text: Some("继续这个飞书兜底情况".to_string()),
            role_id: None,
            account_id: Some("   ".to_string()),
            tenant_id: None,
            sender_id: Some("ou_sender_fallback_fast_path".to_string()),
            chat_type: Some("group".to_string()),
            conversation_id: None,
            base_conversation_id: None,
            parent_conversation_candidates: Vec::new(),
            conversation_scope: None,
        });

        assert_eq!(event.account_id, None);
        assert_eq!(
            event.tenant_id.as_deref(),
            Some("ou_sender_fallback_fast_path")
        );
        assert_eq!(
            event.conversation_id.as_deref(),
            Some("feishu:ou_sender_fallback_fast_path:group:oc_sender_fallback_fast_path")
        );
        assert_eq!(
            event.base_conversation_id.as_deref(),
            Some("feishu:ou_sender_fallback_fast_path:group:oc_sender_fallback_fast_path")
        );
        assert!(event.parent_conversation_candidates.is_empty());
        assert_eq!(event.conversation_scope.as_deref(), Some("peer"));
    }

    async fn setup_projection_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        sqlx::query(
            "CREATE TABLE agent_conversation_bindings (
                conversation_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                account_id TEXT NOT NULL DEFAULT '',
                agent_id TEXT NOT NULL,
                session_key TEXT NOT NULL,
                session_id TEXT NOT NULL DEFAULT '',
                base_conversation_id TEXT NOT NULL DEFAULT '',
                parent_conversation_candidates_json TEXT NOT NULL DEFAULT '[]',
                scope TEXT NOT NULL DEFAULT '',
                peer_kind TEXT NOT NULL DEFAULT '',
                peer_id TEXT NOT NULL DEFAULT '',
                topic_id TEXT NOT NULL DEFAULT '',
                sender_id TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (conversation_id, agent_id)
            )",
        )
        .execute(&pool)
        .await
        .expect("create agent_conversation_bindings");

        sqlx::query(
            "CREATE TABLE channel_delivery_routes (
                session_key TEXT NOT NULL PRIMARY KEY,
                channel TEXT NOT NULL,
                account_id TEXT NOT NULL DEFAULT '',
                conversation_id TEXT NOT NULL,
                reply_target TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create channel_delivery_routes");

        pool
    }

    #[tokio::test]
    async fn bridge_projection_persists_topic_metadata_when_event_only_supplies_conversation_id() {
        let pool = setup_projection_pool().await;
        let event = parse_normalized_im_event_value(&serde_json::json!({
            "channel": "wecom",
            "workspace_id": "corp-1",
            "account_id": "agent-1",
            "thread_id": "room-topic-1",
            "message_id": "msg-topic-1",
            "sender_id": "user-1",
            "chat_type": "group",
            "conversation_id": "wecom:agent-1:group:room-topic-1:topic:topic-bridge-1"
        }))
        .expect("parse normalized event");
        let dispatches = vec![AgentInboundDispatchSession {
            session_id: "session-topic-1".to_string(),
            thread_id: "room-topic-1".to_string(),
            agent_id: "agent-1".to_string(),
            role_id: "role-1".to_string(),
            agent_name: "Agent 1".to_string(),
            route_agent_id: "agent-1".to_string(),
            route_session_key: "route-topic-1".to_string(),
            matched_by: "default".to_string(),
            prompt: "hello".to_string(),
            message_id: "msg-topic-1".to_string(),
        }];

        record_openclaw_binding_projection_with_pool(&pool, &event, &dispatches)
            .await
            .expect("persist bridge projection");

        let binding = find_agent_conversation_binding(
            &pool,
            "wecom:agent-1:group:room-topic-1:topic:topic-bridge-1",
            "agent-1",
        )
        .await
        .expect("find binding")
        .expect("binding exists");
        assert_eq!(
            binding.base_conversation_id,
            "wecom:agent-1:group:room-topic-1"
        );
        assert_eq!(
            binding.parent_conversation_candidates,
            vec!["wecom:agent-1:group:room-topic-1".to_string()]
        );
        assert_eq!(binding.scope, "topic");
        assert_eq!(binding.topic_id, "topic-bridge-1");

        let route =
            find_channel_delivery_route_by_session_id(&pool, "session-topic-1", Some("wecom"))
                .await
                .expect("find route")
                .expect("route exists");
        assert_eq!(
            route.conversation_id,
            "wecom:agent-1:group:room-topic-1:topic:topic-bridge-1"
        );
        assert_eq!(route.reply_target, "room-topic-1");
    }
}
