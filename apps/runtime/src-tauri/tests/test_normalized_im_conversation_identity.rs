use runtime_lib::commands::im_host::parse_normalized_im_event_value;
use runtime_lib::im::{
    build_conversation_id, build_parent_conversation_candidates, ImConversationScope,
    ImConversationSurface, ImPeerKind,
};

#[test]
fn normalized_wecom_event_derives_peer_conversation_metadata() {
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
    .expect("parse normalized peer event");

    assert_eq!(
        event.conversation_id.as_deref(),
        Some("wecom:agent-1:group:room-1")
    );
    assert_eq!(
        event.base_conversation_id.as_deref(),
        Some("wecom:agent-1:group:room-1")
    );
    assert!(event.parent_conversation_candidates.is_empty());
    assert_eq!(event.conversation_scope.as_deref(), Some("peer"));
}

#[test]
fn normalized_wecom_event_derives_topic_conversation_metadata() {
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
    .expect("parse normalized topic event");

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
fn normalized_wecom_event_derives_topic_from_routing_context_topic() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/wecom/normalized-topic-routing.json"))
            .expect("parse wecom normalized routing fixture");
    let event =
        parse_normalized_im_event_value(&fixture).expect("parse normalized routing topic event");

    assert_eq!(
        event.conversation_id.as_deref(),
        Some("wecom:agent-1:group:room-1:topic:topic-from-routing")
    );
    assert_eq!(
        event.base_conversation_id.as_deref(),
        Some("wecom:agent-1:group:room-1")
    );
    assert_eq!(event.conversation_scope.as_deref(), Some("topic"));
}

#[test]
fn normalized_event_backfills_missing_topic_scope_metadata_from_conversation_id() {
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
    .expect("parse normalized event with partial metadata");

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
fn shared_identity_core_degrades_incomplete_surfaces_to_narrower_stable_scopes() {
    let mut surface = ImConversationSurface {
        channel: "wecom".to_string(),
        account_id: "agent-1".to_string(),
        tenant_id: Some("corp-1".to_string()),
        peer_kind: ImPeerKind::Group,
        peer_id: "room-1".to_string(),
        topic_id: None,
        sender_id: Some("user-1".to_string()),
        scope: ImConversationScope::TopicSender,
        message_id: Some("msg-3".to_string()),
        raw_thread_id: Some("room-1".to_string()),
        raw_root_id: None,
    };

    assert_eq!(
        build_conversation_id(&surface),
        "wecom:agent-1:group:room-1:sender:user-1"
    );
    assert_eq!(
        build_parent_conversation_candidates(&surface),
        vec!["wecom:agent-1:group:room-1".to_string()]
    );

    surface.sender_id = None;
    assert_eq!(
        build_conversation_id(&surface),
        "wecom:agent-1:group:room-1"
    );
    assert!(build_parent_conversation_candidates(&surface).is_empty());

    surface.topic_id = Some("topic-42".to_string());
    surface.scope = ImConversationScope::TopicSender;
    assert_eq!(
        build_conversation_id(&surface),
        "wecom:agent-1:group:room-1:topic:topic-42"
    );
    assert_eq!(
        build_parent_conversation_candidates(&surface),
        vec!["wecom:agent-1:group:room-1".to_string()]
    );
}
