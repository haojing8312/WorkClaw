use crate::commands::runtime_preferences::resolve_default_work_dir_with_pool;
use crate::commands::skills::DbState;
use crate::im::types::ImEvent;
use sqlx::{Row, SqlitePool};
use tauri::{Manager, State};
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
    #[serde(default = "default_routing_priority")]
    pub routing_priority: i64,
    pub enabled_scopes: Vec<String>,
    pub enabled: bool,
    pub is_default: bool,
    pub skill_ids: Vec<String>,
}

fn default_routing_priority() -> i64 {
    100
}

fn normalize_member_employee_ids(raw: &[String]) -> Vec<String> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in raw {
        let normalized = item.trim().to_lowercase();
        if normalized.is_empty() {
            continue;
        }
        if seen.insert(normalized.clone()) {
            out.push(normalized);
        }
    }
    out
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EnsuredEmployeeSession {
    pub employee_id: String,
    pub role_id: String,
    pub employee_name: String,
    pub session_id: String,
    pub created: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeGroup {
    pub id: String,
    pub name: String,
    pub coordinator_employee_id: String,
    pub member_employee_ids: Vec<String>,
    pub member_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CreateEmployeeGroupInput {
    pub name: String,
    pub coordinator_employee_id: String,
    pub member_employee_ids: Vec<String>,
}

fn default_group_execution_window() -> usize {
    3
}

fn default_group_max_retry() -> usize {
    1
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StartEmployeeGroupRunInput {
    pub group_id: String,
    pub user_goal: String,
    #[serde(default = "default_group_execution_window")]
    pub execution_window: usize,
    #[serde(default)]
    pub timeout_employee_ids: Vec<String>,
    #[serde(default = "default_group_max_retry")]
    pub max_retry_per_step: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeGroupRunStep {
    pub id: String,
    pub round_no: i64,
    pub assignee_employee_id: String,
    pub status: String,
    pub output: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeGroupRunResult {
    pub run_id: String,
    pub group_id: String,
    pub session_id: String,
    pub session_skill_id: String,
    pub state: String,
    pub current_round: i64,
    pub final_report: String,
    pub steps: Vec<EmployeeGroupRunStep>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeGroupRunSnapshot {
    pub run_id: String,
    pub group_id: String,
    pub session_id: String,
    pub state: String,
    pub current_round: i64,
    pub final_report: String,
    pub steps: Vec<EmployeeGroupRunStep>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeMemorySkillStats {
    pub skill_id: String,
    pub total_files: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeMemoryStats {
    pub employee_id: String,
    pub total_files: u64,
    pub total_bytes: u64,
    pub skills: Vec<EmployeeMemorySkillStats>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeMemoryExportFile {
    pub skill_id: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeMemoryExport {
    pub employee_id: String,
    pub skill_id: Option<String>,
    pub exported_at: String,
    pub total_files: u64,
    pub total_bytes: u64,
    pub files: Vec<EmployeeMemoryExportFile>,
}

fn sanitize_memory_bucket_component(raw: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut prev_sep = false;
    for ch in raw.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_sep = false;
            continue;
        }
        if !prev_sep {
            out.push('_');
            prev_sep = true;
        }
    }
    let normalized = out.trim_matches('_').to_string();
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized
    }
}

fn normalize_employee_id(employee_id: &str) -> Result<String, String> {
    let normalized = employee_id.trim().to_string();
    if normalized.is_empty() {
        return Err("employee_id is required".to_string());
    }
    Ok(normalized)
}

fn normalize_memory_skill_scope(skill_id: Option<&str>) -> Result<Option<String>, String> {
    let normalized = skill_id.map(|v| v.trim()).unwrap_or_default();
    if normalized.is_empty() {
        return Ok(None);
    }
    if normalized.contains("..") || normalized.contains('/') || normalized.contains('\\') {
        return Err("invalid skill_id".to_string());
    }
    Ok(Some(normalized.to_string()))
}

fn employee_memory_skills_root(
    app_data_dir: &std::path::Path,
    employee_id: &str,
) -> std::path::PathBuf {
    let employee_bucket = sanitize_memory_bucket_component(employee_id, "employee");
    app_data_dir
        .join("memory")
        .join("employees")
        .join(employee_bucket)
        .join("skills")
}

fn list_scope_skill_dirs(
    skills_root: &std::path::Path,
    skill_scope: Option<&str>,
) -> Result<Vec<(String, std::path::PathBuf)>, String> {
    if let Some(skill_id) = skill_scope {
        let skill_root = skills_root.join(skill_id);
        if !skill_root.exists() {
            return Ok(Vec::new());
        }
        return Ok(vec![(skill_id.to_string(), skill_root)]);
    }

    if !skills_root.exists() {
        return Ok(Vec::new());
    }

    let mut dirs = Vec::new();
    for entry in std::fs::read_dir(skills_root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_id = entry.file_name().to_string_lossy().to_string();
        if skill_id.trim().is_empty() {
            continue;
        }
        dirs.push((skill_id, path));
    }
    dirs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(dirs)
}

fn system_time_to_rfc3339(system_time: std::time::SystemTime) -> String {
    chrono::DateTime::<chrono::Utc>::from(system_time).to_rfc3339()
}

fn collect_skill_file_entries(
    skill_id: &str,
    skill_root: &std::path::Path,
    include_content: bool,
) -> Result<(u64, u64, Vec<EmployeeMemoryExportFile>), String> {
    let mut entries = Vec::new();
    if !skill_root.exists() {
        return Ok((0, 0, entries));
    }

    let mut total_files = 0u64;
    let mut total_bytes = 0u64;
    let mut stack = vec![skill_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for item in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            let path = item.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !path.is_file() {
                continue;
            }
            let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
            let size_bytes = metadata.len();
            total_files += 1;
            total_bytes += size_bytes;

            let relative_path = path
                .strip_prefix(skill_root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let modified_at = metadata.modified().ok().map(system_time_to_rfc3339);
            let content = if include_content {
                let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                String::from_utf8_lossy(&bytes).to_string()
            } else {
                String::new()
            };

            entries.push(EmployeeMemoryExportFile {
                skill_id: skill_id.to_string(),
                relative_path,
                size_bytes,
                modified_at,
                content,
            });
        }
    }
    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok((total_files, total_bytes, entries))
}

pub(crate) fn collect_employee_memory_stats_from_root(
    employee_id: &str,
    skill_id: Option<&str>,
    skills_root: &std::path::Path,
) -> Result<EmployeeMemoryStats, String> {
    let employee_id = normalize_employee_id(employee_id)?;
    let skill_scope = normalize_memory_skill_scope(skill_id)?;
    let skill_dirs = list_scope_skill_dirs(skills_root, skill_scope.as_deref())?;

    let mut total_files = 0u64;
    let mut total_bytes = 0u64;
    let mut skills = Vec::with_capacity(skill_dirs.len());
    for (skill_key, skill_root) in skill_dirs {
        let (files, bytes, _entries) = collect_skill_file_entries(&skill_key, &skill_root, false)?;
        total_files += files;
        total_bytes += bytes;
        skills.push(EmployeeMemorySkillStats {
            skill_id: skill_key,
            total_files: files,
            total_bytes: bytes,
        });
    }
    skills.sort_by(|a, b| a.skill_id.cmp(&b.skill_id));

    Ok(EmployeeMemoryStats {
        employee_id,
        total_files,
        total_bytes,
        skills,
    })
}

pub(crate) fn export_employee_memory_from_root(
    employee_id: &str,
    skill_id: Option<&str>,
    skills_root: &std::path::Path,
) -> Result<EmployeeMemoryExport, String> {
    let employee_id = normalize_employee_id(employee_id)?;
    let skill_scope = normalize_memory_skill_scope(skill_id)?;
    let skill_dirs = list_scope_skill_dirs(skills_root, skill_scope.as_deref())?;

    let mut total_files = 0u64;
    let mut total_bytes = 0u64;
    let mut files = Vec::new();
    for (skill_key, skill_root) in skill_dirs {
        let (file_count, byte_count, mut entries) =
            collect_skill_file_entries(&skill_key, &skill_root, true)?;
        total_files += file_count;
        total_bytes += byte_count;
        files.append(&mut entries);
    }
    files.sort_by(|a, b| {
        a.skill_id
            .cmp(&b.skill_id)
            .then_with(|| a.relative_path.cmp(&b.relative_path))
    });

    Ok(EmployeeMemoryExport {
        employee_id,
        skill_id: skill_scope,
        exported_at: chrono::Utc::now().to_rfc3339(),
        total_files,
        total_bytes,
        files,
    })
}

pub(crate) fn clear_employee_memory_from_root(
    employee_id: &str,
    skill_id: Option<&str>,
    skills_root: &std::path::Path,
) -> Result<(), String> {
    let _employee_id = normalize_employee_id(employee_id)?;
    let skill_scope = normalize_memory_skill_scope(skill_id)?;
    if let Some(skill) = skill_scope {
        let target = skills_root.join(skill);
        if target.exists() {
            std::fs::remove_dir_all(target).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    if skills_root.exists() {
        std::fs::remove_dir_all(skills_root).map_err(|e| e.to_string())?;
    }
    Ok(())
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
    let openclaw_agent_id = if input.openclaw_agent_id.trim().is_empty() {
        employee_id.clone()
    } else {
        input.openclaw_agent_id.trim().to_string()
    };
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

pub async fn create_employee_group_with_pool(
    pool: &SqlitePool,
    input: CreateEmployeeGroupInput,
) -> Result<String, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("group name is required".to_string());
    }

    let coordinator = input.coordinator_employee_id.trim().to_lowercase();
    if coordinator.is_empty() {
        return Err("coordinator_employee_id is required".to_string());
    }

    let members = normalize_member_employee_ids(&input.member_employee_ids);
    if members.is_empty() {
        return Err("member_employee_ids is required".to_string());
    }
    if members.len() > 10 {
        return Err("member_employee_ids cannot exceed 10".to_string());
    }
    if !members.iter().any(|m| m == &coordinator) {
        return Err("coordinator_employee_id must be included in members".to_string());
    }

    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let members_json = serde_json::to_string(&members).map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT INTO employee_groups (
            id, name, coordinator_employee_id, member_employee_ids_json, member_count, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(&coordinator)
    .bind(members_json)
    .bind(members.len() as i64)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(id)
}

pub async fn list_employee_groups_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<EmployeeGroup>, String> {
    let rows = sqlx::query(
        "SELECT id, name, coordinator_employee_id, member_employee_ids_json, member_count, created_at, updated_at
         FROM employee_groups
         ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let members_json: String = row
            .try_get("member_employee_ids_json")
            .map_err(|e| e.to_string())?;
        let members = serde_json::from_str::<Vec<String>>(&members_json).unwrap_or_default();
        out.push(EmployeeGroup {
            id: row.try_get("id").map_err(|e| e.to_string())?,
            name: row.try_get("name").map_err(|e| e.to_string())?,
            coordinator_employee_id: row
                .try_get("coordinator_employee_id")
                .map_err(|e| e.to_string())?,
            member_employee_ids: members,
            member_count: row.try_get("member_count").map_err(|e| e.to_string())?,
            created_at: row.try_get("created_at").map_err(|e| e.to_string())?,
            updated_at: row.try_get("updated_at").map_err(|e| e.to_string())?,
        });
    }
    Ok(out)
}

pub async fn delete_employee_group_with_pool(pool: &SqlitePool, group_id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM group_run_steps WHERE run_id IN (SELECT id FROM group_runs WHERE group_id = ?)")
        .bind(group_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM group_runs WHERE group_id = ?")
        .bind(group_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM employee_groups WHERE id = ?")
        .bind(group_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub async fn start_employee_group_run_with_pool(
    pool: &SqlitePool,
    input: StartEmployeeGroupRunInput,
) -> Result<EmployeeGroupRunResult, String> {
    let group_id = input.group_id.trim().to_string();
    if group_id.is_empty() {
        return Err("group_id is required".to_string());
    }
    let user_goal = input.user_goal.trim().to_string();
    if user_goal.is_empty() {
        return Err("user_goal is required".to_string());
    }

    let row = sqlx::query(
        "SELECT name, coordinator_employee_id, member_employee_ids_json
         FROM employee_groups WHERE id = ?",
    )
    .bind(&group_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "employee group not found".to_string())?;

    let group_name: String = row.try_get("name").map_err(|e| e.to_string())?;
    let coordinator_employee_id: String = row
        .try_get("coordinator_employee_id")
        .map_err(|e| e.to_string())?;
    let members_json: String = row
        .try_get("member_employee_ids_json")
        .map_err(|e| e.to_string())?;
    let member_employee_ids =
        serde_json::from_str::<Vec<String>>(&members_json).unwrap_or_default();

    let request = crate::agent::group_orchestrator::GroupRunRequest {
        group_id: group_id.clone(),
        coordinator_employee_id: coordinator_employee_id.clone(),
        member_employee_ids,
        user_goal: user_goal.clone(),
        execution_window: input.execution_window,
        timeout_employee_ids: input.timeout_employee_ids,
        max_retry_per_step: input.max_retry_per_step,
    };
    let outcome = crate::agent::group_orchestrator::simulate_group_run(request);
    let state = outcome
        .states
        .last()
        .copied()
        .unwrap_or(crate::agent::group_orchestrator::GroupRunState::Failed)
        .as_str()
        .to_string();
    let current_round = outcome.execution.iter().map(|step| step.round_no).max().unwrap_or(0);
    let now = chrono::Utc::now().to_rfc3339();
    let run_id = Uuid::new_v4().to_string();
    let (session_id, session_skill_id) = ensure_group_run_session_with_pool(
        pool,
        &coordinator_employee_id,
        &group_name,
        &now,
    )
    .await?;

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO group_runs (
            id, group_id, session_id, user_goal, state, current_round, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&run_id)
    .bind(&group_id)
    .bind(&session_id)
    .bind(&user_goal)
    .bind(&state)
    .bind(current_round)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    let user_msg_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, 'user', ?, ?)",
    )
    .bind(&user_msg_id)
    .bind(&session_id)
    .bind(&user_goal)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    let mut steps = Vec::with_capacity(outcome.execution.len());
    for execution in outcome.execution {
        let step_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO group_run_steps (
                id, run_id, round_no, assignee_employee_id, step_type, input, output, status, started_at, finished_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&step_id)
        .bind(&run_id)
        .bind(execution.round_no)
        .bind(&execution.assignee_employee_id)
        .bind("execute")
        .bind(&user_goal)
        .bind(&execution.output)
        .bind(&execution.status)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        steps.push(EmployeeGroupRunStep {
            id: step_id,
            round_no: execution.round_no,
            assignee_employee_id: execution.assignee_employee_id,
            status: execution.status,
            output: execution.output,
        });
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    let assistant_msg_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, 'assistant', ?, ?)",
    )
    .bind(&assistant_msg_id)
    .bind(&session_id)
    .bind(&outcome.final_report)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(EmployeeGroupRunResult {
        run_id,
        group_id,
        session_id,
        session_skill_id,
        state,
        current_round,
        final_report: outcome.final_report,
        steps,
    })
}

async fn ensure_group_run_session_with_pool(
    pool: &SqlitePool,
    coordinator_employee_id: &str,
    group_name: &str,
    now: &str,
) -> Result<(String, String), String> {
    let employee_row = sqlx::query(
        "SELECT primary_skill_id, default_work_dir
         FROM agent_employees
         WHERE lower(employee_id) = lower(?) OR lower(role_id) = lower(?)
         ORDER BY is_default DESC, updated_at DESC
         LIMIT 1",
    )
    .bind(coordinator_employee_id)
    .bind(coordinator_employee_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "coordinator employee not found".to_string())?;

    let skill_id_raw: String = employee_row
        .try_get("primary_skill_id")
        .map_err(|e| e.to_string())?;
    let default_work_dir: String = employee_row
        .try_get("default_work_dir")
        .map_err(|e| e.to_string())?;
    let session_skill_id = if skill_id_raw.trim().is_empty() {
        "builtin-general".to_string()
    } else {
        skill_id_raw.trim().to_string()
    };

    let model_row =
        sqlx::query("SELECT id FROM model_configs WHERE is_default = 1 ORDER BY rowid ASC LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    let model_id = if let Some(row) = model_row {
        row.try_get::<String, _>("id").map_err(|e| e.to_string())?
    } else {
        sqlx::query("SELECT id FROM model_configs ORDER BY rowid ASC LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?
            .and_then(|row| row.try_get::<String, _>("id").ok())
            .ok_or_else(|| "model config not found".to_string())?
    };

    let session_id = Uuid::new_v4().to_string();
    let title = format!("群组协作：{}", group_name.trim());
    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id)
         VALUES (?, ?, ?, ?, ?, 'accept_edits', ?, ?)",
    )
    .bind(&session_id)
    .bind(&session_skill_id)
    .bind(title)
    .bind(now)
    .bind(model_id)
    .bind(default_work_dir)
    .bind(coordinator_employee_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok((session_id, session_skill_id))
}

pub async fn get_employee_group_run_snapshot_with_pool(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<EmployeeGroupRunSnapshot>, String> {
    let run_row = sqlx::query(
        "SELECT id, group_id, session_id, state, current_round, user_goal
         FROM group_runs
         WHERE session_id = ?
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some(run_row) = run_row else {
        return Ok(None);
    };
    let run_id: String = run_row.try_get("id").map_err(|e| e.to_string())?;
    let group_id: String = run_row.try_get("group_id").map_err(|e| e.to_string())?;
    let run_session_id: String = run_row.try_get("session_id").map_err(|e| e.to_string())?;
    let state: String = run_row.try_get("state").map_err(|e| e.to_string())?;
    let current_round: i64 = run_row.try_get("current_round").map_err(|e| e.to_string())?;
    let user_goal: String = run_row.try_get("user_goal").map_err(|e| e.to_string())?;

    let step_rows = sqlx::query(
        "SELECT id, round_no, assignee_employee_id, status, output
         FROM group_run_steps
         WHERE run_id = ?
         ORDER BY round_no ASC, started_at ASC, id ASC",
    )
    .bind(&run_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let mut steps = Vec::with_capacity(step_rows.len());
    for row in step_rows {
        steps.push(EmployeeGroupRunStep {
            id: row.try_get("id").map_err(|e| e.to_string())?,
            round_no: row.try_get("round_no").map_err(|e| e.to_string())?,
            assignee_employee_id: row
                .try_get("assignee_employee_id")
                .map_err(|e| e.to_string())?,
            status: row.try_get("status").map_err(|e| e.to_string())?,
            output: row.try_get("output").map_err(|e| e.to_string())?,
        });
    }
    let completed = steps.iter().filter(|s| s.status == "completed").count();
    let final_report = format!(
        "计划：围绕“{}”共 {} 步。\n执行：已完成 {} 步。\n汇报：当前状态={}",
        user_goal,
        steps.len(),
        completed,
        state
    );
    Ok(Some(EmployeeGroupRunSnapshot {
        run_id,
        group_id,
        session_id: run_session_id,
        state,
        current_round,
        final_report,
        steps,
    }))
}

pub async fn cancel_employee_group_run_with_pool(pool: &SqlitePool, run_id: &str) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE group_runs
         SET state = 'cancelled', updated_at = ?
         WHERE id = ? AND state NOT IN ('done', 'failed', 'cancelled')",
    )
    .bind(&now)
    .bind(run_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn retry_employee_group_run_failed_steps_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(), String> {
    let failed_rows = sqlx::query(
        "SELECT id, output FROM group_run_steps WHERE run_id = ? AND status = 'failed'",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    if failed_rows.is_empty() {
        return Err("no failed steps to retry".to_string());
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    for row in failed_rows {
        let step_id: String = row.try_get("id").map_err(|e| e.to_string())?;
        let old_output: String = row.try_get("output").map_err(|e| e.to_string())?;
        let retried_output = if old_output.trim().is_empty() {
            "重试后完成".to_string()
        } else {
            format!("{old_output}\n重试后完成")
        };
        sqlx::query(
            "UPDATE group_run_steps
             SET status = 'completed', output = ?, finished_at = ?
             WHERE id = ?",
        )
        .bind(retried_output)
        .bind(&now)
        .bind(step_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    sqlx::query(
        "UPDATE group_runs
         SET state = 'done', current_round = current_round + 1, updated_at = ?
         WHERE id = ?",
    )
    .bind(&now)
    .bind(run_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn resolve_target_employees_for_event(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<AgentEmployee>, String> {
    fn text_mentioned(text_lower: &str, alias: &str) -> bool {
        let normalized = alias.trim().to_lowercase();
        if normalized.is_empty() {
            return false;
        }
        text_lower.contains(&format!("@{}", normalized))
    }

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

    if let Some(text) = event.text.as_ref().map(|v| v.trim()).filter(|v| !v.is_empty()) {
        let text_lower = text.to_lowercase();
        let targeted = all_enabled
            .iter()
            .filter(|e| {
                text_mentioned(&text_lower, &e.name)
                    || text_mentioned(&text_lower, &e.employee_id)
                    || text_mentioned(&text_lower, &e.role_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        if !targeted.is_empty() {
            // 1:1 路由模式下，文本 mention 命中时只取首个目标。
            return Ok(vec![targeted[0].clone()]);
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

    // 同一个 IM thread 尽量复用同一个 session_id，保证线程上下文连续。
    let mut shared_thread_session_id = sqlx::query_as::<_, (String,)>(
        "SELECT ts.session_id
         FROM im_thread_sessions ts
         INNER JOIN sessions s ON s.id = ts.session_id
         WHERE ts.thread_id = ?
         ORDER BY ts.updated_at DESC
         LIMIT 1",
    )
    .bind(&event.thread_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .map(|(sid,)| sid);

    let mut results = Vec::with_capacity(employees.len());
    for employee in employees {
        let route_session_key = build_route_session_key(event);
        let existing = sqlx::query_as::<_, (String, i64)>(
            "SELECT ts.session_id,
                    CASE WHEN s.id IS NULL THEN 0 ELSE 1 END AS session_exists
             FROM im_thread_sessions ts
             LEFT JOIN sessions s ON s.id = ts.session_id
             WHERE ts.thread_id = ? AND ts.employee_id = ?
             LIMIT 1",
        )
        .bind(&event.thread_id)
        .bind(&employee.id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

        let (session_id, created) = if let Some((session_id, session_exists)) = existing {
            if session_exists == 1 {
                (session_id, false)
            } else if let Some(shared_session_id) = shared_thread_session_id.clone() {
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
                .bind(&shared_session_id)
                .bind(&route_session_key)
                .bind(&now)
                .bind(&now)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
                (shared_session_id, false)
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
                (session_id, true)
            }
        } else if let Some(session_id) = shared_thread_session_id.clone() {
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
        };

        if shared_thread_session_id.is_none() {
            shared_thread_session_id = Some(session_id.clone());
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

fn build_route_session_key(event: &ImEvent) -> String {
    let tenant = event
        .tenant_id
        .as_ref()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "default".to_string());
    let thread = event.thread_id.trim().to_lowercase();
    format!("feishu:{}:thread:{}", tenant, thread)
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
pub async fn get_employee_memory_stats(
    employee_id: String,
    skill_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<EmployeeMemoryStats, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let normalized_employee_id = normalize_employee_id(&employee_id)?;
    let skills_root = employee_memory_skills_root(&app_data_dir, &normalized_employee_id);
    collect_employee_memory_stats_from_root(
        &normalized_employee_id,
        skill_id.as_deref(),
        &skills_root,
    )
}

#[tauri::command]
pub async fn export_employee_memory(
    employee_id: String,
    skill_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<EmployeeMemoryExport, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let normalized_employee_id = normalize_employee_id(&employee_id)?;
    let skills_root = employee_memory_skills_root(&app_data_dir, &normalized_employee_id);
    export_employee_memory_from_root(&normalized_employee_id, skill_id.as_deref(), &skills_root)
}

#[tauri::command]
pub async fn clear_employee_memory(
    employee_id: String,
    skill_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<EmployeeMemoryStats, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let normalized_employee_id = normalize_employee_id(&employee_id)?;
    let skills_root = employee_memory_skills_root(&app_data_dir, &normalized_employee_id);
    clear_employee_memory_from_root(&normalized_employee_id, skill_id.as_deref(), &skills_root)?;
    collect_employee_memory_stats_from_root(
        &normalized_employee_id,
        skill_id.as_deref(),
        &skills_root,
    )
}

#[tauri::command]
pub async fn create_employee_group(
    input: CreateEmployeeGroupInput,
    db: State<'_, DbState>,
) -> Result<String, String> {
    create_employee_group_with_pool(&db.0, input).await
}

#[tauri::command]
pub async fn list_employee_groups(db: State<'_, DbState>) -> Result<Vec<EmployeeGroup>, String> {
    list_employee_groups_with_pool(&db.0).await
}

#[tauri::command]
pub async fn delete_employee_group(group_id: String, db: State<'_, DbState>) -> Result<(), String> {
    delete_employee_group_with_pool(&db.0, &group_id).await
}

#[tauri::command]
pub async fn start_employee_group_run(
    input: StartEmployeeGroupRunInput,
    db: State<'_, DbState>,
) -> Result<EmployeeGroupRunResult, String> {
    start_employee_group_run_with_pool(&db.0, input).await
}

#[tauri::command]
pub async fn get_employee_group_run_snapshot(
    session_id: String,
    db: State<'_, DbState>,
) -> Result<Option<EmployeeGroupRunSnapshot>, String> {
    get_employee_group_run_snapshot_with_pool(&db.0, session_id.trim()).await
}

#[tauri::command]
pub async fn cancel_employee_group_run(run_id: String, db: State<'_, DbState>) -> Result<(), String> {
    cancel_employee_group_run_with_pool(&db.0, run_id.trim()).await
}

#[tauri::command]
pub async fn retry_employee_group_run_failed_steps(
    run_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    retry_employee_group_run_failed_steps_with_pool(&db.0, run_id.trim()).await
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

#[cfg(test)]
mod tests {
    use super::{
        clear_employee_memory_from_root, collect_employee_memory_stats_from_root,
        export_employee_memory_from_root, UpsertAgentEmployeeInput,
    };
    use std::fs;
    use tempfile::TempDir;

    fn build_skill_file(root: &std::path::Path, skill_id: &str, rel: &str, content: &str) {
        let path = root.join(skill_id).join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create dir");
        }
        fs::write(path, content).expect("write file");
    }

    #[test]
    fn collect_employee_memory_stats_supports_all_and_single_skill_scope() {
        let tmp = TempDir::new().expect("tmp");
        let skills_root = tmp.path().join("skills");
        build_skill_file(
            &skills_root,
            "skill-alpha",
            "roles/main/MEMORY.md",
            "alpha fact",
        );
        build_skill_file(&skills_root, "skill-beta", "sessions/t1.md", "beta fact");

        let all = collect_employee_memory_stats_from_root("sales_lead", None, &skills_root)
            .expect("collect all");
        assert_eq!(all.employee_id, "sales_lead");
        assert_eq!(all.total_files, 2);
        assert_eq!(all.skills.len(), 2);
        assert!(all.total_bytes >= 18);

        let alpha = collect_employee_memory_stats_from_root(
            "sales_lead",
            Some("skill-alpha"),
            &skills_root,
        )
        .expect("collect alpha");
        assert_eq!(alpha.total_files, 1);
        assert_eq!(alpha.skills.len(), 1);
        assert_eq!(alpha.skills[0].skill_id, "skill-alpha");
    }

    #[test]
    fn clear_employee_memory_respects_skill_scope() {
        let tmp = TempDir::new().expect("tmp");
        let skills_root = tmp.path().join("skills");
        build_skill_file(&skills_root, "skill-alpha", "roles/main/MEMORY.md", "alpha");
        build_skill_file(&skills_root, "skill-beta", "sessions/t1.md", "beta");

        clear_employee_memory_from_root("sales_lead", Some("skill-alpha"), &skills_root)
            .expect("clear alpha");

        let remained = collect_employee_memory_stats_from_root("sales_lead", None, &skills_root)
            .expect("collect remained");
        assert_eq!(remained.total_files, 1);
        assert_eq!(remained.skills.len(), 1);
        assert_eq!(remained.skills[0].skill_id, "skill-beta");

        clear_employee_memory_from_root("sales_lead", None, &skills_root).expect("clear all");
        let empty = collect_employee_memory_stats_from_root("sales_lead", None, &skills_root)
            .expect("collect empty");
        assert_eq!(empty.total_files, 0);
        assert_eq!(empty.total_bytes, 0);
        assert!(empty.skills.is_empty());
    }

    #[test]
    fn export_employee_memory_returns_structured_json_payload() {
        let tmp = TempDir::new().expect("tmp");
        let skills_root = tmp.path().join("skills");
        build_skill_file(
            &skills_root,
            "skill-alpha",
            "roles/main/MEMORY.md",
            "customer prefers weekly summary",
        );

        let exported =
            export_employee_memory_from_root("sales_lead", Some("skill-alpha"), &skills_root)
                .expect("export");
        assert_eq!(exported.employee_id, "sales_lead");
        assert_eq!(exported.total_files, 1);
        assert_eq!(exported.total_bytes, 31);
        assert_eq!(exported.files.len(), 1);
        assert_eq!(exported.files[0].skill_id, "skill-alpha");
        assert_eq!(exported.files[0].relative_path, "roles/main/MEMORY.md");
        assert_eq!(exported.files[0].content, "customer prefers weekly summary");
    }

    #[test]
    fn upsert_input_defaults_routing_priority_when_missing() {
        let payload = serde_json::json!({
            "employee_id": "project_manager",
            "name": "项目经理",
            "role_id": "project_manager",
            "persona": "负责推进交付",
            "feishu_open_id": "",
            "feishu_app_id": "",
            "feishu_app_secret": "",
            "primary_skill_id": "builtin-general",
            "default_work_dir": "",
            "openclaw_agent_id": "project_manager",
            "enabled_scopes": ["feishu"],
            "enabled": true,
            "is_default": false,
            "skill_ids": []
        });
        let parsed: UpsertAgentEmployeeInput =
            serde_json::from_value(payload).expect("deserialize upsert input");
        assert_eq!(parsed.routing_priority, 100);
    }
}
