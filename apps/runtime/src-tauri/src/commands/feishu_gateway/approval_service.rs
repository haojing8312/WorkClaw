use crate::approval_bus::{
    ApprovalDecision, ApprovalManager, ApprovalResolveResult, PendingApprovalRecord,
};
use crate::commands::approvals::load_approval_record_with_pool;
use crate::commands::feishu_gateway::{
    send_feishu_text_message_with_pool,
};
use crate::commands::im_host::{
    prepare_channel_interactive_approval_notice_with_pool,
    prepare_channel_interactive_session_thread_with_pool,
    build_im_approval_request_text, build_im_approval_resolution_text,
};
use crate::commands::openclaw_plugins::im_host_contract::ImReplyLifecyclePhase;
use crate::im::types::ImEvent;
use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FeishuApprovalCommand {
    pub(crate) approval_id: String,
    pub(crate) decision: ApprovalDecision,
}

pub(crate) fn parse_feishu_approval_command(text: Option<&str>) -> Option<FeishuApprovalCommand> {
    let raw = text?.trim();
    if raw.is_empty() {
        return None;
    }

    let parts = raw.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 2 || !parts[0].eq_ignore_ascii_case("/approve") {
        return None;
    }

    let approval_id = parts[1].trim();
    if approval_id.is_empty() {
        return None;
    }

    let decision = match parts
        .get(2)
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        None | Some("") | Some("allow_once") | Some("allow-once") | Some("approve") => {
            ApprovalDecision::AllowOnce
        }
        Some("allow_always") | Some("allow-always") => ApprovalDecision::AllowAlways,
        Some("deny") | Some("reject") => ApprovalDecision::Deny,
        Some(_) => return None,
    };

    Some(FeishuApprovalCommand {
        approval_id: approval_id.to_string(),
        decision,
    })
}

pub(crate) fn build_feishu_approval_request_text(record: &PendingApprovalRecord) -> String {
    build_im_approval_request_text(record)
}

pub(crate) fn build_feishu_approval_resolution_text(
    approval_id: &str,
    result: &ApprovalResolveResult,
    summary: Option<&str>,
) -> String {
    build_im_approval_resolution_text(approval_id, result, summary)
}

pub async fn notify_feishu_approval_requested_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    record: &PendingApprovalRecord,
    sidecar_base_url: Option<String>,
) -> Result<(), String> {
    let Some(thread_id) = prepare_channel_interactive_session_thread_with_pool(
        pool,
        "feishu",
        session_id,
        Some("waiting_approval"),
        ImReplyLifecyclePhase::ApprovalRequested,
    )
    .await?
    else {
        return Ok(());
    };
    send_feishu_text_message_with_pool(
        pool,
        &thread_id,
        &build_feishu_approval_request_text(record),
        sidecar_base_url,
    )
    .await?;
    Ok(())
}

pub(crate) async fn notify_feishu_approval_resolved_with_pool(
    pool: &SqlitePool,
    approval_id: &str,
    sidecar_base_url: Option<String>,
) -> Result<(), String> {
    let Some(row) = prepare_channel_interactive_approval_notice_with_pool(pool, approval_id).await?
    else {
        return Ok(());
    };
    let Some(thread_id) = crate::commands::im_host::lookup_channel_thread_for_session_with_pool(
        pool,
        "feishu",
        &row.session_id,
    )
    .await? else {
        return Ok(());
    };
    let text = crate::commands::im_host::build_im_approval_resolved_notice_text(&row);
    send_feishu_text_message_with_pool(pool, &thread_id, &text, sidecar_base_url).await?;
    Ok(())
}

pub async fn maybe_handle_feishu_approval_command_with_pool(
    pool: &SqlitePool,
    approvals: &ApprovalManager,
    event: &ImEvent,
    sidecar_base_url: Option<String>,
) -> Result<Option<ApprovalResolveResult>, String> {
    let Some(command) = parse_feishu_approval_command(event.text.as_deref()) else {
        return Ok(None);
    };

    let resolved_by_user = event
        .account_id
        .as_deref()
        .or(event.tenant_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("feishu");

    let resolution = approvals
        .resolve_with_pool(
            pool,
            &command.approval_id,
            command.decision,
            "feishu",
            resolved_by_user,
        )
        .await?;

    let summary = load_approval_record_with_pool(pool, &command.approval_id)
        .await?
        .map(|record| record.summary);
    let message = build_feishu_approval_resolution_text(
        &command.approval_id,
        &resolution,
        summary.as_deref(),
    );
    super::send_feishu_text_message_with_pool(pool, &event.thread_id, &message, sidecar_base_url)
        .await?;

    Ok(Some(resolution))
}
