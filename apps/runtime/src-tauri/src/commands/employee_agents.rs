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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EnsuredEmployeeSession {
    pub employee_id: String,
    pub role_id: String,
    pub employee_name: String,
    pub session_id: String,
    pub created: bool,
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
        "SELECT session_id
         FROM im_thread_sessions
         WHERE thread_id = ?
         ORDER BY updated_at DESC
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
