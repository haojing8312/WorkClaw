use super::tauri_commands::should_restart_official_feishu_runtime_after_pairing_approval;
use super::types::FeishuInboundGateDecision;
use super::{
    apply_default_feishu_account_id, evaluate_openclaw_feishu_gate, generate_feishu_pairing_code,
    list_feishu_pairing_allow_from_with_pool, metadata_service::FeishuHostMetadata,
    parse_feishu_payload, resolve_fallback_default_feishu_account_id,
    resolve_feishu_pairing_account_id, resolve_feishu_pairing_request_with_pool,
    resolve_ws_role_id, sanitize_ws_inbound_text, upsert_feishu_pairing_request_with_pool,
    FeishuWsEventRecord, ParsedFeishuPayload,
};
use crate::commands::employee_agents::AgentEmployee;
use crate::commands::openclaw_plugins::{
    OpenClawPluginFeishuOutboundDeliveryResult, OpenClawPluginFeishuRuntimeState,
    OpenClawPluginFeishuRuntimeStatus,
};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::sync::{Arc, Mutex};

fn sample_host_metadata(
    account_id: &str,
    allow_from: Vec<&str>,
    account_config: serde_json::Value,
) -> FeishuHostMetadata {
    FeishuHostMetadata {
        channel: "feishu".to_string(),
        host_kind: "openclaw_plugin".to_string(),
        status: "ready".to_string(),
        instance_id: Some(account_id.to_string()),
        default_account_id: Some(account_id.to_string()),
        account_ids: vec![account_id.to_string()],
        accounts: vec![crate::commands::im_host::ImChannelAccountMetadata {
            account_id: account_id.to_string(),
            account: serde_json::json!({
                "accountId": account_id,
                "config": account_config,
            }),
            described_account: serde_json::json!({
                "accountId": account_id,
            }),
            allow_from: allow_from.into_iter().map(str::to_string).collect(),
            warnings: Vec::new(),
        }],
        runtime_status: None,
        plugin_host: None,
    }
}

async fn setup_pairing_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("create sqlite memory pool");

    sqlx::query(
        "CREATE TABLE feishu_pairing_requests (
            id TEXT PRIMARY KEY,
            channel TEXT NOT NULL DEFAULT 'feishu',
            account_id TEXT NOT NULL DEFAULT 'default',
            sender_id TEXT NOT NULL,
            chat_id TEXT NOT NULL DEFAULT '',
            code TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            resolved_at TEXT,
            resolved_by_user TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(&pool)
    .await
    .expect("create feishu_pairing_requests");

    sqlx::query(
        "CREATE UNIQUE INDEX idx_feishu_pairing_requests_pending
         ON feishu_pairing_requests(channel, account_id, sender_id)
         WHERE status = 'pending'",
    )
    .execute(&pool)
    .await
    .expect("create feishu_pairing_requests index");

    sqlx::query(
        "CREATE TABLE feishu_pairing_allow_from (
            channel TEXT NOT NULL DEFAULT 'feishu',
            account_id TEXT NOT NULL DEFAULT 'default',
            sender_id TEXT NOT NULL,
            source_request_id TEXT NOT NULL DEFAULT '',
            approved_at TEXT NOT NULL,
            approved_by_user TEXT NOT NULL DEFAULT '',
            PRIMARY KEY(channel, account_id, sender_id)
        )",
    )
    .execute(&pool)
    .await
    .expect("create feishu_pairing_allow_from");

    pool
}

#[test]
fn parse_feishu_payload_extracts_mention_role_and_cleans_text() {
    let payload = serde_json::json!({
        "header": {
            "event_id": "evt_1",
            "event_type": "im.message.receive_v1",
            "tenant_key": "tenant_1"
        },
        "event": {
            "message": {
                "message_id": "om_1",
                "chat_id": "oc_1",
                "chat_type": "group",
                "content": "{\"text\":\"@_user_1 你细化一下技术方案\"}"
            },
            "sender": {
                "sender_id": {
                    "open_id": "ou_sender"
                }
            },
            "mentions": [
                {
                    "key": "@_user_1",
                    "id": {
                        "open_id": "ou_dev_agent"
                    },
                    "name": "开发团队"
                }
            ]
        }
    });

    let parsed = parse_feishu_payload(&payload.to_string()).expect("payload should parse");
    match parsed {
        ParsedFeishuPayload::Event(event) => {
            assert_eq!(event.thread_id, "oc_1");
            assert_eq!(event.role_id.as_deref(), Some("ou_dev_agent"));
            assert_eq!(event.text.as_deref(), Some("你细化一下技术方案"));
            assert_eq!(event.sender_id.as_deref(), Some("ou_sender"));
            assert_eq!(event.chat_type.as_deref(), Some("group"));
            assert_eq!(event.tenant_id.as_deref(), Some("tenant_1"));
        }
        ParsedFeishuPayload::Challenge(_) => panic!("should parse as event"),
    }
}

#[test]
fn parse_feishu_payload_keeps_plain_text_when_no_mentions() {
    let payload = serde_json::json!({
        "header": {
            "event_id": "evt_2",
            "event_type": "im.message.receive_v1"
        },
        "event": {
            "message": {
                "message_id": "om_2",
                "chat_id": "oc_2",
                "content": "{\"text\":\"请给出实施方案\"}"
            }
        }
    });

    let parsed = parse_feishu_payload(&payload.to_string()).expect("payload should parse");
    match parsed {
        ParsedFeishuPayload::Event(event) => {
            assert_eq!(event.role_id, None);
            assert_eq!(event.text.as_deref(), Some("请给出实施方案"));
        }
        ParsedFeishuPayload::Challenge(_) => panic!("should parse as event"),
    }
}

#[test]
fn sanitize_ws_inbound_text_strips_placeholder_tokens() {
    let cleaned = sanitize_ws_inbound_text("@_user_1  你细化一下技术方案");
    assert_eq!(cleaned.as_deref(), Some("你细化一下技术方案"));
}

#[test]
fn resolve_ws_role_id_prefers_candidate_matching_employee() {
    let employees = vec![
        AgentEmployee {
            id: "1".to_string(),
            employee_id: "project_manager".to_string(),
            name: "项目经理".to_string(),
            role_id: "project_manager".to_string(),
            persona: String::new(),
            feishu_open_id: "ou_pm".to_string(),
            feishu_app_id: String::new(),
            feishu_app_secret: String::new(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: String::new(),
            openclaw_agent_id: "project_manager".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: Vec::new(),
            created_at: "2026-03-05T00:00:00Z".to_string(),
            updated_at: "2026-03-05T00:00:00Z".to_string(),
        },
        AgentEmployee {
            id: "2".to_string(),
            employee_id: "dev_team".to_string(),
            name: "开发团队".to_string(),
            role_id: "dev_team".to_string(),
            persona: String::new(),
            feishu_open_id: "ou_dev_team".to_string(),
            feishu_app_id: String::new(),
            feishu_app_secret: String::new(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: String::new(),
            openclaw_agent_id: "dev_team".to_string(),
            routing_priority: 90,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: Vec::new(),
            created_at: "2026-03-05T00:00:00Z".to_string(),
            updated_at: "2026-03-05T00:00:00Z".to_string(),
        },
    ];
    let event = FeishuWsEventRecord {
        employee_id: "project_manager".to_string(),
        source_employee_ids: vec!["project_manager".to_string(), "dev_team".to_string()],
        id: "oc_chat:om_1".to_string(),
        event_type: "im.message.receive_v1".to_string(),
        chat_id: "oc_chat".to_string(),
        message_id: "om_1".to_string(),
        text: "你细化一下技术方案".to_string(),
        mention_open_id: "ou_sender".to_string(),
        mention_open_ids: vec!["ou_sender".to_string(), "ou_dev_team".to_string()],
        sender_open_id: "ou_sender".to_string(),
        chat_type: "group".to_string(),
        received_at: "2026-03-05T00:00:00Z".to_string(),
    };

    let selected = resolve_ws_role_id(
        &event.mention_open_ids,
        Some(&event.text),
        &event.source_employee_ids,
        &employees,
    );
    assert_eq!(selected.as_deref(), Some("ou_dev_team"));
}

#[test]
fn resolve_ws_role_id_falls_back_to_single_source_employee() {
    let employees = vec![
        AgentEmployee {
            id: "1".to_string(),
            employee_id: "project_manager".to_string(),
            name: "项目经理".to_string(),
            role_id: "project_manager".to_string(),
            persona: String::new(),
            feishu_open_id: String::new(),
            feishu_app_id: String::new(),
            feishu_app_secret: String::new(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: String::new(),
            openclaw_agent_id: "project_manager".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: Vec::new(),
            created_at: "2026-03-05T00:00:00Z".to_string(),
            updated_at: "2026-03-05T00:00:00Z".to_string(),
        },
        AgentEmployee {
            id: "2".to_string(),
            employee_id: "tech_lead".to_string(),
            name: "开发人员".to_string(),
            role_id: "tech_lead".to_string(),
            persona: String::new(),
            feishu_open_id: String::new(),
            feishu_app_id: String::new(),
            feishu_app_secret: String::new(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: String::new(),
            openclaw_agent_id: "tech_lead".to_string(),
            routing_priority: 90,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: Vec::new(),
            created_at: "2026-03-05T00:00:00Z".to_string(),
            updated_at: "2026-03-05T00:00:00Z".to_string(),
        },
    ];

    let selected = resolve_ws_role_id(
        &[],
        Some("请你继续处理"),
        &["tech_lead".to_string()],
        &employees,
    );
    assert_eq!(selected.as_deref(), Some("tech_lead"));
}

#[test]
fn resolve_fallback_default_feishu_account_id_prefers_default_credentials() {
    let resolved = resolve_fallback_default_feishu_account_id(
        true,
        &["employee-a".to_string(), "employee-b".to_string()],
    );
    assert_eq!(resolved.as_deref(), Some("default"));
}

#[test]
fn resolve_fallback_default_feishu_account_id_uses_first_employee_when_needed() {
    let resolved = resolve_fallback_default_feishu_account_id(
        false,
        &["".to_string(), "employee-b".to_string()],
    );
    assert_eq!(resolved.as_deref(), Some("employee-b"));
}

#[test]
fn apply_default_feishu_account_id_only_fills_missing_values() {
    let mut event = crate::im::types::ImEvent {
        channel: "feishu".to_string(),
        event_type: crate::im::types::ImEventType::MessageCreated,
        thread_id: "oc_1".to_string(),
        event_id: None,
        message_id: None,
        text: Some("hello".to_string()),
        role_id: None,
        account_id: None,
        tenant_id: Some("tenant_1".to_string()),
        sender_id: Some("ou_sender".to_string()),
        chat_type: Some("group".to_string()),
    };
    apply_default_feishu_account_id(&mut event, Some("default"));
    assert_eq!(event.account_id.as_deref(), Some("default"));

    event.account_id = Some("tenant_key".to_string());
    apply_default_feishu_account_id(&mut event, Some("another"));
    assert_eq!(event.account_id.as_deref(), Some("tenant_key"));
}

#[test]
fn resolve_feishu_pairing_account_id_prefers_selected_snapshot_account() {
    let metadata = sample_host_metadata(
        "default",
        vec![],
        serde_json::json!({
            "dmPolicy": "pairing"
        }),
    );
    let event = crate::im::types::ImEvent {
        channel: "feishu".to_string(),
        event_type: crate::im::types::ImEventType::MessageCreated,
        thread_id: "oc_chat".to_string(),
        event_id: None,
        message_id: None,
        text: Some("你好".to_string()),
        role_id: None,
        account_id: Some("tenant_key".to_string()),
        sender_id: Some("ou_sender".to_string()),
        chat_type: Some("p2p".to_string()),
        tenant_id: Some("tenant_key".to_string()),
    };

    let resolved = resolve_feishu_pairing_account_id(&event, Some(&metadata));
    assert_eq!(resolved, "default");
}

#[test]
fn evaluate_openclaw_feishu_gate_allows_allowlisted_direct_sender() {
    let metadata = sample_host_metadata(
        "default",
        vec!["ou_allowed"],
        serde_json::json!({
            "dmPolicy": "allowlist"
        }),
    );
    let event = crate::im::types::ImEvent {
        channel: "feishu".to_string(),
        event_type: crate::im::types::ImEventType::MessageCreated,
        thread_id: "ou_allowed".to_string(),
        event_id: None,
        message_id: None,
        text: Some("hello".to_string()),
        role_id: None,
        account_id: Some("default".to_string()),
        tenant_id: Some("tenant_1".to_string()),
        sender_id: Some("ou_allowed".to_string()),
        chat_type: Some("p2p".to_string()),
    };

    assert_eq!(
        evaluate_openclaw_feishu_gate(&event, &metadata),
        FeishuInboundGateDecision::Allow
    );
}

#[test]
fn evaluate_openclaw_feishu_gate_rejects_unpaired_direct_sender() {
    let metadata = sample_host_metadata(
        "default",
        vec![],
        serde_json::json!({
            "dmPolicy": "pairing"
        }),
    );
    let event = crate::im::types::ImEvent {
        channel: "feishu".to_string(),
        event_type: crate::im::types::ImEventType::MessageCreated,
        thread_id: "ou_stranger".to_string(),
        event_id: None,
        message_id: None,
        text: Some("hello".to_string()),
        role_id: None,
        account_id: Some("default".to_string()),
        tenant_id: Some("tenant_1".to_string()),
        sender_id: Some("ou_stranger".to_string()),
        chat_type: Some("p2p".to_string()),
    };

    assert_eq!(
        evaluate_openclaw_feishu_gate(&event, &metadata),
        FeishuInboundGateDecision::Reject {
            reason: "pairing_pending"
        }
    );
}

#[test]
fn evaluate_openclaw_feishu_gate_rejects_group_without_required_mention() {
    let metadata = sample_host_metadata(
        "default",
        vec![],
        serde_json::json!({
            "groupPolicy": "open",
            "requireMention": true
        }),
    );
    let event = crate::im::types::ImEvent {
        channel: "feishu".to_string(),
        event_type: crate::im::types::ImEventType::MessageCreated,
        thread_id: "oc_group_1".to_string(),
        event_id: None,
        message_id: None,
        text: Some("大家看一下".to_string()),
        role_id: None,
        account_id: Some("default".to_string()),
        tenant_id: Some("tenant_1".to_string()),
        sender_id: Some("ou_sender".to_string()),
        chat_type: Some("group".to_string()),
    };

    assert_eq!(
        evaluate_openclaw_feishu_gate(&event, &metadata),
        FeishuInboundGateDecision::Reject {
            reason: "no_mention"
        }
    );
}

#[test]
fn evaluate_openclaw_feishu_gate_rejects_group_outside_allowlist() {
    let metadata = sample_host_metadata(
        "default",
        vec![],
        serde_json::json!({
            "groupPolicy": "allowlist",
            "groups": {
                "oc_allowed": {
                    "enabled": true
                }
            }
        }),
    );
    let event = crate::im::types::ImEvent {
        channel: "feishu".to_string(),
        event_type: crate::im::types::ImEventType::MessageCreated,
        thread_id: "oc_denied".to_string(),
        event_id: None,
        message_id: None,
        text: Some("hello".to_string()),
        role_id: Some("ou_role".to_string()),
        account_id: Some("default".to_string()),
        tenant_id: Some("tenant_1".to_string()),
        sender_id: Some("ou_sender".to_string()),
        chat_type: Some("group".to_string()),
    };

    assert_eq!(
        evaluate_openclaw_feishu_gate(&event, &metadata),
        FeishuInboundGateDecision::Reject {
            reason: "group_not_allowed"
        }
    );
}

#[test]
fn generate_feishu_pairing_code_returns_eight_chars() {
    let code = generate_feishu_pairing_code();
    assert_eq!(code.len(), 8);
    assert!(code.chars().all(|ch| ch.is_ascii_alphanumeric()));
}

#[tokio::test]
async fn upsert_feishu_pairing_request_reuses_existing_pending_record() {
    let pool = setup_pairing_pool().await;

    let (first, created_first) =
        upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender", "oc_chat", None)
            .await
            .expect("create first request");
    let (second, created_second) =
        upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender", "oc_chat_new", None)
            .await
            .expect("reuse pending request");

    assert!(created_first);
    assert!(!created_second);
    assert_eq!(first.id, second.id);
    assert_eq!(second.chat_id, "oc_chat_new");
    assert_eq!(first.code, second.code);
}

#[tokio::test]
async fn approve_feishu_pairing_request_persists_allow_from_entry() {
    let pool = setup_pairing_pool().await;

    let (request, _) =
        upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender", "", None)
            .await
            .expect("create request");
    let resolved = resolve_feishu_pairing_request_with_pool(
        &pool,
        &request.id,
        "approved",
        Some("tester".to_string()),
    )
    .await
    .expect("approve request");

    assert_eq!(resolved.status, "approved");
    assert_eq!(resolved.resolved_by_user, "tester");

    let allow_from = list_feishu_pairing_allow_from_with_pool(&pool, "default")
        .await
        .expect("list allow from");
    assert_eq!(allow_from, vec!["ou_sender".to_string()]);
}

#[test]
fn pairing_approval_requests_runtime_restart_only_for_matching_running_account() {
    let running_default = OpenClawPluginFeishuRuntimeStatus {
        running: true,
        account_id: "default".to_string(),
        ..OpenClawPluginFeishuRuntimeStatus::default()
    };
    let stopped_default = OpenClawPluginFeishuRuntimeStatus {
        running: false,
        account_id: "default".to_string(),
        ..OpenClawPluginFeishuRuntimeStatus::default()
    };
    let running_workspace = OpenClawPluginFeishuRuntimeStatus {
        running: true,
        account_id: "workspace".to_string(),
        ..OpenClawPluginFeishuRuntimeStatus::default()
    };

    assert!(
        should_restart_official_feishu_runtime_after_pairing_approval(&running_default, "default")
    );
    assert!(
        !should_restart_official_feishu_runtime_after_pairing_approval(&stopped_default, "default")
    );
    assert!(
        !should_restart_official_feishu_runtime_after_pairing_approval(
            &running_workspace,
            "default"
        )
    );
}

#[tokio::test]
async fn deny_feishu_pairing_request_does_not_persist_allow_from_entry() {
    let pool = setup_pairing_pool().await;

    let (request, _) =
        upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender", "", None)
            .await
            .expect("create request");
    let resolved = resolve_feishu_pairing_request_with_pool(
        &pool,
        &request.id,
        "denied",
        Some("tester".to_string()),
    )
    .await
    .expect("deny request");

    assert_eq!(resolved.status, "denied");

    let allow_from = list_feishu_pairing_allow_from_with_pool(&pool, "default")
        .await
        .expect("list allow from");
    assert!(allow_from.is_empty());
}

#[tokio::test]
async fn list_feishu_pairing_requests_filters_by_status() {
    let pool = setup_pairing_pool().await;

    let (first, _) =
        upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender_a", "", None)
            .await
            .expect("create first request");
    let (_second, _) =
        upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender_b", "", None)
            .await
            .expect("create second request");
    let _ = resolve_feishu_pairing_request_with_pool(
        &pool,
        &first.id,
        "approved",
        Some("tester".to_string()),
    )
    .await
    .expect("approve request");

    let pending = super::list_feishu_pairing_requests_with_pool(&pool, Some("pending".to_string()))
        .await
        .expect("list pending requests");
    let approved =
        super::list_feishu_pairing_requests_with_pool(&pool, Some("approved".to_string()))
            .await
            .expect("list approved requests");

    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].sender_id, "ou_sender_b");
    assert_eq!(approved.len(), 1);
    assert_eq!(approved[0].sender_id, "ou_sender_a");
}

#[tokio::test]
async fn approve_new_pending_request_still_succeeds_when_sender_has_old_approved_record() {
    let pool = setup_pairing_pool().await;

    let (first, _) =
        upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender", "", None)
            .await
            .expect("create first request");
    let _ = resolve_feishu_pairing_request_with_pool(
        &pool,
        &first.id,
        "approved",
        Some("tester".to_string()),
    )
    .await
    .expect("approve first request");

    let (second, _) =
        upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender", "", None)
            .await
            .expect("create second pending request");
    let resolved = resolve_feishu_pairing_request_with_pool(
        &pool,
        &second.id,
        "approved",
        Some("tester-2".to_string()),
    )
    .await
    .expect("approve second request");

    assert_eq!(resolved.status, "approved");

    let approved =
        super::list_feishu_pairing_requests_with_pool(&pool, Some("approved".to_string()))
            .await
            .expect("list approved requests");
    assert_eq!(approved.len(), 2);
}

#[tokio::test]
async fn upsert_feishu_pairing_request_persists_explicit_runtime_code() {
    let pool = setup_pairing_pool().await;

    let (request, created) = upsert_feishu_pairing_request_with_pool(
        &pool,
        "default",
        "ou_sender",
        "",
        Some("dl1m1d25"),
    )
    .await
    .expect("create request with runtime code");

    assert!(created);
    assert_eq!(request.code, "DL1M1D25");
}

#[tokio::test]
async fn upsert_feishu_pairing_request_updates_pending_code_from_runtime_event() {
    let pool = setup_pairing_pool().await;

    let (first, _) =
        upsert_feishu_pairing_request_with_pool(&pool, "default", "ou_sender", "", None)
            .await
            .expect("create initial request");
    let (second, created) = upsert_feishu_pairing_request_with_pool(
        &pool,
        "default",
        "ou_sender",
        "",
        Some("4965d3b0"),
    )
    .await
    .expect("reuse pending request with runtime code");

    assert!(!created);
    assert_eq!(first.id, second.id);
    assert_eq!(second.code, "4965D3B0");
}

#[tokio::test]
async fn build_direct_outbound_target_uses_latest_inbox_message_as_reply_context() {
    let pool = setup_pairing_pool().await;
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS im_inbox_events (
            id TEXT PRIMARY KEY,
            event_id TEXT NOT NULL DEFAULT '',
            thread_id TEXT NOT NULL,
            message_id TEXT NOT NULL DEFAULT '',
            text_preview TEXT NOT NULL DEFAULT '',
            source TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .expect("create im_inbox_events table");

    sqlx::query(
        "INSERT INTO im_inbox_events (id, event_id, thread_id, message_id, text_preview, source, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("inbox-direct-outbound-1")
    .bind("evt-direct-outbound-1")
    .bind("ou_direct_sender_1")
    .bind("om_direct_latest_1")
    .bind("你好")
    .bind("feishu")
    .bind(&now)
    .execute(&pool)
    .await
    .expect("seed direct feishu inbox event");

    let latest = super::lookup_latest_feishu_inbox_message_id_for_thread_with_pool(
        &pool,
        "ou_direct_sender_1",
    )
    .await
    .expect("lookup latest feishu inbox message id");
    assert_eq!(latest.as_deref(), Some("om_direct_latest_1"));

    let target = super::build_feishu_outbound_route_target("ou_direct_sender_1", latest.as_deref());
    assert_eq!(
        target,
        "ou_direct_sender_1#__feishu_reply_to=om_direct_latest_1&__feishu_thread_id=ou_direct_sender_1"
    );
}

#[tokio::test]
async fn lookup_direct_outbound_target_prefers_known_chat_id_mapping() {
    let pool = setup_pairing_pool().await;

    let (_request, _created) = upsert_feishu_pairing_request_with_pool(
        &pool,
        "default",
        "ou_direct_sender_2",
        "oc_direct_chat_2",
        Some("abcd1234"),
    )
    .await
    .expect("seed direct sender chat mapping");

    let mapped =
        super::lookup_feishu_chat_id_for_sender_with_pool(&pool, "default", "ou_direct_sender_2")
            .await
            .expect("lookup direct chat mapping");
    assert_eq!(mapped.as_deref(), Some("oc_direct_chat_2"));
}

#[tokio::test]
async fn remember_direct_outbound_chat_id_backfills_sender_mapping() {
    let pool = setup_pairing_pool().await;

    let (_request, _created) = upsert_feishu_pairing_request_with_pool(
        &pool,
        "default",
        "ou_direct_sender_3",
        "",
        Some("efgh5678"),
    )
    .await
    .expect("seed pending direct sender record");

    super::remember_feishu_chat_id_for_sender_with_pool(
        &pool,
        "default",
        "ou_direct_sender_3",
        "oc_direct_chat_3",
    )
    .await
    .expect("backfill direct sender chat mapping");

    let mapped =
        super::lookup_feishu_chat_id_for_sender_with_pool(&pool, "default", "ou_direct_sender_3")
            .await
            .expect("lookup backfilled direct chat mapping");
    assert_eq!(mapped.as_deref(), Some("oc_direct_chat_3"));
}

#[tokio::test]
async fn send_feishu_reply_plan_sends_all_chunks() {
    let pool = setup_pairing_pool().await;
    let runtime_state = OpenClawPluginFeishuRuntimeState::default();
    let sent_texts = Arc::new(Mutex::new(Vec::<String>::new()));
    let sent_texts_for_hook = sent_texts.clone();

    super::set_feishu_official_runtime_outbound_send_hook_for_tests(Some(Arc::new(
        move |request| {
            sent_texts_for_hook
                .lock()
                .expect("lock sent texts")
                .push(request.text.clone());
            Ok(OpenClawPluginFeishuOutboundDeliveryResult {
                delivered: true,
                channel: "feishu".to_string(),
                account_id: request.account_id.clone(),
                target: request.target.clone(),
                thread_id: request.thread_id.clone(),
                text: request.text.clone(),
                mode: request.mode.clone(),
                message_id: format!("om_{}", request.request_id),
                chat_id: "oc_chat_plan".to_string(),
                sequence: 1,
            })
        },
    )));

    let original = "A".repeat(4000);
    let plan =
        super::build_feishu_reply_plan("reply-plan-1", "session-plan-1", "oc_chat_plan", &original);
    let result = super::execute_feishu_reply_plan_with_pool(
        &pool,
        &runtime_state,
        &plan,
        Some("default".to_string()),
    )
    .await
    .expect("execute reply plan");

    super::set_feishu_official_runtime_outbound_send_hook_for_tests(None);

    assert!(result.deliveries.len() > 1);
    assert_eq!(result.trace.delivered_chunk_count, result.deliveries.len());
    assert_eq!(
        result.trace.final_state,
        Some(crate::commands::openclaw_plugins::im_host_contract::ImReplyDeliveryState::Completed)
    );

    let rebuilt = sent_texts
        .lock()
        .expect("lock sent texts")
        .iter()
        .map(String::as_str)
        .collect::<String>();
    assert_eq!(rebuilt, original);
    let status = crate::commands::openclaw_plugins::current_feishu_runtime_status(&runtime_state);
    assert!(
        status
            .recent_logs
            .iter()
            .any(|entry: &String| entry.contains("[reply_trace]") && entry.contains("state=Completed"))
    );
}

#[tokio::test]
async fn send_feishu_reply_plan_reports_partial_failure_after_first_chunk() {
    let pool = setup_pairing_pool().await;
    let runtime_state = OpenClawPluginFeishuRuntimeState::default();
    let call_count = Arc::new(Mutex::new(0usize));
    let call_count_for_hook = call_count.clone();

    super::set_feishu_official_runtime_outbound_send_hook_for_tests(Some(Arc::new(
        move |request| {
            let mut guard = call_count_for_hook.lock().expect("lock call count");
            let current = *guard;
            *guard += 1;
            if current == 0 {
                Ok(OpenClawPluginFeishuOutboundDeliveryResult {
                    delivered: true,
                    channel: "feishu".to_string(),
                    account_id: request.account_id.clone(),
                    target: request.target.clone(),
                    thread_id: request.thread_id.clone(),
                    text: request.text.clone(),
                    mode: request.mode.clone(),
                    message_id: format!("om_{}", request.request_id),
                    chat_id: "oc_chat_partial".to_string(),
                    sequence: 1,
                })
            } else {
                Err("simulated chunk delivery failure".to_string())
            }
        },
    )));

    let plan = super::build_feishu_reply_plan(
        "reply-plan-2",
        "session-plan-2",
        "oc_chat_partial",
        &"B".repeat(4000),
    );
    let error = super::execute_feishu_reply_plan_with_pool(
        &pool,
        &runtime_state,
        &plan,
        Some("default".to_string()),
    )
    .await
    .expect_err("second chunk should fail");

    super::set_feishu_official_runtime_outbound_send_hook_for_tests(None);

    assert!(error.contains("simulated chunk delivery failure"));
    assert!(
        error.contains("\"finalState\":\"FailedPartial\"")
            || error.contains("\"final_state\":\"FailedPartial\"")
    );
    let status = crate::commands::openclaw_plugins::current_feishu_runtime_status(&runtime_state);
    assert!(
        status
            .recent_logs
            .iter()
            .any(|entry: &String| entry.contains("[reply_trace]") && entry.contains("state=FailedPartial"))
    );
}
