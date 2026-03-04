use crate::commands::runtime_preferences::resolve_default_work_dir_with_pool;
use crate::commands::skills::DbState;
use crate::im::types::ImEvent;
use sqlx::{Row, SqlitePool};
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AgentEmployee {
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
    pub enabled_scopes: Vec<String>,
    pub enabled: bool,
    pub is_default: bool,
    pub skill_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct UpsertAgentEmployeeInput {
    pub id: Option<String>,
    #[serde(default)]
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
    pub enabled_scopes: Vec<String>,
    pub enabled: bool,
    pub is_default: bool,
    pub skill_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ThreadEmployeeBinding {
    pub thread_id: String,
    pub employee_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EnsuredEmployeeSession {
    pub employee_id: String,
    pub role_id: String,
    pub employee_name: String,
    pub session_id: String,
    pub created: bool,
}

pub async fn list_agent_employees_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<AgentEmployee>, String> {
    let rows = sqlx::query(
        "SELECT id, employee_id, name, role_id, persona, feishu_open_id, feishu_app_id, feishu_app_secret, primary_skill_id, default_work_dir, openclaw_agent_id, routing_priority, enabled_scopes_json, enabled, is_default, created_at, updated_at
         FROM agent_employees
         ORDER BY is_default DESC, updated_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.try_get("id").map_err(|e| e.to_string())?;
        let employee_id_raw: String = row.try_get("employee_id").map_err(|e| e.to_string())?;
        let name: String = row.try_get("name").map_err(|e| e.to_string())?;
        let role_id: String = row.try_get("role_id").map_err(|e| e.to_string())?;
        let persona: String = row.try_get("persona").map_err(|e| e.to_string())?;
        let feishu_open_id: String = row.try_get("feishu_open_id").map_err(|e| e.to_string())?;
        let feishu_app_id: String = row.try_get("feishu_app_id").map_err(|e| e.to_string())?;
        let feishu_app_secret: String = row
            .try_get("feishu_app_secret")
            .map_err(|e| e.to_string())?;
        let primary_skill_id: String =
            row.try_get("primary_skill_id").map_err(|e| e.to_string())?;
        let default_work_dir: String =
            row.try_get("default_work_dir").map_err(|e| e.to_string())?;
        let openclaw_agent_id: String = row
            .try_get("openclaw_agent_id")
            .map_err(|e| e.to_string())?;
        let routing_priority: i64 = row.try_get("routing_priority").map_err(|e| e.to_string())?;
        let enabled_scopes_json: String = row
            .try_get("enabled_scopes_json")
            .map_err(|e| e.to_string())?;
        let enabled: i64 = row.try_get("enabled").map_err(|e| e.to_string())?;
        let is_default: i64 = row.try_get("is_default").map_err(|e| e.to_string())?;
        let created_at: String = row.try_get("created_at").map_err(|e| e.to_string())?;
        let updated_at: String = row.try_get("updated_at").map_err(|e| e.to_string())?;

        let skill_rows = sqlx::query_as::<_, (String,)>(
            "SELECT skill_id FROM agent_employee_skills WHERE employee_id = ? ORDER BY sort_order ASC",
        )
        .bind(&id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
        let enabled_scopes = serde_json::from_str::<Vec<String>>(&enabled_scopes_json)
            .unwrap_or_else(|_| vec!["feishu".to_string()]);
        let employee_id = if employee_id_raw.trim().is_empty() {
            role_id.clone()
        } else {
            employee_id_raw
        };
        result.push(AgentEmployee {
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
            enabled_scopes,
            enabled: enabled != 0,
            is_default: is_default != 0,
            skill_ids: skill_rows.into_iter().map(|(skill_id,)| skill_id).collect(),
            created_at,
            updated_at,
        });
    }
    Ok(result)
}

pub async fn upsert_agent_employee_with_pool(
    pool: &SqlitePool,
    input: UpsertAgentEmployeeInput,
) -> Result<String, String> {
    if input.name.trim().is_empty() {
        return Err("employee name is required".to_string());
    }
    let employee_id = if !input.employee_id.trim().is_empty() {
        input.employee_id.trim().to_string()
    } else if !input.role_id.trim().is_empty() {
        input.role_id.trim().to_string()
    } else if !input.openclaw_agent_id.trim().is_empty() {
        input.openclaw_agent_id.trim().to_string()
    } else {
        return Err("employee employee_id is required".to_string());
    };
    let role_id = employee_id.as_str();
    let existing_role = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM agent_employees WHERE employee_id = ? LIMIT 1",
    )
    .bind(&employee_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    if let Some((existing_id,)) = existing_role {
        if existing_id != id {
            return Err("employee employee_id already exists".to_string());
        }
    }
    let default_work_dir = if input.default_work_dir.trim().is_empty() {
        let base = resolve_default_work_dir_with_pool(pool).await?;
        let by_role = std::path::PathBuf::from(base)
            .join("employees")
            .join(&employee_id)
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&by_role)
            .map_err(|e| format!("failed to create employee work dir: {e}"))?;
        by_role
    } else {
        input.default_work_dir.trim().to_string()
    };
    let openclaw_agent_id = employee_id.clone();
    let enabled_scopes = if input.enabled_scopes.is_empty() {
        vec!["feishu".to_string()]
    } else {
        input
            .enabled_scopes
            .iter()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .map(|v| v.to_lowercase())
            .collect::<Vec<_>>()
    };
    let enabled_scopes_json = serde_json::to_string(&enabled_scopes).map_err(|e| e.to_string())?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    if input.is_default {
        sqlx::query("UPDATE agent_employees SET is_default = 0")
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    sqlx::query(
        "INSERT INTO agent_employees (
            id, employee_id, name, role_id, persona, feishu_open_id, feishu_app_id, feishu_app_secret, primary_skill_id, default_work_dir, openclaw_agent_id, routing_priority, enabled_scopes_json,
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
            updated_at = excluded.updated_at",
    )
    .bind(&id)
    .bind(&employee_id)
    .bind(input.name.trim())
    .bind(role_id)
    .bind(input.persona.trim())
    .bind(input.feishu_open_id.trim())
    .bind(input.feishu_app_id.trim())
    .bind(input.feishu_app_secret.trim())
    .bind(input.primary_skill_id.trim())
    .bind(default_work_dir)
    .bind(openclaw_agent_id)
    .bind(input.routing_priority)
    .bind(enabled_scopes_json)
    .bind(if input.enabled { 1 } else { 0 })
    .bind(if input.is_default { 1 } else { 0 })
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM agent_employee_skills WHERE employee_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    for (idx, skill_id) in input
        .skill_ids
        .iter()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .enumerate()
    {
        sqlx::query(
            "INSERT INTO agent_employee_skills (employee_id, skill_id, sort_order) VALUES (?, ?, ?)",
        )
        .bind(&id)
        .bind(skill_id)
        .bind(idx as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(id)
}

pub async fn delete_agent_employee_with_pool(
    pool: &SqlitePool,
    employee_id: &str,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM agent_employee_skills WHERE employee_id = ?")
        .bind(employee_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM im_thread_employee_bindings WHERE employee_id = ?")
        .bind(employee_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM im_thread_sessions WHERE employee_id = ?")
        .bind(employee_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM agent_employees WHERE id = ?")
        .bind(employee_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn bind_thread_employees_with_pool(
    pool: &SqlitePool,
    thread_id: &str,
    employee_ids: &[String],
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM im_thread_employee_bindings WHERE thread_id = ?")
        .bind(thread_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    for (idx, employee_id) in employee_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO im_thread_employee_bindings (thread_id, employee_id, enabled, role_order)
             VALUES (?, ?, 1, ?)",
        )
        .bind(thread_id)
        .bind(employee_id)
        .bind(idx as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_thread_employee_bindings_with_pool(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<ThreadEmployeeBinding, String> {
    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT employee_id
         FROM im_thread_employee_bindings
         WHERE thread_id = ? AND enabled = 1
         ORDER BY role_order ASC",
    )
    .bind(thread_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(ThreadEmployeeBinding {
        thread_id: thread_id.to_string(),
        employee_ids: rows.into_iter().map(|(id,)| id).collect(),
    })
}

pub async fn resolve_target_employees_for_event(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<AgentEmployee>, String> {
    let all_enabled = list_agent_employees_with_pool(pool)
        .await?
        .into_iter()
        .filter(|e| e.enabled)
        .collect::<Vec<_>>();

    if let Some(role_id) = event
        .role_id
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
    {
        let targeted = all_enabled
            .iter()
            .filter(|e| {
                e.feishu_open_id == role_id || e.role_id == role_id || e.employee_id == role_id
            })
            .cloned()
            .collect::<Vec<_>>();
        if !targeted.is_empty() {
            return Ok(targeted);
        }
    }

    let defaults = all_enabled
        .iter()
        .filter(|e| e.is_default)
        .cloned()
        .collect::<Vec<_>>();
    if !defaults.is_empty() {
        return Ok(vec![defaults[0].clone()]);
    }

    let binding = get_thread_employee_bindings_with_pool(pool, &event.thread_id).await?;
    if !binding.employee_ids.is_empty() {
        let targeted = all_enabled
            .iter()
            .filter(|e| binding.employee_ids.iter().any(|id| id == &e.id))
            .cloned()
            .collect::<Vec<_>>();
        if !targeted.is_empty() {
            return Ok(targeted);
        }
    }

    Ok(all_enabled.iter().take(1).cloned().collect())
}

pub async fn ensure_employee_sessions_for_event_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<EnsuredEmployeeSession>, String> {
    let employees = resolve_target_employees_for_event(pool, event).await?;
    if employees.is_empty() {
        return Ok(Vec::new());
    }

    let default_model_id = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM model_configs ORDER BY is_default DESC, rowid ASC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .map(|(id,)| id)
    .ok_or_else(|| "no model config found".to_string())?;

    let mut results = Vec::with_capacity(employees.len());
    for employee in employees {
        let route_session_key = build_route_session_key(event, &employee);
        let existing = sqlx::query_as::<_, (String,)>(
            "SELECT session_id FROM im_thread_sessions WHERE thread_id = ? AND employee_id = ? LIMIT 1",
        )
        .bind(&event.thread_id)
        .bind(&employee.id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

        let (session_id, created) = if let Some((session_id,)) = existing {
            (session_id, false)
        } else {
            let by_route = sqlx::query_as::<_, (String,)>(
                "SELECT session_id
                 FROM im_thread_sessions
                 WHERE employee_id = ? AND route_session_key = ?
                 ORDER BY updated_at DESC
                 LIMIT 1",
            )
            .bind(&employee.id)
            .bind(&route_session_key)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
            if let Some((session_id,)) = by_route {
                let now = chrono::Utc::now().to_rfc3339();
                sqlx::query(
                    "INSERT INTO im_thread_sessions (thread_id, employee_id, session_id, route_session_key, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?)
                     ON CONFLICT(thread_id, employee_id) DO UPDATE SET
                        session_id = excluded.session_id,
                        route_session_key = excluded.route_session_key,
                        updated_at = excluded.updated_at",
                )
                .bind(&event.thread_id)
                .bind(&employee.id)
                .bind(&session_id)
                .bind(&route_session_key)
                .bind(&now)
                .bind(&now)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
                (session_id, false)
            } else {
                let now = chrono::Utc::now().to_rfc3339();
                let session_id = Uuid::new_v4().to_string();
                let skill_id = if employee.primary_skill_id.trim().is_empty() {
                    "builtin-general".to_string()
                } else {
                    employee.primary_skill_id.clone()
                };

                sqlx::query(
                "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id)
                 VALUES (?, ?, ?, ?, ?, 'accept_edits', ?, ?)",
            )
            .bind(&session_id)
            .bind(&skill_id)
            .bind(format!("IM:{}@{}", employee.name, event.thread_id))
            .bind(&now)
            .bind(&default_model_id)
            .bind(employee.default_work_dir.trim())
            .bind(employee.employee_id.trim())
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

                sqlx::query(
                "INSERT INTO im_thread_sessions (thread_id, employee_id, session_id, route_session_key, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&event.thread_id)
            .bind(&employee.id)
            .bind(&session_id)
            .bind(&route_session_key)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

                (session_id, true)
            }
        };

        let _ = sqlx::query("UPDATE sessions SET employee_id = ? WHERE id = ?")
            .bind(employee.employee_id.trim())
            .bind(&session_id)
            .execute(pool)
            .await;

        results.push(EnsuredEmployeeSession {
            employee_id: employee.id.clone(),
            role_id: employee.role_id.clone(),
            employee_name: employee.name.clone(),
            session_id,
            created,
        });
    }

    Ok(results)
}

fn build_route_session_key(event: &ImEvent, employee: &AgentEmployee) -> String {
    let tenant = event
        .tenant_id
        .as_ref()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "default".to_string());
    let agent_id = if employee.openclaw_agent_id.trim().is_empty() {
        employee.employee_id.trim().to_lowercase()
    } else {
        employee.openclaw_agent_id.trim().to_lowercase()
    };
    format!("feishu:{}:{}", tenant, agent_id)
}

pub async fn link_inbound_event_to_session_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
    employee_id: &str,
    session_id: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO im_message_links (id, thread_id, session_id, employee_id, direction, im_event_id, im_message_id, app_message_id, created_at)
         VALUES (?, ?, ?, ?, 'inbound', ?, ?, '', ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&event.thread_id)
    .bind(session_id)
    .bind(employee_id)
    .bind(event.event_id.clone().unwrap_or_default())
    .bind(event.message_id.clone().unwrap_or_default())
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn list_agent_employees(db: State<'_, DbState>) -> Result<Vec<AgentEmployee>, String> {
    list_agent_employees_with_pool(&db.0).await
}

#[tauri::command]
pub async fn upsert_agent_employee(
    input: UpsertAgentEmployeeInput,
    db: State<'_, DbState>,
    relay: State<'_, crate::commands::feishu_gateway::FeishuEventRelayState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let id = upsert_agent_employee_with_pool(&db.0, input).await?;
    let _ = crate::commands::feishu_gateway::reconcile_feishu_employee_connections_with_pool(
        &db.0, None,
    )
    .await;
    let _ = crate::commands::feishu_gateway::start_feishu_event_relay_with_pool_and_app(
        &db.0,
        relay.inner().clone(),
        Some(app),
        None,
        Some(1500),
        Some(50),
    )
    .await;
    Ok(id)
}

#[tauri::command]
pub async fn delete_agent_employee(
    employee_id: String,
    db: State<'_, DbState>,
    relay: State<'_, crate::commands::feishu_gateway::FeishuEventRelayState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    delete_agent_employee_with_pool(&db.0, &employee_id).await?;
    let _ = crate::commands::feishu_gateway::reconcile_feishu_employee_connections_with_pool(
        &db.0, None,
    )
    .await;
    let _ = crate::commands::feishu_gateway::start_feishu_event_relay_with_pool_and_app(
        &db.0,
        relay.inner().clone(),
        Some(app),
        None,
        Some(1500),
        Some(50),
    )
    .await;
    Ok(())
}

#[tauri::command]
pub async fn bind_thread_employees(
    thread_id: String,
    employee_ids: Vec<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    bind_thread_employees_with_pool(&db.0, &thread_id, &employee_ids).await
}

#[tauri::command]
pub async fn get_thread_employee_bindings(
    thread_id: String,
    db: State<'_, DbState>,
) -> Result<ThreadEmployeeBinding, String> {
    get_thread_employee_bindings_with_pool(&db.0, &thread_id).await
}
