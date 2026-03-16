use super::chat::{ApprovalManagerState, PendingApprovalBridgeState};
use super::feishu_gateway::notify_feishu_approval_resolved_with_pool;
use super::skills::DbState;
use crate::approval_bus::{ApprovalDecision, ApprovalResolveResult, PendingApprovalRecord};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, SqlitePool};
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct PendingApprovalProjection {
    pub id: String,
    pub session_id: String,
    pub run_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub input_json: String,
    pub summary: String,
    pub impact: String,
    pub irreversible: i64,
    pub status: String,
    pub decision: String,
    pub resolved_by_surface: String,
    pub resolved_by_user: String,
    pub created_at: String,
    pub updated_at: String,
}

impl PendingApprovalProjection {
    pub fn into_record(self) -> PendingApprovalRecord {
        PendingApprovalRecord {
            approval_id: self.id,
            session_id: self.session_id,
            run_id: if self.run_id.trim().is_empty() {
                None
            } else {
                Some(self.run_id)
            },
            call_id: self.call_id,
            tool_name: self.tool_name,
            input: serde_json::from_str::<Value>(&self.input_json)
                .unwrap_or_else(|_| serde_json::json!({})),
            summary: self.summary,
            impact: if self.impact.trim().is_empty() {
                None
            } else {
                Some(self.impact)
            },
            irreversible: self.irreversible != 0,
            status: self.status,
        }
    }
}

pub async fn list_pending_approvals_with_pool(
    pool: &SqlitePool,
    session_id: Option<&str>,
) -> Result<Vec<PendingApprovalRecord>, String> {
    let rows = if let Some(session_id) = session_id.filter(|v| !v.trim().is_empty()) {
        sqlx::query_as::<_, PendingApprovalProjection>(
            "SELECT id, session_id, run_id, call_id, tool_name, input_json, summary, impact,
                    irreversible, status, decision, resolved_by_surface, resolved_by_user,
                    created_at, updated_at
             FROM approvals
             WHERE status = 'pending' AND session_id = ?
             ORDER BY created_at ASC, id ASC",
        )
        .bind(session_id.trim())
        .fetch_all(pool)
        .await
        .map_err(|e| format!("读取待审批列表失败: {e}"))?
    } else {
        sqlx::query_as::<_, PendingApprovalProjection>(
            "SELECT id, session_id, run_id, call_id, tool_name, input_json, summary, impact,
                    irreversible, status, decision, resolved_by_surface, resolved_by_user,
                    created_at, updated_at
             FROM approvals
             WHERE status = 'pending'
             ORDER BY created_at ASC, id ASC",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("读取待审批列表失败: {e}"))?
    };

    Ok(rows
        .into_iter()
        .map(PendingApprovalProjection::into_record)
        .collect())
}

pub async fn load_approval_record_with_pool(
    pool: &SqlitePool,
    approval_id: &str,
) -> Result<Option<PendingApprovalRecord>, String> {
    let row = sqlx::query_as::<_, PendingApprovalProjection>(
        "SELECT id, session_id, run_id, call_id, tool_name, input_json, summary, impact,
                irreversible, status, decision, resolved_by_surface, resolved_by_user,
                created_at, updated_at
         FROM approvals
         WHERE id = ?",
    )
    .bind(approval_id.trim())
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("读取审批记录失败: {e}"))?;
    Ok(row.map(PendingApprovalProjection::into_record))
}

#[tauri::command]
pub async fn list_pending_approvals(
    session_id: Option<String>,
    db: State<'_, DbState>,
) -> Result<Vec<PendingApprovalRecord>, String> {
    list_pending_approvals_with_pool(&db.0, session_id.as_deref()).await
}

#[tauri::command]
pub async fn resolve_approval(
    app: AppHandle,
    approval_id: String,
    decision: ApprovalDecision,
    source: String,
    user_id: Option<String>,
    db: State<'_, DbState>,
    approvals: State<'_, ApprovalManagerState>,
    bridge: State<'_, PendingApprovalBridgeState>,
) -> Result<ApprovalResolveResult, String> {
    let resolved_by_user = user_id.unwrap_or_else(|| source.clone());
    let result = approvals
        .0
        .resolve_with_pool(
            &db.0,
            &approval_id,
            decision.clone(),
            &source,
            &resolved_by_user,
        )
        .await?;

    if let Ok(mut guard) = bridge.0.lock() {
        if guard.as_deref() == Some(approval_id.as_str()) {
            *guard = None;
        }
    }

    if let Some(record) = load_approval_record_with_pool(&db.0, &approval_id).await? {
        let _ = app.emit("approval-resolved", &record);
    }
    if matches!(result, ApprovalResolveResult::Applied { .. }) {
        let _ = notify_feishu_approval_resolved_with_pool(&db.0, &approval_id, None).await;
    }

    Ok(result)
}
