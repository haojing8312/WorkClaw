use sqlx::{Row, Sqlite, SqlitePool, Transaction};

#[derive(Debug, Clone)]
pub(super) struct AgentEmployeeRow {
    pub id: String,
    pub employee_id: String,
    pub name: String,
    pub role_id: String,
    pub persona: String,
    pub feishu_open_id: String,
    pub feishu_app_id: String,
    pub feishu_app_secret: String,
    pub primary_skill_id: String,
    pub default_work_dir: String,
    pub openclaw_agent_id: String,
    pub routing_priority: i64,
    pub enabled_scopes_json: String,
    pub enabled: bool,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

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

pub(super) struct ThreadSessionRecord {
    pub session_id: String,
    pub session_exists: bool,
}

pub(super) struct SessionSeedInput<'a> {
    pub id: &'a str,
    pub skill_id: &'a str,
    pub title: &'a str,
    pub created_at: &'a str,
    pub model_id: &'a str,
    pub work_dir: &'a str,
    pub employee_id: &'a str,
}

pub(super) struct ThreadSessionLinkInput<'a> {
    pub thread_id: &'a str,
    pub employee_db_id: &'a str,
    pub session_id: &'a str,
    pub route_session_key: &'a str,
    pub created_at: &'a str,
    pub updated_at: &'a str,
}

pub(super) struct InboundEventLinkInput<'a> {
    pub id: &'a str,
    pub thread_id: &'a str,
    pub session_id: &'a str,
    pub employee_db_id: &'a str,
    pub im_event_id: &'a str,
    pub im_message_id: &'a str,
    pub created_at: &'a str,
}

pub(super) struct GroupRunStateRow {
    pub state: String,
    pub current_phase: String,
}

pub(super) struct FailedGroupRunStepRow {
    pub step_id: String,
    pub output: String,
}

pub(super) struct GroupRunStepReassignRow {
    pub run_id: String,
    pub status: String,
    pub step_type: String,
    pub dispatch_source_employee_id: String,
    pub previous_assignee_employee_id: String,
    pub previous_output_summary: String,
    pub previous_output: String,
}

pub(super) struct GroupRunExecuteStepContextRow {
    pub step_id: String,
    pub run_id: String,
    pub assignee_employee_id: String,
    pub dispatch_source_employee_id: String,
    pub step_type: String,
    pub existing_session_id: String,
    pub step_input: String,
    pub user_goal: String,
}

pub(super) struct GroupStepSessionRow {
    pub skill_id: String,
    pub model_id: String,
    pub work_dir: String,
}

pub(super) struct EmployeeSessionSeedRow {
    pub primary_skill_id: String,
    pub default_work_dir: String,
}

pub(super) struct ModelConfigRow {
    pub api_format: String,
    pub base_url: String,
    pub model_name: String,
    pub api_key: String,
}

pub(super) struct SessionMessageRow {
    pub role: String,
    pub content: String,
}

pub(super) struct PendingReviewStepRow {
    pub step_id: String,
    pub assignee_employee_id: String,
}

pub(super) struct GroupRunFinalizeStateRow {
    pub session_id: String,
    pub user_goal: String,
    pub state: String,
}

pub(super) struct GroupRunSnapshotRow {
    pub run_id: String,
    pub group_id: String,
    pub session_id: String,
    pub state: String,
    pub current_round: i64,
    pub user_goal: String,
    pub current_phase: String,
    pub review_round: i64,
    pub status_reason: String,
    pub waiting_for_employee_id: String,
    pub waiting_for_user: bool,
}

pub(super) struct GroupRunStepSnapshotRow {
    pub id: String,
    pub round_no: i64,
    pub step_type: String,
    pub assignee_employee_id: String,
    pub dispatch_source_employee_id: String,
    pub session_id: String,
    pub attempt_no: i64,
    pub status: String,
    pub output_summary: String,
    pub output: String,
}

pub(super) struct GroupRunEventSnapshotRow {
    pub id: String,
    pub step_id: String,
    pub event_type: String,
    pub payload_json: String,
    pub created_at: String,
}

pub(super) async fn list_agent_employee_rows(
    pool: &SqlitePool,
) -> Result<Vec<AgentEmployeeRow>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
            id,
            employee_id,
            name,
            role_id,
            persona,
            feishu_open_id,
            feishu_app_id,
            feishu_app_secret,
            primary_skill_id,
            default_work_dir,
            openclaw_agent_id,
            routing_priority,
            enabled_scopes_json,
            enabled,
            is_default,
            created_at,
            updated_at
        FROM agent_employees
        ORDER BY is_default DESC, updated_at DESC
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|row| AgentEmployeeRow {
            id: row.try_get("id").expect("employee row id"),
            employee_id: row.try_get("employee_id").expect("employee row employee_id"),
            name: row.try_get("name").expect("employee row name"),
            role_id: row.try_get("role_id").expect("employee row role_id"),
            persona: row.try_get("persona").expect("employee row persona"),
            feishu_open_id: row.try_get("feishu_open_id").expect("employee row feishu_open_id"),
            feishu_app_id: row.try_get("feishu_app_id").expect("employee row feishu_app_id"),
            feishu_app_secret: row
                .try_get("feishu_app_secret")
                .expect("employee row feishu_app_secret"),
            primary_skill_id: row
                .try_get("primary_skill_id")
                .expect("employee row primary_skill_id"),
            default_work_dir: row
                .try_get("default_work_dir")
                .expect("employee row default_work_dir"),
            openclaw_agent_id: row
                .try_get("openclaw_agent_id")
                .expect("employee row openclaw_agent_id"),
            routing_priority: row
                .try_get("routing_priority")
                .expect("employee row routing_priority"),
            enabled_scopes_json: row
                .try_get("enabled_scopes_json")
                .expect("employee row enabled_scopes_json"),
            enabled: row.try_get::<i64, _>("enabled").expect("employee row enabled") != 0,
            is_default: row.try_get::<i64, _>("is_default").expect("employee row is_default") != 0,
            created_at: row.try_get("created_at").expect("employee row created_at"),
            updated_at: row.try_get("updated_at").expect("employee row updated_at"),
        })
        .collect())
}

pub(super) async fn list_skill_ids_for_employee(
    pool: &SqlitePool,
    employee_db_id: &str,
) -> Result<Vec<String>, String> {
    let rows = sqlx::query(
        r#"
        SELECT skill_id
        FROM agent_employee_skills
        WHERE employee_id = ?
        ORDER BY sort_order ASC
        "#,
    )
    .bind(employee_db_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|row| row.try_get("skill_id").expect("skill row skill_id"))
        .collect())
}

pub(super) async fn find_employee_db_id_by_employee_id(
    pool: &SqlitePool,
    employee_id: &str,
) -> Result<Option<String>, String> {
    let row = sqlx::query("SELECT id FROM agent_employees WHERE employee_id = ? LIMIT 1")
        .bind(employee_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.map(|record| record.try_get("id").expect("employee id row id")))
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

pub(super) async fn clear_default_employee_flag(
    tx: &mut Transaction<'_, Sqlite>,
) -> Result<(), String> {
    sqlx::query("UPDATE agent_employees SET is_default = 0")
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn upsert_agent_employee_record(
    tx: &mut Transaction<'_, Sqlite>,
    input: &UpsertAgentEmployeeRecordInput<'_>,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO agent_employees (
            id, employee_id, name, role_id, persona, feishu_open_id, feishu_app_id, feishu_app_secret,
            primary_skill_id, default_work_dir, openclaw_agent_id, routing_priority, enabled_scopes_json,
            enabled, is_default, created_at, updated_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            employee_id = excluded.employee_id,
            name = excluded.name,
            role_id = excluded.role_id,
            persona = excluded.persona,
            feishu_open_id = excluded.feishu_open_id,
            feishu_app_id = excluded.feishu_app_id,
            feishu_app_secret = excluded.feishu_app_secret,
            primary_skill_id = excluded.primary_skill_id,
            default_work_dir = excluded.default_work_dir,
            openclaw_agent_id = excluded.openclaw_agent_id,
            routing_priority = excluded.routing_priority,
            enabled_scopes_json = excluded.enabled_scopes_json,
            enabled = excluded.enabled,
            is_default = excluded.is_default,
            updated_at = excluded.updated_at
        "#
    )
    .bind(input.id)
    .bind(input.employee_id)
    .bind(input.name)
    .bind(input.role_id)
    .bind(input.persona)
    .bind(input.feishu_open_id)
    .bind(input.feishu_app_id)
    .bind(input.feishu_app_secret)
    .bind(input.primary_skill_id)
    .bind(input.default_work_dir)
    .bind(input.openclaw_agent_id)
    .bind(input.routing_priority)
    .bind(input.enabled_scopes_json)
    .bind(if input.enabled { 1_i64 } else { 0_i64 })
    .bind(if input.is_default { 1_i64 } else { 0_i64 })
    .bind(input.now)
    .bind(input.now)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn replace_employee_skill_bindings(
    tx: &mut Transaction<'_, Sqlite>,
    employee_db_id: &str,
    skill_ids: &[String],
) -> Result<(), String> {
    sqlx::query("DELETE FROM agent_employee_skills WHERE employee_id = ?")
        .bind(employee_db_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;

    for (idx, skill_id) in skill_ids.iter().enumerate() {
        sqlx::query("INSERT INTO agent_employee_skills (employee_id, skill_id, sort_order) VALUES (?, ?, ?)")
        .bind(employee_db_id)
        .bind(skill_id)
        .bind(idx as i64)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub(super) async fn delete_agent_employee_record(
    tx: &mut Transaction<'_, Sqlite>,
    employee_db_id: &str,
) -> Result<(), String> {
    sqlx::query("DELETE FROM agent_employee_skills WHERE employee_id = ?")
        .bind(employee_db_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM im_thread_sessions WHERE employee_id = ?")
        .bind(employee_db_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM agent_employees WHERE id = ?")
        .bind(employee_db_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
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

pub(super) async fn find_latest_thread_session_id(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Option<String>, String> {
    let row = sqlx::query(
        "SELECT ts.session_id
         FROM im_thread_sessions ts
         INNER JOIN sessions s ON s.id = ts.session_id
         WHERE ts.thread_id = ?
         ORDER BY ts.updated_at DESC
         LIMIT 1",
    )
    .bind(thread_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|record| record.try_get(0).expect("latest thread session id")))
}

pub(super) async fn find_thread_session_record(
    pool: &SqlitePool,
    thread_id: &str,
    employee_db_id: &str,
) -> Result<Option<ThreadSessionRecord>, String> {
    let row = sqlx::query(
        "SELECT ts.session_id,
                CASE WHEN s.id IS NULL THEN 0 ELSE 1 END AS session_exists
         FROM im_thread_sessions ts
         LEFT JOIN sessions s ON s.id = ts.session_id
         WHERE ts.thread_id = ? AND ts.employee_id = ?
         LIMIT 1",
    )
    .bind(thread_id)
    .bind(employee_db_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|record| ThreadSessionRecord {
        session_id: record.try_get(0).expect("thread session record session_id"),
        session_exists: record
            .try_get::<i64, _>(1)
            .expect("thread session record session_exists")
            != 0,
    }))
}

pub(super) async fn upsert_thread_session_link(
    pool: &SqlitePool,
    input: &ThreadSessionLinkInput<'_>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO im_thread_sessions (thread_id, employee_id, session_id, route_session_key, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(thread_id, employee_id) DO UPDATE SET
            session_id = excluded.session_id,
            route_session_key = excluded.route_session_key,
            updated_at = excluded.updated_at",
    )
    .bind(input.thread_id)
    .bind(input.employee_db_id)
    .bind(input.session_id)
    .bind(input.route_session_key)
    .bind(input.created_at)
    .bind(input.updated_at)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn find_recent_route_session_id(
    pool: &SqlitePool,
    employee_db_id: &str,
    route_session_key: &str,
) -> Result<Option<String>, String> {
    let row = sqlx::query(
        "SELECT ts.session_id
         FROM im_thread_sessions ts
         INNER JOIN sessions s ON s.id = ts.session_id
         WHERE ts.employee_id = ? AND ts.route_session_key = ?
         ORDER BY ts.updated_at DESC
         LIMIT 1",
    )
    .bind(employee_db_id)
    .bind(route_session_key)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|record| record.try_get(0).expect("recent route session id")))
}

pub(super) async fn insert_session_seed(
    pool: &SqlitePool,
    input: &SessionSeedInput<'_>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id)
         VALUES (?, ?, ?, ?, ?, 'standard', ?, ?)",
    )
    .bind(input.id)
    .bind(input.skill_id)
    .bind(input.title)
    .bind(input.created_at)
    .bind(input.model_id)
    .bind(input.work_dir)
    .bind(input.employee_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn update_session_employee_id(
    pool: &SqlitePool,
    session_id: &str,
    employee_id: &str,
) -> Result<(), String> {
    sqlx::query("UPDATE sessions SET employee_id = ? WHERE id = ?")
        .bind(employee_id)
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn insert_inbound_event_link(
    pool: &SqlitePool,
    input: &InboundEventLinkInput<'_>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO im_message_links (id, thread_id, session_id, employee_id, direction, im_event_id, im_message_id, app_message_id, created_at)
         VALUES (?, ?, ?, ?, 'inbound', ?, ?, '', ?)",
    )
    .bind(input.id)
    .bind(input.thread_id)
    .bind(input.session_id)
    .bind(input.employee_db_id)
    .bind(input.im_event_id)
    .bind(input.im_message_id)
    .bind(input.created_at)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn find_group_step_session_row(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<GroupStepSessionRow>, String> {
    let row = sqlx::query(
        "SELECT skill_id, model_id, COALESCE(work_dir, '')
         FROM sessions
         WHERE id = ?",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|record| GroupStepSessionRow {
        skill_id: record.try_get(0).expect("group step session skill_id"),
        model_id: record.try_get(1).expect("group step session model_id"),
        work_dir: record.try_get(2).expect("group step session work_dir"),
    }))
}

pub(super) async fn find_existing_session_skill_id(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<String>, String> {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT COALESCE(skill_id, '') FROM sessions WHERE id = ?",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(row.map(|(skill_id,)| skill_id))
}

pub(super) async fn find_employee_session_seed_row(
    pool: &SqlitePool,
    employee_id: &str,
) -> Result<Option<EmployeeSessionSeedRow>, String> {
    let row = sqlx::query(
        "SELECT primary_skill_id, default_work_dir
         FROM agent_employees
         WHERE lower(employee_id) = lower(?) OR lower(role_id) = lower(?)
         ORDER BY is_default DESC, updated_at DESC
         LIMIT 1",
    )
    .bind(employee_id)
    .bind(employee_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|record| EmployeeSessionSeedRow {
        primary_skill_id: record
            .try_get("primary_skill_id")
            .expect("employee session seed primary_skill_id"),
        default_work_dir: record
            .try_get("default_work_dir")
            .expect("employee session seed default_work_dir"),
    }))
}

pub(super) async fn find_recent_group_step_session_id(
    pool: &SqlitePool,
    run_id: &str,
    assignee_employee_id: &str,
) -> Result<Option<String>, String> {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT session_id
         FROM group_run_steps
         WHERE run_id = ? AND assignee_employee_id = ? AND TRIM(session_id) <> ''
         ORDER BY finished_at DESC, started_at DESC
         LIMIT 1",
    )
    .bind(run_id)
    .bind(assignee_employee_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(row.map(|(session_id,)| session_id))
}

pub(super) async fn find_model_config_row(
    pool: &SqlitePool,
    model_id: &str,
) -> Result<Option<ModelConfigRow>, String> {
    let row = sqlx::query(
        "SELECT api_format, base_url, model_name, api_key
         FROM model_configs
         WHERE id = ?",
    )
    .bind(model_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|record| ModelConfigRow {
        api_format: record.try_get(0).expect("model config api_format"),
        base_url: record.try_get(1).expect("model config base_url"),
        model_name: record.try_get(2).expect("model config model_name"),
        api_key: record.try_get(3).expect("model config api_key"),
    }))
}

pub(super) async fn insert_session_message(
    pool: &SqlitePool,
    session_id: &str,
    role: &str,
    content: &str,
    created_at: &str,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(role)
    .bind(content)
    .bind(created_at)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn list_session_message_rows(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Vec<SessionMessageRow>, String> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT role, content FROM messages WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|(role, content)| SessionMessageRow { role, content })
        .collect())
}

pub(super) async fn pause_group_run(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    reason: &str,
    now: &str,
) -> Result<u64, String> {
    let result = sqlx::query(
        "UPDATE group_runs
         SET state = 'paused',
             status_reason = ?,
             updated_at = ?
         WHERE id = ? AND state NOT IN ('done', 'failed', 'cancelled', 'paused')",
    )
    .bind(reason)
    .bind(now)
    .bind(run_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(result.rows_affected())
}

pub(super) async fn find_group_run_state(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<GroupRunStateRow>, String> {
    let row = sqlx::query(
        "SELECT state, COALESCE(current_phase, 'plan')
         FROM group_runs
         WHERE id = ?",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|record| GroupRunStateRow {
        state: record.try_get(0).expect("group run state"),
        current_phase: record.try_get(1).expect("group run current_phase"),
    }))
}

pub(super) async fn resume_group_run(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    resumed_state: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE group_runs
         SET state = ?,
             status_reason = '',
             updated_at = ?
         WHERE id = ?",
    )
    .bind(resumed_state)
    .bind(now)
    .bind(run_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn insert_group_run_event(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    step_id: &str,
    event_type: &str,
    payload_json: &str,
    created_at: &str,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(run_id)
    .bind(step_id)
    .bind(event_type)
    .bind(payload_json)
    .bind(created_at)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn cancel_group_run(
    pool: &SqlitePool,
    run_id: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE group_runs
         SET state = 'cancelled', updated_at = ?
         WHERE id = ? AND state NOT IN ('done', 'failed', 'cancelled')",
    )
    .bind(now)
    .bind(run_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn list_failed_group_run_steps(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<FailedGroupRunStepRow>, String> {
    let rows = sqlx::query("SELECT id, output FROM group_run_steps WHERE run_id = ? AND status = 'failed'")
        .bind(run_id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|row| FailedGroupRunStepRow {
            step_id: row.try_get("id").expect("failed step id"),
            output: row.try_get("output").expect("failed step output"),
        })
        .collect())
}

pub(super) async fn complete_failed_group_run_step(
    tx: &mut Transaction<'_, Sqlite>,
    step_id: &str,
    output: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE group_run_steps
         SET status = 'completed', output = ?, finished_at = ?
         WHERE id = ?",
    )
    .bind(output)
    .bind(now)
    .bind(step_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn mark_group_run_done_after_retry(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE group_runs
         SET state = 'done', current_round = current_round + 1, updated_at = ?
         WHERE id = ?",
    )
    .bind(now)
    .bind(run_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn find_group_run_step_reassign_row(
    pool: &SqlitePool,
    step_id: &str,
) -> Result<Option<GroupRunStepReassignRow>, String> {
    let row = sqlx::query(
        "SELECT run_id, status, step_type, COALESCE(dispatch_source_employee_id, ''), COALESCE(assignee_employee_id, ''),
                COALESCE(output_summary, ''), COALESCE(output, '')
         FROM group_run_steps
         WHERE id = ?",
    )
    .bind(step_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|record| GroupRunStepReassignRow {
        run_id: record.try_get(0).expect("reassign row run_id"),
        status: record.try_get(1).expect("reassign row status"),
        step_type: record.try_get(2).expect("reassign row step_type"),
        dispatch_source_employee_id: record
            .try_get(3)
            .expect("reassign row dispatch_source_employee_id"),
        previous_assignee_employee_id: record
            .try_get(4)
            .expect("reassign row previous_assignee_employee_id"),
        previous_output_summary: record
            .try_get(5)
            .expect("reassign row previous_output_summary"),
        previous_output: record.try_get(6).expect("reassign row previous_output"),
    }))
}

pub(super) async fn employee_exists_for_reassignment(
    pool: &SqlitePool,
    employee_id: &str,
) -> Result<bool, String> {
    let row = sqlx::query("SELECT id FROM agent_employees WHERE lower(employee_id) = lower(?) OR lower(role_id) = lower(?) LIMIT 1")
        .bind(employee_id)
        .bind(employee_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.is_some())
}

pub(super) async fn reset_group_run_step_for_reassignment(
    tx: &mut Transaction<'_, Sqlite>,
    step_id: &str,
    assignee_employee_id: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE group_run_steps
         SET assignee_employee_id = ?,
             status = 'pending',
             output = '',
             output_summary = '',
             session_id = '',
             started_at = '',
             finished_at = '',
             attempt_no = attempt_no + 1
         WHERE id = ?",
    )
    .bind(assignee_employee_id)
    .bind(step_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn list_failed_execute_assignees(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
) -> Result<Vec<String>, String> {
    sqlx::query_scalar::<_, String>(
        "SELECT assignee_employee_id
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute' AND status = 'failed'
         ORDER BY round_no ASC, id ASC",
    )
    .bind(run_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| e.to_string())
}

pub(super) async fn update_group_run_after_reassignment(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    state: &str,
    waiting_for_employee_id: &str,
    status_reason: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE group_runs
         SET state = ?,
             current_phase = 'execute',
             waiting_for_employee_id = ?,
             status_reason = ?,
             updated_at = ?
         WHERE id = ?",
    )
    .bind(state)
    .bind(waiting_for_employee_id)
    .bind(status_reason)
    .bind(now)
    .bind(run_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn find_group_run_execute_step_context(
    pool: &SqlitePool,
    step_id: &str,
) -> Result<Option<GroupRunExecuteStepContextRow>, String> {
    let row = sqlx::query(
        "SELECT s.id, s.run_id, s.assignee_employee_id, COALESCE(s.dispatch_source_employee_id, ''),
                s.step_type, COALESCE(s.session_id, ''), COALESCE(s.input, ''), COALESCE(r.user_goal, '')
         FROM group_run_steps s
         INNER JOIN group_runs r ON r.id = s.run_id
         WHERE s.id = ?",
    )
    .bind(step_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|record| GroupRunExecuteStepContextRow {
        step_id: record.try_get(0).expect("execute step row step_id"),
        run_id: record.try_get(1).expect("execute step row run_id"),
        assignee_employee_id: record
            .try_get(2)
            .expect("execute step row assignee_employee_id"),
        dispatch_source_employee_id: record
            .try_get(3)
            .expect("execute step row dispatch_source_employee_id"),
        step_type: record.try_get(4).expect("execute step row step_type"),
        existing_session_id: record
            .try_get(5)
            .expect("execute step row existing_session_id"),
        step_input: record.try_get(6).expect("execute step row step_input"),
        user_goal: record.try_get(7).expect("execute step row user_goal"),
    }))
}

pub(super) async fn mark_group_run_step_dispatched(
    tx: &mut Transaction<'_, Sqlite>,
    step_id: &str,
    session_id: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE group_run_steps
         SET status = 'running',
             session_id = ?,
             started_at = CASE WHEN TRIM(started_at) = '' THEN ? ELSE started_at END,
             phase = CASE WHEN TRIM(phase) = '' THEN 'execute' ELSE phase END
         WHERE id = ?",
    )
    .bind(session_id)
    .bind(now)
    .bind(step_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn mark_group_run_executing(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    waiting_for_employee_id: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE group_runs
         SET state = 'executing',
             current_phase = 'execute',
             waiting_for_employee_id = ?,
             status_reason = '',
             updated_at = ?
         WHERE id = ?",
    )
    .bind(waiting_for_employee_id)
    .bind(now)
    .bind(run_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn mark_group_run_step_failed(
    tx: &mut Transaction<'_, Sqlite>,
    step_id: &str,
    output: &str,
    output_summary: &str,
    session_id: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE group_run_steps
         SET status = 'failed',
             output = ?,
             output_summary = ?,
             session_id = ?,
             finished_at = ?,
             phase = CASE WHEN TRIM(phase) = '' THEN 'execute' ELSE phase END
         WHERE id = ?",
    )
    .bind(output)
    .bind(output_summary)
    .bind(session_id)
    .bind(now)
    .bind(step_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn mark_group_run_failed(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    waiting_for_employee_id: &str,
    status_reason: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE group_runs
         SET state = 'failed',
             current_phase = 'execute',
             waiting_for_employee_id = ?,
             status_reason = ?,
             updated_at = ?
         WHERE id = ?",
    )
    .bind(waiting_for_employee_id)
    .bind(status_reason)
    .bind(now)
    .bind(run_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn mark_group_run_step_completed(
    tx: &mut Transaction<'_, Sqlite>,
    step_id: &str,
    output: &str,
    output_summary: &str,
    session_id: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE group_run_steps
         SET status = 'completed',
             output = ?,
             output_summary = ?,
             session_id = ?,
             finished_at = ?,
             phase = CASE WHEN TRIM(phase) = '' THEN 'execute' ELSE phase END
         WHERE id = ?",
    )
    .bind(output)
    .bind(output_summary)
    .bind(session_id)
    .bind(now)
    .bind(step_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn clear_group_run_execute_waiting_state(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE group_runs
         SET state = 'executing',
             current_phase = 'execute',
             status_reason = '',
             waiting_for_employee_id = '',
             updated_at = ?
         WHERE id = ?",
    )
    .bind(now)
    .bind(run_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn find_pending_review_step(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<PendingReviewStepRow>, String> {
    let row = sqlx::query(
        "SELECT id, assignee_employee_id
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'review' AND status IN ('pending', 'running', 'blocked')
         ORDER BY round_no DESC, id DESC
         LIMIT 1",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|record| PendingReviewStepRow {
        step_id: record.try_get(0).expect("pending review step_id"),
        assignee_employee_id: record
            .try_get(1)
            .expect("pending review assignee_employee_id"),
    }))
}

pub(super) async fn review_requested_event_exists(
    pool: &SqlitePool,
    run_id: &str,
    step_id: &str,
) -> Result<bool, String> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM group_run_events
         WHERE run_id = ? AND step_id = ? AND event_type = 'review_requested'",
    )
    .bind(run_id)
    .bind(step_id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(count > 0)
}

pub(super) async fn mark_group_run_waiting_review(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    waiting_for_employee_id: &str,
    default_reason: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE group_runs
         SET state = 'waiting_review',
             current_phase = 'review',
             waiting_for_employee_id = ?,
             status_reason = CASE
               WHEN TRIM(status_reason) = '' THEN ?
               ELSE status_reason
             END,
             updated_at = ?
         WHERE id = ?",
    )
    .bind(waiting_for_employee_id)
    .bind(default_reason)
    .bind(now)
    .bind(run_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn list_pending_execute_step_ids(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<String>, String> {
    sqlx::query_scalar::<_, String>(
        "SELECT id
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute' AND status = 'pending'
         ORDER BY round_no ASC, id ASC",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())
}

pub(super) async fn load_group_run_blocking_counts(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(i64, i64), String> {
    let row = sqlx::query(
        "SELECT
            SUM(CASE WHEN step_type = 'execute' AND status IN ('pending', 'running', 'failed') THEN 1 ELSE 0 END) AS execute_blocking,
            SUM(CASE WHEN step_type = 'review' AND status IN ('pending', 'running') THEN 1 ELSE 0 END) AS review_blocking
         FROM group_run_steps
         WHERE run_id = ?",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok((
        row.try_get::<Option<i64>, _>("execute_blocking")
            .map_err(|e| e.to_string())?
            .unwrap_or(0),
        row.try_get::<Option<i64>, _>("review_blocking")
            .map_err(|e| e.to_string())?
            .unwrap_or(0),
    ))
}

pub(super) async fn find_group_run_finalize_state(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<GroupRunFinalizeStateRow>, String> {
    let row = sqlx::query(
        "SELECT session_id, user_goal, state
         FROM group_runs
         WHERE id = ?",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(row.map(|record| GroupRunFinalizeStateRow {
        session_id: record.try_get(0).expect("finalize state session_id"),
        user_goal: record.try_get(1).expect("finalize state user_goal"),
        state: record.try_get(2).expect("finalize state state"),
    }))
}

pub(super) async fn list_group_run_execute_outputs(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<(String, String)>, String> {
    let rows = sqlx::query(
        "SELECT assignee_employee_id, output
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute'
         ORDER BY round_no ASC, finished_at ASC, id ASC",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.try_get(0).expect("execute output assignee"),
                row.try_get(1).expect("execute output content"),
            )
        })
        .collect())
}

pub(super) async fn insert_group_run_assistant_message(
    tx: &mut Transaction<'_, Sqlite>,
    session_id: &str,
    content: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at)
         VALUES (?, ?, 'assistant', ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(content)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn mark_group_run_finalized(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE group_runs
         SET state = 'done',
             current_phase = 'finalize',
             waiting_for_employee_id = '',
             waiting_for_user = 0,
             status_reason = '',
             updated_at = ?
         WHERE id = ?",
    )
    .bind(now)
    .bind(run_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn get_group_run_session_id(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<String>, String> {
    let row = sqlx::query("SELECT session_id FROM group_runs WHERE id = ?")
        .bind(run_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.map(|record| record.try_get(0).expect("group run session id")))
}

pub(super) async fn find_group_run_snapshot_row(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<GroupRunSnapshotRow>, String> {
    let row = sqlx::query(
        "SELECT id, group_id, session_id, state, current_round, user_goal,
                COALESCE(current_phase, 'plan'), COALESCE(review_round, 0),
                COALESCE(status_reason, ''), COALESCE(waiting_for_employee_id, ''),
                COALESCE(waiting_for_user, 0)
         FROM group_runs
         WHERE session_id = ?
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(row.map(|record| GroupRunSnapshotRow {
        run_id: record.try_get("id").expect("snapshot run_id"),
        group_id: record.try_get("group_id").expect("snapshot group_id"),
        session_id: record.try_get("session_id").expect("snapshot session_id"),
        state: record.try_get("state").expect("snapshot state"),
        current_round: record.try_get("current_round").expect("snapshot current_round"),
        user_goal: record.try_get("user_goal").expect("snapshot user_goal"),
        current_phase: record.try_get(6).expect("snapshot current_phase"),
        review_round: record.try_get(7).expect("snapshot review_round"),
        status_reason: record.try_get(8).expect("snapshot status_reason"),
        waiting_for_employee_id: record
            .try_get(9)
            .expect("snapshot waiting_for_employee_id"),
        waiting_for_user: record
            .try_get::<i64, _>(10)
            .expect("snapshot waiting_for_user")
            != 0,
    }))
}

pub(super) async fn list_group_run_step_snapshot_rows(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<GroupRunStepSnapshotRow>, String> {
    let rows = sqlx::query(
        "SELECT id, round_no, step_type, assignee_employee_id,
                COALESCE(dispatch_source_employee_id, ''), COALESCE(session_id, ''),
                COALESCE(attempt_no, 1), status, COALESCE(output_summary, ''), output
         FROM group_run_steps
         WHERE run_id = ?
         ORDER BY round_no ASC, started_at ASC, id ASC",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|row| GroupRunStepSnapshotRow {
            id: row.try_get("id").expect("step snapshot id"),
            round_no: row.try_get("round_no").expect("step snapshot round_no"),
            step_type: row.try_get("step_type").expect("step snapshot step_type"),
            assignee_employee_id: row
                .try_get("assignee_employee_id")
                .expect("step snapshot assignee"),
            dispatch_source_employee_id: row
                .try_get(4)
                .expect("step snapshot dispatch_source"),
            session_id: row.try_get(5).expect("step snapshot session_id"),
            attempt_no: row.try_get(6).expect("step snapshot attempt_no"),
            status: row.try_get(7).expect("step snapshot status"),
            output_summary: row.try_get(8).expect("step snapshot output_summary"),
            output: row.try_get(9).expect("step snapshot output"),
        })
        .collect())
}

pub(super) async fn list_group_run_event_snapshot_rows(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<GroupRunEventSnapshotRow>, String> {
    let rows = sqlx::query(
        "SELECT id, COALESCE(step_id, ''), event_type, COALESCE(payload_json, '{}'), created_at
         FROM group_run_events
         WHERE run_id = ?
         ORDER BY created_at ASC, id ASC",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|row| GroupRunEventSnapshotRow {
            id: row.try_get("id").expect("event snapshot id"),
            step_id: row.try_get(1).expect("event snapshot step_id"),
            event_type: row.try_get("event_type").expect("event snapshot event_type"),
            payload_json: row.try_get(3).expect("event snapshot payload_json"),
            created_at: row.try_get("created_at").expect("event snapshot created_at"),
        })
        .collect())
}

pub(super) async fn find_latest_assistant_message_content(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<String>, String> {
    let row = sqlx::query(
        "SELECT content
         FROM messages
         WHERE session_id = ? AND role = 'assistant'
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(row.map(|record| record.try_get(0).expect("latest assistant content")))
}

pub(super) async fn delete_feishu_bindings_for_agent(
    tx: &mut Transaction<'_, Sqlite>,
    agent_id: &str,
) -> Result<(), String> {
    sqlx::query("DELETE FROM im_routing_bindings WHERE channel = 'feishu' AND lower(agent_id) = lower(?)")
    .bind(agent_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn find_displaced_default_feishu_agent_ids(
    tx: &mut Transaction<'_, Sqlite>,
    agent_id: &str,
) -> Result<Vec<String>, String> {
    let rows = sqlx::query_scalar::<_, String>(
        r#"
        SELECT DISTINCT agent_id
        FROM im_routing_bindings
        WHERE channel = 'feishu'
          AND trim(peer_id) = ''
          AND lower(agent_id) != lower(?)
        "#
    )
    .bind(agent_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub(super) async fn delete_displaced_default_feishu_bindings(
    tx: &mut Transaction<'_, Sqlite>,
    agent_id: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"
        DELETE FROM im_routing_bindings
        WHERE channel = 'feishu'
          AND trim(peer_id) = ''
          AND lower(agent_id) != lower(?)
        "#
    )
    .bind(agent_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn find_displaced_scoped_feishu_agent_ids(
    tx: &mut Transaction<'_, Sqlite>,
    agent_id: &str,
    peer_kind: &str,
    peer_id: &str,
) -> Result<Vec<String>, String> {
    let rows = sqlx::query_scalar::<_, String>(
        r#"
        SELECT DISTINCT agent_id
        FROM im_routing_bindings
        WHERE channel = 'feishu'
          AND lower(agent_id) != lower(?)
          AND lower(peer_kind) = ?
          AND trim(peer_id) = ?
        "#
    )
    .bind(agent_id)
    .bind(peer_kind)
    .bind(peer_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub(super) async fn delete_displaced_scoped_feishu_bindings(
    tx: &mut Transaction<'_, Sqlite>,
    agent_id: &str,
    peer_kind: &str,
    peer_id: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"
        DELETE FROM im_routing_bindings
        WHERE channel = 'feishu'
          AND lower(agent_id) != lower(?)
          AND lower(peer_kind) = ?
          AND trim(peer_id) = ?
        "#
    )
    .bind(agent_id)
    .bind(peer_kind)
    .bind(peer_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn insert_feishu_binding(
    tx: &mut Transaction<'_, Sqlite>,
    input: &InsertFeishuBindingInput<'_>,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO im_routing_bindings (
            id, agent_id, channel, account_id, peer_kind, peer_id, guild_id, team_id,
            role_ids_json, connector_meta_json, priority, enabled, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#
    )
    .bind(input.id)
    .bind(input.agent_id)
    .bind("feishu")
    .bind("*")
    .bind(input.peer_kind)
    .bind(input.peer_id)
    .bind("")
    .bind("")
    .bind("[]")
    .bind(input.connector_meta_json)
    .bind(input.priority)
    .bind(1_i64)
    .bind(input.now)
    .bind(input.now)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn count_feishu_bindings_for_agent(
    tx: &mut Transaction<'_, Sqlite>,
    agent_id: &str,
) -> Result<i64, String> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(1)
        FROM im_routing_bindings
        WHERE channel = 'feishu' AND lower(agent_id) = lower(?)
        "#
    )
    .bind(agent_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(count)
}

pub(super) async fn list_agent_scope_rows(
    tx: &mut Transaction<'_, Sqlite>,
) -> Result<Vec<(String, String, String, String, String)>, String> {
    let rows = sqlx::query(
        r#"
        SELECT id, employee_id, role_id, openclaw_agent_id, enabled_scopes_json
        FROM agent_employees
        "#
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.try_get("id").expect("scope row id"),
                row.try_get("employee_id").expect("scope row employee_id"),
                row.try_get("role_id").expect("scope row role_id"),
                row.try_get("openclaw_agent_id").expect("scope row openclaw_agent_id"),
                row.try_get("enabled_scopes_json").expect("scope row enabled_scopes_json"),
            )
        })
        .collect())
}

pub(super) struct InsertFeishuBindingInput<'a> {
    pub id: &'a str,
    pub agent_id: &'a str,
    pub peer_kind: &'a str,
    pub peer_id: &'a str,
    pub connector_meta_json: &'a str,
    pub priority: i64,
    pub now: &'a str,
}

pub(super) struct UpsertAgentEmployeeRecordInput<'a> {
    pub id: &'a str,
    pub employee_id: &'a str,
    pub name: &'a str,
    pub role_id: &'a str,
    pub persona: &'a str,
    pub feishu_open_id: &'a str,
    pub feishu_app_id: &'a str,
    pub feishu_app_secret: &'a str,
    pub primary_skill_id: &'a str,
    pub default_work_dir: &'a str,
    pub openclaw_agent_id: &'a str,
    pub routing_priority: i64,
    pub enabled_scopes_json: &'a str,
    pub enabled: bool,
    pub is_default: bool,
    pub now: &'a str,
}
