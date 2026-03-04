mod helpers;

use runtime_lib::commands::employee_agents::{
    list_agent_employees_with_pool, upsert_agent_employee_with_pool, UpsertAgentEmployeeInput,
};

#[tokio::test]
async fn upsert_employee_mirrors_employee_id_to_legacy_ids() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "project_manager".to_string(),
            name: "项目经理".to_string(),
            role_id: "legacy_role_should_be_ignored".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "legacy_agent_should_be_ignored".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert");

    let row: (String, String, String) = sqlx::query_as(
        "SELECT employee_id, role_id, openclaw_agent_id FROM agent_employees WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .expect("query employee row");

    assert_eq!(row.0, "project_manager");
    assert_eq!(row.1, "project_manager");
    assert_eq!(row.2, "project_manager");
}

#[tokio::test]
async fn migration_backfills_employee_id_from_role_id() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let _ =
        sqlx::query("ALTER TABLE agent_employees ADD COLUMN employee_id TEXT NOT NULL DEFAULT ''")
            .execute(&pool)
            .await;

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO agent_employees (
            id, name, employee_id, role_id, persona, feishu_open_id, feishu_app_id, feishu_app_secret,
            primary_skill_id, default_work_dir, openclaw_agent_id, routing_priority, enabled_scopes_json,
            enabled, is_default, created_at, updated_at
        ) VALUES (?, ?, ?, ?, '', '', '', '', '', '', ?, 100, '[]', 1, 0, ?, ?)",
    )
    .bind("emp-legacy")
    .bind("历史员工")
    .bind("")
    .bind("legacy_role")
    .bind("legacy_role")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .expect("insert legacy employee");

    sqlx::query("UPDATE agent_employees SET employee_id = role_id WHERE TRIM(employee_id) = ''")
        .execute(&pool)
        .await
        .expect("backfill employee_id");

    let row: (String,) =
        sqlx::query_as("SELECT employee_id FROM agent_employees WHERE id = 'emp-legacy'")
            .fetch_one(&pool)
            .await
            .expect("query backfilled row");
    assert_eq!(row.0, "legacy_role");

    let list = list_agent_employees_with_pool(&pool)
        .await
        .expect("list employees");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].employee_id, "legacy_role");
}
