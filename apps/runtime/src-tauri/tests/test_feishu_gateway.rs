mod helpers;

use runtime_lib::approval_bus::{ApprovalDecision, ApprovalManager, CreateApprovalRequest};
use runtime_lib::commands::feishu_gateway::{
    calculate_feishu_signature, clear_feishu_runtime_state_for_outbound,
    maybe_handle_feishu_approval_command_with_pool, notify_feishu_approval_requested_with_pool,
    parse_feishu_payload, plan_role_dispatch_requests_for_feishu, plan_role_events_for_feishu,
    remember_feishu_runtime_state_for_outbound, resolve_feishu_app_credentials,
    resolve_feishu_sidecar_base_url, send_feishu_text_message_with_pool, set_app_setting,
    validate_feishu_auth_with_pool, validate_feishu_signature_with_pool, ParsedFeishuPayload,
};
use runtime_lib::commands::openclaw_plugins::{
    handle_openclaw_plugin_feishu_runtime_send_result_event,
    OpenClawPluginFeishuOutboundSendResult, OpenClawPluginFeishuRuntimeState,
    OpenClawPluginFeishuRuntimeStatus,
};
use runtime_lib::commands::im_config::bind_thread_roles_with_pool;
use runtime_lib::im::types::{ImEvent, ImEventType};
use std::fs;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex as TokioMutex;

fn feishu_runtime_test_lock() -> &'static TokioMutex<()> {
    static LOCK: OnceLock<TokioMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| TokioMutex::new(()))
}

struct MockFeishuRuntime {
    state: OpenClawPluginFeishuRuntimeState,
    capture_path: PathBuf,
    script_path: PathBuf,
    process_slot: Arc<Mutex<Option<Child>>>,
    stdout_thread: thread::JoinHandle<()>,
}

#[repr(C)]
struct FeishuRuntimeStoreMirror {
    process: Option<Arc<Mutex<Option<Child>>>>,
    stdin: Option<Arc<Mutex<ChildStdin>>>,
    pending_outbound_send_results: HashMap<
        String,
        std::sync::mpsc::SyncSender<Result<OpenClawPluginFeishuOutboundSendResult, String>>,
    >,
    status: OpenClawPluginFeishuRuntimeStatus,
}

fn unique_temp_path(prefix: &str, suffix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "{}-{}-{}{}",
        prefix,
        std::process::id(),
        stamp,
        suffix
    ))
}

async fn spawn_mock_feishu_runtime() -> MockFeishuRuntime {
    clear_feishu_runtime_state_for_outbound();
    eprintln!("[test-fixture] spawn mock feishu runtime start");
    let capture_path = unique_temp_path("workclaw-feishu-runtime-capture", ".json");
    let script_path = unique_temp_path("workclaw-feishu-runtime-mock", ".cjs");
    let script = r#"
const fs = require('fs');
const readline = require('readline');
const capturePath = process.env.FEISHU_RUNTIME_CAPTURE_PATH;
let sequence = 0;
const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
rl.on('line', (line) => {
  const trimmed = line.trim();
  if (!trimmed) {
    return;
  }
  let payload;
  try {
    payload = JSON.parse(trimmed);
  } catch (error) {
    return;
  }
  fs.writeFileSync(capturePath, JSON.stringify(payload, null, 2));
  sequence += 1;
  const response = {
    event: 'send_result',
    requestId: payload.request_id,
    request: {
      requestId: payload.request_id,
      accountId: payload.account_id,
      target: payload.target,
      threadId: payload.thread_id ?? null,
      text: payload.text,
      mode: payload.mode,
    },
    result: {
      delivered: true,
      channel: 'feishu',
      accountId: payload.account_id,
      target: payload.target,
      threadId: payload.thread_id ?? null,
      text: payload.text,
      mode: payload.mode,
      messageId: `om_mock_${sequence}`,
      chatId: payload.target,
      sequence,
    },
  };
  process.stdout.write(JSON.stringify(response) + '\n');
});
"#;
    fs::write(
        &script_path,
        script,
    )
    .expect("write mock runtime script");

    let mut child = Command::new("node")
        .arg(&script_path)
        .env("FEISHU_RUNTIME_CAPTURE_PATH", &capture_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mock runtime");
    let stdout = child.stdout.take().expect("runtime stdout");
    let process_slot = Arc::new(Mutex::new(Some(child)));
    let stdin_slot = Arc::new(Mutex::new(
        process_slot
            .lock()
            .expect("lock runtime process")
            .as_mut()
            .expect("child process")
            .stdin
            .take()
            .expect("runtime stdin"),
    ));
    let state = OpenClawPluginFeishuRuntimeState::default();
    eprintln!("[test-fixture] created default runtime state");
    let mirror_state: Arc<Mutex<FeishuRuntimeStoreMirror>> =
        unsafe { std::mem::transmute(state.0.clone()) };
    eprintln!("[test-fixture] transmuted runtime state arc");
    {
        let mut guard = mirror_state.lock().expect("lock mirror runtime state");
        eprintln!("[test-fixture] locked mirror runtime state");
        guard.process = Some(process_slot.clone());
        eprintln!("[test-fixture] wrote process");
        guard.stdin = Some(stdin_slot.clone());
        eprintln!("[test-fixture] wrote stdin");
        guard.pending_outbound_send_results = HashMap::new();
        eprintln!("[test-fixture] wrote pending map");
        guard.status = OpenClawPluginFeishuRuntimeStatus {
            running: true,
            ..Default::default()
        };
        eprintln!("[test-fixture] wrote mirror runtime state");
    }
    remember_feishu_runtime_state_for_outbound(&state);
    eprintln!("[test-fixture] remembered runtime state for outbound");

    let state_for_stdout = state.clone();
    let stdout_thread = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
                continue;
            };
            let _ = handle_openclaw_plugin_feishu_runtime_send_result_event(&state_for_stdout, &value);
        }
    });

    MockFeishuRuntime {
        state,
        capture_path,
        script_path,
        process_slot,
        stdout_thread,
    }
}

fn shutdown_mock_feishu_runtime(runtime: MockFeishuRuntime) {
    let MockFeishuRuntime {
        state,
        capture_path,
        script_path,
        process_slot,
        stdout_thread,
    } = runtime;
    clear_feishu_runtime_state_for_outbound();
    drop(state);
    if let Ok(mut guard) = process_slot.lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.wait();
        }
    }
    let _ = stdout_thread.join();
    let _ = fs::remove_file(capture_path);
    let _ = fs::remove_file(script_path);
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
async fn feishu_outbound_send_uses_official_runtime_helper() {
    let _guard = feishu_runtime_test_lock().lock().await;
    let (pool, _tmp) = helpers::setup_test_db().await;
    let runtime = spawn_mock_feishu_runtime().await;
    eprintln!("[test-fixture] outbound send about to call helper");

    let result_json = send_feishu_text_message_with_pool(
        &pool,
        "chat-outbound-1",
        "你好，来自官方 runtime",
        Some("http://127.0.0.1:9000".to_string()),
    )
    .await
    .expect("send via official runtime");
    eprintln!("[test-fixture] outbound send helper returned");

    let result: OpenClawPluginFeishuOutboundSendResult =
        serde_json::from_str(&result_json).expect("parse outbound send result");
    assert_eq!(result.request.account_id, "default");
    assert_eq!(result.request.target, "chat-outbound-1");
    assert_eq!(result.request.thread_id.as_deref(), Some("chat-outbound-1"));
    assert_eq!(result.request.text, "你好，来自官方 runtime");
    assert_eq!(result.request.mode, "text");
    assert!(result.result.delivered);
    assert_eq!(result.result.channel, "feishu");
    assert_eq!(result.result.account_id, "default");
    assert_eq!(result.result.target, "chat-outbound-1");
    assert_eq!(result.result.chat_id, "chat-outbound-1");
    assert_eq!(result.result.text, "你好，来自官方 runtime");

    shutdown_mock_feishu_runtime(runtime);
}

#[tokio::test]
async fn feishu_outbound_send_requires_registered_runtime() {
    let _guard = feishu_runtime_test_lock().lock().await;
    let (pool, _tmp) = helpers::setup_test_db().await;
    clear_feishu_runtime_state_for_outbound();

    let error = send_feishu_text_message_with_pool(&pool, "chat-outbound-2", "你好", None)
        .await
        .expect_err("runtime should be required");
    assert!(error.contains("official feishu runtime is not registered"));
}

#[tokio::test]
async fn feishu_pending_approval_notification_targets_bound_thread() {
    let _guard = feishu_runtime_test_lock().lock().await;
    let (pool, _tmp) = helpers::setup_test_db().await;
    let runtime = spawn_mock_feishu_runtime().await;

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

    shutdown_mock_feishu_runtime(runtime);
}

#[tokio::test]
async fn feishu_approve_command_resolves_pending_approval() {
    let _guard = feishu_runtime_test_lock().lock().await;
    let (pool, _tmp) = helpers::setup_test_db().await;
    let runtime = spawn_mock_feishu_runtime().await;

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
                work_dir: None,
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
            sender_id: Some("ou_approver_1".to_string()),
            chat_type: Some("direct".to_string()),
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

    shutdown_mock_feishu_runtime(runtime);
}
