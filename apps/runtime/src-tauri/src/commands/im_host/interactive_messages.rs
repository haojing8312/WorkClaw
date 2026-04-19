use crate::approval_bus::{ApprovalDecision, ApprovalResolveResult, PendingApprovalRecord};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, FromRow)]
pub(crate) struct ApprovalResolutionNotificationRow {
    pub id: String,
    pub session_id: String,
    pub summary: String,
    pub status: String,
    pub decision: String,
    pub resolved_by_surface: String,
    pub resolved_by_user: String,
}

pub(crate) fn build_im_approval_request_text(record: &PendingApprovalRecord) -> String {
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

pub(crate) fn build_im_approval_resolution_text(
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

pub(crate) fn build_im_ask_user_request_text(question: &str, options: &[String]) -> String {
    let normalized_question = question.trim();
    let normalized_options = options
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    if normalized_options.is_empty() {
        return format!("{normalized_question}\n请直接回复你的选择或补充信息。");
    }

    format!(
        "{question}\n可选项：{options}\n请直接回复你的选择或补充信息。",
        question = normalized_question,
        options = normalized_options.join(" / "),
    )
}

pub(crate) fn build_im_approval_resolved_notice_text(
    row: &ApprovalResolutionNotificationRow,
) -> String {
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

    format!(
        "{} 处理人：{}。",
        build_im_approval_resolution_text(&row.id, &result, Some(&row.summary)),
        if resolved_by.is_empty() {
            "unknown"
        } else {
            resolved_by
        }
    )
}

pub(crate) async fn load_approval_resolution_notification_with_pool(
    pool: &SqlitePool,
    approval_id: &str,
) -> Result<Option<ApprovalResolutionNotificationRow>, String> {
    sqlx::query_as::<_, ApprovalResolutionNotificationRow>(
        "SELECT id, session_id, summary, status, decision, resolved_by_surface, resolved_by_user
         FROM approvals
         WHERE id = ?",
    )
    .bind(approval_id.trim())
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("读取审批结果通知数据失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{
        build_im_approval_request_text, build_im_approval_resolution_text,
        build_im_ask_user_request_text,
    };
    use crate::approval_bus::{ApprovalDecision, ApprovalResolveResult, PendingApprovalRecord};
    use serde_json::json;

    #[test]
    fn build_im_ask_user_request_text_includes_options() {
        let text = build_im_ask_user_request_text(
            "请选择方案",
            &["方案A".to_string(), "方案B".to_string()],
        );
        assert!(text.contains("请选择方案"));
        assert!(text.contains("可选项：方案A / 方案B"));
    }

    #[test]
    fn build_im_approval_request_text_contains_command() {
        let record = PendingApprovalRecord {
            approval_id: "a-1".to_string(),
            session_id: "s-1".to_string(),
            run_id: None,
            call_id: "c-1".to_string(),
            tool_name: "shell".to_string(),
            input: json!({}),
            summary: "执行命令".to_string(),
            impact: None,
            irreversible: true,
            status: "pending".to_string(),
        };
        let text = build_im_approval_request_text(&record);
        assert!(text.contains("/approve a-1"));
    }

    #[test]
    fn build_im_approval_resolution_text_formats_applied() {
        let text = build_im_approval_resolution_text(
            "a-1",
            &ApprovalResolveResult::Applied {
                approval_id: "a-1".to_string(),
                status: "approved".to_string(),
                decision: ApprovalDecision::AllowOnce,
            },
            Some("执行命令"),
        );
        assert!(text.contains("审批 a-1 已处理"));
        assert!(text.contains("摘要：执行命令。"));
    }
}
