use crate::commands::feishu_gateway::outbound_service::FeishuReplyPlanExecutionResult;
use crate::commands::feishu_gateway::{
    execute_registered_feishu_reply_plan_with_pool, lookup_feishu_thread_for_session_with_pool,
};
use crate::commands::im_host::{plan_text_chunks, ImReplyDeliveryPlan};
use uuid::Uuid;

pub(crate) fn build_feishu_reply_plan(
    logical_reply_id: &str,
    session_id: &str,
    thread_id: &str,
    text: &str,
) -> ImReplyDeliveryPlan {
    try_build_feishu_reply_plan(logical_reply_id, session_id, thread_id, text)
        .expect("feishu reply plan should be valid")
}

pub(crate) fn try_build_feishu_reply_plan(
    logical_reply_id: &str,
    session_id: &str,
    thread_id: &str,
    text: &str,
) -> Result<ImReplyDeliveryPlan, String> {
    let normalized_reply_id = logical_reply_id.trim();
    let normalized_session_id = session_id.trim();
    let normalized_thread_id = thread_id.trim();
    let normalized_text = text.trim();

    if normalized_reply_id.is_empty() {
        return Err("logical_reply_id is required".to_string());
    }
    if normalized_session_id.is_empty() {
        return Err("session_id is required".to_string());
    }
    if normalized_thread_id.is_empty() {
        return Err("thread_id is required".to_string());
    }
    if normalized_text.is_empty() {
        return Err("final reply text is required".to_string());
    }

    Ok(ImReplyDeliveryPlan {
        logical_reply_id: normalized_reply_id.to_string(),
        session_id: normalized_session_id.to_string(),
        channel: "feishu".to_string(),
        thread_id: normalized_thread_id.to_string(),
        chunks: plan_text_chunks(normalized_text, 1800),
    })
}

pub(crate) async fn maybe_dispatch_feishu_session_reply_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    text: &str,
) -> Result<Option<FeishuReplyPlanExecutionResult>, String> {
    let normalized_session_id = session_id.trim();
    let normalized_text = text.trim();
    if normalized_session_id.is_empty() || normalized_text.is_empty() {
        return Ok(None);
    }

    let Some(thread_id) =
        lookup_feishu_thread_for_session_with_pool(pool, normalized_session_id).await?
    else {
        return Ok(None);
    };

    let plan = build_feishu_reply_plan(
        &Uuid::new_v4().to_string(),
        normalized_session_id,
        &thread_id,
        normalized_text,
    );

    execute_registered_feishu_reply_plan_with_pool(pool, &plan, None)
        .await
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::{build_feishu_reply_plan, try_build_feishu_reply_plan};

    #[test]
    fn reply_host_service_builds_multichunk_feishu_plan() {
        let plan = build_feishu_reply_plan("reply-1", "session-1", "chat-1", &"A".repeat(4000));
        assert_eq!(plan.channel, "feishu");
        assert_eq!(plan.logical_reply_id, "reply-1");
        assert!(plan.chunks.len() > 1);
    }

    #[test]
    fn reply_host_service_rejects_empty_final_reply() {
        let result = try_build_feishu_reply_plan("reply-1", "session-1", "chat-1", "   ");
        assert!(result.is_err());
    }
}
