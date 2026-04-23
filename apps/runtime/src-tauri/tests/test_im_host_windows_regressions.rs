mod helpers;

use runtime_lib::approval_bus::PendingApprovalRecord;
use runtime_lib::commands::feishu_gateway::{
    clear_feishu_runtime_state_for_outbound, parse_feishu_payload,
    remember_feishu_runtime_state_for_outbound,
    set_feishu_official_runtime_outbound_send_hook_for_tests, ParsedFeishuPayload,
};
use runtime_lib::commands::im_host::test_support::{
    maybe_dispatch_registered_im_session_reply_with_pool,
    maybe_emit_registered_host_lifecycle_phase_for_session_with_pool,
    maybe_notify_registered_approval_requested_with_pool,
    maybe_notify_registered_ask_user_requested_with_pool,
};
use runtime_lib::commands::openclaw_plugins::{
    OpenClawPluginFeishuOutboundDeliveryResult, OpenClawPluginFeishuRuntimeState,
};
use runtime_lib::commands::wecom_gateway::test_support::{
    clear_wecom_test_hooks, install_recording_wecom_interactive_lifecycle_hooks,
    install_recording_wecom_lifecycle_hook, install_recording_wecom_send_hook,
};
use runtime_lib::im::types::{ImEvent, ImEventType};
use serde_json::json;
use sqlx::SqlitePool;
use std::sync::{Arc, Mutex};

async fn setup_legacy_only_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("create sqlite memory pool");

    sqlx::query(
        "CREATE TABLE im_thread_sessions (
            thread_id TEXT NOT NULL,
            employee_id TEXT NOT NULL DEFAULT '',
            session_id TEXT NOT NULL,
            route_session_key TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(&pool)
    .await
    .expect("create legacy im_thread_sessions");

    sqlx::query(
        "CREATE TABLE im_inbox_events (
            id TEXT PRIMARY KEY,
            event_id TEXT NOT NULL DEFAULT '',
            thread_id TEXT NOT NULL,
            message_id TEXT NOT NULL DEFAULT '',
            text_preview TEXT NOT NULL DEFAULT '',
            source TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(&pool)
    .await
    .expect("create legacy im_inbox_events");

    sqlx::query(
        "CREATE TABLE app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .expect("create legacy app_settings");

    pool
}

async fn seed_session_channel(
    pool: &SqlitePool,
    session_id: &str,
    thread_id: &str,
    source: &str,
    message_id: &str,
) {
    sqlx::query(
        "INSERT INTO im_thread_sessions (thread_id, employee_id, session_id, route_session_key, created_at, updated_at)
         VALUES (?, '', ?, '', '2026-04-19T00:00:00Z', '2026-04-19T00:00:01Z')",
    )
    .bind(thread_id)
    .bind(session_id)
    .execute(pool)
    .await
    .expect("seed im_thread_sessions");

    sqlx::query(
        "INSERT INTO im_inbox_events (id, event_id, thread_id, message_id, text_preview, source, created_at)
         VALUES (?, ?, ?, ?, 'hello', ?, '2026-04-19T00:00:02Z')",
    )
    .bind(format!("evt-{thread_id}"))
    .bind(format!("evt-{thread_id}"))
    .bind(thread_id)
    .bind(message_id)
    .bind(source)
    .execute(pool)
    .await
    .expect("seed im_inbox_events");
}

async fn ensure_authority_store_tables(pool: &SqlitePool) {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS agent_conversation_bindings (
            conversation_id TEXT NOT NULL,
            channel TEXT NOT NULL,
            account_id TEXT NOT NULL DEFAULT '',
            agent_id TEXT NOT NULL,
            session_key TEXT NOT NULL,
            session_id TEXT NOT NULL,
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
    .execute(pool)
    .await
    .expect("create agent_conversation_bindings");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS channel_delivery_routes (
            session_key TEXT PRIMARY KEY,
            channel TEXT NOT NULL,
            account_id TEXT NOT NULL DEFAULT '',
            conversation_id TEXT NOT NULL DEFAULT '',
            reply_target TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .expect("create channel_delivery_routes");
}

async fn seed_authority_delivery_route(
    pool: &SqlitePool,
    session_id: &str,
    session_key: &str,
    conversation_id: &str,
    thread_id: &str,
    source: &str,
    account_id: &str,
    message_id: &str,
) {
    ensure_authority_store_tables(pool).await;

    sqlx::query(
        "INSERT INTO agent_conversation_bindings (
            conversation_id,
            channel,
            account_id,
            agent_id,
            session_key,
            session_id,
            base_conversation_id,
            parent_conversation_candidates_json,
            scope,
            peer_kind,
            peer_id,
            topic_id,
            sender_id,
            created_at,
            updated_at
        )
        VALUES (?, ?, ?, 'agent-test', ?, ?, ?, '[]', 'peer', 'group', ?, '', '', '2026-04-19T00:00:00Z', '2026-04-19T00:00:01Z')",
    )
    .bind(conversation_id)
    .bind(source)
    .bind(account_id)
    .bind(session_key)
    .bind(session_id)
    .bind(conversation_id)
    .bind(thread_id)
    .execute(pool)
    .await
    .expect("seed agent_conversation_bindings");

    sqlx::query(
        "INSERT INTO channel_delivery_routes (
            session_key,
            channel,
            account_id,
            conversation_id,
            reply_target,
            updated_at
        )
        VALUES (?, ?, ?, ?, ?, '2026-04-19T00:00:02Z')",
    )
    .bind(session_key)
    .bind(source)
    .bind(account_id)
    .bind(conversation_id)
    .bind(thread_id)
    .execute(pool)
    .await
    .expect("seed channel_delivery_routes");

    sqlx::query(
        "INSERT INTO im_inbox_events (id, event_id, thread_id, message_id, text_preview, source, created_at)
         VALUES (?, ?, ?, ?, 'hello', ?, '2026-04-19T00:00:03Z')",
    )
    .bind(format!("evt-authority-{thread_id}"))
    .bind(format!("evt-authority-{thread_id}"))
    .bind(thread_id)
    .bind(message_id)
    .bind(source)
    .execute(pool)
    .await
    .expect("seed authority-backed im_inbox_events");
}

fn install_recording_feishu_send_hook() -> Arc<Mutex<Vec<(String, String, Option<String>, String)>>>
{
    let runtime_state = OpenClawPluginFeishuRuntimeState::default();
    remember_feishu_runtime_state_for_outbound(&runtime_state);

    let captured = Arc::new(Mutex::new(
        Vec::<(String, String, Option<String>, String)>::new(),
    ));
    let captured_for_hook = captured.clone();
    set_feishu_official_runtime_outbound_send_hook_for_tests(Some(Arc::new(move |request| {
        captured_for_hook
            .lock()
            .expect("lock feishu outbound captures")
            .push((
                request.account_id.clone(),
                request.target.clone(),
                request.thread_id.clone(),
                request.text.clone(),
            ));
        Ok(OpenClawPluginFeishuOutboundDeliveryResult {
            delivered: true,
            channel: "feishu".to_string(),
            account_id: request.account_id.clone(),
            target: request.target.clone(),
            thread_id: request.thread_id.clone(),
            text: request.text.clone(),
            mode: request.mode.clone(),
            message_id: format!("om_{}", request.request_id),
            chat_id: request
                .thread_id
                .clone()
                .unwrap_or_else(|| "oc_chat_test".to_string()),
            sequence: 1,
        })
    })));

    captured
}

fn clear_feishu_test_hooks() {
    set_feishu_official_runtime_outbound_send_hook_for_tests(None);
    clear_feishu_runtime_state_for_outbound();
}

#[test]
fn feishu_bridge_payload_backfills_topic_projection_for_normalized_event_contract() {
    let payload = serde_json::to_string(&ImEvent {
        channel: "feishu".to_string(),
        event_type: ImEventType::MessageCreated,
        thread_id: "oc_topic_bridge".to_string(),
        event_id: Some("evt_topic_bridge".to_string()),
        message_id: Some("om_topic_bridge_reply".to_string()),
        text: Some("继续这个桥接话题".to_string()),
        role_id: None,
        account_id: Some("tenant_bridge".to_string()),
        tenant_id: Some("tenant_bridge".to_string()),
        sender_id: Some("ou_topic_sender".to_string()),
        chat_type: Some("group".to_string()),
        conversation_id: Some(
            "feishu:tenant_bridge:group:oc_topic_bridge:topic:om_topic_bridge_root".to_string(),
        ),
        base_conversation_id: None,
        parent_conversation_candidates: Vec::new(),
        conversation_scope: None,
    })
    .expect("serialize bridge payload");

    let parsed = parse_feishu_payload(&payload).expect("parse normalized bridge payload");
    let ParsedFeishuPayload::Event(event) = parsed else {
        panic!("expected feishu event");
    };

    assert_eq!(
        event.conversation_id.as_deref(),
        Some("feishu:tenant_bridge:group:oc_topic_bridge:topic:om_topic_bridge_root")
    );
    assert_eq!(
        event.base_conversation_id.as_deref(),
        Some("feishu:tenant_bridge:group:oc_topic_bridge")
    );
    assert_eq!(
        event.parent_conversation_candidates,
        vec!["feishu:tenant_bridge:group:oc_topic_bridge".to_string()]
    );
    assert_eq!(event.conversation_scope.as_deref(), Some("topic"));
}

#[test]
fn feishu_bridge_payload_falls_back_from_blank_account_id_to_tenant_id() {
    let payload = serde_json::to_string(&ImEvent {
        channel: "feishu".to_string(),
        event_type: ImEventType::MessageCreated,
        thread_id: "oc_blank_account_bridge".to_string(),
        event_id: Some("evt_blank_account_bridge".to_string()),
        message_id: Some("om_blank_account_bridge".to_string()),
        text: Some("继续这个桥接边界情况".to_string()),
        role_id: None,
        account_id: Some("   ".to_string()),
        tenant_id: Some("tenant-bridge-fallback".to_string()),
        sender_id: Some("ou_sender_bridge".to_string()),
        chat_type: Some("group".to_string()),
        conversation_id: None,
        base_conversation_id: None,
        parent_conversation_candidates: Vec::new(),
        conversation_scope: None,
    })
    .expect("serialize bridge payload");

    let parsed = parse_feishu_payload(&payload).expect("parse normalized bridge payload");
    let ParsedFeishuPayload::Event(event) = parsed else {
        panic!("expected feishu event");
    };

    assert_eq!(
        event.conversation_id.as_deref(),
        Some("feishu:tenant-bridge-fallback:group:oc_blank_account_bridge")
    );
    assert_eq!(
        event.base_conversation_id.as_deref(),
        Some("feishu:tenant-bridge-fallback:group:oc_blank_account_bridge")
    );
    assert!(event.parent_conversation_candidates.is_empty());
    assert_eq!(event.conversation_scope.as_deref(), Some("peer"));
}

#[test]
fn feishu_bridge_payload_falls_back_to_sender_id_for_sparse_account_inputs() {
    let payload = serde_json::to_string(&ImEvent {
        channel: "feishu".to_string(),
        event_type: ImEventType::MessageCreated,
        thread_id: "oc_sparse_account_bridge".to_string(),
        event_id: Some("evt_sparse_account_bridge".to_string()),
        message_id: Some("om_sparse_account_bridge".to_string()),
        text: Some("继续这个仅 sender 的桥接情况".to_string()),
        role_id: None,
        account_id: Some("   ".to_string()),
        tenant_id: None,
        sender_id: Some("ou_sparse_sender_bridge".to_string()),
        chat_type: Some("group".to_string()),
        conversation_id: None,
        base_conversation_id: None,
        parent_conversation_candidates: Vec::new(),
        conversation_scope: None,
    })
    .expect("serialize sparse-account bridge payload");

    let parsed = parse_feishu_payload(&payload).expect("parse sparse-account bridge payload");
    let ParsedFeishuPayload::Event(event) = parsed else {
        panic!("expected feishu event");
    };

    assert_eq!(event.tenant_id.as_deref(), Some("ou_sparse_sender_bridge"));
    assert_eq!(
        event.conversation_id.as_deref(),
        Some("feishu:ou_sparse_sender_bridge:group:oc_sparse_account_bridge")
    );
    assert_eq!(
        event.base_conversation_id.as_deref(),
        Some("feishu:ou_sparse_sender_bridge:group:oc_sparse_account_bridge")
    );
    assert!(event.parent_conversation_candidates.is_empty());
    assert_eq!(event.conversation_scope.as_deref(), Some("peer"));
}

#[tokio::test]
async fn wecom_unified_host_regressions_run_in_windows_safe_target() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    seed_session_channel(
        &pool,
        "session-wecom-ask-user",
        "wecom_chat_ask_user",
        "wecom",
        "wm_parent_ask_user",
    )
    .await;
    let ask_user_sent = install_recording_wecom_send_hook();
    let ask_user_lifecycle = install_recording_wecom_interactive_lifecycle_hooks();

    let ask_user_result = maybe_notify_registered_ask_user_requested_with_pool(
        &pool,
        "session-wecom-ask-user",
        "请确认企微方案",
        &["方案一".to_string(), "方案二".to_string()],
        None,
    )
    .await
    .expect("notify wecom ask_user");

    assert!(ask_user_result);
    let ask_user_texts = ask_user_sent.lock().expect("lock ask_user sent");
    assert_eq!(ask_user_texts.len(), 1);
    assert!(ask_user_texts[0].contains("请确认企微方案"));
    assert!(ask_user_texts[0].contains("可选项：方案一 / 方案二"));
    assert_eq!(
        ask_user_lifecycle
            .lock()
            .expect("lock ask_user lifecycle")
            .as_slice(),
        [
            "processing_stop:wm_parent_ask_user:ask_user",
            "lifecycle:wm_parent_ask_user:\"ask_user_requested\"",
        ]
    );
    clear_wecom_test_hooks();

    seed_session_channel(
        &pool,
        "session-wecom-approval-request",
        "wecom_chat_approval_request",
        "wecom",
        "wm_parent_approval_request",
    )
    .await;
    let approval_sent = install_recording_wecom_send_hook();
    let approval_lifecycle = install_recording_wecom_interactive_lifecycle_hooks();
    let approval_record = PendingApprovalRecord {
        approval_id: "approval-wecom-1".to_string(),
        session_id: "session-wecom-approval-request".to_string(),
        run_id: None,
        call_id: "call-wecom-1".to_string(),
        tool_name: "shell".to_string(),
        input: json!({"command": "rm -rf /tmp/wecom-demo"}),
        summary: "执行企微高风险命令".to_string(),
        impact: Some("可能修改企微关联工作目录内容".to_string()),
        irreversible: true,
        status: "pending".to_string(),
    };

    let approval_result = maybe_notify_registered_approval_requested_with_pool(
        &pool,
        "session-wecom-approval-request",
        &approval_record,
        None,
    )
    .await
    .expect("notify wecom approval requested");

    assert!(approval_result);
    let approval_sent_text = approval_sent.lock().expect("lock approval sent");
    assert_eq!(approval_sent_text.len(), 1);
    assert!(approval_sent_text[0].contains("待审批 #approval-wecom-1"));
    assert!(approval_sent_text[0]
        .contains("/approve approval-wecom-1 allow_once | allow_always | deny"));
    assert_eq!(
        approval_lifecycle
            .lock()
            .expect("lock approval lifecycle")
            .as_slice(),
        [
            "processing_stop:wm_parent_approval_request:waiting_approval",
            "lifecycle:wm_parent_approval_request:\"approval_requested\"",
        ]
    );
    clear_wecom_test_hooks();

    seed_session_channel(
        &pool,
        "session-wecom-lifecycle",
        "wecom_chat_lifecycle",
        "wecom",
        "wm_parent_lifecycle",
    )
    .await;
    let lifecycle_records = install_recording_wecom_lifecycle_hook();

    let answered = maybe_emit_registered_host_lifecycle_phase_for_session_with_pool(
        &pool,
        "session-wecom-lifecycle",
        None,
        "ask_user_answered",
        None,
    )
    .await
    .expect("emit wecom ask_user_answered");
    let resolved = maybe_emit_registered_host_lifecycle_phase_for_session_with_pool(
        &pool,
        "session-wecom-lifecycle",
        Some("reply-wecom-approval"),
        "approval_resolved",
        None,
    )
    .await
    .expect("emit wecom approval_resolved");
    let resumed = maybe_emit_registered_host_lifecycle_phase_for_session_with_pool(
        &pool,
        "session-wecom-lifecycle",
        Some("reply-wecom-resumed"),
        "resumed",
        None,
    )
    .await
    .expect("emit wecom resumed");

    assert!(answered);
    assert!(resolved);
    assert!(resumed);
    assert_eq!(
        lifecycle_records
            .lock()
            .expect("lock lifecycle records")
            .as_slice(),
        [
            "lifecycle:wm_parent_lifecycle:\"ask_user_answered\"",
            "lifecycle:wm_parent_lifecycle:\"approval_resolved\"",
            "lifecycle:wm_parent_lifecycle:\"resumed\"",
        ]
    );
    clear_wecom_test_hooks();

    seed_session_channel(
        &pool,
        "session-wecom-dispatch",
        "wecom_chat_dispatch",
        "wecom",
        "wm_parent_dispatch",
    )
    .await;
    let dispatch_sent = install_recording_wecom_send_hook();

    let dispatch_result = maybe_dispatch_registered_im_session_reply_with_pool(
        &pool,
        "session-wecom-dispatch",
        "企微 unified host 最终回复",
    )
    .await
    .expect("dispatch wecom reply");

    assert!(dispatch_result);
    assert_eq!(
        dispatch_sent.lock().expect("lock dispatch sent").as_slice(),
        ["企微 unified host 最终回复"]
    );
    clear_wecom_test_hooks();
}

#[tokio::test]
async fn unified_host_uses_authority_store_without_legacy_thread_bindings() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    seed_authority_delivery_route(
        &pool,
        "session-authority-ask-user",
        "wecom:tenant-a:agent-test:conversation-ask-user",
        "wecom:tenant-a:group:conversation-ask-user",
        "wecom_chat_authority_ask_user",
        "wecom",
        "",
        "wm_parent_authority_ask_user",
    )
    .await;

    let ask_user_sent = install_recording_wecom_send_hook();
    let ask_user_lifecycle = install_recording_wecom_interactive_lifecycle_hooks();
    let ask_user_result = maybe_notify_registered_ask_user_requested_with_pool(
        &pool,
        "session-authority-ask-user",
        "请确认 authority store 路由",
        &["是".to_string(), "否".to_string()],
        None,
    )
    .await
    .expect("notify ask_user via authority store");

    assert!(ask_user_result);
    let ask_user_messages = ask_user_sent.lock().expect("lock authority ask_user sent");
    assert_eq!(ask_user_messages.len(), 1);
    assert!(ask_user_messages[0].contains("请确认 authority store 路由"));
    assert!(ask_user_messages[0].contains("可选项：是 / 否"));
    assert_eq!(
        ask_user_lifecycle
            .lock()
            .expect("lock authority ask_user lifecycle")
            .as_slice(),
        [
            "processing_stop:wm_parent_authority_ask_user:ask_user",
            "lifecycle:wm_parent_authority_ask_user:\"ask_user_requested\"",
        ]
    );
    clear_wecom_test_hooks();

    seed_authority_delivery_route(
        &pool,
        "session-authority-dispatch",
        "wecom:tenant-a:agent-test:conversation-dispatch",
        "wecom:tenant-a:group:conversation-dispatch",
        "wecom_chat_authority_dispatch",
        "wecom",
        "",
        "wm_parent_authority_dispatch",
    )
    .await;

    let dispatch_sent = install_recording_wecom_send_hook();
    let dispatch_result = maybe_dispatch_registered_im_session_reply_with_pool(
        &pool,
        "session-authority-dispatch",
        "authority store 直接派发回复",
    )
    .await
    .expect("dispatch reply via authority store");

    assert!(dispatch_result);
    assert_eq!(
        dispatch_sent
            .lock()
            .expect("lock authority dispatch sent")
            .as_slice(),
        ["authority store 直接派发回复"]
    );
    clear_wecom_test_hooks();
}

#[tokio::test]
async fn feishu_session_reply_uses_authority_route_account_id() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    seed_authority_delivery_route(
        &pool,
        "session-feishu-authority-dispatch",
        "feishu:tenant-a:agent-test:conversation-dispatch",
        "feishu:tenant-a:group:conversation-dispatch",
        "oc_chat_authority_dispatch",
        "feishu",
        "feishu-account-a",
        "om_parent_authority_dispatch",
    )
    .await;

    let captured = install_recording_feishu_send_hook();
    let handled = maybe_dispatch_registered_im_session_reply_with_pool(
        &pool,
        "session-feishu-authority-dispatch",
        "authority route 飞书回复",
    )
    .await
    .expect("dispatch feishu reply via authority store");

    assert!(handled);
    let captured = captured.lock().expect("lock feishu outbound captures");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].0, "feishu-account-a");
    assert_eq!(captured[0].1, "oc_chat_authority_dispatch");
    assert_eq!(captured[0].2.as_deref(), Some("oc_chat_authority_dispatch"));
    assert_eq!(captured[0].3, "authority route 飞书回复");
    clear_feishu_test_hooks();
}

#[tokio::test]
async fn authority_store_topic_projection_still_routes_wecom_reply_dispatch() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    seed_authority_delivery_route(
        &pool,
        "session-authority-topic-dispatch",
        "wecom:tenant-a:agent-test:conversation-topic-dispatch",
        "wecom:tenant-a:group:wecom_chat_topic_dispatch:topic:topic-bridge-dispatch",
        "wecom_chat_topic_dispatch",
        "wecom",
        "",
        "wm_parent_authority_topic_dispatch",
    )
    .await;

    let dispatch_sent = install_recording_wecom_send_hook();
    let dispatch_result = maybe_dispatch_registered_im_session_reply_with_pool(
        &pool,
        "session-authority-topic-dispatch",
        "authority topic 直接派发回复",
    )
    .await
    .expect("dispatch topic reply via authority store");

    assert!(dispatch_result);
    assert_eq!(
        dispatch_sent
            .lock()
            .expect("lock authority topic dispatch sent")
            .as_slice(),
        ["authority topic 直接派发回复"]
    );
    clear_wecom_test_hooks();
}

#[tokio::test]
async fn unified_host_still_falls_back_to_legacy_thread_bindings() {
    let pool = setup_legacy_only_pool().await;

    seed_session_channel(
        &pool,
        "session-legacy-ask-user",
        "wecom_chat_legacy_ask_user",
        "wecom",
        "wm_parent_legacy_ask_user",
    )
    .await;

    let ask_user_sent = install_recording_wecom_send_hook();
    let ask_user_lifecycle = install_recording_wecom_interactive_lifecycle_hooks();
    let ask_user_result = maybe_notify_registered_ask_user_requested_with_pool(
        &pool,
        "session-legacy-ask-user",
        "legacy thread 还在吗",
        &["在".to_string(), "不在".to_string()],
        None,
    )
    .await
    .expect("notify ask_user via legacy bindings");

    assert!(ask_user_result);
    let sent = ask_user_sent.lock().expect("lock legacy ask_user sent");
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains("legacy thread 还在吗"));
    assert!(sent[0].contains("可选项：在 / 不在"));
    assert_eq!(
        ask_user_lifecycle
            .lock()
            .expect("lock legacy ask_user lifecycle")
            .as_slice(),
        [
            "processing_stop:wm_parent_legacy_ask_user:ask_user",
            "lifecycle:wm_parent_legacy_ask_user:\"ask_user_requested\"",
        ]
    );
    clear_wecom_test_hooks();
}
