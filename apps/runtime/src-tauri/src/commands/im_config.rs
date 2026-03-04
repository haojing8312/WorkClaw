use crate::commands::skills::DbState;
use sqlx::SqlitePool;
use tauri::State;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ThreadRoleConfig {
    pub thread_id: String,
    pub tenant_id: String,
    pub scenario_template: String,
    pub status: String,
    pub roles: Vec<String>,
}

pub async fn bind_thread_roles_with_pool(
    pool: &SqlitePool,
    thread_id: &str,
    tenant_id: &str,
    scenario_template: &str,
    roles: &[String],
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT INTO im_thread_bindings (thread_id, tenant_id, scenario_template, status, created_at, updated_at)
         VALUES (?, ?, ?, 'active', ?, ?)
         ON CONFLICT(thread_id) DO UPDATE SET
           tenant_id = excluded.tenant_id,
           scenario_template = excluded.scenario_template,
           status = 'active',
           updated_at = excluded.updated_at"
    )
    .bind(thread_id)
    .bind(tenant_id)
    .bind(scenario_template)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM im_thread_roles WHERE thread_id = ?")
        .bind(thread_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    for (idx, role_id) in roles.iter().enumerate() {
        sqlx::query(
            "INSERT INTO im_thread_roles (thread_id, role_id, role_order, enabled) VALUES (?, ?, ?, 1)"
        )
        .bind(thread_id)
        .bind(role_id)
        .bind(idx as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_thread_role_config_with_pool(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<ThreadRoleConfig, String> {
    let (thread_id_v, tenant_id, scenario_template, status): (String, String, String, String) =
        sqlx::query_as(
            "SELECT thread_id, tenant_id, scenario_template, status FROM im_thread_bindings WHERE thread_id = ?"
        )
        .bind(thread_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

    let role_rows: Vec<(String,)> = sqlx::query_as(
        "SELECT role_id FROM im_thread_roles WHERE thread_id = ? AND enabled = 1 ORDER BY role_order ASC"
    )
    .bind(thread_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(ThreadRoleConfig {
        thread_id: thread_id_v,
        tenant_id,
        scenario_template,
        status,
        roles: role_rows.into_iter().map(|(r,)| r).collect(),
    })
}

#[tauri::command]
pub async fn bind_thread_roles(
    thread_id: String,
    tenant_id: String,
    scenario_template: String,
    roles: Vec<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    bind_thread_roles_with_pool(&db.0, &thread_id, &tenant_id, &scenario_template, &roles).await
}

#[tauri::command]
pub async fn get_thread_role_config(
    thread_id: String,
    db: State<'_, DbState>,
) -> Result<ThreadRoleConfig, String> {
    get_thread_role_config_with_pool(&db.0, &thread_id).await
}
