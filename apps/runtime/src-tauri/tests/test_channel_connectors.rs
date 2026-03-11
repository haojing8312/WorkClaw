mod helpers;

use runtime_lib::commands::channel_connectors::{
    ack_channel_events_with_pool, get_channel_connector_diagnostics_with_pool,
    list_channel_connectors_with_pool, replay_channel_events_with_pool,
};
use runtime_lib::commands::feishu_gateway::set_app_setting;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn spawn_mock_connector_sidecar(
    expected_requests: usize,
) -> (String, tokio::task::JoinHandle<Vec<(String, serde_json::Value)>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock connector sidecar");
    let addr = listener.local_addr().expect("local addr");
    let handle = tokio::spawn(async move {
        let mut requests = Vec::new();
        for _ in 0..expected_requests {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut buf = vec![0u8; 64 * 1024];
            let n = socket.read(&mut buf).await.expect("read request");
            let raw = String::from_utf8_lossy(&buf[..n]).to_string();
            let request_line = raw.lines().next().unwrap_or_default().to_string();
            let path = request_line
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .to_string();
            let body = raw.split("\r\n\r\n").nth(1).unwrap_or("{}");
            let body_json: serde_json::Value =
                serde_json::from_str(body).unwrap_or_else(|_| serde_json::json!({}));
            requests.push((path.clone(), body_json.clone()));

            let output = match path.as_str() {
                "/api/channels/catalog" => serde_json::json!([
                    {
                        "channel": "feishu",
                        "display_name": "飞书连接器",
                        "capabilities": ["receive_text", "send_text", "group_route", "direct_route"]
                    },
                    {
                        "channel": "wecom",
                        "display_name": "企业微信连接器",
                        "capabilities": ["receive_text", "send_text", "group_route", "direct_route"]
                    }
                ]),
                "/api/channels/diagnostics" => serde_json::json!({
                    "connector": {
                        "channel": "wecom",
                        "display_name": "企业微信连接器",
                        "capabilities": ["receive_text", "send_text", "group_route", "direct_route"]
                    },
                    "status": "authentication_error",
                    "health": {
                        "adapter_name": "wecom",
                        "instance_id": "wecom:wecom-main",
                        "state": "error",
                        "last_ok_at": "2026-03-11T10:00:00Z",
                        "last_error": "signature mismatch",
                        "reconnect_attempts": 1,
                        "queue_depth": 2,
                        "issue": {
                            "code": "signature_mismatch",
                            "category": "authentication_error",
                            "user_message": "签名校验失败",
                            "technical_message": "signature mismatch",
                            "retryable": false,
                            "occurred_at": "2026-03-11T10:00:00Z"
                        }
                    },
                    "replay": {
                        "retained_events": 1,
                        "acked_events": 0
                    }
                }),
                "/api/channels/ack" => serde_json::json!({ "ok": true }),
                "/api/channels/replay-events" => serde_json::json!([
                    {
                        "channel": "wecom",
                        "workspace_id": "corp-123",
                        "account_id": "1000002",
                        "thread_id": "room-001",
                        "message_id": "msg-001",
                        "sender_id": "zhangsan",
                        "sender_name": "张三",
                        "text": "hello",
                        "mentions": [],
                        "raw_event_type": "message.receive",
                        "occurred_at": "2026-03-11T10:00:00Z",
                        "reply_target": "room-001",
                        "routing_context": {
                            "peer": { "kind": "group", "id": "room-001" },
                            "parent_peer": null,
                            "guild_id": null,
                            "team_id": null,
                            "member_role_ids": [],
                            "identity_links": []
                        },
                        "raw_payload": { "ok": true }
                    }
                ]),
                _ => serde_json::json!({}),
            };

            let response_body = serde_json::json!({
                "output": output.to_string()
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
        requests
    });
    (format!("http://{}", addr), handle)
}

#[tokio::test]
async fn connector_commands_bridge_catalog_diagnostics_ack_and_replay() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let (base_url, server_task) = spawn_mock_connector_sidecar(4).await;
    set_app_setting(&pool, "im_sidecar_base_url", &base_url)
        .await
        .expect("set generic sidecar url");

    let connectors = list_channel_connectors_with_pool(&pool, None)
        .await
        .expect("list connectors");
    assert_eq!(connectors.len(), 2);
    assert_eq!(connectors[1].channel, "wecom");
    assert_eq!(connectors[1].capabilities.len(), 4);

    let diagnostics = get_channel_connector_diagnostics_with_pool(
        &pool,
        "wecom:wecom-main".to_string(),
        None,
    )
    .await
    .expect("get diagnostics");
    assert_eq!(diagnostics.connector.display_name, "企业微信连接器");
    assert_eq!(diagnostics.status, "authentication_error");
    assert_eq!(
        diagnostics.health.issue.as_ref().map(|issue| issue.user_message.as_str()),
        Some("签名校验失败")
    );

    ack_channel_events_with_pool(
        &pool,
        "wecom:wecom-main".to_string(),
        vec!["msg-001".to_string()],
        Some("processed".to_string()),
        None,
    )
    .await
    .expect("ack channel events");

    let replayed = replay_channel_events_with_pool(
        &pool,
        "wecom:wecom-main".to_string(),
        Some(10),
        None,
    )
    .await
    .expect("replay channel events");
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].message_id.as_deref(), Some("msg-001"));

    let requests = server_task.await.expect("mock sidecar task");
    assert_eq!(requests[0].0, "/api/channels/catalog");
    assert_eq!(requests[1].0, "/api/channels/diagnostics");
    assert_eq!(requests[1].1["instance_id"], "wecom:wecom-main");
    assert_eq!(requests[2].0, "/api/channels/ack");
    assert_eq!(requests[2].1["instance_id"], "wecom:wecom-main");
    assert_eq!(requests[2].1["message_id"], "msg-001");
    assert_eq!(requests[2].1["status"], "processed");
    assert_eq!(requests[3].0, "/api/channels/replay-events");
    assert_eq!(requests[3].1["limit"], 10);
}
