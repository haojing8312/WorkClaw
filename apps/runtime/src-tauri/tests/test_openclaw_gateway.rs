mod helpers;

use runtime_lib::commands::im_config::bind_thread_roles_with_pool;
use runtime_lib::commands::im_routing::{
    upsert_im_routing_binding_with_pool, UpsertImRoutingBindingInput,
};
use runtime_lib::commands::openclaw_gateway::{
    parse_openclaw_payload, plan_role_events_for_openclaw, resolve_openclaw_route_with_pool,
    validate_openclaw_auth_with_pool,
};
use runtime_lib::im::types::{ImEvent, ImEventType};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

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

#[test]
fn parse_openclaw_payload_keeps_channel_and_account_metadata() {
    let raw = r#"{
      "channel": "discord",
      "event_type": "message.created",
      "thread_id": "discord-thread-1",
      "account_id": "guild-account-9",
      "tenant_id": "legacy-tenant"
    }"#;

    let evt = parse_openclaw_payload(raw).expect("payload should parse");
    assert_eq!(evt.channel, "discord");
    assert_eq!(evt.thread_id, "discord-thread-1");
    assert_eq!(evt.account_id.as_deref(), Some("guild-account-9"));
}

#[test]
fn parse_openclaw_payload_defaults_missing_channel_to_app() {
    let raw = r#"{
      "event_type": "message.created",
      "thread_id": "thread-app-1",
      "message_id": "msg-app-1"
    }"#;

    let evt = parse_openclaw_payload(raw).expect("payload should parse");
    assert_eq!(evt.channel, "app");
}

#[tokio::test]
async fn validate_openclaw_auth_honors_configured_token() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ('openclaw_ingress_token', 'secret-1')",
    )
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

#[tokio::test]
async fn plan_role_events_preserves_wecom_source_channel() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    bind_thread_roles_with_pool(
        &pool,
        "wecom-thread-1",
        "corp-123",
        "wecom_review",
        &["architect".to_string()],
    )
    .await
    .expect("bind roles");

    let evt = ImEvent {
        channel: "wecom".to_string(),
        event_type: ImEventType::MessageCreated,
        thread_id: "wecom-thread-1".to_string(),
        event_id: Some("evt-wecom-role".to_string()),
        message_id: Some("msg-wecom-role".to_string()),
        text: Some("企业微信触发评审".to_string()),
        role_id: None,
        account_id: Some("agent-1000002".to_string()),
        tenant_id: Some("corp-123".to_string()),
    };

    let planned = plan_role_events_for_openclaw(&pool, &evt)
        .await
        .expect("plan wecom events");
    assert_eq!(planned.len(), 1);
    assert_eq!(planned[0].source_channel, "wecom");
}

async fn spawn_mock_sidecar_once() -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock sidecar");
    let addr = listener.local_addr().expect("local addr");
    let handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let mut buf = vec![0u8; 64 * 1024];
        let n = socket.read(&mut buf).await.expect("read request");
        let raw = String::from_utf8_lossy(&buf[..n]).to_string();
        let body = raw.split("\r\n\r\n").nth(1).unwrap_or("{}");
        let body_json: serde_json::Value =
            serde_json::from_str(body).unwrap_or_else(|_| serde_json::json!({}));

        let peer_id = body_json
            .get("peer")
            .and_then(|v| v.get("id"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let bindings = body_json
            .get("bindings")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut resolved = serde_json::json!({
            "agentId": "main",
            "matchedBy": "default",
        });
        for binding in bindings {
            let binding_peer_id = binding
                .get("match")
                .and_then(|m| m.get("peer"))
                .and_then(|p| p.get("id"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            if !binding_peer_id.is_empty() && binding_peer_id == peer_id {
                let agent_id = binding
                    .get("agentId")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("main");
                resolved = serde_json::json!({
                    "agentId": agent_id,
                    "matchedBy": "binding.peer",
                });
                break;
            }
        }

        let response_body = serde_json::json!({
            "output": resolved.to_string()
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("write response");
    });
    (format!("http://{}", addr), handle)
}

#[tokio::test]
async fn resolve_route_prefers_peer_binding() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let (sidecar_base, server_task) = spawn_mock_sidecar_once().await;

    sqlx::query(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('feishu_sidecar_base_url', ?)",
    )
    .bind(&sidecar_base)
    .execute(&pool)
    .await
    .expect("seed sidecar base url");

    upsert_im_routing_binding_with_pool(
        &pool,
        UpsertImRoutingBindingInput {
            id: None,
            agent_id: "account-agent".to_string(),
            channel: "feishu".to_string(),
            account_id: "tenant-a".to_string(),
            peer_kind: "".to_string(),
            peer_id: "".to_string(),
            guild_id: "".to_string(),
            team_id: "".to_string(),
            role_ids: vec![],
            connector_meta: serde_json::json!({}),
            priority: 200,
            enabled: true,
        },
    )
    .await
    .expect("seed account binding");

    upsert_im_routing_binding_with_pool(
        &pool,
        UpsertImRoutingBindingInput {
            id: None,
            agent_id: "peer-agent".to_string(),
            channel: "feishu".to_string(),
            account_id: "tenant-a".to_string(),
            peer_kind: "group".to_string(),
            peer_id: "chat-1".to_string(),
            guild_id: "".to_string(),
            team_id: "".to_string(),
            role_ids: vec![],
            connector_meta: serde_json::json!({}),
            priority: 100,
            enabled: true,
        },
    )
    .await
    .expect("seed peer binding");

    let route = resolve_openclaw_route_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-1".to_string(),
            event_id: Some("evt-rt-1".to_string()),
            message_id: Some("msg-rt-1".to_string()),
            text: Some("hello".to_string()),
            role_id: None,
            account_id: Some("tenant-a".to_string()),
            tenant_id: Some("tenant-a".to_string()),
        },
    )
    .await
    .expect("resolve route");

    assert_eq!(route["agentId"], "peer-agent");
    assert_eq!(route["matchedBy"], "binding.peer");

    server_task.await.expect("mock sidecar task");
}
