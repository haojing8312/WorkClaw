use crate::commands::im_config::get_thread_role_config_with_pool;
use crate::im::runtime_bridge::{
    build_im_role_dispatch_request_for_channel, build_im_role_event_payload_for_channel,
    ImRoleDispatchRequest, ImRoleEventPayload,
};
use crate::im::types::ImEvent;
use sqlx::SqlitePool;

pub async fn plan_role_events_for_feishu(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<ImRoleEventPayload>, String> {
    let cfg = match get_thread_role_config_with_pool(pool, &event.thread_id).await {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()),
    };
    let session_id = format!("im-{}", event.thread_id);
    let text = event.text.clone().unwrap_or_default();

    let roles: Vec<String> = event
        .role_id
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .and_then(|role_id| {
            if cfg.roles.iter().any(|r| r == role_id) {
                Some(vec![role_id.to_string()])
            } else {
                None
            }
        })
        .unwrap_or_else(|| cfg.roles.clone());

    Ok(roles
        .into_iter()
        .map(|role_id| {
            build_im_role_event_payload_for_channel(
                &session_id,
                &event.thread_id,
                &role_id,
                &role_id,
                "feishu",
                "running",
                &format!("飞书事件触发：{}", text),
                None,
            )
        })
        .collect())
}

pub async fn plan_role_dispatch_requests_for_feishu(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<ImRoleDispatchRequest>, String> {
    let cfg = match get_thread_role_config_with_pool(pool, &event.thread_id).await {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()),
    };
    let session_id = format!("im-{}", event.thread_id);
    let user_text = event
        .text
        .clone()
        .unwrap_or_else(|| "请基于当前上下文继续协作".to_string());

    let roles: Vec<String> = event
        .role_id
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .and_then(|role_id| {
            if cfg.roles.iter().any(|r| r == role_id) {
                Some(vec![role_id.to_string()])
            } else {
                None
            }
        })
        .unwrap_or_else(|| cfg.roles.clone());

    let agent_type = if cfg.scenario_template == "opportunity_review" {
        "plan"
    } else {
        "general-purpose"
    };

    Ok(roles
        .into_iter()
        .map(|role_id| {
            let mut req = build_im_role_dispatch_request_for_channel(
                &session_id,
                &event.thread_id,
                &role_id,
                &role_id,
                "feishu",
                &format!("场景={}。用户输入：{}", cfg.scenario_template, user_text),
                agent_type,
            );
            req.message_id = event.message_id.clone().unwrap_or_default();
            req
        })
        .collect())
}
