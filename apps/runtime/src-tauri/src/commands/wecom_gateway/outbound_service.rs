use super::send_wecom_text_message_with_pool;
use crate::commands::im_host::{
    emit_registered_lifecycle_phase_for_session_with_pool, execute_reply_plan_with_transport,
    stop_registered_processing_for_session_with_pool, ImReplyDeliveryPlan, ImReplyLifecyclePhase,
    ImReplyPlanTransport,
};
use async_trait::async_trait;
use sqlx::SqlitePool;

pub(crate) async fn maybe_emit_registered_wecom_lifecycle_phase_for_session_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    logical_reply_id: Option<&str>,
    phase: ImReplyLifecyclePhase,
    account_id: Option<&str>,
) -> Result<bool, String> {
    emit_registered_lifecycle_phase_for_session_with_pool(
        pool,
        "wecom",
        session_id,
        logical_reply_id,
        phase,
        account_id,
        |dispatch| {
            let hook = super::wecom_lifecycle_event_hook_slot()
                .lock()
                .ok()
                .and_then(|guard| guard.clone());
            if let Some(hook) = hook {
                return hook(&dispatch);
            }
            Ok(())
        },
    )
    .await
}

pub(crate) async fn maybe_stop_registered_wecom_processing_for_session_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    logical_reply_id: Option<&str>,
    final_state: Option<&str>,
    account_id: Option<&str>,
) -> Result<bool, String> {
    stop_registered_processing_for_session_with_pool(
        pool,
        "wecom",
        session_id,
        logical_reply_id,
        final_state,
        account_id,
        |dispatch| {
            let hook = super::wecom_processing_stop_hook_slot()
                .lock()
                .ok()
                .and_then(|guard| guard.clone());
            if let Some(hook) = hook {
                return hook(&dispatch);
            }
            Ok(())
        },
    )
    .await
}

pub(crate) async fn execute_registered_wecom_reply_plan_with_pool(
    pool: &SqlitePool,
    plan: &ImReplyDeliveryPlan,
    sidecar_base_url: Option<String>,
) -> Result<(), String> {
    let transport = WecomReplyPlanTransport {
        pool,
        sidecar_base_url,
    };
    let _ = execute_reply_plan_with_transport(&transport, plan).await?;
    Ok(())
}

struct WecomReplyPlanTransport<'a> {
    pool: &'a SqlitePool,
    sidecar_base_url: Option<String>,
}

#[async_trait]
impl ImReplyPlanTransport for WecomReplyPlanTransport<'_> {
    type Delivery = String;

    async fn on_processing_started(&self, plan: &ImReplyDeliveryPlan) -> Result<(), String> {
        let _ = maybe_emit_registered_wecom_lifecycle_phase_for_session_with_pool(
            self.pool,
            &plan.session_id,
            Some(&plan.logical_reply_id),
            ImReplyLifecyclePhase::ProcessingStarted,
            None,
        )
        .await;
        Ok(())
    }

    async fn send_chunk(
        &self,
        plan: &ImReplyDeliveryPlan,
        _chunk_index: usize,
        text: &str,
    ) -> Result<Self::Delivery, String> {
        send_wecom_text_message_with_pool(
            self.pool,
            plan.thread_id.clone(),
            text.to_string(),
            None,
            self.sidecar_base_url.clone(),
        )
        .await
    }

    async fn on_processing_finished(
        &self,
        plan: &ImReplyDeliveryPlan,
        final_state: &str,
    ) -> Result<(), String> {
        let _ = maybe_stop_registered_wecom_processing_for_session_with_pool(
            self.pool,
            &plan.session_id,
            Some(&plan.logical_reply_id),
            Some(final_state),
            None,
        )
        .await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::execute_registered_wecom_reply_plan_with_pool;
    use crate::commands::im_host::{plan_text_chunks, ImReplyDeliveryPlan};
    use crate::commands::wecom_gateway::set_wecom_outbound_send_hook_for_tests;
    use sqlx::SqlitePool;
    use std::sync::{Arc, Mutex};

    async fn setup_wecom_reply_plan_pool() -> SqlitePool {
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
            "INSERT INTO im_thread_sessions (thread_id, employee_id, session_id, route_session_key, created_at, updated_at)
             VALUES ('wecom_reply_thread', '', 'wecom-session-1', '', '2026-04-19T00:00:00Z', '2026-04-19T00:00:01Z')",
        )
        .execute(&pool)
        .await
        .expect("seed im_thread_sessions");

        sqlx::query(
            "INSERT INTO im_inbox_events (id, event_id, thread_id, message_id, text_preview, source, created_at)
             VALUES ('evt-wecom-1', 'evt-wecom-1', 'wecom_reply_thread', 'wm_parent_1', 'hello', 'wecom', '2026-04-19T00:00:02Z')",
        )
        .execute(&pool)
        .await
        .expect("seed im_inbox_events");

        pool
    }

    #[tokio::test]
    async fn execute_registered_wecom_reply_plan_sends_all_chunks() {
        let pool = setup_wecom_reply_plan_pool().await;
        let sent_texts = Arc::new(Mutex::new(Vec::<String>::new()));
        let sent_texts_for_hook = sent_texts.clone();
        set_wecom_outbound_send_hook_for_tests(Some(Arc::new(move |_thread_id, text| {
            sent_texts_for_hook
                .lock()
                .expect("lock wecom sent texts")
                .push(text.to_string());
            Ok(serde_json::json!({
                "message_id": "wm_chunk_ok",
            }))
        })));

        let original = "W".repeat(4000);
        let plan = ImReplyDeliveryPlan {
            logical_reply_id: "wecom-reply-plan-1".to_string(),
            session_id: "wecom-session-1".to_string(),
            channel: "wecom".to_string(),
            thread_id: "wecom_reply_thread".to_string(),
            chunks: plan_text_chunks(&original, 1800),
        };

        execute_registered_wecom_reply_plan_with_pool(&pool, &plan, None)
            .await
            .expect("execute wecom reply plan");

        set_wecom_outbound_send_hook_for_tests(None);

        let rebuilt = sent_texts
            .lock()
            .expect("lock wecom sent texts")
            .iter()
            .map(String::as_str)
            .collect::<String>();
        assert_eq!(rebuilt, original);
    }

    #[tokio::test]
    async fn execute_registered_wecom_reply_plan_reports_partial_failure_after_first_chunk() {
        let pool = setup_wecom_reply_plan_pool().await;
        let call_count = Arc::new(Mutex::new(0usize));
        let call_count_for_hook = call_count.clone();
        set_wecom_outbound_send_hook_for_tests(Some(Arc::new(move |_thread_id, _text| {
            let mut guard = call_count_for_hook.lock().expect("lock call count");
            let current = *guard;
            *guard += 1;
            if current == 0 {
                Ok(serde_json::json!({
                    "message_id": "wm_first_ok",
                }))
            } else {
                Err("simulated wecom chunk delivery failure".to_string())
            }
        })));

        let plan = ImReplyDeliveryPlan {
            logical_reply_id: "wecom-reply-plan-2".to_string(),
            session_id: "wecom-session-1".to_string(),
            channel: "wecom".to_string(),
            thread_id: "wecom_reply_thread".to_string(),
            chunks: plan_text_chunks(&"Z".repeat(4000), 1800),
        };

        let error = execute_registered_wecom_reply_plan_with_pool(&pool, &plan, None)
            .await
            .expect_err("second wecom chunk should fail");

        set_wecom_outbound_send_hook_for_tests(None);

        assert!(error.contains("simulated wecom chunk delivery failure"));
        assert!(
            error.contains("\"finalState\":\"FailedPartial\"")
                || error.contains("\"final_state\":\"FailedPartial\"")
        );
    }
}
