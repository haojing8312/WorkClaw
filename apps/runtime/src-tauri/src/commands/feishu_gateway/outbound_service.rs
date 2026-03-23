use crate::commands::openclaw_plugins::{
    send_openclaw_plugin_feishu_runtime_outbound_message_in_state,
    OpenClawPluginFeishuOutboundDeliveryResult, OpenClawPluginFeishuOutboundSendRequest,
    OpenClawPluginFeishuOutboundSendResult, OpenClawPluginFeishuRuntimeState,
};
use sqlx::SqlitePool;
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub fn build_feishu_outbound_route_target(thread_id: &str, reply_to_message_id: Option<&str>) -> String {
    let normalized_thread_id = thread_id.trim();
    let normalized_reply_to = reply_to_message_id
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if normalized_thread_id.is_empty() {
        return String::new();
    }

    if normalized_thread_id.starts_with("ou_") {
        if let Some(reply_to_message_id) = normalized_reply_to {
            return format!(
                "{thread}#__feishu_reply_to={reply}&__feishu_thread_id={thread}",
                thread = normalized_thread_id,
                reply = reply_to_message_id,
            );
        }
    }

    normalized_thread_id.to_string()
}

fn feishu_runtime_outbound_state_slot() -> &'static Mutex<Option<OpenClawPluginFeishuRuntimeState>>
{
    static SLOT: OnceLock<Mutex<Option<OpenClawPluginFeishuRuntimeState>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

pub type FeishuOfficialRuntimeOutboundSendHook =
    dyn Fn(&OpenClawPluginFeishuOutboundSendRequest) -> Result<OpenClawPluginFeishuOutboundDeliveryResult, String>
        + Send
        + Sync;

fn feishu_official_runtime_outbound_send_hook_slot()
-> &'static Mutex<Option<Arc<FeishuOfficialRuntimeOutboundSendHook>>> {
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

pub(crate) async fn lookup_feishu_thread_for_session_with_pool(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<String>, String> {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT ts.thread_id
         FROM im_thread_sessions ts
         WHERE ts.session_id = ?
           AND EXISTS (
             SELECT 1
             FROM im_inbox_events e
             WHERE e.thread_id = ts.thread_id AND e.source = 'feishu'
           )
         ORDER BY ts.updated_at DESC, ts.created_at DESC
         LIMIT 1",
    )
    .bind(session_id.trim())
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("查询飞书线程映射失败: {e}"))?;

    Ok(row.map(|(thread_id,)| thread_id))
}

pub(crate) async fn lookup_latest_feishu_inbox_message_id_for_thread_with_pool(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Option<String>, String> {
    let normalized_thread_id = thread_id.trim();
    if normalized_thread_id.is_empty() {
        return Ok(None);
    }

    let row = sqlx::query_as::<_, (String,)>(
        "SELECT message_id
         FROM im_inbox_events
         WHERE thread_id = ?
           AND source = 'feishu'
           AND message_id <> ''
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
    )
    .bind(normalized_thread_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("查询飞书线程最新消息失败: {e}"))?;

    Ok(row.map(|(message_id,)| message_id))
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
    send_feishu_text_message_via_official_runtime_with_pool(pool, &runtime_state, chat_id, text, None)
        .await
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
    let normalized_thread_id = chat_id.trim().to_string();
    let outbound_target = if normalized_thread_id.starts_with("ou_") {
        if let Some(mapped_chat_id) =
            lookup_feishu_chat_id_for_sender_with_pool(pool, &resolved_account_id, &normalized_thread_id)
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

    if let Ok(guard) = feishu_official_runtime_outbound_send_hook_slot().lock() {
        if let Some(hook) = guard.as_ref() {
            let request = OpenClawPluginFeishuOutboundSendRequest {
                request_id: Uuid::new_v4().to_string(),
                account_id: resolved_account_id.clone(),
                target: outbound_target.clone(),
                thread_id: Some(chat_id.trim().to_string()),
                text: text.trim().to_string(),
                mode: "text".to_string(),
            };
            let result = hook(&request)?;
            let outbound = OpenClawPluginFeishuOutboundSendResult {
                request_id: request.request_id.clone(),
                request,
                result,
            };
            return serde_json::to_string(&outbound)
                .map_err(|error| format!("failed to serialize outbound send result: {error}"));
        }
    }

    let result = send_openclaw_plugin_feishu_runtime_outbound_message_in_state(
        runtime_state,
        OpenClawPluginFeishuOutboundSendRequest {
            request_id: Uuid::new_v4().to_string(),
            account_id: resolved_account_id,
            target: outbound_target,
            thread_id: Some(chat_id.trim().to_string()),
            text: text.trim().to_string(),
            mode: "text".to_string(),
        },
    )?;

    if normalized_thread_id.starts_with("ou_") && result.result.chat_id.trim().starts_with("oc_") {
        remember_feishu_chat_id_for_sender_with_pool(
            pool,
            &result.request.account_id,
            &normalized_thread_id,
            &result.result.chat_id,
        )
        .await?;
    }

    serde_json::to_string(&result)
        .map_err(|error| format!("failed to serialize outbound send result: {error}"))
}
