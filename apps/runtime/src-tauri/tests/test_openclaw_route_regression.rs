mod helpers;

use runtime_lib::commands::im_routing::{
    upsert_im_routing_binding_with_pool, UpsertImRoutingBindingInput,
};
use runtime_lib::commands::openclaw_gateway::resolve_openclaw_route_with_pool;
use runtime_lib::im::types::{ImEvent, ImEventType};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn account_matches(pattern: &str, actual: &str) -> bool {
    let p = pattern.trim();
    if p.is_empty() || p == "*" {
        return true;
    }
    p == actual
}

fn pick_route(body_json: &serde_json::Value) -> serde_json::Value {
    let channel = body_json
        .get("channel")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("feishu");
    let account_id = body_json
        .get("account_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let peer_id = body_json
        .get("peer")
        .and_then(|v| v.get("id"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let default_agent = body_json
        .get("default_agent_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("main");
    let bindings = body_json
        .get("bindings")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();

    for binding in &bindings {
        let b_channel = binding
            .get("match")
            .and_then(|m| m.get("channel"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let b_account = binding
            .get("match")
            .and_then(|m| m.get("accountId"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("*");
        let b_peer = binding
            .get("match")
            .and_then(|m| m.get("peer"))
            .and_then(|p| p.get("id"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if b_channel == channel
            && account_matches(b_account, account_id)
            && !b_peer.is_empty()
            && b_peer == peer_id
        {
            let agent_id = binding
                .get("agentId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(default_agent);
            return serde_json::json!({ "agentId": agent_id, "matchedBy": "binding.peer" });
        }
    }

    for binding in &bindings {
        let b_channel = binding
            .get("match")
            .and_then(|m| m.get("channel"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let b_account = binding
            .get("match")
            .and_then(|m| m.get("accountId"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("*");
        let b_peer = binding
            .get("match")
            .and_then(|m| m.get("peer"))
            .and_then(|p| p.get("id"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if b_channel == channel && !b_peer.is_empty() {
            continue;
        }
        if b_channel == channel && b_account != "*" && b_account == account_id {
            let agent_id = binding
                .get("agentId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(default_agent);
            return serde_json::json!({ "agentId": agent_id, "matchedBy": "binding.account" });
        }
    }

    for binding in &bindings {
        let b_channel = binding
            .get("match")
            .and_then(|m| m.get("channel"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let b_account = binding
            .get("match")
            .and_then(|m| m.get("accountId"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("*");
        let b_peer = binding
            .get("match")
            .and_then(|m| m.get("peer"))
            .and_then(|p| p.get("id"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if b_channel == channel && b_account == "*" && b_peer.is_empty() {
            let agent_id = binding
                .get("agentId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(default_agent);
            return serde_json::json!({ "agentId": agent_id, "matchedBy": "binding.channel" });
        }
    }

    serde_json::json!({ "agentId": default_agent, "matchedBy": "default" })
}

async fn spawn_mock_sidecar_with_priority(
    expected_requests: usize,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock sidecar");
    let addr = listener.local_addr().expect("local addr");
    let handle = tokio::spawn(async move {
        for _ in 0..expected_requests {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut buf = vec![0u8; 64 * 1024];
            let n = socket.read(&mut buf).await.expect("read request");
            let raw = String::from_utf8_lossy(&buf[..n]).to_string();
            let body = raw.split("\r\n\r\n").nth(1).unwrap_or("{}");
            let body_json: serde_json::Value =
                serde_json::from_str(body).unwrap_or_else(|_| serde_json::json!({}));
            let resolved = pick_route(&body_json);
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
        }
    });
    (format!("http://{}", addr), handle)
}

#[tokio::test]
async fn resolve_route_uses_event_channel_instead_of_feishu_default() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let (base_url, server_task) = spawn_mock_sidecar_with_priority(1).await;
    sqlx::query(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('feishu_sidecar_base_url', ?)",
    )
    .bind(&base_url)
    .execute(&pool)
    .await
    .expect("seed sidecar base");

    upsert_im_routing_binding_with_pool(
        &pool,
        UpsertImRoutingBindingInput {
            id: None,
            agent_id: "discord-agent".to_string(),
            channel: "discord".to_string(),
            account_id: "*".to_string(),
            peer_kind: "".to_string(),
            peer_id: "".to_string(),
            guild_id: "".to_string(),
            team_id: "".to_string(),
            role_ids: vec![],
            connector_meta: serde_json::json!({}),
            priority: 100,
            enabled: true,
        },
    )
    .await
    .expect("seed discord binding");

    let out = resolve_openclaw_route_with_pool(
        &pool,
        &ImEvent {
            channel: "discord".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "discord-room-1".to_string(),
            event_id: Some("evt-discord".to_string()),
            message_id: Some("msg-discord".to_string()),
            text: Some("hello".to_string()),
            role_id: None,
            account_id: Some("tenant-discord".to_string()),
            tenant_id: Some("tenant-discord".to_string()),
        },
    )
    .await
    .expect("resolve route");

    assert_eq!(out["agentId"], "discord-agent");
    assert_eq!(out["matchedBy"], "binding.channel");

    server_task.await.expect("mock sidecar task");
}

#[tokio::test]
async fn route_regression_vectors_match_expected_priority() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let vectors = vec![
        (
            "peer",
            "chat-peer",
            "tenant-a",
            "peer-agent",
            "binding.peer",
        ),
        (
            "account",
            "chat-account",
            "tenant-a",
            "account-agent",
            "binding.account",
        ),
        (
            "channel",
            "chat-channel",
            "tenant-b",
            "channel-agent",
            "binding.channel",
        ),
        ("default", "chat-default", "tenant-c", "main", "default"),
    ];
    let (base_url, server_task) = spawn_mock_sidecar_with_priority(vectors.len()).await;
    sqlx::query(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('feishu_sidecar_base_url', ?)",
    )
    .bind(&base_url)
    .execute(&pool)
    .await
    .expect("seed sidecar base");

    for (name, thread_id, tenant_id, expected_agent, expected_matched_by) in vectors {
        sqlx::query("DELETE FROM im_routing_bindings")
            .execute(&pool)
            .await
            .expect("clear bindings");

        if name != "default" {
            upsert_im_routing_binding_with_pool(
                &pool,
                UpsertImRoutingBindingInput {
                    id: None,
                    agent_id: "channel-agent".to_string(),
                    channel: "feishu".to_string(),
                    account_id: "*".to_string(),
                    peer_kind: "".to_string(),
                    peer_id: "".to_string(),
                    guild_id: "".to_string(),
                    team_id: "".to_string(),
                    role_ids: vec![],
                    connector_meta: serde_json::json!({}),
                    priority: 300,
                    enabled: true,
                },
            )
            .await
            .expect("seed channel binding");
        }

        if name == "peer" || name == "account" {
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
        }

        if name == "peer" {
            upsert_im_routing_binding_with_pool(
                &pool,
                UpsertImRoutingBindingInput {
                    id: None,
                    agent_id: "peer-agent".to_string(),
                    channel: "feishu".to_string(),
                    account_id: "tenant-a".to_string(),
                    peer_kind: "group".to_string(),
                    peer_id: "chat-peer".to_string(),
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
        }

        let out = resolve_openclaw_route_with_pool(
            &pool,
            &ImEvent {
                channel: "feishu".to_string(),
                event_type: ImEventType::MessageCreated,
                thread_id: thread_id.to_string(),
                event_id: Some(format!("evt-{}", name)),
                message_id: Some(format!("msg-{}", name)),
                text: Some("hello".to_string()),
                role_id: None,
                account_id: Some(tenant_id.to_string()),
                tenant_id: Some(tenant_id.to_string()),
            },
        )
        .await
        .expect("resolve route");

        assert_eq!(out["agentId"], expected_agent, "vector={}", name);
        assert_eq!(out["matchedBy"], expected_matched_by, "vector={}", name);
    }

    server_task.await.expect("mock sidecar task");
}
