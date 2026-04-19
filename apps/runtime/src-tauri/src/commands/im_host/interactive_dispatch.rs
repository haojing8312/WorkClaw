use super::{
    emit_registered_lifecycle_phase_for_session_with_pool,
    load_approval_resolution_notification_with_pool, lookup_channel_source_for_session_with_pool,
    lookup_channel_thread_for_session_with_pool,
    stop_registered_processing_for_session_with_pool,
};
use crate::commands::im_host::interactive_messages::ApprovalResolutionNotificationRow;
use crate::approval_bus::PendingApprovalRecord;
use crate::commands::openclaw_plugins::im_host_contract::ImReplyLifecyclePhase;
use sqlx::SqlitePool;

pub(crate) async fn prepare_channel_interactive_session_thread_with_pool(
    pool: &SqlitePool,
    source: &str,
    session_id: &str,
    final_state: Option<&str>,
    phase: ImReplyLifecyclePhase,
) -> Result<Option<String>, String> {
    let Some(thread_id) = lookup_channel_thread_for_session_with_pool(pool, source, session_id).await? else {
        return Ok(None);
    };

    match source.trim() {
        "feishu" => {
            let _ = crate::commands::feishu_gateway::maybe_stop_registered_feishu_processing_for_session_with_pool(
                pool,
                session_id,
                None,
                final_state,
                None,
            )
            .await;
            let _ = crate::commands::feishu_gateway::maybe_emit_registered_feishu_lifecycle_phase_for_session_with_pool(
                pool,
                session_id,
                None,
                phase,
                None,
            )
            .await;
        }
        "wecom" => {
            let _ = crate::commands::wecom_gateway::maybe_stop_registered_wecom_processing_for_session_with_pool(
                pool,
                session_id,
                None,
                final_state,
                None,
            )
            .await;
            let _ = crate::commands::wecom_gateway::maybe_emit_registered_wecom_lifecycle_phase_for_session_with_pool(
                pool,
                session_id,
                None,
                phase,
                None,
            )
            .await;
        }
        _ => {
            let _ = stop_registered_processing_for_session_with_pool(
                pool,
                source,
                session_id,
                None,
                final_state,
                None,
                |_dispatch| Ok(()),
            )
            .await;
            let _ = emit_registered_lifecycle_phase_for_session_with_pool(
                pool,
                source,
                session_id,
                None,
                phase,
                None,
                |_dispatch| Ok(()),
            )
            .await;
        }
    }

    Ok(Some(thread_id))
}

pub(crate) async fn prepare_channel_interactive_approval_notice_with_pool(
    pool: &SqlitePool,
    approval_id: &str,
) -> Result<Option<ApprovalResolutionNotificationRow>, String> {
    load_approval_resolution_notification_with_pool(pool, approval_id).await
}

pub(crate) async fn maybe_notify_registered_ask_user_requested_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    question: &str,
    options: &[String],
    sidecar_base_url: Option<String>,
) -> Result<bool, String> {
    match lookup_channel_source_for_session_with_pool(pool, session_id)
        .await?
        .as_deref()
    {
        Some("feishu") => {
            crate::commands::feishu_gateway::notify_feishu_ask_user_requested_with_pool(
                pool,
                session_id,
                question,
                options,
                sidecar_base_url,
            )
            .await?;
            Ok(true)
        }
        Some("wecom") => {
            crate::commands::wecom_gateway::notify_wecom_ask_user_requested_with_pool(
                pool,
                session_id,
                question,
                options,
                sidecar_base_url,
            )
            .await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub(crate) async fn maybe_notify_registered_approval_requested_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    record: &PendingApprovalRecord,
    sidecar_base_url: Option<String>,
) -> Result<bool, String> {
    match lookup_channel_source_for_session_with_pool(pool, session_id)
        .await?
        .as_deref()
    {
        Some("feishu") => {
            crate::commands::feishu_gateway::notify_feishu_approval_requested_with_pool(
                pool,
                session_id,
                record,
                sidecar_base_url,
            )
            .await?;
            Ok(true)
        }
        Some("wecom") => {
            crate::commands::wecom_gateway::notify_wecom_approval_requested_with_pool(
                pool,
                session_id,
                record,
                sidecar_base_url,
            )
            .await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub(crate) async fn maybe_notify_registered_approval_resolved_with_pool(
    pool: &SqlitePool,
    approval_id: &str,
    sidecar_base_url: Option<String>,
) -> Result<bool, String> {
    let Some(row) = load_approval_resolution_notification_with_pool(pool, approval_id).await? else {
        return Ok(false);
    };

    match lookup_channel_source_for_session_with_pool(pool, &row.session_id)
        .await?
        .as_deref()
    {
        Some("feishu") => {
            crate::commands::feishu_gateway::notify_feishu_approval_resolved_with_pool(
                pool,
                approval_id,
                sidecar_base_url,
            )
            .await?;
            Ok(true)
        }
        Some("wecom") => {
            crate::commands::wecom_gateway::notify_wecom_approval_resolved_with_pool(
                pool,
                approval_id,
                sidecar_base_url,
            )
            .await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        maybe_notify_registered_approval_requested_with_pool,
        maybe_notify_registered_approval_resolved_with_pool,
        maybe_notify_registered_ask_user_requested_with_pool,
    };
    use crate::approval_bus::PendingApprovalRecord;
    use crate::commands::feishu_gateway::{
        clear_feishu_runtime_state_for_outbound, remember_feishu_runtime_state_for_outbound,
        set_feishu_official_runtime_outbound_send_hook_for_tests,
    };
    use crate::commands::openclaw_plugins::{
        OpenClawPluginFeishuOutboundDeliveryResult, OpenClawPluginFeishuRuntimeState,
        OpenClawPluginFeishuLifecycleEventRequest, OpenClawPluginFeishuProcessingStopRequest,
    };
    use crate::commands::openclaw_plugins::{
        set_feishu_runtime_lifecycle_event_hook_for_tests,
        set_feishu_runtime_processing_stop_hook_for_tests,
    };
    use crate::commands::wecom_gateway::{
        set_wecom_lifecycle_event_hook_for_tests, set_wecom_outbound_send_hook_for_tests,
        set_wecom_processing_stop_hook_for_tests,
    };
    use serde_json::json;
    use sqlx::SqlitePool;
    use std::sync::{Arc, Mutex};

    async fn setup_interactive_dispatch_pool() -> SqlitePool {
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
        .expect("create im_thread_sessions");

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
        .expect("create im_inbox_events");

        sqlx::query(
            "CREATE TABLE approvals (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                summary TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT '',
                decision TEXT NOT NULL DEFAULT '',
                resolved_by_surface TEXT NOT NULL DEFAULT '',
                resolved_by_user TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .expect("create approvals");

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
        .bind(format!("evt-{session_id}"))
        .bind(format!("evt-{session_id}"))
        .bind(thread_id)
        .bind(message_id)
        .bind(source)
        .execute(pool)
        .await
        .expect("seed im_inbox_events");
    }

    fn install_feishu_send_hook() -> Arc<Mutex<Vec<String>>> {
        let runtime_state = OpenClawPluginFeishuRuntimeState::default();
        remember_feishu_runtime_state_for_outbound(&runtime_state);

        let sent_texts = Arc::new(Mutex::new(Vec::<String>::new()));
        let sent_texts_for_hook = sent_texts.clone();
        set_feishu_official_runtime_outbound_send_hook_for_tests(Some(Arc::new(
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
                    chat_id: request
                        .thread_id
                        .clone()
                        .unwrap_or_else(|| "oc_chat_test".to_string()),
                    sequence: 1,
                })
            },
        )));

        sent_texts
    }

    fn cleanup_feishu_send_hook() {
        set_feishu_official_runtime_outbound_send_hook_for_tests(None);
        set_feishu_runtime_processing_stop_hook_for_tests(None);
        set_feishu_runtime_lifecycle_event_hook_for_tests(None);
        clear_feishu_runtime_state_for_outbound();
    }

    fn install_feishu_interactive_lifecycle_hooks() -> Arc<Mutex<Vec<String>>> {
        let recorded = Arc::new(Mutex::new(Vec::<String>::new()));

        let processing_events = recorded.clone();
        set_feishu_runtime_processing_stop_hook_for_tests(Some(Arc::new(
            move |request: &OpenClawPluginFeishuProcessingStopRequest| {
                processing_events
                    .lock()
                    .expect("lock feishu lifecycle records")
                    .push(format!(
                        "processing_stop:{}:{}",
                        request.message_id,
                        request.final_state.as_deref().unwrap_or("")
                    ));
                Ok(())
            },
        )));

        let lifecycle_events = recorded.clone();
        set_feishu_runtime_lifecycle_event_hook_for_tests(Some(Arc::new(
            move |request: &OpenClawPluginFeishuLifecycleEventRequest| {
                lifecycle_events
                    .lock()
                    .expect("lock feishu lifecycle records")
                    .push(format!(
                        "lifecycle:{}:{}",
                        request
                            .message_id
                            .as_deref()
                            .unwrap_or(""),
                        serde_json::to_string(&request.phase).unwrap_or_else(|_| "\"unknown\"".to_string())
                    ));
                Ok(())
            },
        )));

        recorded
    }

    fn install_wecom_send_hook() -> Arc<Mutex<Vec<String>>> {
        let sent_texts = Arc::new(Mutex::new(Vec::<String>::new()));
        let sent_texts_for_hook = sent_texts.clone();
        set_wecom_outbound_send_hook_for_tests(Some(Arc::new(move |_thread_id, text| {
            sent_texts_for_hook
                .lock()
                .expect("lock wecom sent texts")
                .push(text.to_string());
            Ok(serde_json::json!({
                "message_id": "wm_test_1",
                "conversation_id": "wecom_test_conversation",
            }))
        })));
        sent_texts
    }

    fn cleanup_wecom_send_hook() {
        set_wecom_outbound_send_hook_for_tests(None);
        set_wecom_processing_stop_hook_for_tests(None);
        set_wecom_lifecycle_event_hook_for_tests(None);
    }

    fn install_wecom_interactive_lifecycle_hooks() -> Arc<Mutex<Vec<String>>> {
        let recorded = Arc::new(Mutex::new(Vec::<String>::new()));

        let processing_events = recorded.clone();
        set_wecom_processing_stop_hook_for_tests(Some(Arc::new(move |request| {
            processing_events
                .lock()
                .expect("lock wecom lifecycle records")
                .push(format!(
                    "processing_stop:{}:{}",
                    request.message_id,
                    request.final_state.as_deref().unwrap_or("")
                ));
            Ok(())
        })));

        let lifecycle_events = recorded.clone();
        set_wecom_lifecycle_event_hook_for_tests(Some(Arc::new(move |request| {
            lifecycle_events
                .lock()
                .expect("lock wecom lifecycle records")
                .push(format!(
                    "lifecycle:{}:{}",
                    request.message_id.as_deref().unwrap_or(""),
                    serde_json::to_string(&request.phase).unwrap_or_else(|_| "\"unknown\"".to_string())
                ));
            Ok(())
        })));

        recorded
    }

    #[tokio::test]
    async fn maybe_notify_registered_ask_user_routes_feishu_session_via_unified_host() {
        let pool = setup_interactive_dispatch_pool().await;
        seed_session_channel(
            &pool,
            "session-feishu-ask-user",
            "oc_chat_ask_user",
            "feishu",
            "om_parent_ask_user",
        )
        .await;
        let sent_texts = install_feishu_send_hook();
        let lifecycle_records = install_feishu_interactive_lifecycle_hooks();

        let result = maybe_notify_registered_ask_user_requested_with_pool(
            &pool,
            "session-feishu-ask-user",
            "请选择方案",
            &["方案A".to_string(), "方案B".to_string()],
            None,
        )
        .await
        .expect("notify ask_user");

        cleanup_feishu_send_hook();

        assert!(result);
        let sent = sent_texts.lock().expect("lock sent texts");
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains("请选择方案"));
        assert!(sent[0].contains("可选项：方案A / 方案B"));
        let lifecycle = lifecycle_records.lock().expect("lock lifecycle records");
        assert_eq!(
            lifecycle.as_slice(),
            [
                "processing_stop:om_parent_ask_user:ask_user",
                "lifecycle:om_parent_ask_user:\"ask_user_requested\"",
            ]
        );
    }

    #[tokio::test]
    async fn maybe_notify_registered_approval_requested_routes_feishu_session_via_unified_host() {
        let pool = setup_interactive_dispatch_pool().await;
        seed_session_channel(
            &pool,
            "session-feishu-approval-request",
            "oc_chat_approval_request",
            "feishu",
            "om_parent_approval_request",
        )
        .await;
        let sent_texts = install_feishu_send_hook();
        let lifecycle_records = install_feishu_interactive_lifecycle_hooks();
        let record = PendingApprovalRecord {
            approval_id: "approval-1".to_string(),
            session_id: "session-feishu-approval-request".to_string(),
            run_id: None,
            call_id: "call-1".to_string(),
            tool_name: "shell".to_string(),
            input: json!({"command": "rm -rf /tmp/demo"}),
            summary: "执行高风险命令".to_string(),
            impact: Some("可能修改工作目录内容".to_string()),
            irreversible: true,
            status: "pending".to_string(),
        };

        let result = maybe_notify_registered_approval_requested_with_pool(
            &pool,
            "session-feishu-approval-request",
            &record,
            None,
        )
        .await
        .expect("notify approval requested");

        cleanup_feishu_send_hook();

        assert!(result);
        let sent = sent_texts.lock().expect("lock sent texts");
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains("待审批 #approval-1"));
        assert!(sent[0].contains("/approve approval-1 allow_once | allow_always | deny"));
        let lifecycle = lifecycle_records.lock().expect("lock lifecycle records");
        assert_eq!(
            lifecycle.as_slice(),
            [
                "processing_stop:om_parent_approval_request:waiting_approval",
                "lifecycle:om_parent_approval_request:\"approval_requested\"",
            ]
        );
    }

    #[tokio::test]
    async fn maybe_notify_registered_ask_user_routes_wecom_session_via_unified_host() {
        let pool = setup_interactive_dispatch_pool().await;
        seed_session_channel(
            &pool,
            "session-wecom-ask-user",
            "wecom_chat_ask_user",
            "wecom",
            "wm_parent_ask_user",
        )
        .await;
        let sent_texts = install_wecom_send_hook();
        let lifecycle_records = install_wecom_interactive_lifecycle_hooks();

        let result = maybe_notify_registered_ask_user_requested_with_pool(
            &pool,
            "session-wecom-ask-user",
            "请确认企微方案",
            &["方案一".to_string(), "方案二".to_string()],
            None,
        )
        .await
        .expect("notify wecom ask_user");

        cleanup_wecom_send_hook();

        assert!(result);
        let sent = sent_texts.lock().expect("lock wecom sent texts");
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains("请确认企微方案"));
        assert!(sent[0].contains("可选项：方案一 / 方案二"));
        let lifecycle = lifecycle_records.lock().expect("lock wecom lifecycle records");
        assert_eq!(
            lifecycle.as_slice(),
            [
                "processing_stop:wm_parent_ask_user:ask_user",
                "lifecycle:wm_parent_ask_user:\"ask_user_requested\"",
            ]
        );
    }

    #[tokio::test]
    async fn maybe_notify_registered_approval_requested_routes_wecom_session_via_unified_host() {
        let pool = setup_interactive_dispatch_pool().await;
        seed_session_channel(
            &pool,
            "session-wecom-approval-request",
            "wecom_chat_approval_request",
            "wecom",
            "wm_parent_approval_request",
        )
        .await;
        let sent_texts = install_wecom_send_hook();
        let lifecycle_records = install_wecom_interactive_lifecycle_hooks();
        let record = PendingApprovalRecord {
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

        let result = maybe_notify_registered_approval_requested_with_pool(
            &pool,
            "session-wecom-approval-request",
            &record,
            None,
        )
        .await
        .expect("notify wecom approval requested");

        cleanup_wecom_send_hook();

        assert!(result);
        let sent = sent_texts.lock().expect("lock wecom sent texts");
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains("待审批 #approval-wecom-1"));
        assert!(sent[0].contains("/approve approval-wecom-1 allow_once | allow_always | deny"));
        let lifecycle = lifecycle_records.lock().expect("lock wecom lifecycle records");
        assert_eq!(
            lifecycle.as_slice(),
            [
                "processing_stop:wm_parent_approval_request:waiting_approval",
                "lifecycle:wm_parent_approval_request:\"approval_requested\"",
            ]
        );
    }

    #[tokio::test]
    async fn maybe_notify_registered_approval_resolved_routes_feishu_session_via_unified_host() {
        let pool = setup_interactive_dispatch_pool().await;
        seed_session_channel(
            &pool,
            "session-feishu-approval-resolved",
            "oc_chat_approval_resolved",
            "feishu",
            "om_parent_approval_resolved",
        )
        .await;
        sqlx::query(
            "INSERT INTO approvals (id, session_id, summary, status, decision, resolved_by_surface, resolved_by_user)
             VALUES ('approval-2', 'session-feishu-approval-resolved', '执行高风险命令', 'approved', 'allow_once', 'im', 'alice')",
        )
        .execute(&pool)
        .await
        .expect("seed approval row");
        let sent_texts = install_feishu_send_hook();

        let result =
            maybe_notify_registered_approval_resolved_with_pool(&pool, "approval-2", None)
                .await
                .expect("notify approval resolved");

        cleanup_feishu_send_hook();

        assert!(result);
        let sent = sent_texts.lock().expect("lock sent texts");
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains("审批 approval-2 已被处理"));
        assert!(sent[0].contains("处理人：alice"));
    }

    #[tokio::test]
    async fn maybe_notify_registered_approval_resolved_routes_wecom_session_via_unified_host() {
        let pool = setup_interactive_dispatch_pool().await;
        seed_session_channel(
            &pool,
            "session-wecom-approval-resolved",
            "wecom_chat_approval_resolved",
            "wecom",
            "wm_parent_approval_resolved",
        )
        .await;
        sqlx::query(
            "INSERT INTO approvals (id, session_id, summary, status, decision, resolved_by_surface, resolved_by_user)
             VALUES ('approval-wecom-2', 'session-wecom-approval-resolved', '执行企微高风险命令', 'approved', 'allow_once', 'im', 'bob')",
        )
        .execute(&pool)
        .await
        .expect("seed wecom approval row");
        let sent_texts = install_wecom_send_hook();

        let result = maybe_notify_registered_approval_resolved_with_pool(
            &pool,
            "approval-wecom-2",
            None,
        )
        .await
        .expect("notify wecom approval resolved");

        cleanup_wecom_send_hook();

        assert!(result);
        let sent = sent_texts.lock().expect("lock wecom sent texts");
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains("审批 approval-wecom-2 已被处理"));
        assert!(sent[0].contains("处理人：bob"));
    }

    #[tokio::test]
    async fn maybe_notify_registered_ask_user_returns_false_for_unknown_channel() {
        let pool = setup_interactive_dispatch_pool().await;
        seed_session_channel(
            &pool,
            "session-unknown",
            "oc_chat_unknown",
            "dingtalk",
            "om_parent_unknown",
        )
        .await;

        let result = maybe_notify_registered_ask_user_requested_with_pool(
            &pool,
            "session-unknown",
            "补充信息",
            &[],
            None,
        )
        .await
        .expect("notify unknown channel");

        assert!(!result);
    }
}
