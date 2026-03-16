mod helpers;

use runtime_lib::agent::ToolRegistry;
use runtime_lib::approval_bus::approval_bus_rollout_enabled_with_pool;
use runtime_lib::approval_bus::recover_approved_pending_work_with_pool;
use runtime_lib::approval_bus::{ApprovalDecision, ApprovalManager, ApprovalResolveResult};
use runtime_lib::approval_rules::{
    find_matching_approval_rule_with_pool, list_approval_rules_with_pool,
};
use runtime_lib::commands::approvals::list_pending_approvals_with_pool;
use runtime_lib::commands::session_runs::{
    append_session_run_event_with_pool, list_session_runs_with_pool,
};
use runtime_lib::session_journal::{SessionJournalStore, SessionRunEvent, SessionRunStatus};
use serde_json::json;

#[tokio::test]
async fn approval_records_persist_and_project_waiting_status() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let journal_dir = tempfile::tempdir().expect("create journal dir");
    let journal = SessionJournalStore::new(journal_dir.path().to_path_buf());

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-approval",
        SessionRunEvent::RunStarted {
            run_id: "run-approval".into(),
            user_message_id: "user-approval".into(),
        },
    )
    .await
    .expect("append run started");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-approval",
        SessionRunEvent::ApprovalRequested {
            run_id: "run-approval".into(),
            approval_id: "approval-1".into(),
            tool_name: "file_delete".into(),
            call_id: "call-1".into(),
            input: json!({ "path": "E:\\workspace\\danger.txt", "recursive": true }),
            summary: "将递归删除 E:\\workspace\\danger.txt".into(),
            impact: Some("该操作不可逆，删除后无法自动恢复。".into()),
            irreversible: true,
        },
    )
    .await
    .expect("append approval requested");

    let runs = list_session_runs_with_pool(&pool, "sess-approval")
        .await
        .expect("list session runs");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, "run-approval");
    assert_eq!(runs[0].status, "waiting_approval");

    let journal_state = journal
        .read_state("sess-approval")
        .await
        .expect("read journal state");
    assert_eq!(
        journal_state.current_run_id.as_deref(),
        Some("run-approval")
    );
    assert_eq!(
        journal_state.runs[0].status,
        SessionRunStatus::WaitingApproval
    );

    let (approval_status, approval_tool, approval_summary): (String, String, String) =
        sqlx::query_as(
            "SELECT status, tool_name, summary
         FROM approvals
         WHERE id = ?",
        )
        .bind("approval-1")
        .fetch_one(&pool)
        .await
        .expect("load approval row");
    assert_eq!(approval_status, "pending");
    assert_eq!(approval_tool, "file_delete");
    assert_eq!(approval_summary, "将递归删除 E:\\workspace\\danger.txt");

    let (event_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM session_run_events
         WHERE run_id = ? AND event_type = ?",
    )
    .bind("run-approval")
    .bind("approval_requested")
    .fetch_one(&pool)
    .await
    .expect("count approval events");
    assert_eq!(event_count, 1);

    let pending = list_pending_approvals_with_pool(&pool, Some("sess-approval"))
        .await
        .expect("list pending approvals");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].approval_id, "approval-1");
    assert_eq!(pending[0].tool_name, "file_delete");
}

#[tokio::test]
async fn approval_bus_rollout_flag_defaults_true_and_allows_disable() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let enabled_by_default = approval_bus_rollout_enabled_with_pool(&pool)
        .await
        .expect("load default rollout flag");
    assert!(enabled_by_default);

    sqlx::query("INSERT OR REPLACE INTO app_settings (key, value) VALUES (?, ?)")
        .bind("approval_bus_v1")
        .bind("false")
        .execute(&pool)
        .await
        .expect("disable approval bus");

    let disabled = approval_bus_rollout_enabled_with_pool(&pool)
        .await
        .expect("load disabled rollout flag");
    assert!(!disabled);
}

#[tokio::test]
async fn approval_manager_allows_first_resolver_only() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let manager = ApprovalManager::default();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO approvals (
            id, session_id, run_id, call_id, tool_name, input_json, summary, impact,
            irreversible, status, decision, notify_targets_json, resume_payload_json,
            resolved_by_surface, resolved_by_user, resolved_at, resumed_at, expires_at,
            created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL, ?, ?)",
    )
    .bind("approval-cas")
    .bind("sess-approval")
    .bind("run-approval")
    .bind("call-approval")
    .bind("file_delete")
    .bind("{}")
    .bind("删除危险目录")
    .bind("")
    .bind(1_i64)
    .bind("pending")
    .bind("")
    .bind("[]")
    .bind("{}")
    .bind("")
    .bind("")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .expect("insert pending approval");

    let first = manager
        .resolve_with_pool(
            &pool,
            "approval-cas",
            ApprovalDecision::AllowOnce,
            "desktop",
            "user-desktop",
        )
        .await
        .expect("resolve approval first time");
    assert_eq!(
        first,
        ApprovalResolveResult::Applied {
            approval_id: "approval-cas".into(),
            status: "approved".into(),
            decision: ApprovalDecision::AllowOnce,
        }
    );

    let second = manager
        .resolve_with_pool(
            &pool,
            "approval-cas",
            ApprovalDecision::Deny,
            "feishu",
            "user-feishu",
        )
        .await
        .expect("resolve approval second time");
    assert_eq!(
        second,
        ApprovalResolveResult::AlreadyResolved {
            approval_id: "approval-cas".into(),
            status: "approved".into(),
            decision: Some(ApprovalDecision::AllowOnce),
        }
    );

    let (status, decision, surface, user_id): (String, String, String, String) = sqlx::query_as(
        "SELECT status, decision, resolved_by_surface, resolved_by_user
         FROM approvals
         WHERE id = ?",
    )
    .bind("approval-cas")
    .fetch_one(&pool)
    .await
    .expect("load resolved approval");
    assert_eq!(status, "approved");
    assert_eq!(decision, "allow_once");
    assert_eq!(surface, "desktop");
    assert_eq!(user_id, "user-desktop");
}

#[tokio::test]
async fn allow_always_creates_reusable_rule_and_skips_reapproval() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let manager = ApprovalManager::default();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO approvals (
            id, session_id, run_id, call_id, tool_name, input_json, summary, impact,
            irreversible, status, decision, notify_targets_json, resume_payload_json,
            resolved_by_surface, resolved_by_user, resolved_at, resumed_at, expires_at,
            created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL, ?, ?)",
    )
    .bind("approval-rule-file-delete")
    .bind("sess-rule")
    .bind("run-rule")
    .bind("call-rule-file-delete")
    .bind("file_delete")
    .bind(r#"{"path":"E:\\workspace\\danger.txt","recursive":true}"#)
    .bind("删除危险目录")
    .bind("目录会被永久删除")
    .bind(1_i64)
    .bind("pending")
    .bind("")
    .bind("[]")
    .bind("{}")
    .bind("")
    .bind("")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .expect("insert file_delete approval");

    sqlx::query(
        "INSERT INTO approvals (
            id, session_id, run_id, call_id, tool_name, input_json, summary, impact,
            irreversible, status, decision, notify_targets_json, resume_payload_json,
            resolved_by_surface, resolved_by_user, resolved_at, resumed_at, expires_at,
            created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL, ?, ?)",
    )
    .bind("approval-rule-bash")
    .bind("sess-rule")
    .bind("run-rule")
    .bind("call-rule-bash")
    .bind("bash")
    .bind(r#"{"command":"Remove-Item -Recurse C:\\temp\\danger"}"#)
    .bind("执行危险 bash 删除命令")
    .bind("命令会递归删除目录")
    .bind(1_i64)
    .bind("pending")
    .bind("")
    .bind("[]")
    .bind("{}")
    .bind("")
    .bind("")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .expect("insert bash approval");

    manager
        .resolve_with_pool(
            &pool,
            "approval-rule-file-delete",
            ApprovalDecision::AllowAlways,
            "desktop",
            "user-desktop",
        )
        .await
        .expect("resolve file_delete approval as allow_always");
    manager
        .resolve_with_pool(
            &pool,
            "approval-rule-bash",
            ApprovalDecision::AllowAlways,
            "feishu",
            "ou_approver",
        )
        .await
        .expect("resolve bash approval as allow_always");

    let rules = list_approval_rules_with_pool(&pool)
        .await
        .expect("list approval rules");
    assert_eq!(rules.len(), 2);

    let matched_delete = find_matching_approval_rule_with_pool(
        &pool,
        "file_delete",
        &json!({
            "path": "E:\\workspace\\danger.txt",
            "recursive": true
        }),
    )
    .await
    .expect("match file_delete rule");
    assert!(matched_delete.is_some());

    let unmatched_delete = find_matching_approval_rule_with_pool(
        &pool,
        "file_delete",
        &json!({
            "path": "E:\\workspace\\other.txt",
            "recursive": true
        }),
    )
    .await
    .expect("mismatch file_delete rule");
    assert!(unmatched_delete.is_none());

    let matched_bash = find_matching_approval_rule_with_pool(
        &pool,
        "bash",
        &json!({
            "command": "Remove-Item -Recurse C:\\temp\\danger"
        }),
    )
    .await
    .expect("match bash rule");
    assert!(matched_bash.is_some());

    let unmatched_bash = find_matching_approval_rule_with_pool(
        &pool,
        "bash",
        &json!({
            "command": "Remove-Item -Recurse C:\\temp\\other"
        }),
    )
    .await
    .expect("mismatch bash rule");
    assert!(unmatched_bash.is_none());
}

#[tokio::test]
async fn approved_pending_work_resumes_after_restart() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let journal_dir = tempfile::tempdir().expect("create journal dir");
    let journal = SessionJournalStore::new(journal_dir.path().to_path_buf());
    let registry = ToolRegistry::with_standard_tools();
    let work_dir = tempfile::tempdir().expect("create work dir");
    let target_dir = work_dir.path().join("danger");
    std::fs::create_dir_all(target_dir.join("nested")).expect("create nested dir");
    std::fs::write(target_dir.join("nested").join("file.txt"), "danger").expect("seed file");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-restart",
        SessionRunEvent::RunStarted {
            run_id: "run-restart".into(),
            user_message_id: "user-restart".into(),
        },
    )
    .await
    .expect("append run started");

    append_session_run_event_with_pool(
        &pool,
        &journal,
        "sess-restart",
        SessionRunEvent::ApprovalRequested {
            run_id: "run-restart".into(),
            approval_id: "approval-approved".into(),
            tool_name: "file_delete".into(),
            call_id: "call-approved".into(),
            input: json!({
                "path": target_dir.to_string_lossy().to_string(),
                "recursive": true
            }),
            summary: "删除危险目录".into(),
            impact: Some("目录会被永久删除".into()),
            irreversible: true,
        },
    )
    .await
    .expect("append approval requested");

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE approvals
         SET status = 'approved',
             decision = 'allow_once',
             resolved_by_surface = 'desktop',
             resolved_by_user = 'user-desktop',
             resolved_at = ?,
             resume_payload_json = ?
         WHERE id = ?",
    )
    .bind(&now)
    .bind(
        json!({
            "session_id": "sess-restart",
            "run_id": "run-restart",
            "call_id": "call-approved",
            "tool_name": "file_delete",
            "input": {
                "path": target_dir.to_string_lossy().to_string(),
                "recursive": true
            },
            "work_dir": work_dir.path().to_string_lossy().to_string()
        })
        .to_string(),
    )
    .bind("approval-approved")
    .execute(&pool)
    .await
    .expect("mark approval approved");

    sqlx::query(
        "INSERT INTO approvals (
            id, session_id, run_id, call_id, tool_name, input_json, summary, impact,
            irreversible, status, decision, notify_targets_json, resume_payload_json,
            resolved_by_surface, resolved_by_user, resolved_at, resumed_at, expires_at,
            created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL, ?, ?)",
    )
    .bind("approval-pending")
    .bind("sess-restart")
    .bind("run-restart")
    .bind("call-pending")
    .bind("file_delete")
    .bind("{}")
    .bind("另一个待审批目录")
    .bind("")
    .bind(1_i64)
    .bind("pending")
    .bind("")
    .bind("[]")
    .bind("{}")
    .bind("")
    .bind("")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .expect("insert unrelated pending approval");

    let recovered = recover_approved_pending_work_with_pool(&pool, &journal, &registry)
        .await
        .expect("recover approved approvals");
    assert_eq!(recovered, 1);
    assert!(
        !target_dir.exists(),
        "approved tool should replay after restart recovery"
    );

    let (resumed_at,): (Option<String>,) = sqlx::query_as(
        "SELECT resumed_at
         FROM approvals
         WHERE id = ?",
    )
    .bind("approval-approved")
    .fetch_one(&pool)
    .await
    .expect("load resumed_at");
    assert!(resumed_at.is_some());

    let runs = list_session_runs_with_pool(&pool, "sess-restart")
        .await
        .expect("list recovered runs");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "failed");
    assert_eq!(runs[0].error_kind.as_deref(), Some("approval_recovery"));

    let (tool_completed_count, run_failed_count): (i64, i64) = sqlx::query_as(
        "SELECT
            SUM(CASE WHEN event_type = 'tool_completed' THEN 1 ELSE 0 END),
            SUM(CASE WHEN event_type = 'run_failed' THEN 1 ELSE 0 END)
         FROM session_run_events
         WHERE run_id = ?",
    )
    .bind("run-restart")
    .fetch_one(&pool)
    .await
    .expect("count recovery events");
    assert_eq!(tool_completed_count, 1);
    assert_eq!(run_failed_count, 1);

    let pending = list_pending_approvals_with_pool(&pool, Some("sess-restart"))
        .await
        .expect("list pending approvals after recovery");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].approval_id, "approval-pending");
}
