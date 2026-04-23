use super::repo::{
    clear_group_run_execute_waiting_state, find_group_run_execute_step_context,
    insert_group_run_event, mark_group_run_executing, mark_group_run_failed,
    mark_group_run_step_completed, mark_group_run_step_dispatched, mark_group_run_step_failed,
};
use sqlx::SqlitePool;

#[path = "profile_service.rs"]
mod profile_service;

#[path = "feishu_service.rs"]
mod feishu_service;

#[path = "routing_service.rs"]
mod routing_service;

#[path = "session_service.rs"]
mod session_service;

#[path = "group_run_service.rs"]
mod group_run_service;

#[path = "group_run_snapshot_service.rs"]
mod group_run_snapshot_service;

#[path = "group_run_action_service.rs"]
mod group_run_action_service;

#[path = "group_run_progress_service.rs"]
mod group_run_progress_service;

#[path = "group_run_execution_service.rs"]
mod group_run_execution_service;

pub(crate) use feishu_service::save_feishu_employee_association_with_pool;
pub(crate) use group_run_action_service::{
    reassign_group_run_step_with_pool, retry_employee_group_run_failed_steps_with_pool,
    review_group_run_step_with_pool,
};
pub(super) use group_run_execution_service::{
    ensure_group_step_session_with_pool, execute_group_step_in_employee_context_with_pool,
    start_employee_group_run_internal_with_pool,
};
pub(super) use group_run_progress_service::{
    list_pending_execute_steps_for_continue, load_group_run_continue_state,
    maybe_finalize_group_run_with_pool, maybe_mark_group_run_waiting_review,
};
pub(crate) use group_run_service::{
    cancel_employee_group_run_with_pool, pause_employee_group_run_with_pool,
    resume_employee_group_run_with_pool,
};
pub(crate) use group_run_snapshot_service::{
    get_employee_group_run_snapshot_by_run_id_with_pool, get_employee_group_run_snapshot_with_pool,
};
pub(crate) use profile_service::{
    delete_agent_employee_with_pool, list_agent_employees_with_pool,
    normalize_enabled_scopes_for_storage, resolve_agent_employee_for_agent_id_with_pool,
    resolve_employee_agent_id, upsert_agent_employee_with_pool,
};
pub(crate) use routing_service::{
    resolve_target_employees_for_event, resolve_team_entry_employee_for_event_with_pool,
};
pub(crate) use session_service::{
    bridge_inbound_event_to_employee_sessions_with_pool,
    ensure_employee_sessions_for_event_with_pool, link_inbound_event_to_session_with_pool,
};

pub(super) async fn load_group_run_execute_step_context(
    pool: &SqlitePool,
    step_id: &str,
) -> Result<
    (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    ),
    String,
> {
    let row = find_group_run_execute_step_context(pool, step_id)
        .await?
        .ok_or_else(|| "group run step not found".to_string())?;
    if row.step_type != "execute" {
        return Err("only execute steps can be run".to_string());
    }
    Ok((
        row.step_id,
        row.run_id,
        row.assignee_employee_id,
        row.dispatch_source_employee_id,
        row.existing_session_id,
        row.step_input,
        row.user_goal,
        row.step_type,
    ))
}

pub(super) async fn mark_group_run_step_dispatched_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    step_id: &str,
    session_id: &str,
    assignee_employee_id: &str,
    dispatch_source_employee_id: &str,
    now: &str,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    mark_group_run_step_dispatched(&mut tx, step_id, session_id, now).await?;
    insert_group_run_event(
        &mut tx,
        run_id,
        step_id,
        "step_dispatched",
        &serde_json::json!({
            "step_id": step_id,
            "session_id": session_id,
            "assignee_employee_id": assignee_employee_id,
            "dispatch_source_employee_id": dispatch_source_employee_id,
        })
        .to_string(),
        now,
    )
    .await?;
    mark_group_run_executing(&mut tx, run_id, assignee_employee_id, now).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn mark_group_run_step_failed_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    step_id: &str,
    session_id: &str,
    assignee_employee_id: &str,
    dispatch_source_employee_id: &str,
    error: &str,
    now: &str,
) -> Result<(), String> {
    let failed_summary = error.chars().take(120).collect::<String>();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    mark_group_run_step_failed(&mut tx, step_id, error, &failed_summary, session_id, now).await?;
    insert_group_run_event(
        &mut tx,
        run_id,
        step_id,
        "step_failed",
        &serde_json::json!({
            "step_id": step_id,
            "session_id": session_id,
            "status": "failed",
            "error": error,
            "assignee_employee_id": assignee_employee_id,
            "dispatch_source_employee_id": dispatch_source_employee_id,
        })
        .to_string(),
        now,
    )
    .await?;
    mark_group_run_failed(&mut tx, run_id, assignee_employee_id, error, now).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn mark_group_run_step_completed_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    step_id: &str,
    session_id: &str,
    assignee_employee_id: &str,
    dispatch_source_employee_id: &str,
    output: &str,
    now: &str,
) -> Result<(), String> {
    let output_summary = output.chars().take(120).collect::<String>();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    mark_group_run_step_completed(&mut tx, step_id, output, &output_summary, session_id, now)
        .await?;
    insert_group_run_event(
        &mut tx,
        run_id,
        step_id,
        "step_completed",
        &serde_json::json!({
            "step_id": step_id,
            "session_id": session_id,
            "status": "completed",
            "output_summary": output_summary,
            "assignee_employee_id": assignee_employee_id,
            "dispatch_source_employee_id": dispatch_source_employee_id,
        })
        .to_string(),
        now,
    )
    .await?;
    clear_group_run_execute_waiting_state(&mut tx, run_id, now).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::repo::AgentEmployeeRow;
    use super::super::SaveFeishuEmployeeAssociationInput;
    use super::feishu_service::save_feishu_employee_association_with_pool;
    use super::group_run_action_service::{
        reassign_group_run_step_with_pool, retry_employee_group_run_failed_steps_with_pool,
        review_group_run_step_with_pool,
    };
    use super::group_run_service::resume_employee_group_run_with_pool;
    use super::group_run_snapshot_service::get_group_run_session_id_with_pool;
    use super::profile_service::build_agent_employee;
    use super::routing_service::resolve_target_employees_for_event;
    use super::session_service::ensure_employee_sessions_for_event_with_pool;
    use crate::im::types::{ImEvent, ImEventType};
    use sqlx::SqlitePool;

    #[test]
    fn build_agent_employee_falls_back_to_role_id_and_default_scope() {
        let employee = build_agent_employee(
            AgentEmployeeRow {
                id: "emp-1".to_string(),
                employee_id: String::new(),
                name: "Planner".to_string(),
                role_id: "planner".to_string(),
                persona: "Owns planning".to_string(),
                feishu_open_id: String::new(),
                feishu_app_id: String::new(),
                feishu_app_secret: String::new(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: "D:/work".to_string(),
                openclaw_agent_id: "planner".to_string(),
                routing_priority: 100,
                enabled_scopes_json: "not-json".to_string(),
                enabled: true,
                is_default: true,
                created_at: "2026-03-23T00:00:00Z".to_string(),
                updated_at: "2026-03-23T00:00:00Z".to_string(),
            },
            vec!["skill-a".to_string(), "skill-b".to_string()],
        );

        assert_eq!(employee.employee_id, "planner");
        assert_eq!(employee.agent_id(), "planner");
        assert_eq!(employee.enabled_scopes, vec!["app".to_string()]);
        assert_eq!(
            employee.skill_ids,
            vec!["skill-a".to_string(), "skill-b".to_string()]
        );
    }

    #[test]
    fn agent_employee_prefers_agent_id_but_keeps_employee_alias_compatible() {
        let employee = build_agent_employee(
            AgentEmployeeRow {
                id: "emp-2".to_string(),
                employee_id: "legacy-employee".to_string(),
                name: "Ops".to_string(),
                role_id: "ops".to_string(),
                persona: "Handles operations".to_string(),
                feishu_open_id: String::new(),
                feishu_app_id: String::new(),
                feishu_app_secret: String::new(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: "D:/work".to_string(),
                openclaw_agent_id: "agent-ops".to_string(),
                routing_priority: 100,
                enabled_scopes_json: "[\"app\"]".to_string(),
                enabled: true,
                is_default: false,
                created_at: "2026-03-23T00:00:00Z".to_string(),
                updated_at: "2026-03-23T00:00:00Z".to_string(),
            },
            Vec::new(),
        );

        assert_eq!(employee.employee_id, "legacy-employee");
        assert_eq!(employee.agent_id(), "agent-ops");
    }

    #[tokio::test]
    async fn save_feishu_employee_association_rejects_invalid_mode() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        let err = save_feishu_employee_association_with_pool(
            &pool,
            SaveFeishuEmployeeAssociationInput {
                employee_db_id: "employee-db-id".to_string(),
                enabled: true,
                mode: "unsupported".to_string(),
                peer_kind: String::new(),
                peer_id: String::new(),
                priority: 10,
            },
        )
        .await
        .expect_err("invalid mode should fail before db lookup");

        assert_eq!(err, "mode must be default or scoped");
    }

    #[tokio::test]
    async fn resolve_target_employees_for_event_prefers_explicit_role_match() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            r#"
            CREATE TABLE agent_employees (
                id TEXT PRIMARY KEY NOT NULL,
                employee_id TEXT NOT NULL,
                name TEXT NOT NULL,
                role_id TEXT NOT NULL,
                persona TEXT NOT NULL,
                feishu_open_id TEXT NOT NULL,
                feishu_app_id TEXT NOT NULL,
                feishu_app_secret TEXT NOT NULL,
                primary_skill_id TEXT NOT NULL,
                default_work_dir TEXT NOT NULL,
                openclaw_agent_id TEXT NOT NULL,
                routing_priority INTEGER NOT NULL,
                enabled_scopes_json TEXT NOT NULL,
                enabled INTEGER NOT NULL,
                is_default INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create agent_employees");

        sqlx::query(
            r#"
            CREATE TABLE agent_employee_skills (
                employee_id TEXT NOT NULL,
                skill_id TEXT NOT NULL,
                sort_order INTEGER NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create agent_employee_skills");

        for (id, employee_id, name, role_id, is_default) in [
            (
                "emp-default",
                "default-worker",
                "Default Worker",
                "default-role",
                1_i64,
            ),
            (
                "emp-target",
                "target-worker",
                "Target Worker",
                "target-role",
                0_i64,
            ),
        ] {
            sqlx::query(
                r#"
                INSERT INTO agent_employees (
                    id, employee_id, name, role_id, persona, feishu_open_id, feishu_app_id,
                    feishu_app_secret, primary_skill_id, default_work_dir, openclaw_agent_id,
                    routing_priority, enabled_scopes_json, enabled, is_default, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(id)
            .bind(employee_id)
            .bind(name)
            .bind(role_id)
            .bind("persona")
            .bind("")
            .bind("")
            .bind("")
            .bind("builtin-general")
            .bind("D:/work")
            .bind(employee_id)
            .bind(100_i64)
            .bind("[\"feishu\"]")
            .bind(1_i64)
            .bind(is_default)
            .bind("2026-03-23T00:00:00Z")
            .bind("2026-03-23T00:00:00Z")
            .execute(&pool)
            .await
            .expect("insert employee");
        }

        let targeted = resolve_target_employees_for_event(
            &pool,
            &ImEvent {
                channel: "feishu".to_string(),
                event_type: ImEventType::MessageCreated,
                thread_id: "thread-1".to_string(),
                event_id: None,
                message_id: None,
                text: Some("hello team".to_string()),
                role_id: Some("target-role".to_string()),
                account_id: None,
                tenant_id: None,
                sender_id: None,
                chat_type: None,
                conversation_id: None,
                base_conversation_id: None,
                parent_conversation_candidates: Vec::new(),
                conversation_scope: None,
            },
        )
        .await
        .expect("resolve employees");

        assert_eq!(targeted.len(), 1);
        assert_eq!(targeted[0].employee_id, "target-worker");
    }

    #[tokio::test]
    async fn ensure_employee_sessions_for_event_returns_empty_when_no_employee_matches() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            r#"
            CREATE TABLE agent_employees (
                id TEXT PRIMARY KEY NOT NULL,
                employee_id TEXT NOT NULL,
                name TEXT NOT NULL,
                role_id TEXT NOT NULL,
                persona TEXT NOT NULL,
                feishu_open_id TEXT NOT NULL,
                feishu_app_id TEXT NOT NULL,
                feishu_app_secret TEXT NOT NULL,
                primary_skill_id TEXT NOT NULL,
                default_work_dir TEXT NOT NULL,
                openclaw_agent_id TEXT NOT NULL,
                routing_priority INTEGER NOT NULL,
                enabled_scopes_json TEXT NOT NULL,
                enabled INTEGER NOT NULL,
                is_default INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create agent_employees");

        sqlx::query(
            r#"
            CREATE TABLE im_routing_bindings (
                id TEXT PRIMARY KEY NOT NULL,
                agent_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                account_id TEXT NOT NULL,
                peer_kind TEXT NOT NULL,
                peer_id TEXT NOT NULL,
                guild_id TEXT NOT NULL,
                team_id TEXT NOT NULL,
                role_ids_json TEXT NOT NULL,
                connector_meta_json TEXT,
                priority INTEGER NOT NULL,
                enabled INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create im_routing_bindings");

        let sessions = ensure_employee_sessions_for_event_with_pool(
            &pool,
            &ImEvent {
                channel: "feishu".to_string(),
                event_type: ImEventType::MessageCreated,
                thread_id: "thread-empty".to_string(),
                event_id: None,
                message_id: None,
                text: Some("hello".to_string()),
                role_id: None,
                account_id: None,
                tenant_id: None,
                sender_id: None,
                chat_type: None,
                conversation_id: None,
                base_conversation_id: None,
                parent_conversation_candidates: Vec::new(),
                conversation_scope: None,
            },
        )
        .await
        .expect("empty employee set should not fail");

        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn resume_employee_group_run_requires_paused_state() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            r#"
            CREATE TABLE group_runs (
                id TEXT PRIMARY KEY NOT NULL,
                group_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                user_goal TEXT NOT NULL,
                state TEXT NOT NULL,
                current_round INTEGER NOT NULL,
                current_phase TEXT NOT NULL,
                entry_session_id TEXT NOT NULL,
                main_employee_id TEXT NOT NULL,
                review_round INTEGER NOT NULL,
                status_reason TEXT NOT NULL,
                template_version TEXT NOT NULL,
                waiting_for_employee_id TEXT NOT NULL,
                waiting_for_user INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create employee_group_runs");

        sqlx::query(
            r#"
            CREATE TABLE group_run_events (
                id TEXT PRIMARY KEY NOT NULL,
                run_id TEXT NOT NULL,
                step_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create group_run_events");

        sqlx::query(
            r#"
            INSERT INTO group_runs (
                id, group_id, session_id, user_goal, state, current_round, current_phase,
                entry_session_id, main_employee_id, review_round, status_reason, template_version,
                waiting_for_employee_id, waiting_for_user, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind("run-1")
        .bind("group-1")
        .bind("session-1")
        .bind("Ship feature")
        .bind("planning")
        .bind(1_i64)
        .bind("plan")
        .bind("session-1")
        .bind("coordinator-1")
        .bind(0_i64)
        .bind("")
        .bind("")
        .bind("")
        .bind(0_i64)
        .bind("2026-03-23T00:00:00Z")
        .bind("2026-03-23T00:00:00Z")
        .execute(&pool)
        .await
        .expect("insert group run");

        let err = resume_employee_group_run_with_pool(&pool, "run-1")
            .await
            .expect_err("non-paused run should be rejected");

        assert_eq!(err, "group run is not paused");
    }

    #[tokio::test]
    async fn get_group_run_session_id_returns_not_found_for_missing_run() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            r#"
            CREATE TABLE group_runs (
                id TEXT PRIMARY KEY NOT NULL,
                session_id TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create group_runs");

        let err = get_group_run_session_id_with_pool(&pool, "missing-run")
            .await
            .expect_err("missing run should fail");

        assert_eq!(err, "group run not found");
    }

    #[tokio::test]
    async fn load_group_run_continue_state_requires_run_id() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        let err = super::group_run_progress_service::load_group_run_continue_state(&pool, "   ")
            .await
            .expect_err("blank run id should fail");

        assert_eq!(err, "run_id is required");
    }

    #[tokio::test]
    async fn retry_employee_group_run_failed_steps_requires_failed_rows() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            "CREATE TABLE group_run_steps (id TEXT PRIMARY KEY NOT NULL, run_id TEXT NOT NULL, status TEXT NOT NULL, output TEXT NOT NULL)"
        )
        .execute(&pool)
        .await
        .expect("create group_run_steps");

        let err = retry_employee_group_run_failed_steps_with_pool(&pool, "run-empty")
            .await
            .expect_err("retry should reject when no failed rows exist");

        assert_eq!(err, "no failed steps to retry");
    }

    #[tokio::test]
    async fn reassign_group_run_step_requires_assignee() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        let err = reassign_group_run_step_with_pool(&pool, "step-1", "   ")
            .await
            .expect_err("blank assignee should fail");

        assert_eq!(err, "assignee_employee_id is required");
    }

    #[tokio::test]
    async fn review_group_run_step_requires_valid_action() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        let err = review_group_run_step_with_pool(&pool, "run-1", "hold", "comment")
            .await
            .expect_err("unsupported action should fail");

        assert_eq!(err, "review action must be approve or reject");
    }
}
