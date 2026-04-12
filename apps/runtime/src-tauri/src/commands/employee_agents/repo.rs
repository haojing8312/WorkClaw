use sqlx::{Row, Sqlite, SqlitePool, Transaction};

#[path = "profile_repo.rs"]
mod profile_repo;

#[path = "group_run_repo.rs"]
mod group_run_repo;

#[path = "session_repo.rs"]
mod session_repo;

#[path = "feishu_binding_repo.rs"]
mod feishu_binding_repo;

pub(crate) use profile_repo::{
    clear_default_employee_flag, delete_agent_employee_record, find_employee_db_id_by_employee_id,
    list_agent_employee_rows, list_skill_ids_for_employee, replace_employee_skill_bindings,
    upsert_agent_employee_record, AgentEmployeeRow, UpsertAgentEmployeeRecordInput,
};
pub(super) use group_run_repo::{
    cancel_group_run, clear_group_run_execute_waiting_state, complete_failed_group_run_step,
    employee_exists_for_reassignment, find_employee_session_seed_row,
    find_existing_session_skill_id, find_group_run_execute_step_context,
    find_group_run_finalize_state, find_group_run_review_state, find_group_run_snapshot_row,
    find_group_run_start_config, find_group_run_state, find_group_run_step_reassign_row,
    find_group_step_session_row, find_latest_assistant_message_content, find_model_config_row,
    find_pending_review_step, find_plan_revision_seed, find_recent_group_step_session_id,
    get_group_run_session_id, insert_group_run_assistant_message, insert_group_run_event,
    insert_group_run_record, insert_group_run_step_seed, insert_plan_revision_step,
    insert_session_message, insert_tx_session_message, list_failed_execute_assignees,
    list_failed_group_run_steps, list_group_run_event_snapshot_rows,
    list_group_run_execute_outputs, list_group_run_step_snapshot_rows,
    list_pending_execute_step_ids, list_session_message_rows, load_group_run_blocking_counts,
    mark_group_run_done_after_retry, mark_group_run_executing, mark_group_run_failed,
    mark_group_run_finalized, mark_group_run_review_approved, mark_group_run_review_rejected,
    mark_group_run_step_completed, mark_group_run_step_dispatched, mark_group_run_step_failed,
    mark_group_run_waiting_review, mark_review_step_completed, pause_group_run,
    reset_group_run_step_for_reassignment, resume_group_run, review_requested_event_exists,
    update_group_run_after_reassignment, GroupRunEventSnapshotRow, GroupRunStepSnapshotRow,
    PlanRevisionSeedRow,
};
pub(super) use session_repo::{
    find_latest_thread_session_id, find_recent_route_session_id, find_thread_session_record,
    insert_inbound_event_link, insert_session_seed, update_session_employee_id,
    upsert_thread_session_link, InboundEventLinkInput, SessionSeedInput, ThreadSessionLinkInput,
};
pub(super) use feishu_binding_repo::{
    count_feishu_bindings_for_agent, delete_displaced_default_feishu_bindings,
    delete_displaced_scoped_feishu_bindings, delete_feishu_bindings_for_agent,
    find_displaced_default_feishu_agent_ids, find_displaced_scoped_feishu_agent_ids,
    insert_feishu_binding, list_agent_scope_rows, InsertFeishuBindingInput,
};

pub(super) struct EmployeeAssociationRow {
    pub employee_id: String,
    pub role_id: String,
    pub openclaw_agent_id: String,
    pub enabled_scopes_json: String,
}

pub(super) struct EmployeeGroupEntryRow {
    pub entry_employee_id: String,
    pub coordinator_employee_id: String,
}

pub(super) async fn get_employee_association_row(
    pool: &SqlitePool,
    employee_db_id: &str,
) -> Result<Option<EmployeeAssociationRow>, String> {
    let row = sqlx::query(
        r#"
        SELECT employee_id, role_id, openclaw_agent_id, enabled_scopes_json
        FROM agent_employees
        WHERE id = ?
        "#,
    )
    .bind(employee_db_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|record| EmployeeAssociationRow {
        employee_id: record.try_get("employee_id").expect("association employee_id"),
        role_id: record.try_get("role_id").expect("association role_id"),
        openclaw_agent_id: record
            .try_get("openclaw_agent_id")
            .expect("association openclaw_agent_id"),
        enabled_scopes_json: record
            .try_get("enabled_scopes_json")
            .expect("association enabled_scopes_json"),
    }))
}

pub(super) async fn get_employee_group_entry_row(
    pool: &SqlitePool,
    group_id: &str,
) -> Result<Option<EmployeeGroupEntryRow>, String> {
    let row = sqlx::query(
        "SELECT COALESCE(entry_employee_id, ''), coordinator_employee_id
         FROM employee_groups
         WHERE id = ?",
    )
    .bind(group_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|record| EmployeeGroupEntryRow {
        entry_employee_id: record.try_get(0).expect("group entry row entry_employee_id"),
        coordinator_employee_id: record
            .try_get(1)
            .expect("group entry row coordinator_employee_id"),
    }))
}

pub(super) async fn update_employee_enabled_scopes(
    tx: &mut Transaction<'_, Sqlite>,
    employee_db_id: &str,
    enabled_scopes_json: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query("UPDATE agent_employees SET enabled_scopes_json = ?, updated_at = ? WHERE id = ?")
        .bind(enabled_scopes_json)
        .bind(now)
        .bind(employee_db_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn list_agent_employee_rows_orders_default_before_recent_updates() {
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
        .expect("create agent_employees table");

        for (id, employee_id, is_default, updated_at) in [
            ("emp-1", "employee-1", 0_i64, "2026-03-20T08:00:00Z"),
            ("emp-2", "employee-2", 1_i64, "2026-03-19T08:00:00Z"),
            ("emp-3", "employee-3", 0_i64, "2026-03-21T08:00:00Z"),
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
            .bind(format!("Name {employee_id}"))
            .bind(format!("role-{employee_id}"))
            .bind("persona")
            .bind("")
            .bind("")
            .bind("")
            .bind("primary-skill")
            .bind("C:/tmp")
            .bind("openclaw-agent")
            .bind(0_i64)
            .bind("[\"app\"]")
            .bind(1_i64)
            .bind(is_default)
            .bind(updated_at)
            .bind(updated_at)
            .execute(&pool)
            .await
            .expect("insert agent employee row");
        }

        let rows = super::profile_repo::list_agent_employee_rows(&pool)
            .await
            .expect("list rows");

        let ordered_employee_ids: Vec<_> = rows.into_iter().map(|row| row.employee_id).collect();
        assert_eq!(
            ordered_employee_ids,
            vec!["employee-2", "employee-3", "employee-1"]
        );
    }

    #[tokio::test]
    async fn group_run_repo_find_group_run_state_uses_plan_default_phase() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            r#"
            CREATE TABLE group_runs (
                id TEXT PRIMARY KEY,
                state TEXT NOT NULL,
                current_phase TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create group_runs table");

        sqlx::query("INSERT INTO group_runs (id, state, current_phase) VALUES (?, ?, NULL)")
            .bind("run-1")
            .bind("planning")
            .execute(&pool)
            .await
            .expect("insert group run row");

        let row = group_run_repo::find_group_run_state(&pool, "run-1")
            .await
            .expect("query group run state")
            .expect("group run state row");

        assert_eq!(row.state, "planning");
        assert_eq!(row.current_phase, "plan");
    }

    #[tokio::test]
    async fn session_repo_find_recent_route_session_id_prefers_latest_update() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            r#"
            CREATE TABLE sessions (
                id TEXT PRIMARY KEY NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create sessions table");

        sqlx::query(
            r#"
            CREATE TABLE im_thread_sessions (
                thread_id TEXT NOT NULL,
                employee_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                route_session_key TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create im_thread_sessions table");

        for session_id in ["session-old", "session-new"] {
            sqlx::query("INSERT INTO sessions (id) VALUES (?)")
                .bind(session_id)
                .execute(&pool)
                .await
                .expect("insert session row");
        }

        for (session_id, updated_at) in [
            ("session-old", "2026-03-20T08:00:00Z"),
            ("session-new", "2026-03-21T08:00:00Z"),
        ] {
            sqlx::query(
                r#"
                INSERT INTO im_thread_sessions (
                    thread_id, employee_id, session_id, route_session_key, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind("thread-1")
            .bind("employee-1")
            .bind(session_id)
            .bind("channel:peer-1")
            .bind(updated_at)
            .bind(updated_at)
            .execute(&pool)
            .await
            .expect("insert thread session row");
        }

        let session_id = session_repo::find_recent_route_session_id(
            &pool,
            "employee-1",
            "channel:peer-1",
        )
        .await
        .expect("find recent route session id");

        assert_eq!(session_id.as_deref(), Some("session-new"));
    }

    #[tokio::test]
    async fn feishu_binding_repo_insert_binding_makes_agent_count_visible() {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

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
                connector_meta_json TEXT NOT NULL,
                priority INTEGER NOT NULL,
                enabled INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create im_routing_bindings table");

        let mut tx = pool.begin().await.expect("begin transaction");
        feishu_binding_repo::insert_feishu_binding(
            &mut tx,
            &feishu_binding_repo::InsertFeishuBindingInput {
                id: "binding-1",
                agent_id: "agent-1",
                peer_kind: "group",
                peer_id: "",
                connector_meta_json: "{\"connector_id\":\"feishu\"}",
                priority: 0,
                now: "2026-03-24T08:00:00Z",
            },
        )
        .await
        .expect("insert feishu binding");

        let count = feishu_binding_repo::count_feishu_bindings_for_agent(&mut tx, "agent-1")
            .await
            .expect("count feishu bindings");
        tx.commit().await.expect("commit transaction");

        assert_eq!(count, 1);
    }
}
