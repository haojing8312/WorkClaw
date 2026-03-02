mod helpers;

use runtime_lib::commands::im_config::bind_thread_roles_with_pool;
use runtime_lib::commands::openclaw_gateway::{
    parse_openclaw_payload, plan_role_events_for_openclaw, validate_openclaw_auth_with_pool,
};
use runtime_lib::im::types::ImEventType;

#[test]
fn parse_openclaw_payload_supports_wrapped_event() {
    let raw = r#"{
      "event": {
        "event_type": "message.created",
        "thread_id": "thread-1",
        "event_id": "evt-1",
        "message_id": "msg-1",
        "text": "hello"
      }
    }"#;

    let evt = parse_openclaw_payload(raw).expect("payload should parse");
    assert_eq!(evt.event_type, ImEventType::MessageCreated);
    assert_eq!(evt.thread_id, "thread-1");
    assert_eq!(evt.event_id.as_deref(), Some("evt-1"));
}

#[test]
fn parse_openclaw_payload_maps_nested_message_and_chat_fields() {
    let raw = r#"{
      "event": {
        "event_type": "message.created",
        "event_id": "evt-2",
        "chat": { "id": "group-42" },
        "message": { "id": "msg-2", "text": "商机来了" },
        "sender": { "id": "user-1001" }
      }
    }"#;

    let evt = parse_openclaw_payload(raw).expect("payload should parse");
    assert_eq!(evt.event_type, ImEventType::MessageCreated);
    assert_eq!(evt.thread_id, "group-42");
    assert_eq!(evt.message_id.as_deref(), Some("msg-2"));
    assert_eq!(evt.text.as_deref(), Some("商机来了"));
    assert_eq!(evt.tenant_id.as_deref(), Some("user-1001"));
}

#[test]
fn parse_openclaw_payload_extracts_mentioned_role() {
    let raw = r#"{
      "event_type": "mention.role",
      "thread_id": "thread-9",
      "mentions": [
        { "type": "user", "id": "u-1" },
        { "type": "role", "id": "architect" }
      ]
    }"#;

    let evt = parse_openclaw_payload(raw).expect("payload should parse");
    assert_eq!(evt.event_type, ImEventType::MentionRole);
    assert_eq!(evt.role_id.as_deref(), Some("architect"));
}

#[tokio::test]
async fn validate_openclaw_auth_honors_configured_token() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query("INSERT INTO app_settings (key, value) VALUES ('openclaw_ingress_token', 'secret-1')")
        .execute(&pool)
        .await
        .expect("seed token");

    let ok = validate_openclaw_auth_with_pool(&pool, Some("secret-1".to_string())).await;
    assert!(ok.is_ok());

    let bad = validate_openclaw_auth_with_pool(&pool, Some("wrong".to_string())).await;
    assert!(bad.is_err());
}

#[tokio::test]
async fn plan_role_events_uses_thread_bindings() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    bind_thread_roles_with_pool(
        &pool,
        "thread-1",
        "tenant-a",
        "opportunity_review",
        &["presales".to_string(), "architect".to_string()],
    )
    .await
    .expect("bind roles");

    let evt = parse_openclaw_payload(
        r#"{"event_type":"message.created","thread_id":"thread-1","text":"请开始评审"}"#,
    )
    .expect("parse");
    let planned = plan_role_events_for_openclaw(&pool, &evt)
        .await
        .expect("plan events");
    assert_eq!(planned.len(), 2);
    assert_eq!(planned[0].thread_id, "thread-1");
    assert_eq!(planned[0].status, "running");
}
