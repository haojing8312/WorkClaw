mod helpers;

use runtime_lib::approval_bus::{ApprovalDecision, ApprovalManager, CreateApprovalRequest};
use runtime_lib::commands::feishu_gateway::{
    maybe_handle_feishu_approval_command_with_pool, notify_feishu_approval_requested_with_pool,
    calculate_feishu_signature, parse_feishu_payload, plan_role_dispatch_requests_for_feishu,
    plan_role_events_for_feishu, resolve_feishu_app_credentials, resolve_feishu_sidecar_base_url,
    set_app_setting, validate_feishu_auth_with_pool, validate_feishu_signature_with_pool,
    ParsedFeishuPayload,
};
use runtime_lib::commands::im_config::bind_thread_roles_with_pool;
use runtime_lib::im::types::{ImEvent, ImEventType};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn spawn_mock_feishu_sidecar(
    expected_requests: usize,
) -> (
    String,
    tokio::task::JoinHandle<Vec<(String, serde_json::Value)>>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind feishu mock sidecar");
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
            requests.push((path, body_json));

            let response_body = serde_json::json!({
                "output": serde_json::json!({ "message_id": "om_approval_reply_1" }).to_string()
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
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ('feishu_ingress_token', 'feishu-secret')",
    )
    .execute(&pool)
    .await
    .expect("seed token");

    assert!(
        validate_feishu_auth_with_pool(&pool, Some("feishu-secret".to_string()))
            .await
            .is_ok()
    );
    assert!(
        validate_feishu_auth_with_pool(&pool, Some("wrong".to_string()))
            .await
            .is_err()
    );
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
    assert_eq!(planned[0].message_type, "system");
    assert_eq!(planned[0].sender_role, "main_agent");
    assert_eq!(planned[0].source_channel, "feishu");

    let dispatches = plan_role_dispatch_requests_for_feishu(&pool, &evt)
        .await
        .expect("plan dispatch");
    assert_eq!(dispatches.len(), 2);
    assert_eq!(dispatches[0].thread_id, "chat-1");
    assert_eq!(dispatches[0].agent_type, "plan");
    assert!(dispatches[0].prompt.contains("场景=opportunity_review"));
    assert_eq!(dispatches[0].message_type, "user_input");
    assert_eq!(dispatches[0].sender_role, "main_agent");
    assert_eq!(dispatches[0].sender_employee_id, dispatches[0].role_id);
    assert_eq!(dispatches[0].target_employee_id, dispatches[0].role_id);
    assert_eq!(dispatches[0].source_channel, "feishu");
}

#[tokio::test]
async fn plan_role_dispatch_falls_back_to_thread_roles_when_mention_role_unknown() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    bind_thread_roles_with_pool(
        &pool,
        "chat-unknown-role",
        "tenant-x",
        "opportunity_review",
        &["presales".to_string(), "architect".to_string()],
    )
    .await
    .expect("bind roles");

    let parsed = parse_feishu_payload(
        r#"{
          "header":{"event_id":"evt-unknown","event_type":"im.message.receive_v1"},
          "event":{"message":{"message_id":"msg-unknown","chat_id":"chat-unknown-role","content":"{\"text\":\"@某人 请先分析\"}"}}
        }"#,
    )
    .expect("parse");

    let mut evt = match parsed {
        ParsedFeishuPayload::Event(e) => e,
        _ => panic!("expected event"),
    };
    evt.role_id = Some("ou_unknown_mention".to_string());

    let planned = plan_role_events_for_feishu(&pool, &evt)
        .await
        .expect("plan role events");
    assert_eq!(planned.len(), 2);

    let dispatches = plan_role_dispatch_requests_for_feishu(&pool, &evt)
        .await
        .expect("plan dispatch");
    assert_eq!(dispatches.len(), 2);
}

#[tokio::test]
async fn resolve_feishu_settings_reads_from_app_settings() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    set_app_setting(&pool, "feishu_app_id", "cli_app")
        .await
        .expect("set app id");
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

#[tokio::test]
async fn resolve_feishu_sidecar_base_url_falls_back_to_generic_im_sidecar_key() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    set_app_setting(&pool, "im_sidecar_base_url", "http://127.0.0.1:9100")
        .await
        .expect("set generic sidecar url");

    let base_url = resolve_feishu_sidecar_base_url(&pool, None)
        .await
        .expect("resolve generic sidecar");
    assert_eq!(base_url.as_deref(), Some("http://127.0.0.1:9100"));
}

#[tokio::test]
async fn feishu_pending_approval_notification_targets_bound_thread() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let (base_url, server_task) = spawn_mock_feishu_sidecar(1).await;
    set_app_setting(&pool, "feishu_sidecar_base_url", &base_url)
        .await
        .expect("set sidecar url");

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO im_thread_sessions (thread_id, employee_id, session_id, route_session_key, created_at, updated_at)
         VALUES (?, ?, ?, '', ?, ?)",
    )
    .bind("chat-approval-1")
    .bind("employee-1")
    .bind("session-feishu-approval")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .expect("seed thread session");
    sqlx::query(
        "INSERT INTO im_inbox_events (id, event_id, thread_id, message_id, text_preview, source, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("inbox-1")
    .bind("evt-approval-1")
    .bind("chat-approval-1")
    .bind("msg-approval-1")
    .bind("hello")
    .bind("feishu")
    .bind(&now)
    .execute(&pool)
    .await
    .expect("seed feishu inbox event");

    notify_feishu_approval_requested_with_pool(
        &pool,
        "session-feishu-approval",
        &runtime_lib::approval_bus::PendingApprovalRecord {
            approval_id: "approval-feishu-1".to_string(),
            session_id: "session-feishu-approval".to_string(),
            run_id: Some("run-feishu-1".to_string()),
            call_id: "call-feishu-1".to_string(),
            tool_name: "file_delete".to_string(),
            input: serde_json::json!({
                "path": "C:\\\\Users\\\\demo\\\\danger",
                "recursive": true
            }),
            summary: "删除目录 C:\\Users\\demo\\danger".to_string(),
            impact: Some("目录及其全部子文件会被永久删除".to_string()),
            irreversible: true,
            status: "pending".to_string(),
        },
        None,
    )
    .await
    .expect("notify approval request");

    let requests = server_task.await.expect("mock server task");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "/api/feishu/send-message");
    assert_eq!(requests[0].1["receive_id"], "chat-approval-1");
    let content = requests[0].1["content"].as_str().unwrap_or_default();
    assert!(content.contains("approval-feishu-1"));
    assert!(content.contains("/approve approval-feishu-1 allow_once"));
    assert!(content.contains("allow_always"));
    assert!(content.contains("deny"));
}

#[tokio::test]
async fn feishu_approve_command_resolves_pending_approval() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let (base_url, server_task) = spawn_mock_feishu_sidecar(1).await;
    set_app_setting(&pool, "feishu_sidecar_base_url", &base_url)
        .await
        .expect("set sidecar url");

    let approvals = ApprovalManager::default();
    approvals
        .create_pending_with_pool(
            &pool,
            None,
            CreateApprovalRequest {
                approval_id: "approval-feishu-cmd-1".to_string(),
                session_id: "session-feishu-cmd-1".to_string(),
                run_id: Some("run-feishu-cmd-1".to_string()),
                call_id: "call-feishu-cmd-1".to_string(),
                tool_name: "file_delete".to_string(),
                input: serde_json::json!({
                    "path": "C:\\\\Users\\\\demo\\\\danger",
                    "recursive": true
                }),
                summary: "删除目录 C:\\Users\\demo\\danger".to_string(),
                impact: Some("目录及其全部子文件会被永久删除".to_string()),
                irreversible: true,
            },
        )
        .await
        .expect("create pending approval");

    let result = maybe_handle_feishu_approval_command_with_pool(
        &pool,
        &approvals,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-approval-2".to_string(),
            event_id: Some("evt-approval-cmd-1".to_string()),
            message_id: Some("msg-approval-cmd-1".to_string()),
            text: Some("/approve approval-feishu-cmd-1 allow_once".to_string()),
            role_id: None,
            account_id: Some("ou_approver_1".to_string()),
            tenant_id: Some("ou_approver_1".to_string()),
        },
        None,
    )
    .await
    .expect("handle approval command");

    let applied = result.expect("command should be handled");
    match applied {
        runtime_lib::approval_bus::ApprovalResolveResult::Applied {
            approval_id,
            status,
            decision,
        } => {
            assert_eq!(approval_id, "approval-feishu-cmd-1");
            assert_eq!(status, "approved");
            assert_eq!(decision, ApprovalDecision::AllowOnce);
        }
        other => panic!("expected applied resolution, got {:?}", other),
    }

    let row: (String, String, String, String) = sqlx::query_as(
        "SELECT status, decision, resolved_by_surface, resolved_by_user
         FROM approvals WHERE id = ?",
    )
    .bind("approval-feishu-cmd-1")
    .fetch_one(&pool)
    .await
    .expect("load approval row");
    assert_eq!(row.0, "approved");
    assert_eq!(row.1, "allow_once");
    assert_eq!(row.2, "feishu");
    assert_eq!(row.3, "ou_approver_1");

    let requests = server_task.await.expect("mock server task");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "/api/feishu/send-message");
    assert_eq!(requests[0].1["receive_id"], "chat-approval-2");
    let content = requests[0].1["content"].as_str().unwrap_or_default();
    assert!(content.contains("approval-feishu-cmd-1"));
    assert!(content.contains("allow_once"));
}
