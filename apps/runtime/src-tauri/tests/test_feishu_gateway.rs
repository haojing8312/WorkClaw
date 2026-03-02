mod helpers;

use runtime_lib::commands::feishu_gateway::{
    calculate_feishu_signature, parse_feishu_payload, plan_role_dispatch_requests_for_feishu,
    plan_role_events_for_feishu, resolve_feishu_app_credentials, resolve_feishu_sidecar_base_url,
    set_app_setting, validate_feishu_auth_with_pool, validate_feishu_signature_with_pool,
    ParsedFeishuPayload,
};
use runtime_lib::commands::im_config::bind_thread_roles_with_pool;
use runtime_lib::im::types::ImEventType;

#[test]
fn parse_feishu_payload_supports_challenge() {
    let raw = r#"{"challenge":"abc123"}"#;
    let parsed = parse_feishu_payload(raw).expect("challenge parse");
    match parsed {
        ParsedFeishuPayload::Challenge(v) => assert_eq!(v, "abc123"),
        _ => panic!("expected challenge"),
    }
}

#[test]
fn parse_feishu_payload_maps_message_event() {
    let raw = r#"{
      "header": {
        "event_id": "evt-feishu-1",
        "event_type": "im.message.receive_v1",
        "tenant_key": "tenant-x"
      },
      "event": {
        "message": {
          "message_id": "msg-1",
          "chat_id": "chat-1",
          "content": "{\"text\":\"你好，帮我评审商机\"}"
        },
        "sender": {
          "sender_id": { "open_id": "ou_xxx" }
        }
      }
    }"#;
    let parsed = parse_feishu_payload(raw).expect("event parse");
    let evt = match parsed {
        ParsedFeishuPayload::Event(e) => e,
        _ => panic!("expected event"),
    };
    assert_eq!(evt.event_type, ImEventType::MessageCreated);
    assert_eq!(evt.thread_id, "chat-1");
    assert_eq!(evt.event_id.as_deref(), Some("evt-feishu-1"));
    assert_eq!(evt.text.as_deref(), Some("你好，帮我评审商机"));
}

#[tokio::test]
async fn validate_feishu_auth_honors_configured_token() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query("INSERT INTO app_settings (key, value) VALUES ('feishu_ingress_token', 'feishu-secret')")
        .execute(&pool)
        .await
        .expect("seed token");

    assert!(validate_feishu_auth_with_pool(&pool, Some("feishu-secret".to_string()))
        .await
        .is_ok());
    assert!(validate_feishu_auth_with_pool(&pool, Some("wrong".to_string()))
        .await
        .is_err());
}

#[tokio::test]
async fn validate_feishu_signature_honors_encrypt_key() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query("INSERT INTO app_settings (key, value) VALUES ('feishu_encrypt_key', 'enc-key-1')")
        .execute(&pool)
        .await
        .expect("seed encrypt key");

    let payload = r#"{"header":{"event_id":"evt-1","event_type":"im.message.receive_v1"},"event":{"message":{"message_id":"m1","chat_id":"c1","content":"{\"text\":\"x\"}"}}}"#;
    let timestamp = "1700000000";
    let nonce = "abc123";
    let signature = calculate_feishu_signature(timestamp, nonce, "enc-key-1", payload);

    let ok = validate_feishu_signature_with_pool(
        &pool,
        payload,
        Some(timestamp.to_string()),
        Some(nonce.to_string()),
        Some(signature),
    )
    .await;
    assert!(ok.is_ok());

    let bad = validate_feishu_signature_with_pool(
        &pool,
        payload,
        Some(timestamp.to_string()),
        Some(nonce.to_string()),
        Some("wrong".to_string()),
    )
    .await;
    assert!(bad.is_err());
}

#[tokio::test]
async fn plan_role_events_for_feishu_uses_thread_bindings() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    bind_thread_roles_with_pool(
        &pool,
        "chat-1",
        "tenant-x",
        "opportunity_review",
        &["presales".to_string(), "architect".to_string()],
    )
    .await
    .expect("bind roles");

    let parsed = parse_feishu_payload(
        r#"{
          "header":{"event_id":"evt-2","event_type":"im.message.receive_v1"},
          "event":{"message":{"message_id":"msg-2","chat_id":"chat-1","content":"{\"text\":\"开始\"}"}}
        }"#,
    )
    .expect("parse");

    let evt = match parsed {
        ParsedFeishuPayload::Event(e) => e,
        _ => panic!("expected event"),
    };
    let planned = plan_role_events_for_feishu(&pool, &evt)
        .await
        .expect("plan role events");
    assert_eq!(planned.len(), 2);
    assert_eq!(planned[0].thread_id, "chat-1");
    assert_eq!(planned[0].status, "running");

    let dispatches = plan_role_dispatch_requests_for_feishu(&pool, &evt)
        .await
        .expect("plan dispatch");
    assert_eq!(dispatches.len(), 2);
    assert_eq!(dispatches[0].thread_id, "chat-1");
    assert_eq!(dispatches[0].agent_type, "plan");
    assert!(dispatches[0].prompt.contains("场景=opportunity_review"));
}

#[tokio::test]
async fn resolve_feishu_settings_reads_from_app_settings() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    set_app_setting(&pool, "feishu_app_id", "cli_app").await.expect("set app id");
    set_app_setting(&pool, "feishu_app_secret", "cli_secret")
        .await
        .expect("set app secret");
    set_app_setting(&pool, "feishu_sidecar_base_url", "http://127.0.0.1:9000")
        .await
        .expect("set sidecar url");

    let (app_id, app_secret) = resolve_feishu_app_credentials(&pool, None, None)
        .await
        .expect("resolve creds");
    assert_eq!(app_id.as_deref(), Some("cli_app"));
    assert_eq!(app_secret.as_deref(), Some("cli_secret"));

    let base_url = resolve_feishu_sidecar_base_url(&pool, None)
        .await
        .expect("resolve sidecar");
    assert_eq!(base_url.as_deref(), Some("http://127.0.0.1:9000"));
}
