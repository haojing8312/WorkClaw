use crate::approval_bus::{
    ApprovalDecision, ApprovalManager, ApprovalResolveResult, PendingApprovalRecord,
};
use crate::commands::approvals::load_approval_record_with_pool;
use crate::im::types::ImEvent;
use sqlx::FromRow;
use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FeishuApprovalCommand {
    pub(crate) approval_id: String,
    pub(crate) decision: ApprovalDecision,
}

#[derive(Debug, Clone, FromRow)]
struct ApprovalResolutionNotificationRow {
    id: String,
    session_id: String,
    summary: String,
    status: String,
    decision: String,
    resolved_by_surface: String,
    resolved_by_user: String,
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

fn build_feishu_approval_request_text(record: &PendingApprovalRecord) -> String {
    let impact = record
        .impact
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("此操作属于高风险动作，请确认后继续。");
    let irreversible = if record.irreversible {
        "不可逆"
    } else {
        "可恢复性未知"
    };

    format!(
        "待审批 #{approval_id}\n工具：{tool_name}\n摘要：{summary}\n影响：{impact}\n风险：{irreversible}\n回复命令：/approve {approval_id} allow_once | allow_always | deny",
        approval_id = record.approval_id,
        tool_name = record.tool_name,
        summary = record.summary,
        impact = impact,
        irreversible = irreversible,
    )
}

fn build_feishu_approval_resolution_text(
    approval_id: &str,
    result: &ApprovalResolveResult,
    summary: Option<&str>,
) -> String {
    match result {
        ApprovalResolveResult::Applied {
            status, decision, ..
        } => {
            let action = match decision {
                ApprovalDecision::AllowOnce => "allow_once",
                ApprovalDecision::AllowAlways => "allow_always",
                ApprovalDecision::Deny => "deny",
            };
            let suffix = if *decision == ApprovalDecision::Deny {
                "本次操作已取消。"
            } else {
                "任务将继续执行。"
            };
            format!(
                "审批 {approval_id} 已处理：{status}（{action}）。{summary_line}{suffix}",
                approval_id = approval_id,
                status = status,
                action = action,
                summary_line = summary
                    .map(|value| format!("摘要：{}。", value.trim()))
                    .unwrap_or_default(),
                suffix = suffix,
            )
        }
        ApprovalResolveResult::AlreadyResolved {
            status, decision, ..
        } => {
            let decision_label = decision
                .as_ref()
                .map(|value| match value {
                    ApprovalDecision::AllowOnce => "allow_once",
                    ApprovalDecision::AllowAlways => "allow_always",
                    ApprovalDecision::Deny => "deny",
                })
                .unwrap_or("unknown");
            format!(
                "审批 {approval_id} 已被处理，当前状态：{status}（{decision_label}）。",
                approval_id = approval_id,
                status = status,
                decision_label = decision_label,
            )
        }
        ApprovalResolveResult::NotFound { .. } => {
            format!(
                "未找到待审批项 {approval_id}，请确认审批编号是否正确。",
                approval_id = approval_id,
            )
        }
    }
}

pub async fn notify_feishu_approval_requested_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    record: &PendingApprovalRecord,
    sidecar_base_url: Option<String>,
) -> Result<(), String> {
    let Some(thread_id) = super::lookup_feishu_thread_for_session_with_pool(pool, session_id).await?
    else {
        return Ok(());
    };

    super::send_feishu_text_message_with_pool(
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
    let Some(row) = sqlx::query_as::<_, ApprovalResolutionNotificationRow>(
        "SELECT id, session_id, summary, status, decision, resolved_by_surface, resolved_by_user
         FROM approvals
         WHERE id = ?",
    )
    .bind(approval_id.trim())
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("读取审批结果通知数据失败: {e}"))?
    else {
        return Ok(());
    };

    let Some(thread_id) =
        super::lookup_feishu_thread_for_session_with_pool(pool, &row.session_id).await?
    else {
        return Ok(());
    };

    let decision = match row.decision.as_str() {
        "allow_once" => Some(ApprovalDecision::AllowOnce),
        "allow_always" => Some(ApprovalDecision::AllowAlways),
        "deny" => Some(ApprovalDecision::Deny),
        _ => None,
    };
    let result = ApprovalResolveResult::AlreadyResolved {
        approval_id: row.id.clone(),
        status: row.status.clone(),
        decision,
    };
    let resolved_by = if row.resolved_by_user.trim().is_empty() {
        row.resolved_by_surface.trim()
    } else {
        row.resolved_by_user.trim()
    };
    let text = format!(
        "{} 处理人：{}。",
        build_feishu_approval_resolution_text(&row.id, &result, Some(&row.summary)),
        if resolved_by.is_empty() {
            "unknown"
        } else {
            resolved_by
        }
    );
    super::send_feishu_text_message_with_pool(pool, &thread_id, &text, sidecar_base_url).await?;
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
    let message =
        build_feishu_approval_resolution_text(&command.approval_id, &resolution, summary.as_deref());
    super::send_feishu_text_message_with_pool(pool, &event.thread_id, &message, sidecar_base_url)
        .await?;

    Ok(Some(resolution))
}
