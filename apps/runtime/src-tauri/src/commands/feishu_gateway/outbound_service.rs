use super::build_feishu_reply_plan;
use crate::commands::im_host::{
    build_direct_reply_route_target, emit_registered_lifecycle_phase_for_session_with_pool,
    execute_reply_plan_with_transport,
    lookup_channel_thread_for_session_with_pool,
    lookup_latest_inbox_message_id_for_thread_with_pool,
    stop_registered_processing_for_session_with_pool, ImDirectRouteTargetOptions,
    ImReplyDeliveryPlan, ImReplyDeliveryState, ImReplyLifecyclePhase, ImReplyPlanTransport,
    ReplyDeliveryTrace,
};
use crate::commands::openclaw_plugins::{
    classify_feishu_runtime_outbound_failure, infer_feishu_runtime_outbound_failure_kind,
    record_feishu_runtime_reply_trace, record_feishu_runtime_reply_trace_error,
    send_openclaw_plugin_feishu_runtime_lifecycle_event_in_state,
    send_openclaw_plugin_feishu_runtime_outbound_message_in_state,
    send_openclaw_plugin_feishu_runtime_processing_stop_in_state,
    OpenClawPluginFeishuLifecycleEventRequest, OpenClawPluginFeishuOutboundDeliveryResult,
    OpenClawPluginFeishuOutboundSendRequest, OpenClawPluginFeishuOutboundSendResult,
    OpenClawPluginFeishuProcessingStopRequest, OpenClawPluginFeishuRuntimeState,
};
use async_trait::async_trait;
use sqlx::SqlitePool;
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuReplyPlanExecutionResult {
    pub trace: ReplyDeliveryTrace,
    pub deliveries: Vec<OpenClawPluginFeishuOutboundSendResult>,
}

pub fn build_feishu_outbound_route_target(
    thread_id: &str,
    reply_to_message_id: Option<&str>,
) -> String {
    build_direct_reply_route_target(
        thread_id,
        reply_to_message_id,
        ImDirectRouteTargetOptions {
            direct_sender_prefix: "ou_",
            reply_to_param_key: "__feishu_reply_to",
            thread_id_param_key: "__feishu_thread_id",
        },
    )
}

fn feishu_runtime_outbound_state_slot() -> &'static Mutex<Option<OpenClawPluginFeishuRuntimeState>>
{
    static SLOT: OnceLock<Mutex<Option<OpenClawPluginFeishuRuntimeState>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

pub type FeishuOfficialRuntimeOutboundSendHook = dyn Fn(
        &OpenClawPluginFeishuOutboundSendRequest,
    ) -> Result<OpenClawPluginFeishuOutboundDeliveryResult, String>
    + Send
    + Sync;

fn feishu_official_runtime_outbound_send_hook_slot(
) -> &'static Mutex<Option<Arc<FeishuOfficialRuntimeOutboundSendHook>>> {
    static SLOT: OnceLock<Mutex<Option<Arc<FeishuOfficialRuntimeOutboundSendHook>>>> =
        OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

pub fn remember_feishu_runtime_state_for_outbound(
    runtime_state: &OpenClawPluginFeishuRuntimeState,
) {
    if let Ok(mut guard) = feishu_runtime_outbound_state_slot().lock() {
        *guard = Some(runtime_state.clone());
    }
}

pub fn clear_feishu_runtime_state_for_outbound() {
    if let Ok(mut guard) = feishu_runtime_outbound_state_slot().lock() {
        *guard = None;
    }
}

#[doc(hidden)]
pub fn set_feishu_official_runtime_outbound_send_hook_for_tests(
    hook: Option<Arc<FeishuOfficialRuntimeOutboundSendHook>>,
) {
    if let Ok(mut guard) = feishu_official_runtime_outbound_send_hook_slot().lock() {
        *guard = hook;
    }
}

fn resolve_registered_feishu_runtime_state_for_outbound(
) -> Result<OpenClawPluginFeishuRuntimeState, String> {
    feishu_runtime_outbound_state_slot()
        .lock()
        .map_err(|_| "failed to lock feishu runtime registration".to_string())?
        .clone()
        .ok_or_else(|| "official feishu runtime is not registered for outbound sends".to_string())
}

pub(crate) async fn execute_registered_feishu_reply_plan_with_pool(
    pool: &SqlitePool,
    plan: &ImReplyDeliveryPlan,
    account_id: Option<String>,
) -> Result<FeishuReplyPlanExecutionResult, String> {
    let runtime_state = resolve_registered_feishu_runtime_state_for_outbound()?;
    execute_feishu_reply_plan_with_pool(pool, &runtime_state, plan, account_id).await
}

pub(crate) async fn maybe_stop_registered_feishu_processing_for_session_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    logical_reply_id: Option<&str>,
    final_state: Option<&str>,
    account_id: Option<&str>,
) -> Result<bool, String> {
    let runtime_state = resolve_registered_feishu_runtime_state_for_outbound()?;
    stop_registered_processing_for_session_with_pool(
        pool,
        "feishu",
        session_id,
        logical_reply_id,
        final_state,
        account_id,
        |dispatch| {
            let request = OpenClawPluginFeishuProcessingStopRequest {
                request_id: dispatch.request_id,
                account_id: dispatch.account_id,
                message_id: dispatch.message_id,
                logical_reply_id: dispatch.logical_reply_id,
                final_state: dispatch.final_state,
            };
            send_openclaw_plugin_feishu_runtime_processing_stop_in_state(&runtime_state, request)
        },
    )
    .await
}

pub(crate) async fn maybe_emit_registered_feishu_lifecycle_phase_for_session_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    logical_reply_id: Option<&str>,
    phase: ImReplyLifecyclePhase,
    account_id: Option<&str>,
) -> Result<bool, String> {
    let runtime_state = resolve_registered_feishu_runtime_state_for_outbound()?;
    emit_registered_lifecycle_phase_for_session_with_pool(
        pool,
        "feishu",
        session_id,
        logical_reply_id,
        phase,
        account_id,
        |dispatch| {
            let request = OpenClawPluginFeishuLifecycleEventRequest {
                request_id: dispatch.request_id,
                account_id: dispatch.account_id,
                phase: dispatch.phase,
                logical_reply_id: dispatch.logical_reply_id,
                thread_id: Some(dispatch.thread_id),
                message_id: dispatch.message_id,
            };
            send_openclaw_plugin_feishu_runtime_lifecycle_event_in_state(&runtime_state, request)
        },
    )
    .await
}

pub(crate) async fn lookup_feishu_thread_for_session_with_pool(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<String>, String> {
    lookup_channel_thread_for_session_with_pool(pool, "feishu", session_id).await
}

pub(crate) async fn lookup_latest_feishu_inbox_message_id_for_thread_with_pool(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Option<String>, String> {
    lookup_latest_inbox_message_id_for_thread_with_pool(pool, "feishu", thread_id).await
}

pub(crate) async fn lookup_feishu_chat_id_for_sender_with_pool(
    pool: &SqlitePool,
    account_id: &str,
    sender_id: &str,
) -> Result<Option<String>, String> {
    let normalized_account_id = account_id.trim();
    let normalized_sender_id = sender_id.trim();
    if normalized_account_id.is_empty() || normalized_sender_id.is_empty() {
        return Ok(None);
    }

    let row = sqlx::query_as::<_, (String,)>(
        "SELECT chat_id
         FROM feishu_pairing_requests
         WHERE channel = 'feishu'
           AND account_id = ?
           AND sender_id = ?
           AND chat_id <> ''
         ORDER BY updated_at DESC, created_at DESC
         LIMIT 1",
    )
    .bind(normalized_account_id)
    .bind(normalized_sender_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("查询飞书发送者 chat_id 失败: {e}"))?;

    Ok(row.map(|(chat_id,)| chat_id))
}

pub(crate) async fn remember_feishu_chat_id_for_sender_with_pool(
    pool: &SqlitePool,
    account_id: &str,
    sender_id: &str,
    chat_id: &str,
) -> Result<(), String> {
    let normalized_account_id = account_id.trim();
    let normalized_sender_id = sender_id.trim();
    let normalized_chat_id = chat_id.trim();
    if normalized_account_id.is_empty()
        || normalized_sender_id.is_empty()
        || normalized_chat_id.is_empty()
        || !normalized_chat_id.starts_with("oc_")
    {
        return Ok(());
    }

    sqlx::query(
        "UPDATE feishu_pairing_requests
         SET chat_id = ?, updated_at = ?
         WHERE channel = 'feishu'
           AND account_id = ?
           AND sender_id = ?",
    )
    .bind(normalized_chat_id)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(normalized_account_id)
    .bind(normalized_sender_id)
    .execute(pool)
    .await
    .map_err(|e| format!("回填飞书 chat_id 失败: {e}"))?;

    Ok(())
}

pub async fn send_feishu_text_message_with_pool(
    pool: &SqlitePool,
    chat_id: &str,
    text: &str,
    _sidecar_base_url: Option<String>,
) -> Result<String, String> {
    let runtime_state = resolve_registered_feishu_runtime_state_for_outbound()?;
    send_feishu_text_message_via_official_runtime_with_pool(
        pool,
        &runtime_state,
        chat_id,
        text,
        None,
    )
    .await
}

async fn resolve_feishu_outbound_target_with_pool(
    pool: &SqlitePool,
    resolved_account_id: &str,
    chat_id: &str,
) -> Result<(String, String), String> {
    let normalized_thread_id = chat_id.trim().to_string();
    let outbound_target = if normalized_thread_id.starts_with("ou_") {
        if let Some(mapped_chat_id) = lookup_feishu_chat_id_for_sender_with_pool(
            pool,
            resolved_account_id,
            &normalized_thread_id,
        )
        .await?
        {
            mapped_chat_id
        } else {
            let reply_to_message_id =
                lookup_latest_feishu_inbox_message_id_for_thread_with_pool(pool, chat_id).await?;
            build_feishu_outbound_route_target(chat_id, reply_to_message_id.as_deref())
        }
    } else {
        normalized_thread_id.clone()
    };

    Ok((normalized_thread_id, outbound_target))
}

fn send_feishu_outbound_chunk_with_runtime(
    runtime_state: &OpenClawPluginFeishuRuntimeState,
    request: OpenClawPluginFeishuOutboundSendRequest,
) -> Result<OpenClawPluginFeishuOutboundSendResult, String> {
    if let Ok(guard) = feishu_official_runtime_outbound_send_hook_slot().lock() {
        if let Some(hook) = guard.as_ref() {
            let result = hook(&request)?;
            return Ok(OpenClawPluginFeishuOutboundSendResult {
                request_id: request.request_id.clone(),
                request,
                result,
            });
        }
    }

    send_openclaw_plugin_feishu_runtime_outbound_message_in_state(runtime_state, request)
}

pub async fn execute_feishu_reply_plan_with_pool(
    pool: &SqlitePool,
    runtime_state: &OpenClawPluginFeishuRuntimeState,
    plan: &ImReplyDeliveryPlan,
    account_id: Option<String>,
) -> Result<FeishuReplyPlanExecutionResult, String> {
    let resolved_account_id = match account_id {
        Some(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                super::resolve_default_feishu_account_id_with_pool(pool)
                    .await?
                    .unwrap_or_else(|| "default".to_string())
            } else {
                trimmed
            }
        }
        None => super::resolve_default_feishu_account_id_with_pool(pool)
            .await?
            .unwrap_or_else(|| "default".to_string()),
    };

    let (normalized_thread_id, outbound_target) =
        resolve_feishu_outbound_target_with_pool(pool, &resolved_account_id, &plan.thread_id)
            .await?;

    let transport = FeishuReplyPlanTransport {
        pool,
        runtime_state,
        resolved_account_id,
        normalized_thread_id,
        outbound_target,
    };
    let result = match execute_reply_plan_with_transport(&transport, plan).await {
        Ok(result) => {
            record_feishu_runtime_reply_trace(runtime_state, &result.trace);
            result
        }
        Err(error) => {
            record_feishu_runtime_reply_trace_error(runtime_state, &error);
            return Err(error);
        }
    };
    Ok(FeishuReplyPlanExecutionResult {
        trace: result.trace,
        deliveries: result.deliveries,
    })
}

struct FeishuReplyPlanTransport<'a> {
    pool: &'a SqlitePool,
    runtime_state: &'a OpenClawPluginFeishuRuntimeState,
    resolved_account_id: String,
    normalized_thread_id: String,
    outbound_target: String,
}

#[async_trait]
impl ImReplyPlanTransport for FeishuReplyPlanTransport<'_> {
    type Delivery = OpenClawPluginFeishuOutboundSendResult;

    async fn on_processing_started(&self, _plan: &ImReplyDeliveryPlan) -> Result<(), String> {
        Ok(())
    }

    async fn send_chunk(
        &self,
        plan: &ImReplyDeliveryPlan,
        _chunk_index: usize,
        text: &str,
    ) -> Result<Self::Delivery, String> {
        let request = OpenClawPluginFeishuOutboundSendRequest {
            request_id: Uuid::new_v4().to_string(),
            account_id: self.resolved_account_id.clone(),
            target: self.outbound_target.clone(),
            thread_id: Some(plan.thread_id.clone()),
            text: text.to_string(),
            mode: "text".to_string(),
        };
        send_feishu_outbound_chunk_with_runtime(self.runtime_state, request)
    }

    async fn handle_delivery(
        &self,
        _plan: &ImReplyDeliveryPlan,
        _chunk_index: usize,
        delivery: &Self::Delivery,
    ) -> Result<(), String> {
        if self.normalized_thread_id.starts_with("ou_")
            && delivery.result.chat_id.trim().starts_with("oc_")
        {
            remember_feishu_chat_id_for_sender_with_pool(
                self.pool,
                &delivery.request.account_id,
                &self.normalized_thread_id,
                &delivery.result.chat_id,
            )
            .await?;
        }
        Ok(())
    }

    async fn on_processing_finished(
        &self,
        plan: &ImReplyDeliveryPlan,
        final_state: &str,
    ) -> Result<(), String> {
        let _ = maybe_stop_registered_feishu_processing_for_session_with_pool(
            self.pool,
            &plan.session_id,
            Some(&plan.logical_reply_id),
            Some(final_state),
            Some(&self.resolved_account_id),
        )
        .await;
        Ok(())
    }

    fn classify_failure(&self, delivered_count: usize, error: &str) -> ImReplyDeliveryState {
        classify_feishu_runtime_outbound_failure(
            delivered_count,
            infer_feishu_runtime_outbound_failure_kind(error),
        )
    }
}

pub async fn send_feishu_text_message_via_official_runtime_with_pool(
    pool: &SqlitePool,
    runtime_state: &OpenClawPluginFeishuRuntimeState,
    chat_id: &str,
    text: &str,
    account_id: Option<String>,
) -> Result<String, String> {
    let resolved_account_id = match account_id {
        Some(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                super::resolve_default_feishu_account_id_with_pool(pool)
                    .await?
                    .unwrap_or_else(|| "default".to_string())
            } else {
                trimmed
            }
        }
        None => super::resolve_default_feishu_account_id_with_pool(pool)
            .await?
            .unwrap_or_else(|| "default".to_string()),
    };
    let plan = build_feishu_reply_plan(
        &Uuid::new_v4().to_string(),
        chat_id.trim(),
        chat_id.trim(),
        text,
    );
    let result =
        execute_feishu_reply_plan_with_pool(pool, runtime_state, &plan, Some(resolved_account_id))
            .await?;
    serde_json::to_string(&result)
        .map_err(|error| format!("failed to serialize reply plan execution result: {error}"))
}
