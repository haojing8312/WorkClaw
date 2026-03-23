use super::super::repo::{
    clear_default_employee_flag, delete_agent_employee_record, find_employee_db_id_by_employee_id,
    list_agent_employee_rows, list_skill_ids_for_employee, replace_employee_skill_bindings,
    upsert_agent_employee_record, AgentEmployeeRow, UpsertAgentEmployeeRecordInput,
};
use super::super::{AgentEmployee, UpsertAgentEmployeeInput};
use crate::commands::runtime_preferences::resolve_default_work_dir_with_pool;
use sqlx::SqlitePool;
use std::path::PathBuf;
use uuid::Uuid;

pub(crate) fn normalize_enabled_scopes_for_storage(enabled_scopes: &[String]) -> Vec<String> {
    let normalized = enabled_scopes
        .iter()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_lowercase())
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        vec!["app".to_string()]
    } else {
        normalized
    }
}

pub(crate) fn resolve_employee_agent_id(
    employee_id: &str,
    role_id: &str,
    openclaw_agent_id: &str,
) -> String {
    let openclaw_agent_id = openclaw_agent_id.trim();
    if !openclaw_agent_id.is_empty() {
        return openclaw_agent_id.to_string();
    }
    let employee_id = employee_id.trim();
    if !employee_id.is_empty() {
        return employee_id.to_string();
    }
    role_id.trim().to_string()
}

pub(super) fn build_agent_employee(
    row: AgentEmployeeRow,
    skill_ids: Vec<String>,
) -> AgentEmployee {
    let enabled_scopes = serde_json::from_str::<Vec<String>>(&row.enabled_scopes_json)
        .unwrap_or_else(|_| vec!["app".to_string()]);
    let employee_id = if row.employee_id.trim().is_empty() {
        row.role_id.clone()
    } else {
        row.employee_id
    };

    AgentEmployee {
        id: row.id,
        employee_id,
        name: row.name,
        role_id: row.role_id,
        persona: row.persona,
        feishu_open_id: row.feishu_open_id,
        feishu_app_id: row.feishu_app_id,
        feishu_app_secret: row.feishu_app_secret,
        primary_skill_id: row.primary_skill_id,
        default_work_dir: row.default_work_dir,
        openclaw_agent_id: row.openclaw_agent_id,
        routing_priority: row.routing_priority,
        enabled_scopes,
        enabled: row.enabled,
        is_default: row.is_default,
        skill_ids,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub(crate) async fn list_agent_employees_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<AgentEmployee>, String> {
    let rows = list_agent_employee_rows(pool).await?;
    let mut result = Vec::with_capacity(rows.len());

    for row in rows {
        let skill_ids = list_skill_ids_for_employee(pool, &row.id).await?;
        result.push(build_agent_employee(row, skill_ids));
    }

    Ok(result)
}

pub(crate) async fn upsert_agent_employee_with_pool(
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

    let id = input.id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
    if let Some(existing_id) = find_employee_db_id_by_employee_id(pool, &employee_id).await? {
        if existing_id != id {
            return Err("employee employee_id already exists".to_string());
        }
    }

    let now = chrono::Utc::now().to_rfc3339();
    let default_work_dir = if input.default_work_dir.trim().is_empty() {
        let base = resolve_default_work_dir_with_pool(pool).await?;
        let employee_dir = PathBuf::from(base).join("employees").join(&employee_id);
        std::fs::create_dir_all(&employee_dir)
            .map_err(|e| format!("failed to create employee work dir: {e}"))?;
        employee_dir.to_string_lossy().to_string()
    } else {
        input.default_work_dir.trim().to_string()
    };

    let openclaw_agent_id = if input.openclaw_agent_id.trim().is_empty() {
        employee_id.clone()
    } else {
        input.openclaw_agent_id.trim().to_string()
    };
    let role_id = employee_id.as_str();
    let enabled_scopes = normalize_enabled_scopes_for_storage(&input.enabled_scopes);
    let enabled_scopes_json = serde_json::to_string(&enabled_scopes).map_err(|e| e.to_string())?;
    let skill_ids = input
        .skill_ids
        .iter()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    if input.is_default {
        clear_default_employee_flag(&mut tx).await?;
    }

    upsert_agent_employee_record(
        &mut tx,
        &UpsertAgentEmployeeRecordInput {
            id: &id,
            employee_id: &employee_id,
            name: input.name.trim(),
            role_id,
            persona: input.persona.trim(),
            feishu_open_id: input.feishu_open_id.trim(),
            feishu_app_id: input.feishu_app_id.trim(),
            feishu_app_secret: input.feishu_app_secret.trim(),
            primary_skill_id: input.primary_skill_id.trim(),
            default_work_dir: &default_work_dir,
            openclaw_agent_id: &openclaw_agent_id,
            routing_priority: input.routing_priority,
            enabled_scopes_json: &enabled_scopes_json,
            enabled: input.enabled,
            is_default: input.is_default,
            now: &now,
        },
    )
    .await?;

    replace_employee_skill_bindings(&mut tx, &id, &skill_ids).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(id)
}

pub(crate) async fn delete_agent_employee_with_pool(
    pool: &SqlitePool,
    employee_id: &str,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    delete_agent_employee_record(&mut tx, employee_id).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}
