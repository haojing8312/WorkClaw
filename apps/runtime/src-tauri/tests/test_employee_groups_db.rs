mod helpers;

use runtime_lib::commands::employee_agents::list_employee_group_runs_with_pool;

async fn table_columns(pool: &sqlx::SqlitePool, table_name: &str) -> Vec<String> {
    let pragma_sql = format!("SELECT name FROM pragma_table_info('{table_name}')");
    sqlx::query_as::<_, (String,)>(&pragma_sql)
        .fetch_all(pool)
        .await
        .expect("query pragma_table_info")
        .into_iter()
        .map(|row| row.0)
        .collect()
}

#[tokio::test]
async fn group_orchestrator_tables_exist() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'table'
         AND name IN ('employee_groups', 'group_runs', 'group_run_steps')",
    )
    .fetch_one(&pool)
    .await
    .expect("query sqlite_master");

    assert_eq!(count, 3, "expected employee group orchestration tables");
}

#[tokio::test]
async fn team_template_rule_and_event_tables_exist_and_accept_rows() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'table'
         AND name IN ('employee_group_rules', 'group_run_events', 'seeded_team_templates')",
    )
    .fetch_one(&pool)
    .await
    .expect("query sqlite_master for team template tables");

    assert_eq!(count, 3, "expected team template support tables");

    sqlx::query(
        "INSERT INTO employee_group_rules (
            id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("rule-1")
    .bind("group-1")
    .bind("zhongshu")
    .bind("menxia")
    .bind("review")
    .bind("plan")
    .bind(1_i64)
    .bind(100_i64)
    .bind("2026-03-07T00:00:00Z")
    .execute(&pool)
    .await
    .expect("insert employee group rule");

    sqlx::query(
        "INSERT INTO group_run_events (
            id, run_id, step_id, event_type, payload_json, created_at
         ) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("event-1")
    .bind("run-1")
    .bind("step-1")
    .bind("step_created")
    .bind(r#"{"phase":"plan"}"#)
    .bind("2026-03-07T00:00:00Z")
    .execute(&pool)
    .await
    .expect("insert group run event");

    sqlx::query(
        "INSERT INTO seeded_team_templates (
            template_id, template_version, instance_group_id, instance_employee_ids_json, seed_mode, seeded_at
         ) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind("sansheng-liubu")
    .bind("1.0.0")
    .bind("group-1")
    .bind(r#"["taizi","zhongshu","menxia","shangshu"]"#)
    .bind("first_run")
    .bind("2026-03-07T00:00:00Z")
    .execute(&pool)
    .await
    .expect("insert seeded team template");
}

#[tokio::test]
async fn team_template_runtime_columns_exist_on_group_tables() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let employee_group_columns = table_columns(&pool, "employee_groups").await;
    for required in [
        "template_id",
        "entry_employee_id",
        "review_mode",
        "execution_mode",
        "visibility_mode",
        "is_bootstrap_seeded",
        "config_json",
    ] {
        assert!(
            employee_group_columns
                .iter()
                .any(|column| column == required),
            "missing employee_groups column: {required}"
        );
    }

    let group_run_columns = table_columns(&pool, "group_runs").await;
    for required in [
        "current_phase",
        "entry_session_id",
        "main_employee_id",
        "review_round",
        "status_reason",
        "template_version",
        "waiting_for_employee_id",
        "waiting_for_user",
    ] {
        assert!(
            group_run_columns.iter().any(|column| column == required),
            "missing group_runs column: {required}"
        );
    }

    let group_run_step_columns = table_columns(&pool, "group_run_steps").await;
    for required in [
        "parent_step_id",
        "phase",
        "step_kind",
        "requires_review",
        "review_status",
        "attempt_no",
        "session_id",
        "input_summary",
        "output_summary",
        "visibility",
    ] {
        assert!(
            group_run_step_columns
                .iter()
                .any(|column| column == required),
            "missing group_run_steps column: {required}"
        );
    }
}

#[tokio::test]
async fn employee_groups_enforce_member_count_limit() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    sqlx::query(
        "INSERT INTO employee_groups (
            id, name, coordinator_employee_id, member_employee_ids_json, member_count, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("group-ok")
    .bind("产品交付群")
    .bind("project_manager")
    .bind(r#"[\"project_manager\",\"dev_team\",\"qa_team\"]"#)
    .bind(3_i64)
    .bind("2026-03-05T00:00:00Z")
    .bind("2026-03-05T00:00:00Z")
    .execute(&pool)
    .await
    .expect("insert group with valid member_count");

    let err = sqlx::query(
        "INSERT INTO employee_groups (
            id, name, coordinator_employee_id, member_employee_ids_json, member_count, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("group-too-many")
    .bind("超限群")
    .bind("project_manager")
    .bind(r#"[\"e1\",\"e2\",\"e3\",\"e4\",\"e5\",\"e6\",\"e7\",\"e8\",\"e9\",\"e10\",\"e11\"]"#)
    .bind(11_i64)
    .bind("2026-03-05T00:00:00Z")
    .bind("2026-03-05T00:00:00Z")
    .execute(&pool)
    .await
    .expect_err("member_count > 10 should violate check constraint");

    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("check constraint") || msg.contains("constraint failed"),
        "unexpected sqlite error: {msg}"
    );
}

#[tokio::test]
async fn sessions_support_explicit_mode_and_team_columns() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let session_columns = table_columns(&pool, "sessions").await;
    for required in ["session_mode", "team_id"] {
        assert!(
            session_columns.iter().any(|column| column == required),
            "missing sessions column: {required}"
        );
    }

    sqlx::query(
        "INSERT INTO sessions (
            id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("session-general")
    .bind("builtin-general")
    .bind("General")
    .bind("2026-03-08T00:00:00Z")
    .bind("model-1")
    .bind("standard")
    .bind("")
    .bind("")
    .bind("general")
    .bind("")
    .execute(&pool)
    .await
    .expect("insert general session");

    sqlx::query(
        "INSERT INTO sessions (
            id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("session-team")
    .bind("builtin-general")
    .bind("Team Entry")
    .bind("2026-03-08T00:00:00Z")
    .bind("model-1")
    .bind("standard")
    .bind("")
    .bind("taizi")
    .bind("team_entry")
    .bind("group-1")
    .execute(&pool)
    .await
    .expect("insert team entry session");

    let general_row: (String, String) =
        sqlx::query_as("SELECT session_mode, team_id FROM sessions WHERE id = 'session-general'")
            .fetch_one(&pool)
            .await
            .expect("query general session");
    assert_eq!(general_row.0, "general");
    assert_eq!(general_row.1, "");

    let team_row: (String, String) =
        sqlx::query_as("SELECT session_mode, team_id FROM sessions WHERE id = 'session-team'")
            .fetch_one(&pool)
            .await
            .expect("query team session");
    assert_eq!(team_row.0, "team_entry");
    assert_eq!(team_row.1, "group-1");
}

#[tokio::test]
async fn employee_group_runs_can_be_listed_for_recent_overview_cards() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    sqlx::query(
        "INSERT INTO employee_groups (
            id, name, coordinator_employee_id, member_employee_ids_json, member_count, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("group-1")
    .bind("交付协作群")
    .bind("pm")
    .bind(r#"["pm","dev"]"#)
    .bind(2_i64)
    .bind("2026-03-08T09:00:00Z")
    .bind("2026-03-08T09:00:00Z")
    .execute(&pool)
    .await
    .expect("insert group 1");

    sqlx::query(
        "INSERT INTO employee_groups (
            id, name, coordinator_employee_id, member_employee_ids_json, member_count, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("group-2")
    .bind("复盘协作群")
    .bind("ops")
    .bind(r#"["ops","qa"]"#)
    .bind(2_i64)
    .bind("2026-03-08T08:00:00Z")
    .bind("2026-03-08T08:00:00Z")
    .execute(&pool)
    .await
    .expect("insert group 2");

    sqlx::query(
        "INSERT INTO sessions (
            id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("session-run-1")
    .bind("builtin-general")
    .bind("交付运行")
    .bind("2026-03-08T10:00:00Z")
    .bind("model-1")
    .bind("standard")
    .bind("")
    .bind("pm")
    .bind("team_entry")
    .bind("group-1")
    .execute(&pool)
    .await
    .expect("insert session 1");

    sqlx::query(
        "INSERT INTO sessions (
            id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("session-run-2")
    .bind("builtin-general")
    .bind("复盘运行")
    .bind("2026-03-08T09:00:00Z")
    .bind("model-1")
    .bind("standard")
    .bind("")
    .bind("ops")
    .bind("team_entry")
    .bind("group-2")
    .execute(&pool)
    .await
    .expect("insert session 2");

    sqlx::query(
        "INSERT INTO group_runs (
            id, group_id, session_id, user_goal, state, current_round, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("run-new")
    .bind("group-1")
    .bind("session-run-1")
    .bind("处理紧急交付")
    .bind("running")
    .bind(1_i64)
    .bind("2026-03-08T10:00:00Z")
    .bind("2026-03-08T10:10:00Z")
    .execute(&pool)
    .await
    .expect("insert newest run");

    sqlx::query(
        "INSERT INTO group_runs (
            id, group_id, session_id, user_goal, state, current_round, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("run-old")
    .bind("group-2")
    .bind("session-run-2")
    .bind("整理复盘结论")
    .bind("done")
    .bind(2_i64)
    .bind("2026-03-08T09:00:00Z")
    .bind("2026-03-08T09:30:00Z")
    .execute(&pool)
    .await
    .expect("insert older run");

    let runs = list_employee_group_runs_with_pool(&pool, Some(10))
        .await
        .expect("list recent employee group runs");

    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].id, "run-new");
    assert_eq!(runs[0].goal, "处理紧急交付");
    assert_eq!(runs[0].status, "running");
    assert_eq!(runs[0].session_skill_id, "builtin-general");
    assert_eq!(runs[1].id, "run-old");
    assert_eq!(runs[1].status, "completed");
}
