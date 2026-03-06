mod helpers;

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
