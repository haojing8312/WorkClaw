use crate::agent::{ToolContext, ToolRegistry};
use crate::approval_rules::persist_allow_always_rule_with_tx;
use crate::commands::session_runs::append_session_run_event_with_pool;
use crate::session_journal::{
    SessionJournalStore, SessionRunEvent, SessionRunTaskContinuationSnapshot,
    SessionRunTaskIdentitySnapshot,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::oneshot;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    AllowOnce,
    AllowAlways,
    Deny,
}

impl ApprovalDecision {
    fn as_db_value(&self) -> &'static str {
        match self {
            ApprovalDecision::AllowOnce => "allow_once",
            ApprovalDecision::AllowAlways => "allow_always",
            ApprovalDecision::Deny => "deny",
        }
    }

    fn resolved_status(&self) -> &'static str {
        match self {
            ApprovalDecision::Deny => "denied",
            ApprovalDecision::AllowOnce | ApprovalDecision::AllowAlways => "approved",
        }
    }

    fn from_db_value(value: &str) -> Option<Self> {
        match value {
            "allow_once" => Some(Self::AllowOnce),
            "allow_always" => Some(Self::AllowAlways),
            "deny" => Some(Self::Deny),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalResolution {
    pub approval_id: String,
    pub status: String,
    pub decision: ApprovalDecision,
    pub resolved_by_surface: String,
    pub resolved_by_user: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApprovalResolveResult {
    Applied {
        approval_id: String,
        status: String,
        decision: ApprovalDecision,
    },
    AlreadyResolved {
        approval_id: String,
        status: String,
        decision: Option<ApprovalDecision>,
    },
    NotFound {
        approval_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingApprovalRecord {
    pub approval_id: String,
    pub session_id: String,
    pub run_id: Option<String>,
    pub call_id: String,
    pub tool_name: String,
    pub input: Value,
    pub summary: String,
    pub impact: Option<String>,
    pub irreversible: bool,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct CreateApprovalRequest {
    pub approval_id: String,
    pub session_id: String,
    pub run_id: Option<String>,
    pub task_identity: Option<SessionRunTaskIdentitySnapshot>,
    pub task_continuation: Option<SessionRunTaskContinuationSnapshot>,
    pub call_id: String,
    pub tool_name: String,
    pub input: Value,
    pub summary: String,
    pub impact: Option<String>,
    pub irreversible: bool,
    pub work_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalResumePayload {
    pub session_id: String,
    pub run_id: Option<String>,
    pub call_id: String,
    pub tool_name: String,
    pub input: Value,
    pub work_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_continuation: Option<SessionRunTaskContinuationSnapshot>,
}

#[derive(Debug, Clone, Default)]
pub struct ApprovalManager {
    waiters: Arc<Mutex<HashMap<String, oneshot::Sender<ApprovalResolution>>>>,
}

pub async fn approval_bus_rollout_enabled_with_pool(pool: &SqlitePool) -> Result<bool, String> {
    let stored = match sqlx::query_as::<_, (String,)>(
        "SELECT value FROM app_settings WHERE key = ? LIMIT 1",
    )
    .bind("approval_bus_v1")
    .fetch_optional(pool)
    .await
    {
        Ok(row) => row,
        Err(error) => {
            let message = error.to_string();
            if message.contains("no such table: app_settings") {
                return Ok(true);
            }
            return Err(format!("读取 approval_bus_v1 配置失败: {message}"));
        }
    };

    Ok(stored
        .map(|(value,)| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "off" | "disabled"
            )
        })
        .unwrap_or(true))
}

impl ApprovalManager {
    pub fn register_waiter(
        &self,
        approval_id: impl Into<String>,
    ) -> oneshot::Receiver<ApprovalResolution> {
        let approval_id = approval_id.into();
        let (tx, rx) = oneshot::channel();
        if let Ok(mut guard) = self.waiters.lock() {
            guard.insert(approval_id, tx);
        }
        rx
    }

    pub async fn resolve_with_pool(
        &self,
        pool: &SqlitePool,
        approval_id: &str,
        decision: ApprovalDecision,
        resolved_by_surface: &str,
        resolved_by_user: &str,
    ) -> Result<ApprovalResolveResult, String> {
        let now = Utc::now().to_rfc3339();
        let status = decision.resolved_status().to_string();
        let decision_value = decision.as_db_value().to_string();

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| format!("创建审批事务失败: {e}"))?;
        let result = sqlx::query(
            "UPDATE approvals
             SET status = ?, decision = ?, resolved_by_surface = ?, resolved_by_user = ?, resolved_at = ?, updated_at = ?
             WHERE id = ? AND status = 'pending'",
        )
        .bind(&status)
        .bind(&decision_value)
        .bind(resolved_by_surface.trim())
        .bind(resolved_by_user.trim())
        .bind(&now)
        .bind(&now)
        .bind(approval_id.trim())
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("更新 approval 状态失败: {e}"))?;

        if result.rows_affected() > 0 {
            if decision == ApprovalDecision::AllowAlways {
                persist_allow_always_rule_with_tx(&mut tx, approval_id).await?;
            }
            tx.commit()
                .await
                .map_err(|e| format!("提交审批事务失败: {e}"))?;

            self.notify_waiter(ApprovalResolution {
                approval_id: approval_id.to_string(),
                status: status.clone(),
                decision: decision.clone(),
                resolved_by_surface: resolved_by_surface.trim().to_string(),
                resolved_by_user: resolved_by_user.trim().to_string(),
            });

            return Ok(ApprovalResolveResult::Applied {
                approval_id: approval_id.to_string(),
                status,
                decision,
            });
        }

        tx.rollback().await.ok();

        let current: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT status, NULLIF(decision, '')
             FROM approvals
             WHERE id = ?",
        )
        .bind(approval_id.trim())
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("读取 approval 当前状态失败: {e}"))?;

        match current {
            Some((status, decision_value)) => Ok(ApprovalResolveResult::AlreadyResolved {
                approval_id: approval_id.to_string(),
                status,
                decision: decision_value
                    .as_deref()
                    .and_then(ApprovalDecision::from_db_value),
            }),
            None => Ok(ApprovalResolveResult::NotFound {
                approval_id: approval_id.to_string(),
            }),
        }
    }

    fn notify_waiter(&self, resolution: ApprovalResolution) {
        let sender = self
            .waiters
            .lock()
            .ok()
            .and_then(|mut guard| guard.remove(&resolution.approval_id));
        if let Some(sender) = sender {
            let _ = sender.send(resolution);
        }
    }

    pub async fn create_pending_with_pool(
        &self,
        pool: &SqlitePool,
        journal: Option<&SessionJournalStore>,
        request: CreateApprovalRequest,
    ) -> Result<PendingApprovalRecord, String> {
        let run_id = request.run_id.clone();
        let resume_payload = ApprovalResumePayload {
            session_id: request.session_id.clone(),
            run_id: request.run_id.clone(),
            call_id: request.call_id.clone(),
            tool_name: request.tool_name.clone(),
            input: request.input.clone(),
            work_dir: request.work_dir.clone(),
            task_continuation: request.task_continuation.clone(),
        };
        let resume_payload_json = serde_json::to_string(&resume_payload)
            .map_err(|e| format!("序列化 approval 恢复载荷失败: {e}"))?;
        if let (Some(run_id_value), Some(journal_store)) = (run_id.clone(), journal) {
            append_session_run_event_with_pool(
                pool,
                journal_store,
                &request.session_id,
                SessionRunEvent::ApprovalRequested {
                    run_id: run_id_value,
                    approval_id: request.approval_id.clone(),
                    task_identity: request.task_identity.clone(),
                    task_continuation: request.task_continuation.clone(),
                    tool_name: request.tool_name.clone(),
                    call_id: request.call_id.clone(),
                    input: request.input.clone(),
                    summary: request.summary.clone(),
                    impact: request.impact.clone(),
                    irreversible: request.irreversible,
                },
            )
            .await?;
            sqlx::query(
                "UPDATE approvals
                 SET resume_payload_json = ?, updated_at = ?
                 WHERE id = ?",
            )
            .bind(&resume_payload_json)
            .bind(Utc::now().to_rfc3339())
            .bind(&request.approval_id)
            .execute(pool)
            .await
            .map_err(|e| format!("更新 approval 恢复载荷失败: {e}"))?;
        } else {
            let now = Utc::now().to_rfc3339();
            sqlx::query(
                "INSERT INTO approvals (
                    id, session_id, run_id, call_id, tool_name, input_json, summary, impact,
                    irreversible, status, decision, notify_targets_json, resume_payload_json,
                    resolved_by_surface, resolved_by_user, resolved_at, resumed_at, expires_at,
                    created_at, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, '', '[]', ?, '', '', NULL, NULL, NULL, ?, ?)",
            )
            .bind(&request.approval_id)
            .bind(&request.session_id)
            .bind(run_id.clone().unwrap_or_default())
            .bind(&request.call_id)
            .bind(&request.tool_name)
            .bind(request.input.to_string())
            .bind(&request.summary)
            .bind(request.impact.clone().unwrap_or_default())
            .bind(if request.irreversible { 1_i64 } else { 0_i64 })
            .bind("pending")
            .bind(&resume_payload_json)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("写入 approval 记录失败: {e}"))?;
        }

        Ok(PendingApprovalRecord {
            approval_id: request.approval_id,
            session_id: request.session_id,
            run_id,
            call_id: request.call_id,
            tool_name: request.tool_name,
            input: request.input,
            summary: request.summary,
            impact: request.impact,
            irreversible: request.irreversible,
            status: "pending".to_string(),
        })
    }

    pub async fn wait_for_resolution(
        &self,
        receiver: oneshot::Receiver<ApprovalResolution>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<ApprovalResolution, String> {
        let mut receiver = receiver;
        loop {
            tokio::select! {
                resolution = &mut receiver => {
                    return resolution.map_err(|_| "审批等待通道已关闭".to_string());
                }
                _ = async {
                    loop {
                        if let Some(flag) = cancel_flag.as_ref() {
                            if flag.load(Ordering::SeqCst) {
                                return;
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }, if cancel_flag.is_some() => {
                    return Err("工具执行被用户取消".to_string());
                }
            }
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct RecoverableApprovalRow {
    id: String,
    session_id: String,
    run_id: String,
    call_id: String,
    tool_name: String,
    resume_payload_json: String,
}

pub async fn recover_approved_pending_work_with_pool(
    pool: &SqlitePool,
    journal: &SessionJournalStore,
    registry: &ToolRegistry,
) -> Result<usize, String> {
    let rows = sqlx::query_as::<_, RecoverableApprovalRow>(
        "SELECT id, session_id, run_id, call_id, tool_name, resume_payload_json
         FROM approvals
         WHERE status = 'approved' AND resumed_at IS NULL
         ORDER BY resolved_at ASC, created_at ASC, id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("读取待恢复 approval 失败: {e}"))?;

    let mut recovered = 0usize;
    for row in rows {
        let now = Utc::now().to_rfc3339();
        let payload = serde_json::from_str::<ApprovalResumePayload>(&row.resume_payload_json)
            .unwrap_or_else(|_| ApprovalResumePayload {
                session_id: row.session_id.clone(),
                run_id: if row.run_id.trim().is_empty() {
                    None
                } else {
                    Some(row.run_id.clone())
                },
                call_id: row.call_id.clone(),
                tool_name: row.tool_name.clone(),
                input: serde_json::json!({}),
                work_dir: None,
                task_continuation: None,
            });

        if let Some(run_id) = payload
            .run_id
            .clone()
            .filter(|value| !value.trim().is_empty())
        {
            if tool_completed_event_exists_with_pool(
                pool,
                &payload.session_id,
                &run_id,
                &payload.call_id,
                &payload.tool_name,
            )
            .await?
            {
                mark_approval_resumed_with_pool(pool, &row.id).await?;
                recovered += 1;
                continue;
            }

            let tool_result = if let Some(tool) = registry.get(&payload.tool_name) {
                let ctx = ToolContext {
                    work_dir: payload.work_dir.as_deref().map(PathBuf::from),
                    allowed_tools: None,
                    session_id: None,
                    task_temp_dir: None,
                    execution_caps: None,
                    file_task_caps: None,
                };
                match tool.execute(payload.input.clone(), &ctx) {
                    Ok(output) => (output, false),
                    Err(error) => (error.to_string(), true),
                }
            } else {
                (
                    format!("恢复审批后执行工具失败：未找到工具 {}", payload.tool_name),
                    true,
                )
            };

            append_session_run_event_with_pool(
                pool,
                journal,
                &payload.session_id,
                SessionRunEvent::ToolCompleted {
                    run_id: run_id.clone(),
                    tool_name: payload.tool_name.clone(),
                    call_id: payload.call_id.clone(),
                    task_identity: None,
                    task_continuation: payload.task_continuation.clone(),
                    input: payload.input.clone(),
                    output: tool_result.0.clone(),
                    is_error: tool_result.1,
                },
            )
            .await?;

            append_session_run_event_with_pool(
                pool,
                journal,
                &payload.session_id,
                SessionRunEvent::RunFailed {
                    run_id,
                    error_kind: "approval_recovery".to_string(),
                    error_message:
                        "审批已在应用重启后恢复并补执行工具，请重新发送消息继续后续推理。"
                            .to_string(),
                    turn_state: None,
                },
            )
            .await?;
        }

        mark_approval_resumed_at_with_pool(pool, &row.id, &now).await?;
        recovered += 1;
    }

    Ok(recovered)
}

pub(crate) async fn mark_approved_tool_completion_resumed_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    run_id: &str,
    call_id: &str,
    tool_name: &str,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE approvals
         SET resumed_at = ?, updated_at = ?
         WHERE status = 'approved'
           AND resumed_at IS NULL
           AND session_id = ?
           AND run_id = ?
           AND call_id = ?
           AND tool_name = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(session_id.trim())
    .bind(run_id.trim())
    .bind(call_id.trim())
    .bind(tool_name.trim())
    .execute(pool)
    .await
    .map_err(|e| format!("标记 approval 工具完成失败: {e}"))?;

    Ok(())
}

async fn mark_approval_resumed_with_pool(
    pool: &SqlitePool,
    approval_id: &str,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    mark_approval_resumed_at_with_pool(pool, approval_id, &now).await
}

async fn mark_approval_resumed_at_with_pool(
    pool: &SqlitePool,
    approval_id: &str,
    resumed_at: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE approvals
         SET resumed_at = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(resumed_at)
    .bind(resumed_at)
    .bind(approval_id.trim())
    .execute(pool)
    .await
    .map_err(|e| format!("更新 approval 恢复时间失败: {e}"))?;

    Ok(())
}

async fn tool_completed_event_exists_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    run_id: &str,
    call_id: &str,
    tool_name: &str,
) -> Result<bool, String> {
    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT payload_json
         FROM session_run_events
         WHERE session_id = ?
           AND run_id = ?
           AND event_type = 'tool_completed'
         ORDER BY created_at ASC, id ASC",
    )
    .bind(session_id.trim())
    .bind(run_id.trim())
    .fetch_all(pool)
    .await
    .map_err(|e| format!("读取工具完成事件失败: {e}"))?;

    Ok(rows.into_iter().any(|(payload_json,)| {
        matches!(
            serde_json::from_str::<SessionRunEvent>(&payload_json),
            Ok(SessionRunEvent::ToolCompleted {
                call_id: event_call_id,
                tool_name: event_tool_name,
                ..
            }) if event_call_id == call_id.trim() && event_tool_name == tool_name.trim()
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::{recover_approved_pending_work_with_pool, ApprovalResumePayload};
    use crate::agent::{Tool, ToolContext, ToolRegistry};
    use crate::session_journal::{SessionJournalStore, SessionRunEvent};
    use anyhow::Result;
    use serde_json::{json, Value};
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tempfile::tempdir;

    struct CountingTool {
        executions: Arc<AtomicUsize>,
    }

    impl Tool for CountingTool {
        fn name(&self) -> &str {
            "counting_tool"
        }

        fn description(&self) -> &str {
            "counts executions"
        }

        fn input_schema(&self) -> Value {
            json!({})
        }

        fn execute(&self, _input: Value, _ctx: &ToolContext) -> Result<String> {
            self.executions.fetch_add(1, Ordering::SeqCst);
            Ok("counted".to_string())
        }
    }

    async fn setup_approval_recovery_pool() -> sqlx::SqlitePool {
        let db_dir = tempdir().expect("create db dir");
        let db_url = format!(
            "sqlite://{}?mode=rwc",
            db_dir.path().join("approval-recovery.db").to_string_lossy()
        );
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("connect sqlite");

        sqlx::query(
            "CREATE TABLE session_runs (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                user_message_id TEXT NOT NULL DEFAULT '',
                assistant_message_id TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'queued',
                buffered_text TEXT NOT NULL DEFAULT '',
                error_kind TEXT NOT NULL DEFAULT '',
                error_message TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create session_runs");

        sqlx::query(
            "CREATE TABLE session_run_events (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create session_run_events");

        sqlx::query(
            "CREATE TABLE approvals (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                call_id TEXT NOT NULL DEFAULT '',
                tool_name TEXT NOT NULL,
                input_json TEXT NOT NULL DEFAULT '{}',
                summary TEXT NOT NULL DEFAULT '',
                impact TEXT NOT NULL DEFAULT '',
                irreversible INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'pending',
                decision TEXT NOT NULL DEFAULT '',
                notify_targets_json TEXT NOT NULL DEFAULT '[]',
                resume_payload_json TEXT NOT NULL DEFAULT '{}',
                resolved_by_surface TEXT NOT NULL DEFAULT '',
                resolved_by_user TEXT NOT NULL DEFAULT '',
                resolved_at TEXT,
                resumed_at TEXT,
                expires_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create approvals");

        pool
    }

    #[tokio::test]
    async fn recovery_does_not_replay_when_tool_completed_event_already_exists() {
        let pool = setup_approval_recovery_pool().await;
        let journal_dir = tempdir().expect("create journal dir");
        let journal = SessionJournalStore::new(journal_dir.path().to_path_buf());
        let executions = Arc::new(AtomicUsize::new(0));
        let registry = ToolRegistry::new();
        registry.register(Arc::new(CountingTool {
            executions: Arc::clone(&executions),
        }));

        let payload = ApprovalResumePayload {
            session_id: "session-approval-recovery".to_string(),
            run_id: Some("run-approval-recovery".to_string()),
            call_id: "call-approval-recovery".to_string(),
            tool_name: "counting_tool".to_string(),
            input: json!({"value": 1}),
            work_dir: None,
            task_continuation: None,
        };

        sqlx::query(
            "INSERT INTO approvals (
                id, session_id, run_id, call_id, tool_name, input_json, summary, impact,
                irreversible, status, decision, notify_targets_json, resume_payload_json,
                resolved_by_surface, resolved_by_user, resolved_at, resumed_at, expires_at,
                created_at, updated_at
             ) VALUES (
                'approval-recovery-1', ?, ?, ?, ?, ?, 'Run counting tool', '',
                0, 'approved', 'allow_once', '[]', ?, 'desktop', 'tester',
                '2026-05-03T00:00:00Z', NULL, NULL,
                '2026-05-03T00:00:00Z', '2026-05-03T00:00:00Z'
             )",
        )
        .bind(&payload.session_id)
        .bind(payload.run_id.as_deref().unwrap_or_default())
        .bind(&payload.call_id)
        .bind(&payload.tool_name)
        .bind(payload.input.to_string())
        .bind(serde_json::to_string(&payload).expect("serialize resume payload"))
        .execute(&pool)
        .await
        .expect("insert approval");

        let completed_event = SessionRunEvent::ToolCompleted {
            run_id: payload.run_id.clone().expect("run id"),
            tool_name: payload.tool_name.clone(),
            call_id: payload.call_id.clone(),
            task_identity: None,
            task_continuation: None,
            input: payload.input.clone(),
            output: "already counted".to_string(),
            is_error: false,
        };
        sqlx::query(
            "INSERT INTO session_run_events (id, run_id, session_id, event_type, payload_json, created_at)
             VALUES ('event-tool-completed-1', ?, ?, 'tool_completed', ?, '2026-05-03T00:00:01Z')",
        )
        .bind(payload.run_id.as_deref().unwrap_or_default())
        .bind(&payload.session_id)
        .bind(serde_json::to_string(&completed_event).expect("serialize completed event"))
        .execute(&pool)
        .await
        .expect("insert completed event");

        let recovered = recover_approved_pending_work_with_pool(&pool, &journal, &registry)
            .await
            .expect("recover approvals");

        assert_eq!(recovered, 1);
        assert_eq!(
            executions.load(Ordering::SeqCst),
            0,
            "completed approvals must not replay tools during recovery"
        );
        let resumed_at = sqlx::query_as::<_, (Option<String>,)>(
            "SELECT resumed_at FROM approvals WHERE id = 'approval-recovery-1'",
        )
        .fetch_one(&pool)
        .await
        .expect("load approval")
        .0;
        assert!(resumed_at.is_some());
    }
}
