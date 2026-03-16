use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
pub struct ApprovalRuleRecord {
    pub id: String,
    pub tool_name: String,
    pub fingerprint: String,
    pub source_approval_id: String,
    pub created_by_surface: String,
    pub created_by_user: String,
    pub enabled: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
struct ApprovalRuleSourceRow {
    tool_name: String,
    input_json: String,
    decision: String,
    resolved_by_surface: String,
    resolved_by_user: String,
}

pub async fn list_approval_rules_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<ApprovalRuleRecord>, String> {
    sqlx::query_as::<_, ApprovalRuleRecord>(
        "SELECT id, tool_name, fingerprint, source_approval_id, created_by_surface,
                created_by_user, enabled, created_at, updated_at
         FROM approval_rules
         ORDER BY created_at ASC, id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("读取 approval rules 失败: {e}"))
}

pub async fn find_matching_approval_rule_with_pool(
    pool: &SqlitePool,
    tool_name: &str,
    input: &Value,
) -> Result<Option<ApprovalRuleRecord>, String> {
    let normalized_tool_name = runtime_policy::normalize_tool_name(tool_name);
    let Some(fingerprint) = runtime_policy::approval_rule_fingerprint(&normalized_tool_name, input) else {
        return Ok(None);
    };

    sqlx::query_as::<_, ApprovalRuleRecord>(
        "SELECT id, tool_name, fingerprint, source_approval_id, created_by_surface,
                created_by_user, enabled, created_at, updated_at
         FROM approval_rules
         WHERE enabled = 1 AND tool_name = ? AND fingerprint = ?
         LIMIT 1",
    )
    .bind(&normalized_tool_name)
    .bind(&fingerprint)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("匹配 approval rule 失败: {e}"))
}

pub async fn persist_allow_always_rule_with_pool(
    pool: &SqlitePool,
    approval_id: &str,
) -> Result<Option<ApprovalRuleRecord>, String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("创建 approval rule 事务失败: {e}"))?;
    let record = persist_allow_always_rule_with_tx(&mut tx, approval_id).await?;
    tx.commit()
        .await
        .map_err(|e| format!("提交 approval rule 事务失败: {e}"))?;
    Ok(record)
}

pub async fn persist_allow_always_rule_with_tx(
    tx: &mut Transaction<'_, Sqlite>,
    approval_id: &str,
) -> Result<Option<ApprovalRuleRecord>, String> {
    let Some(source) = sqlx::query_as::<_, ApprovalRuleSourceRow>(
        "SELECT tool_name, input_json, decision, resolved_by_surface, resolved_by_user
         FROM approvals
         WHERE id = ?",
    )
    .bind(approval_id.trim())
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| format!("读取 allow_always 来源审批失败: {e}"))? else {
        return Ok(None);
    };

    if source.decision.trim() != "allow_always" {
        return Ok(None);
    }

    let input = serde_json::from_str::<Value>(&source.input_json)
        .map_err(|e| format!("解析审批输入失败: {e}"))?;
    let normalized_tool_name = runtime_policy::normalize_tool_name(&source.tool_name);
    let Some(fingerprint) =
        runtime_policy::approval_rule_fingerprint(&normalized_tool_name, &input)
    else {
        return Ok(None);
    };

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO approval_rules (
            id, tool_name, fingerprint, source_approval_id, created_by_surface,
            created_by_user, enabled, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)
         ON CONFLICT(tool_name, fingerprint) DO UPDATE SET
            source_approval_id = excluded.source_approval_id,
            created_by_surface = excluded.created_by_surface,
            created_by_user = excluded.created_by_user,
            enabled = 1,
            updated_at = excluded.updated_at",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&normalized_tool_name)
    .bind(&fingerprint)
    .bind(approval_id.trim())
    .bind(source.resolved_by_surface.trim())
    .bind(source.resolved_by_user.trim())
    .bind(&now)
    .bind(&now)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("写入 approval rule 失败: {e}"))?;

    sqlx::query_as::<_, ApprovalRuleRecord>(
        "SELECT id, tool_name, fingerprint, source_approval_id, created_by_surface,
                created_by_user, enabled, created_at, updated_at
         FROM approval_rules
         WHERE tool_name = ? AND fingerprint = ?
         LIMIT 1",
    )
    .bind(&normalized_tool_name)
    .bind(&fingerprint)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| format!("读取 approval rule 失败: {e}"))
}
