use crate::commands::skills::DbState;
use crate::employee_runtime_adapter::employee_adapter::{
    build_group_run_execute_targets, build_team_runtime_view,
};
use crate::im::types::ImEvent;
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use tauri::State;

#[path = "employee_agents/group_management.rs"]
mod group_management;
#[path = "employee_agents/group_run_entry.rs"]
mod group_run_entry;
#[path = "employee_agents/memory_commands.rs"]
mod memory_commands;
#[path = "employee_agents/repo.rs"]
mod repo;
#[path = "employee_agents/service.rs"]
mod service;
#[path = "employee_agents/tauri_commands.rs"]
mod tauri_commands;
#[path = "employee_agents/team_rules.rs"]
mod team_rules;
#[path = "employee_agents/test_support.rs"]
#[doc(hidden)]
pub mod test_support;
#[path = "employee_agents/types.rs"]
mod types;

pub(crate) use group_management::{
    clone_employee_group_template_with_pool, create_employee_group_with_pool,
    create_employee_team_with_pool, delete_employee_group_with_pool,
    list_employee_group_rules_with_pool, list_employee_group_runs_with_pool,
    list_employee_groups_with_pool,
};
pub(crate) use group_run_entry::{
    build_group_step_iteration_fallback_output, build_group_step_system_prompt,
    build_group_step_user_prompt, continue_employee_group_run_with_pool_and_journal,
    extract_assistant_text, maybe_handle_team_entry_session_message_with_pool,
    run_group_step_with_pool_and_journal, start_employee_group_run_with_pool_and_journal,
};
use team_rules::{group_rule_matches_relation_types, normalize_member_employee_ids};
use types::{default_group_execution_window, default_group_max_retry};
pub use types::{
    AgentEmployee, CloneEmployeeGroupTemplateInput, CreateEmployeeGroupInput,
    CreateEmployeeTeamInput, CreateEmployeeTeamRuleInput, EmployeeGroup, EmployeeGroupRule,
    EmployeeGroupRunEvent, EmployeeGroupRunResult, EmployeeGroupRunSnapshot, EmployeeGroupRunStep,
    EmployeeGroupRunSummary, EmployeeInboundDispatchSession, EmployeeMemoryExport,
    EmployeeMemoryExportFile, EmployeeMemorySkillStats, EmployeeMemoryStats,
    EnsuredEmployeeSession, GroupStepExecutionResult, SaveFeishuEmployeeAssociationInput,
    StartEmployeeGroupRunInput, UpsertAgentEmployeeInput,
};

async fn load_execute_reassignment_targets_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    dispatch_source_override: Option<&str>,
) -> Result<(Vec<String>, bool), String> {
    let row = sqlx::query(
        "SELECT g.id,
                COALESCE(g.member_employee_ids_json, '[]'),
                COALESCE(r.main_employee_id, ''),
                COALESCE(g.coordinator_employee_id, ''),
                COALESCE(g.entry_employee_id, ''),
                COALESCE(g.review_mode, 'none')
         FROM group_runs r
         INNER JOIN employee_groups g ON g.id = r.group_id
         WHERE r.id = ?",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "group run not found".to_string())?;

    let group_id: String = row.try_get(0).map_err(|e| e.to_string())?;
    let member_employee_ids_json: String = row.try_get(1).map_err(|e| e.to_string())?;
    let main_employee_id: String = row.try_get(2).map_err(|e| e.to_string())?;
    let coordinator_employee_id: String = row.try_get(3).map_err(|e| e.to_string())?;
    let entry_employee_id: String = row.try_get(4).map_err(|e| e.to_string())?;
    let review_mode: String = row.try_get(5).map_err(|e| e.to_string())?;
    let member_employee_ids =
        serde_json::from_str::<Vec<String>>(&member_employee_ids_json).unwrap_or_default();
    let normalized_member_ids = normalize_member_employee_ids(&member_employee_ids);
    let run_dispatch_source_employee_id = if main_employee_id.trim().is_empty() {
        coordinator_employee_id.trim().to_lowercase()
    } else {
        main_employee_id.trim().to_lowercase()
    };
    let dispatch_source_employee_id = if let Some(dispatch_source_override) =
        dispatch_source_override
            .map(str::trim)
            .filter(|value| !value.is_empty())
    {
        dispatch_source_override.to_lowercase()
    } else {
        run_dispatch_source_employee_id.clone()
    };

    let rules = list_employee_group_rules_with_pool(pool, &group_id).await?;
    let employees = service::list_agent_employees_with_pool(pool).await?;
    let team_runtime_view = build_team_runtime_view(
        &employees,
        &coordinator_employee_id,
        &entry_employee_id,
        &member_employee_ids,
        &review_mode,
        &rules,
        &[dispatch_source_employee_id, run_dispatch_source_employee_id],
    );
    let targets = build_group_run_execute_targets(&team_runtime_view);
    let has_execute_rules = !team_runtime_view.delegation_policy.targets.is_empty();
    if !has_execute_rules {
        return Ok((normalized_member_ids, false));
    }
    Ok((
        targets
            .into_iter()
            .map(|target| target.assignee_employee_id)
            .collect(),
        true,
    ))
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
    memory_root: &std::path::Path,
    employee_id: &str,
) -> std::path::PathBuf {
    let employee_bucket = sanitize_memory_bucket_component(employee_id, "employee");
    memory_root
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
    service::list_agent_employees_with_pool(pool).await
}

#[cfg(test)]
fn normalize_enabled_scopes_for_storage(enabled_scopes: &[String]) -> Vec<String> {
    service::normalize_enabled_scopes_for_storage(enabled_scopes)
}

pub async fn save_feishu_employee_association_with_pool(
    pool: &SqlitePool,
    input: SaveFeishuEmployeeAssociationInput,
) -> Result<(), String> {
    service::save_feishu_employee_association_with_pool(pool, input).await
}

pub async fn upsert_agent_employee_with_pool(
    pool: &SqlitePool,
    input: UpsertAgentEmployeeInput,
) -> Result<String, String> {
    service::upsert_agent_employee_with_pool(pool, input).await
}

pub async fn delete_agent_employee_with_pool(
    pool: &SqlitePool,
    employee_id: &str,
) -> Result<(), String> {
    service::delete_agent_employee_with_pool(pool, employee_id).await
}

pub async fn review_group_run_step_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    action: &str,
    comment: &str,
) -> Result<(), String> {
    service::review_group_run_step_with_pool(pool, run_id, action, comment).await
}

pub async fn pause_employee_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    reason: &str,
) -> Result<(), String> {
    service::pause_employee_group_run_with_pool(pool, run_id, reason).await
}

pub async fn resume_employee_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(), String> {
    service::resume_employee_group_run_with_pool(pool, run_id).await
}

pub async fn reassign_group_run_step_with_pool(
    pool: &SqlitePool,
    step_id: &str,
    assignee_employee_id: &str,
) -> Result<(), String> {
    service::reassign_group_run_step_with_pool(pool, step_id, assignee_employee_id).await
}

pub async fn get_employee_group_run_snapshot_with_pool(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<EmployeeGroupRunSnapshot>, String> {
    service::get_employee_group_run_snapshot_with_pool(pool, session_id).await
}

pub async fn cancel_employee_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(), String> {
    service::cancel_employee_group_run_with_pool(pool, run_id).await
}

pub async fn retry_employee_group_run_failed_steps_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(), String> {
    service::retry_employee_group_run_failed_steps_with_pool(pool, run_id).await
}

fn employee_scope_matches_event(employee: &AgentEmployee, event: &ImEvent) -> bool {
    let event_channel = event.channel.trim().to_lowercase();
    let normalized_event_channel = if event_channel.is_empty() {
        "app"
    } else {
        event_channel.as_str()
    };
    let normalized_scopes = if employee.enabled_scopes.is_empty() {
        vec!["app".to_string()]
    } else {
        employee
            .enabled_scopes
            .iter()
            .map(|scope| scope.trim().to_lowercase())
            .filter(|scope| !scope.is_empty())
            .collect::<Vec<_>>()
    };
    normalized_scopes.iter().any(|scope| {
        scope == normalized_event_channel || (scope == "app" && normalized_event_channel == "app")
    })
}

pub async fn resolve_target_employees_for_event(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<AgentEmployee>, String> {
    service::resolve_target_employees_for_event(pool, event).await
}

fn im_binding_matches_event(
    binding: &crate::commands::im_routing::ImRoutingBinding,
    event: &ImEvent,
) -> bool {
    if !binding.enabled {
        return false;
    }
    if !binding.channel.trim().is_empty()
        && !binding.channel.eq_ignore_ascii_case(event.channel.trim())
    {
        return false;
    }
    if !binding.account_id.trim().is_empty() {
        let tenant_id = event.tenant_id.as_deref().unwrap_or_default().trim();
        if !binding.account_id.trim().eq_ignore_ascii_case(tenant_id) {
            return false;
        }
    }
    if !binding.peer_kind.trim().is_empty() && !binding.peer_kind.eq_ignore_ascii_case("group") {
        return false;
    }
    if !binding.peer_id.trim().is_empty() && binding.peer_id.trim() != event.thread_id.trim() {
        return false;
    }
    true
}

pub async fn ensure_employee_sessions_for_event_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<EnsuredEmployeeSession>, String> {
    service::ensure_employee_sessions_for_event_with_pool(pool, event).await
}

fn build_route_session_key(event: &ImEvent, employee: &AgentEmployee) -> String {
    let channel = event.channel.trim().to_lowercase();
    let tenant = event
        .tenant_id
        .as_ref()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "default".to_string());
    let agent_id = if employee.openclaw_agent_id.trim().is_empty() {
        employee.role_id.trim().to_lowercase()
    } else {
        employee.openclaw_agent_id.trim().to_lowercase()
    };
    format!(
        "{}:{}:{}",
        if channel.is_empty() {
            "app"
        } else {
            channel.as_str()
        },
        tenant,
        agent_id
    )
}

pub async fn link_inbound_event_to_session_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
    employee_id: &str,
    session_id: &str,
) -> Result<(), String> {
    service::link_inbound_event_to_session_with_pool(pool, event, employee_id, session_id).await
}

pub async fn bridge_inbound_event_to_employee_sessions_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
    route_decision: Option<&Value>,
) -> Result<Vec<EmployeeInboundDispatchSession>, String> {
    service::bridge_inbound_event_to_employee_sessions_with_pool(pool, event, route_decision).await
}

#[tauri::command]
pub async fn get_employee_memory_stats(
    employee_id: String,
    skill_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<EmployeeMemoryStats, String> {
    memory_commands::get_employee_memory_stats(employee_id, skill_id, app).await
}

#[tauri::command]
pub async fn export_employee_memory(
    employee_id: String,
    skill_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<EmployeeMemoryExport, String> {
    memory_commands::export_employee_memory(employee_id, skill_id, app).await
}

#[tauri::command]
pub async fn clear_employee_memory(
    employee_id: String,
    skill_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<EmployeeMemoryStats, String> {
    memory_commands::clear_employee_memory(employee_id, skill_id, app).await
}

#[tauri::command]
pub async fn create_employee_group(
    input: CreateEmployeeGroupInput,
    db: State<'_, DbState>,
) -> Result<String, String> {
    tauri_commands::create_employee_group(input, db).await
}

#[tauri::command]
pub async fn create_employee_team(
    input: CreateEmployeeTeamInput,
    db: State<'_, DbState>,
) -> Result<String, String> {
    tauri_commands::create_employee_team(input, db).await
}

#[tauri::command]
pub async fn clone_employee_group_template(
    input: CloneEmployeeGroupTemplateInput,
    db: State<'_, DbState>,
) -> Result<String, String> {
    tauri_commands::clone_employee_group_template(input, db).await
}

#[tauri::command]
pub async fn list_employee_groups(db: State<'_, DbState>) -> Result<Vec<EmployeeGroup>, String> {
    tauri_commands::list_employee_groups(db).await
}

#[tauri::command]
pub async fn list_employee_group_runs(
    limit: Option<i64>,
    db: State<'_, DbState>,
) -> Result<Vec<EmployeeGroupRunSummary>, String> {
    tauri_commands::list_employee_group_runs(limit, db).await
}

#[tauri::command]
pub async fn list_employee_group_rules(
    group_id: String,
    db: State<'_, DbState>,
) -> Result<Vec<EmployeeGroupRule>, String> {
    tauri_commands::list_employee_group_rules(group_id, db).await
}

#[tauri::command]
pub async fn delete_employee_group(group_id: String, db: State<'_, DbState>) -> Result<(), String> {
    tauri_commands::delete_employee_group(group_id, db).await
}

#[tauri::command]
pub async fn start_employee_group_run(
    input: StartEmployeeGroupRunInput,
    db: State<'_, DbState>,
    journal: State<'_, crate::session_journal::SessionJournalStateHandle>,
) -> Result<EmployeeGroupRunResult, String> {
    tauri_commands::start_employee_group_run(input, db, journal).await
}

#[tauri::command]
pub async fn continue_employee_group_run(
    run_id: String,
    db: State<'_, DbState>,
    journal: State<'_, crate::session_journal::SessionJournalStateHandle>,
) -> Result<EmployeeGroupRunSnapshot, String> {
    tauri_commands::continue_employee_group_run(run_id, db, journal).await
}

#[tauri::command]
pub async fn run_group_step(
    step_id: String,
    db: State<'_, DbState>,
    journal: State<'_, crate::session_journal::SessionJournalStateHandle>,
) -> Result<GroupStepExecutionResult, String> {
    tauri_commands::run_group_step(step_id, db, journal).await
}

#[tauri::command]
pub async fn get_employee_group_run_snapshot(
    session_id: String,
    db: State<'_, DbState>,
) -> Result<Option<EmployeeGroupRunSnapshot>, String> {
    tauri_commands::get_employee_group_run_snapshot(session_id, db).await
}

#[tauri::command]
pub async fn cancel_employee_group_run(
    run_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    tauri_commands::cancel_employee_group_run(run_id, db).await
}

#[tauri::command]
pub async fn retry_employee_group_run_failed_steps(
    run_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    tauri_commands::retry_employee_group_run_failed_steps(run_id, db).await
}

#[tauri::command]
pub async fn review_group_run_step(
    run_id: String,
    action: String,
    comment: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    tauri_commands::review_group_run_step(run_id, action, comment, db).await
}

#[tauri::command]
pub async fn pause_employee_group_run(
    run_id: String,
    reason: Option<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    tauri_commands::pause_employee_group_run(run_id, reason, db).await
}

#[tauri::command]
pub async fn resume_employee_group_run(
    run_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    tauri_commands::resume_employee_group_run(run_id, db).await
}

#[tauri::command]
pub async fn reassign_group_run_step(
    step_id: String,
    assignee_employee_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    tauri_commands::reassign_group_run_step(step_id, assignee_employee_id, db).await
}

#[tauri::command]
pub async fn list_agent_employees(db: State<'_, DbState>) -> Result<Vec<AgentEmployee>, String> {
    tauri_commands::list_agent_employees(db).await
}

#[tauri::command]
pub async fn upsert_agent_employee(
    input: UpsertAgentEmployeeInput,
    db: State<'_, DbState>,
    relay: State<'_, crate::commands::feishu_gateway::FeishuEventRelayState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    tauri_commands::upsert_agent_employee(input, db, relay, app).await
}

#[tauri::command]
pub async fn save_feishu_employee_association(
    input: SaveFeishuEmployeeAssociationInput,
    db: State<'_, DbState>,
    relay: State<'_, crate::commands::feishu_gateway::FeishuEventRelayState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tauri_commands::save_feishu_employee_association(input, db, relay, app).await
}

#[tauri::command]
pub async fn delete_agent_employee(
    employee_id: String,
    db: State<'_, DbState>,
    relay: State<'_, crate::commands::feishu_gateway::FeishuEventRelayState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tauri_commands::delete_agent_employee(employee_id, db, relay, app).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn im_binding_matches_event_respects_non_feishu_channels() {
        let binding = crate::commands::im_routing::ImRoutingBinding {
            id: "binding-1".to_string(),
            agent_id: "architect".to_string(),
            channel: "wecom".to_string(),
            account_id: "tenant-wecom".to_string(),
            peer_kind: "group".to_string(),
            peer_id: "wecom-room-1".to_string(),
            guild_id: String::new(),
            team_id: String::new(),
            role_ids: Vec::new(),
            connector_meta: serde_json::json!({}),
            priority: 100,
            enabled: true,
            created_at: "2026-03-11T00:00:00Z".to_string(),
            updated_at: "2026-03-11T00:00:00Z".to_string(),
        };

        let wecom_event = crate::im::types::ImEvent {
            channel: "wecom".to_string(),
            event_type: crate::im::types::ImEventType::MessageCreated,
            thread_id: "wecom-room-1".to_string(),
            event_id: Some("evt-wecom".to_string()),
            message_id: Some("msg-wecom".to_string()),
            text: Some("企业微信消息".to_string()),
            role_id: None,
            account_id: Some("tenant-wecom".to_string()),
            tenant_id: Some("tenant-wecom".to_string()),
            sender_id: None,
            chat_type: Some("group".to_string()),
        };
        assert!(super::im_binding_matches_event(&binding, &wecom_event));

        let feishu_event = crate::im::types::ImEvent {
            channel: "feishu".to_string(),
            ..wecom_event.clone()
        };
        assert!(!super::im_binding_matches_event(&binding, &feishu_event));
    }

    #[test]
    fn build_route_session_key_uses_event_channel_namespace() {
        let employee = super::AgentEmployee {
            id: "emp-1".to_string(),
            employee_id: "main".to_string(),
            name: "主员工".to_string(),
            role_id: "main".to_string(),
            persona: String::new(),
            feishu_open_id: String::new(),
            feishu_app_id: String::new(),
            feishu_app_secret: String::new(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: String::new(),
            openclaw_agent_id: "main-agent".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["app".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: Vec::new(),
            created_at: "2026-03-11T00:00:00Z".to_string(),
            updated_at: "2026-03-11T00:00:00Z".to_string(),
        };

        let wecom_event = crate::im::types::ImEvent {
            channel: "wecom".to_string(),
            event_type: crate::im::types::ImEventType::MessageCreated,
            thread_id: "wecom-room-1".to_string(),
            event_id: Some("evt-wecom".to_string()),
            message_id: Some("msg-wecom".to_string()),
            text: Some("企业微信消息".to_string()),
            role_id: None,
            account_id: Some("tenant-wecom".to_string()),
            tenant_id: Some("tenant-wecom".to_string()),
            sender_id: None,
            chat_type: Some("group".to_string()),
        };

        assert_eq!(
            super::build_route_session_key(&wecom_event, &employee),
            "wecom:tenant-wecom:main-agent"
        );
    }

    #[test]
    fn build_route_session_key_defaults_empty_channel_to_app_namespace() {
        let employee = super::AgentEmployee {
            id: "emp-1".to_string(),
            employee_id: "main".to_string(),
            name: "主员工".to_string(),
            role_id: "main".to_string(),
            persona: String::new(),
            feishu_open_id: String::new(),
            feishu_app_id: String::new(),
            feishu_app_secret: String::new(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: String::new(),
            openclaw_agent_id: "main-agent".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["app".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: Vec::new(),
            created_at: "2026-03-11T00:00:00Z".to_string(),
            updated_at: "2026-03-11T00:00:00Z".to_string(),
        };

        let app_event = crate::im::types::ImEvent {
            channel: String::new(),
            event_type: crate::im::types::ImEventType::MessageCreated,
            thread_id: "room-1".to_string(),
            event_id: Some("evt-app".to_string()),
            message_id: Some("msg-app".to_string()),
            text: Some("本地消息".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: None,
            sender_id: None,
            chat_type: None,
        };

        assert_eq!(
            super::build_route_session_key(&app_event, &employee),
            "app:default:main-agent"
        );
    }

    #[test]
    fn normalize_enabled_scopes_defaults_to_app_scope() {
        assert_eq!(
            super::normalize_enabled_scopes_for_storage(&[]),
            vec!["app".to_string()]
        );
        assert_eq!(
            super::normalize_enabled_scopes_for_storage(&["wecom".to_string()]),
            vec!["wecom".to_string()]
        );
    }
}
