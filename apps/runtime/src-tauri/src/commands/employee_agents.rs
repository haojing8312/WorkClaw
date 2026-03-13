use crate::agent::permissions::PermissionMode;
use crate::agent::skill_config::SkillConfig;
use crate::agent::tools::{EmployeeManageTool, MemoryTool};
use crate::agent::{AgentExecutor, ToolRegistry};
use crate::commands::chat_runtime_io::extract_assistant_text_content;
use crate::commands::im_routing::list_im_routing_bindings_with_pool;
use crate::commands::models::resolve_default_model_id_with_pool;
use crate::commands::runtime_preferences::resolve_default_work_dir_with_pool;
use crate::commands::skills::DbState;
use crate::im::types::ImEvent;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;
use std::sync::Arc;
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

fn group_rule_allows_execute_reassignment(rule: &EmployeeGroupRule) -> bool {
    let relation_type = rule.relation_type.trim().to_lowercase();
    let phase_scope = rule.phase_scope.trim().to_lowercase();
    let relation_allowed = relation_type == "delegate" || relation_type == "handoff";
    let phase_allowed = phase_scope.is_empty()
        || phase_scope == "execute"
        || phase_scope == "all"
        || phase_scope == "*";
    relation_allowed && phase_allowed
}

fn group_rule_matches_phase_scope(rule: &EmployeeGroupRule, phase_scope: &str) -> bool {
    let normalized_phase_scope = rule.phase_scope.trim().to_lowercase();
    normalized_phase_scope.is_empty()
        || normalized_phase_scope == phase_scope
        || normalized_phase_scope == "all"
        || normalized_phase_scope == "*"
}

fn group_rule_matches_relation_types(rule: &EmployeeGroupRule, relation_types: &[&str]) -> bool {
    let normalized_relation_type = rule.relation_type.trim().to_lowercase();
    relation_types
        .iter()
        .any(|relation_type| normalized_relation_type == *relation_type)
}

fn resolve_group_planner_employee_id(
    entry_employee_id: &str,
    coordinator_employee_id: &str,
    rules: &[EmployeeGroupRule],
) -> String {
    if let Some(planner_employee_id) = rules
        .iter()
        .find(|rule| {
            group_rule_matches_relation_types(rule, &["review"])
                && group_rule_matches_phase_scope(rule, "plan")
                && !rule.from_employee_id.trim().is_empty()
        })
        .map(|rule| rule.from_employee_id.trim().to_lowercase())
    {
        return planner_employee_id;
    }

    let normalized_entry_employee_id = entry_employee_id.trim().to_lowercase();
    if !normalized_entry_employee_id.is_empty() {
        if let Some(planner_employee_id) = rules
            .iter()
            .find(|rule| {
                group_rule_matches_relation_types(rule, &["delegate", "handoff"])
                    && group_rule_matches_phase_scope(rule, "intake")
                    && rule
                        .from_employee_id
                        .trim()
                        .eq_ignore_ascii_case(&normalized_entry_employee_id)
                    && !rule.to_employee_id.trim().is_empty()
            })
            .map(|rule| rule.to_employee_id.trim().to_lowercase())
        {
            return planner_employee_id;
        }
        return normalized_entry_employee_id;
    }

    coordinator_employee_id.trim().to_lowercase()
}

fn resolve_group_reviewer_employee_id(
    review_mode: &str,
    planner_employee_id: &str,
    rules: &[EmployeeGroupRule],
) -> Option<String> {
    if review_mode.trim().eq_ignore_ascii_case("none") {
        return None;
    }

    let normalized_planner_employee_id = planner_employee_id.trim().to_lowercase();
    rules
        .iter()
        .find(|rule| {
            group_rule_matches_relation_types(rule, &["review"])
                && group_rule_matches_phase_scope(rule, "plan")
                && (!normalized_planner_employee_id.is_empty()
                    && rule
                        .from_employee_id
                        .trim()
                        .eq_ignore_ascii_case(&normalized_planner_employee_id))
                && !rule.to_employee_id.trim().is_empty()
        })
        .map(|rule| rule.to_employee_id.trim().to_lowercase())
        .or_else(|| {
            rules
                .iter()
                .find(|rule| {
                    group_rule_matches_relation_types(rule, &["review"])
                        && group_rule_matches_phase_scope(rule, "plan")
                        && !rule.to_employee_id.trim().is_empty()
                })
                .map(|rule| rule.to_employee_id.trim().to_lowercase())
        })
}

fn select_group_execute_dispatch_targets(
    rules: &[EmployeeGroupRule],
    member_employee_ids: &[String],
    preferred_dispatch_sources: &[String],
) -> (
    Vec<crate::agent::group_orchestrator::GroupRunExecuteTarget>,
    bool,
) {
    let member_set = normalize_member_employee_ids(member_employee_ids)
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    let execute_rules = rules
        .iter()
        .filter(|rule| group_rule_allows_execute_reassignment(rule))
        .filter_map(|rule| {
            let assignee_employee_id = rule.to_employee_id.trim().to_lowercase();
            if assignee_employee_id.is_empty() {
                return None;
            }
            if !member_set.is_empty() && !member_set.contains(&assignee_employee_id) {
                return None;
            }
            let dispatch_source_employee_id = rule.from_employee_id.trim().to_lowercase();
            if dispatch_source_employee_id.is_empty() {
                return None;
            }
            Some(crate::agent::group_orchestrator::GroupRunExecuteTarget {
                dispatch_source_employee_id,
                assignee_employee_id,
            })
        })
        .collect::<Vec<_>>();

    if execute_rules.is_empty() {
        return (Vec::new(), false);
    }

    let preferred_sources = preferred_dispatch_sources
        .iter()
        .map(|employee_id| employee_id.trim().to_lowercase())
        .filter(|employee_id| !employee_id.is_empty())
        .collect::<Vec<_>>();

    let selected_rules = preferred_sources
        .iter()
        .find_map(|dispatch_source_employee_id| {
            let matching_rules = execute_rules
                .iter()
                .filter(|target| target.dispatch_source_employee_id == *dispatch_source_employee_id)
                .cloned()
                .collect::<Vec<_>>();
            if matching_rules.is_empty() {
                None
            } else {
                Some(matching_rules)
            }
        })
        .unwrap_or_else(|| execute_rules.clone());

    let mut seen_assignees = std::collections::HashSet::new();
    (
        selected_rules
            .into_iter()
            .filter(|target| seen_assignees.insert(target.assignee_employee_id.clone()))
            .collect(),
        true,
    )
}

async fn load_execute_reassignment_targets_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    dispatch_source_override: Option<&str>,
) -> Result<(Vec<String>, bool), String> {
    let row = sqlx::query(
        "SELECT g.id,
                COALESCE(g.member_employee_ids_json, '[]'),
                COALESCE(r.main_employee_id, ''),
                COALESCE(g.coordinator_employee_id, '')
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
    let (targets, has_execute_rules) = select_group_execute_dispatch_targets(
        &rules,
        &member_employee_ids,
        &[dispatch_source_employee_id, run_dispatch_source_employee_id],
    );
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
    pub template_id: String,
    pub entry_employee_id: String,
    pub review_mode: String,
    pub execution_mode: String,
    pub visibility_mode: String,
    pub is_bootstrap_seeded: bool,
    pub config_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeGroupRule {
    pub id: String,
    pub group_id: String,
    pub from_employee_id: String,
    pub to_employee_id: String,
    pub relation_type: String,
    pub phase_scope: String,
    pub required: bool,
    pub priority: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CreateEmployeeGroupInput {
    pub name: String,
    pub coordinator_employee_id: String,
    pub member_employee_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CreateEmployeeTeamRuleInput {
    pub from_employee_id: String,
    pub to_employee_id: String,
    pub relation_type: String,
    pub phase_scope: String,
    pub required: bool,
    pub priority: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CreateEmployeeTeamInput {
    pub name: String,
    pub coordinator_employee_id: String,
    pub member_employee_ids: Vec<String>,
    #[serde(default)]
    pub entry_employee_id: String,
    #[serde(default)]
    pub planner_employee_id: String,
    #[serde(default)]
    pub reviewer_employee_id: String,
    #[serde(default = "default_team_review_mode")]
    pub review_mode: String,
    #[serde(default = "default_team_execution_mode")]
    pub execution_mode: String,
    #[serde(default = "default_team_visibility_mode")]
    pub visibility_mode: String,
    #[serde(default)]
    pub rules: Vec<CreateEmployeeTeamRuleInput>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CloneEmployeeGroupTemplateInput {
    pub source_group_id: String,
    pub name: String,
}

fn default_group_execution_window() -> usize {
    3
}

fn default_group_max_retry() -> usize {
    1
}

fn default_team_review_mode() -> String {
    "none".to_string()
}

fn default_team_execution_mode() -> String {
    "sequential".to_string()
}

fn default_team_visibility_mode() -> String {
    "internal".to_string()
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
    pub step_type: String,
    pub assignee_employee_id: String,
    pub dispatch_source_employee_id: String,
    pub session_id: String,
    pub attempt_no: i64,
    pub status: String,
    pub output_summary: String,
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
pub struct EmployeeGroupRunSummary {
    pub id: String,
    pub group_id: String,
    pub group_name: String,
    pub goal: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: String,
    pub session_id: String,
    pub session_skill_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeGroupRunSnapshot {
    pub run_id: String,
    pub group_id: String,
    pub session_id: String,
    pub state: String,
    pub current_round: i64,
    pub current_phase: String,
    pub review_round: i64,
    pub status_reason: String,
    pub waiting_for_employee_id: String,
    pub waiting_for_user: bool,
    pub final_report: String,
    pub steps: Vec<EmployeeGroupRunStep>,
    pub events: Vec<EmployeeGroupRunEvent>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeGroupRunEvent {
    pub id: String,
    pub step_id: String,
    pub event_type: String,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GroupStepExecutionResult {
    pub step_id: String,
    pub run_id: String,
    pub assignee_employee_id: String,
    pub session_id: String,
    pub status: String,
    pub output: String,
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
            .unwrap_or_else(|_| vec!["app".to_string()]);
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

fn normalize_enabled_scopes_for_storage(enabled_scopes: &[String]) -> Vec<String> {
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
    let enabled_scopes = normalize_enabled_scopes_for_storage(&input.enabled_scopes);
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

fn normalize_team_mode(
    raw: &str,
    allowed: &[&str],
    default_value: &str,
    field_name: &str,
) -> Result<String, String> {
    let normalized = raw.trim().to_lowercase();
    let value = if normalized.is_empty() {
        default_value.to_string()
    } else {
        normalized
    };
    if allowed.iter().any(|candidate| *candidate == value) {
        Ok(value)
    } else {
        Err(format!("invalid {field_name}"))
    }
}

fn build_default_employee_team_rules(
    coordinator_employee_id: &str,
    entry_employee_id: &str,
    planner_employee_id: &str,
    reviewer_employee_id: &str,
    review_mode: &str,
    member_employee_ids: &[String],
) -> Vec<CreateEmployeeTeamRuleInput> {
    let mut rules = Vec::new();
    let mut priority = 100_i64;

    if !entry_employee_id.is_empty() && entry_employee_id != planner_employee_id {
        rules.push(CreateEmployeeTeamRuleInput {
            from_employee_id: entry_employee_id.to_string(),
            to_employee_id: planner_employee_id.to_string(),
            relation_type: "delegate".to_string(),
            phase_scope: "intake".to_string(),
            required: true,
            priority,
        });
        priority += 10;
    }

    if !review_mode.eq_ignore_ascii_case("none")
        && !planner_employee_id.is_empty()
        && !reviewer_employee_id.is_empty()
        && planner_employee_id != reviewer_employee_id
    {
        rules.push(CreateEmployeeTeamRuleInput {
            from_employee_id: planner_employee_id.to_string(),
            to_employee_id: reviewer_employee_id.to_string(),
            relation_type: "review".to_string(),
            phase_scope: "plan".to_string(),
            required: true,
            priority,
        });
        priority += 10;
    }

    let management_ids = [entry_employee_id, planner_employee_id, reviewer_employee_id]
        .iter()
        .map(|employee_id| employee_id.trim().to_lowercase())
        .filter(|employee_id| !employee_id.is_empty())
        .collect::<std::collections::HashSet<_>>();
    let mut execute_targets = member_employee_ids
        .iter()
        .map(|employee_id| employee_id.trim().to_lowercase())
        .filter(|employee_id| !employee_id.is_empty())
        .filter(|employee_id| employee_id != coordinator_employee_id)
        .filter(|employee_id| !management_ids.contains(employee_id))
        .collect::<Vec<_>>();
    if execute_targets.is_empty() {
        execute_targets = member_employee_ids
            .iter()
            .map(|employee_id| employee_id.trim().to_lowercase())
            .filter(|employee_id| !employee_id.is_empty())
            .filter(|employee_id| employee_id != reviewer_employee_id)
            .filter(|employee_id| employee_id != entry_employee_id)
            .collect::<Vec<_>>();
    }
    if execute_targets.is_empty() && !coordinator_employee_id.is_empty() {
        execute_targets.push(coordinator_employee_id.to_string());
    }

    let mut seen_execute_targets = std::collections::HashSet::new();
    for execute_target in execute_targets {
        if !seen_execute_targets.insert(execute_target.clone()) {
            continue;
        }
        rules.push(CreateEmployeeTeamRuleInput {
            from_employee_id: coordinator_employee_id.to_string(),
            to_employee_id: execute_target,
            relation_type: "delegate".to_string(),
            phase_scope: "execute".to_string(),
            required: true,
            priority,
        });
        priority += 10;
    }

    if !entry_employee_id.is_empty() && entry_employee_id != coordinator_employee_id {
        rules.push(CreateEmployeeTeamRuleInput {
            from_employee_id: coordinator_employee_id.to_string(),
            to_employee_id: entry_employee_id.to_string(),
            relation_type: "report".to_string(),
            phase_scope: "finalize".to_string(),
            required: true,
            priority,
        });
    }

    rules
}

pub async fn create_employee_team_with_pool(
    pool: &SqlitePool,
    input: CreateEmployeeTeamInput,
) -> Result<String, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("team name is required".to_string());
    }

    let coordinator_employee_id = input.coordinator_employee_id.trim().to_lowercase();
    if coordinator_employee_id.is_empty() {
        return Err("coordinator_employee_id is required".to_string());
    }

    let member_employee_ids = normalize_member_employee_ids(&input.member_employee_ids);
    if member_employee_ids.is_empty() {
        return Err("member_employee_ids is required".to_string());
    }
    if member_employee_ids.len() > 10 {
        return Err("member_employee_ids cannot exceed 10".to_string());
    }
    if !member_employee_ids
        .iter()
        .any(|employee_id| employee_id == &coordinator_employee_id)
    {
        return Err("coordinator_employee_id must be included in members".to_string());
    }

    let entry_employee_id = if input.entry_employee_id.trim().is_empty() {
        coordinator_employee_id.clone()
    } else {
        input.entry_employee_id.trim().to_lowercase()
    };
    let planner_employee_id = if input.planner_employee_id.trim().is_empty() {
        entry_employee_id.clone()
    } else {
        input.planner_employee_id.trim().to_lowercase()
    };
    let reviewer_employee_id = input.reviewer_employee_id.trim().to_lowercase();
    for (field_name, employee_id) in [
        ("entry_employee_id", &entry_employee_id),
        ("planner_employee_id", &planner_employee_id),
        ("reviewer_employee_id", &reviewer_employee_id),
    ] {
        if !employee_id.is_empty()
            && !member_employee_ids
                .iter()
                .any(|member_id| member_id == employee_id)
        {
            return Err(format!("{field_name} must be included in members"));
        }
    }

    let review_mode = normalize_team_mode(
        &input.review_mode,
        &["none", "soft", "hard"],
        "none",
        "review_mode",
    )?;
    let execution_mode = normalize_team_mode(
        &input.execution_mode,
        &["sequential", "parallel"],
        "sequential",
        "execution_mode",
    )?;
    let visibility_mode = normalize_team_mode(
        &input.visibility_mode,
        &["internal", "shared"],
        "internal",
        "visibility_mode",
    )?;

    if !review_mode.eq_ignore_ascii_case("none") && reviewer_employee_id.is_empty() {
        return Err("reviewer_employee_id is required when review_mode is enabled".to_string());
    }

    let rules = if input.rules.is_empty() {
        build_default_employee_team_rules(
            &coordinator_employee_id,
            &entry_employee_id,
            &planner_employee_id,
            &reviewer_employee_id,
            &review_mode,
            &member_employee_ids,
        )
    } else {
        input
            .rules
            .into_iter()
            .map(|rule| CreateEmployeeTeamRuleInput {
                from_employee_id: rule.from_employee_id.trim().to_lowercase(),
                to_employee_id: rule.to_employee_id.trim().to_lowercase(),
                relation_type: rule.relation_type.trim().to_lowercase(),
                phase_scope: rule.phase_scope.trim().to_lowercase(),
                required: rule.required,
                priority: rule.priority,
            })
            .collect::<Vec<_>>()
    };

    let valid_members = member_employee_ids
        .iter()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    for rule in &rules {
        if rule.from_employee_id.is_empty()
            || rule.to_employee_id.is_empty()
            || rule.relation_type.is_empty()
            || rule.phase_scope.is_empty()
        {
            return Err("team rules require from/to/relation_type/phase_scope".to_string());
        }
        if !valid_members.contains(&rule.from_employee_id)
            || !valid_members.contains(&rule.to_employee_id)
        {
            return Err("team rules must reference members in the team".to_string());
        }
    }

    let executor_employee_ids = rules
        .iter()
        .filter(|rule| {
            group_rule_matches_relation_types(
                &EmployeeGroupRule {
                    id: String::new(),
                    group_id: String::new(),
                    from_employee_id: rule.from_employee_id.clone(),
                    to_employee_id: rule.to_employee_id.clone(),
                    relation_type: rule.relation_type.clone(),
                    phase_scope: rule.phase_scope.clone(),
                    required: rule.required,
                    priority: rule.priority,
                    created_at: String::new(),
                },
                &["delegate", "handoff"],
            ) && (rule.phase_scope == "execute"
                || rule.phase_scope == "all"
                || rule.phase_scope == "*")
        })
        .map(|rule| rule.to_employee_id.clone())
        .collect::<std::collections::HashSet<_>>();
    let mut role_entries = Vec::<Value>::new();
    let mut seen_role_keys = std::collections::HashSet::<String>::new();
    for (role_type, employee_id) in [
        ("entry", entry_employee_id.clone()),
        ("planner", planner_employee_id.clone()),
        ("reviewer", reviewer_employee_id.clone()),
        ("coordinator", coordinator_employee_id.clone()),
    ] {
        if employee_id.is_empty() {
            continue;
        }
        let dedupe_key = format!("{role_type}:{employee_id}");
        if !seen_role_keys.insert(dedupe_key) {
            continue;
        }
        role_entries.push(json!({
            "role_type": role_type,
            "employee_id": employee_id,
        }));
    }
    for employee_id in executor_employee_ids {
        let dedupe_key = format!("executor:{employee_id}");
        if !seen_role_keys.insert(dedupe_key) {
            continue;
        }
        role_entries.push(json!({
            "role_type": "executor",
            "employee_id": employee_id,
        }));
    }
    let config_json =
        serde_json::to_string(&json!({ "roles": role_entries })).map_err(|e| e.to_string())?;
    let member_employee_ids_json =
        serde_json::to_string(&member_employee_ids).map_err(|e| e.to_string())?;
    let group_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO employee_groups (
            id, name, coordinator_employee_id, member_employee_ids_json, member_count, template_id,
            entry_employee_id, review_mode, execution_mode, visibility_mode, is_bootstrap_seeded,
            config_json, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, '', ?, ?, ?, ?, 0, ?, ?, ?)",
    )
    .bind(&group_id)
    .bind(name)
    .bind(&coordinator_employee_id)
    .bind(&member_employee_ids_json)
    .bind(member_employee_ids.len() as i64)
    .bind(&entry_employee_id)
    .bind(&review_mode)
    .bind(&execution_mode)
    .bind(&visibility_mode)
    .bind(&config_json)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    for rule in rules {
        sqlx::query(
            "INSERT INTO employee_group_rules (
                id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&group_id)
        .bind(&rule.from_employee_id)
        .bind(&rule.to_employee_id)
        .bind(&rule.relation_type)
        .bind(&rule.phase_scope)
        .bind(if rule.required { 1_i64 } else { 0_i64 })
        .bind(rule.priority)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(group_id)
}

pub async fn clone_employee_group_template_with_pool(
    pool: &SqlitePool,
    input: CloneEmployeeGroupTemplateInput,
) -> Result<String, String> {
    let source_group_id = input.source_group_id.trim();
    if source_group_id.is_empty() {
        return Err("source_group_id is required".to_string());
    }
    let name = input.name.trim();
    if name.is_empty() {
        return Err("name is required".to_string());
    }

    let source_row = sqlx::query(
        "SELECT coordinator_employee_id, member_employee_ids_json, COALESCE(member_count, 0),
                COALESCE(template_id, ''), COALESCE(entry_employee_id, ''),
                COALESCE(review_mode, 'none'), COALESCE(execution_mode, 'sequential'),
                COALESCE(visibility_mode, 'internal'), COALESCE(config_json, '{}')
         FROM employee_groups
         WHERE id = ?",
    )
    .bind(source_group_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "source employee group not found".to_string())?;

    let coordinator_employee_id: String = source_row
        .try_get("coordinator_employee_id")
        .map_err(|e| e.to_string())?;
    let source_members_json: String = source_row
        .try_get("member_employee_ids_json")
        .map_err(|e| e.to_string())?;
    let source_members =
        serde_json::from_str::<Vec<String>>(&source_members_json).unwrap_or_default();
    let members = normalize_member_employee_ids(&source_members);
    if members.is_empty() {
        return Err("source group has no members".to_string());
    }

    let cloned_group_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let members_json = serde_json::to_string(&members).map_err(|e| e.to_string())?;
    let template_id: String = source_row.try_get(3).map_err(|e| e.to_string())?;
    let entry_employee_id: String = source_row.try_get(4).map_err(|e| e.to_string())?;
    let review_mode: String = source_row.try_get(5).map_err(|e| e.to_string())?;
    let execution_mode: String = source_row.try_get(6).map_err(|e| e.to_string())?;
    let visibility_mode: String = source_row.try_get(7).map_err(|e| e.to_string())?;
    let config_json: String = source_row.try_get(8).map_err(|e| e.to_string())?;

    let source_rules = sqlx::query(
        "SELECT from_employee_id, to_employee_id, relation_type, phase_scope, required, priority
         FROM employee_group_rules
         WHERE group_id = ?
         ORDER BY priority DESC, created_at ASC",
    )
    .bind(source_group_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO employee_groups (
            id, name, coordinator_employee_id, member_employee_ids_json, member_count,
            template_id, entry_employee_id, review_mode, execution_mode, visibility_mode,
            is_bootstrap_seeded, config_json, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&cloned_group_id)
    .bind(name)
    .bind(&coordinator_employee_id)
    .bind(members_json)
    .bind(members.len() as i64)
    .bind(&template_id)
    .bind(&entry_employee_id)
    .bind(&review_mode)
    .bind(&execution_mode)
    .bind(&visibility_mode)
    .bind(0_i64)
    .bind(&config_json)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    for row in source_rules {
        let from_employee_id: String =
            row.try_get("from_employee_id").map_err(|e| e.to_string())?;
        let to_employee_id: String = row.try_get("to_employee_id").map_err(|e| e.to_string())?;
        let relation_type: String = row.try_get("relation_type").map_err(|e| e.to_string())?;
        let phase_scope: String = row.try_get("phase_scope").map_err(|e| e.to_string())?;
        let required = row
            .try_get::<i64, _>("required")
            .map_err(|e| e.to_string())?;
        let priority: i64 = row.try_get("priority").map_err(|e| e.to_string())?;
        sqlx::query(
            "INSERT INTO employee_group_rules (
                id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&cloned_group_id)
        .bind(from_employee_id)
        .bind(to_employee_id)
        .bind(relation_type)
        .bind(phase_scope)
        .bind(required)
        .bind(priority)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(cloned_group_id)
}

pub async fn list_employee_groups_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<EmployeeGroup>, String> {
    let rows = sqlx::query(
        "SELECT id, name, coordinator_employee_id, member_employee_ids_json, member_count,
                COALESCE(template_id, ''), COALESCE(entry_employee_id, ''), COALESCE(review_mode, 'none'),
                COALESCE(execution_mode, 'sequential'), COALESCE(visibility_mode, 'internal'),
                COALESCE(is_bootstrap_seeded, 0), COALESCE(config_json, '{}'), created_at, updated_at
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
            template_id: row.try_get(5).map_err(|e| e.to_string())?,
            entry_employee_id: row.try_get(6).map_err(|e| e.to_string())?,
            review_mode: row.try_get(7).map_err(|e| e.to_string())?,
            execution_mode: row.try_get(8).map_err(|e| e.to_string())?,
            visibility_mode: row.try_get(9).map_err(|e| e.to_string())?,
            is_bootstrap_seeded: row.try_get::<i64, _>(10).map_err(|e| e.to_string())? != 0,
            config_json: row.try_get(11).map_err(|e| e.to_string())?,
            created_at: row.try_get("created_at").map_err(|e| e.to_string())?,
            updated_at: row.try_get("updated_at").map_err(|e| e.to_string())?,
        });
    }
    Ok(out)
}

fn summarize_group_run_status(state: &str) -> String {
    match state.trim().to_lowercase().as_str() {
        "done" => "completed".to_string(),
        "planning" | "executing" => "running".to_string(),
        other => other.to_string(),
    }
}

pub async fn list_employee_group_runs_with_pool(
    pool: &SqlitePool,
    limit: Option<i64>,
) -> Result<Vec<EmployeeGroupRunSummary>, String> {
    let normalized_limit = match limit.unwrap_or(10) {
        value if value > 0 => value.min(50),
        _ => 10,
    };

    let rows = sqlx::query(
        "SELECT r.id,
                r.group_id,
                g.name,
                COALESCE(r.user_goal, ''),
                COALESCE(r.state, ''),
                COALESCE(r.created_at, ''),
                COALESCE(r.updated_at, ''),
                COALESCE(r.session_id, ''),
                COALESCE(s.skill_id, '')
         FROM group_runs r
         INNER JOIN employee_groups g ON g.id = r.group_id
         LEFT JOIN sessions s ON s.id = r.session_id
         ORDER BY r.created_at DESC, r.id DESC
         LIMIT ?",
    )
    .bind(normalized_limit)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut runs = Vec::with_capacity(rows.len());
    for row in rows {
        let raw_state: String = row.try_get(4).map_err(|e| e.to_string())?;
        let finished_at = if matches!(raw_state.trim(), "done" | "failed" | "cancelled") {
            row.try_get::<String, _>(6).map_err(|e| e.to_string())?
        } else {
            String::new()
        };

        runs.push(EmployeeGroupRunSummary {
            id: row.try_get("id").map_err(|e| e.to_string())?,
            group_id: row.try_get("group_id").map_err(|e| e.to_string())?,
            group_name: row.try_get::<String, _>(2).map_err(|e| e.to_string())?,
            goal: row.try_get::<String, _>(3).map_err(|e| e.to_string())?,
            status: summarize_group_run_status(&raw_state),
            started_at: row.try_get::<String, _>(5).map_err(|e| e.to_string())?,
            finished_at,
            session_id: row.try_get::<String, _>(7).map_err(|e| e.to_string())?,
            session_skill_id: row.try_get::<String, _>(8).map_err(|e| e.to_string())?,
        });
    }

    Ok(runs)
}

pub async fn list_employee_group_rules_with_pool(
    pool: &SqlitePool,
    group_id: &str,
) -> Result<Vec<EmployeeGroupRule>, String> {
    let rows = sqlx::query(
        "SELECT id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
         FROM employee_group_rules
         WHERE group_id = ?
         ORDER BY priority DESC, created_at ASC",
    )
    .bind(group_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut rules = Vec::with_capacity(rows.len());
    for row in rows {
        rules.push(EmployeeGroupRule {
            id: row.try_get("id").map_err(|e| e.to_string())?,
            group_id: row.try_get("group_id").map_err(|e| e.to_string())?,
            from_employee_id: row.try_get("from_employee_id").map_err(|e| e.to_string())?,
            to_employee_id: row.try_get("to_employee_id").map_err(|e| e.to_string())?,
            relation_type: row.try_get("relation_type").map_err(|e| e.to_string())?,
            phase_scope: row.try_get("phase_scope").map_err(|e| e.to_string())?,
            required: row
                .try_get::<i64, _>("required")
                .map_err(|e| e.to_string())?
                != 0,
            priority: row.try_get("priority").map_err(|e| e.to_string())?,
            created_at: row.try_get("created_at").map_err(|e| e.to_string())?,
        });
    }

    Ok(rules)
}

pub async fn delete_employee_group_with_pool(
    pool: &SqlitePool,
    group_id: &str,
) -> Result<(), String> {
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
    start_employee_group_run_internal_with_pool(pool, input, None, true).await
}

async fn start_employee_group_run_internal_with_pool(
    pool: &SqlitePool,
    input: StartEmployeeGroupRunInput,
    preferred_session_id: Option<&str>,
    persist_user_message: bool,
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
        "SELECT name,
                coordinator_employee_id,
                member_employee_ids_json,
                COALESCE(review_mode, 'none'),
                COALESCE(entry_employee_id, '')
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
    let review_mode: String = row.try_get(3).map_err(|e| e.to_string())?;
    let entry_employee_id: String = row.try_get(4).map_err(|e| e.to_string())?;
    let member_employee_ids =
        serde_json::from_str::<Vec<String>>(&members_json).unwrap_or_default();
    let rules = list_employee_group_rules_with_pool(pool, &group_id).await?;
    let planner_employee_id =
        resolve_group_planner_employee_id(&entry_employee_id, &coordinator_employee_id, &rules);
    let reviewer_employee_id =
        resolve_group_reviewer_employee_id(&review_mode, &planner_employee_id, &rules);
    let (execute_targets, _) = select_group_execute_dispatch_targets(
        &rules,
        &member_employee_ids,
        &[
            coordinator_employee_id.clone(),
            planner_employee_id.clone(),
            entry_employee_id.clone(),
        ],
    );

    let plan = crate::agent::group_orchestrator::build_group_run_plan(
        crate::agent::group_orchestrator::GroupRunRequest {
            group_id: group_id.clone(),
            coordinator_employee_id: coordinator_employee_id.clone(),
            planner_employee_id: Some(planner_employee_id.clone()),
            reviewer_employee_id: reviewer_employee_id.clone(),
            member_employee_ids,
            execute_targets,
            user_goal: user_goal.clone(),
            execution_window: input.execution_window,
            timeout_employee_ids: input.timeout_employee_ids,
            max_retry_per_step: input.max_retry_per_step,
        },
    );
    let initial_report = plan.final_report.clone();
    let initial_state = plan.state.clone();
    let initial_round = plan.current_round;
    let now = chrono::Utc::now().to_rfc3339();
    let run_id = Uuid::new_v4().to_string();
    let (session_id, session_skill_id) = ensure_group_run_session_with_pool(
        pool,
        &coordinator_employee_id,
        &group_name,
        &now,
        preferred_session_id,
    )
    .await?;

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO group_runs (
            id, group_id, session_id, user_goal, state, current_round, current_phase, entry_session_id,
            main_employee_id, review_round, status_reason, template_version, waiting_for_employee_id, waiting_for_user,
            created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&run_id)
    .bind(&group_id)
    .bind(&session_id)
    .bind(&user_goal)
    .bind(&initial_state)
    .bind(initial_round)
    .bind(&plan.current_phase)
    .bind(&session_id)
    .bind(&coordinator_employee_id)
    .bind(0_i64)
    .bind("")
    .bind("")
    .bind(
        reviewer_employee_id
            .as_deref()
            .filter(|employee_id| !employee_id.trim().is_empty())
            .unwrap_or(coordinator_employee_id.as_str()),
    )
    .bind(0_i64)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    if persist_user_message {
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
    }

    for event in &plan.events {
        sqlx::query(
            "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
             VALUES (?, ?, '', ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&run_id)
        .bind(&event.event_type)
        .bind(&event.payload_json)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    for step in plan.steps {
        let step_id = Uuid::new_v4().to_string();
        let dispatch_source_employee_id = step.dispatch_source_employee_id.clone();
        sqlx::query(
            "INSERT INTO group_run_steps (
                id, run_id, round_no, parent_step_id, assignee_employee_id, dispatch_source_employee_id,
                phase, step_type, step_kind, input, input_summary, output, output_summary, status,
                requires_review, review_status, attempt_no, session_id, visibility, started_at, finished_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&step_id)
        .bind(&run_id)
        .bind(step.round_no)
        .bind("")
        .bind(&step.assignee_employee_id)
        .bind(&dispatch_source_employee_id)
        .bind(&step.phase)
        .bind(&step.step_type)
        .bind(&step.step_type)
        .bind(&user_goal)
        .bind(if step.step_type == "plan" {
            "已生成结构化计划"
        } else {
            ""
        })
        .bind(&step.output)
        .bind(&step.output)
        .bind(&step.status)
        .bind(if step.requires_review { 1_i64 } else { 0_i64 })
        .bind(&step.review_status)
        .bind(0_i64)
        .bind("")
        .bind("internal")
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        let step_event_payload = serde_json::json!({
            "phase": step.phase,
            "step_type": step.step_type,
            "assignee_employee_id": step.assignee_employee_id,
            "dispatch_source_employee_id": dispatch_source_employee_id,
            "status": step.status
        })
        .to_string();
        sqlx::query(
            "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
             VALUES (?, ?, ?, 'step_created', ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&run_id)
        .bind(&step_id)
        .bind(step_event_payload)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    let snapshot = continue_employee_group_run_with_pool(pool, &run_id).await?;
    if snapshot.state != "done" {
        append_group_run_assistant_message_with_pool(pool, &session_id, &initial_report).await?;
    }
    let final_snapshot = get_employee_group_run_snapshot_by_run_id_with_pool(pool, &run_id).await?;

    Ok(EmployeeGroupRunResult {
        run_id,
        group_id,
        session_id,
        session_skill_id,
        state: final_snapshot.state,
        current_round: final_snapshot.current_round,
        final_report: final_snapshot.final_report,
        steps: final_snapshot.steps,
    })
}

async fn ensure_group_run_session_with_pool(
    pool: &SqlitePool,
    coordinator_employee_id: &str,
    group_name: &str,
    now: &str,
    preferred_session_id: Option<&str>,
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

    if let Some(existing_session_id) = preferred_session_id
        .map(str::trim)
        .filter(|session_id| !session_id.is_empty())
    {
        let existing_skill_row = sqlx::query_as::<_, (String,)>(
            "SELECT COALESCE(skill_id, '') FROM sessions WHERE id = ?",
        )
        .bind(existing_session_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "preferred group run session not found".to_string())?;
        let existing_skill_id = if existing_skill_row.0.trim().is_empty() {
            session_skill_id.clone()
        } else {
            existing_skill_row.0.trim().to_string()
        };
        return Ok((existing_session_id.to_string(), existing_skill_id));
    }

    let model_id = resolve_default_model_id_with_pool(pool)
        .await?
        .ok_or_else(|| "model config not found".to_string())?;

    let session_id = Uuid::new_v4().to_string();
    let title = format!("群组协作：{}", group_name.trim());
    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id)
         VALUES (?, ?, ?, ?, ?, 'standard', ?, ?)",
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

async fn append_group_run_assistant_message_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    content: &str,
) -> Result<(), String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at)
         VALUES (?, ?, 'assistant', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(trimmed)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn ensure_group_step_session_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    assignee_employee_id: &str,
    now: &str,
) -> Result<String, String> {
    if let Some((session_id,)) = sqlx::query_as::<_, (String,)>(
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
    .map_err(|e| e.to_string())?
    {
        return Ok(session_id);
    }

    let employee_row = sqlx::query(
        "SELECT primary_skill_id, default_work_dir
         FROM agent_employees
         WHERE lower(employee_id) = lower(?) OR lower(role_id) = lower(?)
         ORDER BY is_default DESC, updated_at DESC
         LIMIT 1",
    )
    .bind(assignee_employee_id)
    .bind(assignee_employee_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "assignee employee not found".to_string())?;

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

    let model_id = resolve_default_model_id_with_pool(pool)
        .await?
        .ok_or_else(|| "model config not found".to_string())?;

    let session_id = Uuid::new_v4().to_string();
    let title = format!("群组执行:{}@{}", run_id, assignee_employee_id);
    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id)
         VALUES (?, ?, ?, ?, ?, 'standard', ?, ?)",
    )
    .bind(&session_id)
    .bind(&session_skill_id)
    .bind(title)
    .bind(now)
    .bind(model_id)
    .bind(default_work_dir)
    .bind(assignee_employee_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(session_id)
}

fn load_group_step_profile_markdown(employee: &AgentEmployee) -> String {
    if employee.default_work_dir.trim().is_empty() {
        return String::new();
    }

    let profile_dir = PathBuf::from(employee.default_work_dir.trim())
        .join("openclaw")
        .join(employee.employee_id.trim());
    let mut sections = Vec::new();
    for name in ["AGENTS.md", "SOUL.md", "USER.md"] {
        let path = profile_dir.join(name);
        if let Ok(content) = std::fs::read_to_string(path) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                sections.push(format!("## {name}\n{trimmed}"));
            }
        }
    }
    sections.join("\n\n")
}

fn default_group_step_allowed_tools() -> Vec<String> {
    vec![
        "read_file".to_string(),
        "write_file".to_string(),
        "glob".to_string(),
        "grep".to_string(),
        "edit".to_string(),
        "list_dir".to_string(),
        "file_stat".to_string(),
        "file_copy".to_string(),
        "bash".to_string(),
        "web_fetch".to_string(),
    ]
}

fn build_group_step_iteration_fallback_output(
    employee: &AgentEmployee,
    user_goal: &str,
    step_input: &str,
    error: &str,
) -> String {
    let focus = if step_input.trim().is_empty() {
        user_goal.trim()
    } else {
        step_input.trim()
    };
    let responsibility = if employee.persona.trim().is_empty() {
        format!("负责围绕“{}”完成分配到本岗位的执行项", focus)
    } else {
        employee.persona.trim().to_string()
    };
    format!(
        "{} ({}) 在执行步骤时触发了迭代上限，现切换为保守交付模式。\n- 当前步骤: {}\n- 岗位职责: {}\n- 对用户目标“{}”可立即提供: 基于本岗位职责给出能力范围说明、所需补充信息以及下一步执行建议。\n- 备注: {}",
        employee.name,
        employee.employee_id,
        focus,
        responsibility,
        user_goal.trim(),
        error.trim(),
    )
}

fn build_group_step_system_prompt(
    employee: &AgentEmployee,
    session_skill_id: &str,
) -> (String, Option<Vec<String>>, usize) {
    let skill_config = SkillConfig::parse(crate::builtin_skills::builtin_general_skill_markdown());
    let base_prompt = if skill_config.system_prompt.trim().is_empty() {
        "你是一名专业、可靠、注重交付结果的 AI 员工。".to_string()
    } else {
        skill_config.system_prompt.clone()
    };
    let profile_markdown = load_group_step_profile_markdown(employee);
    let mut sections = vec![
        base_prompt,
        "---".to_string(),
        "你当前正在复杂任务团队中，以真实员工身份执行内部步骤。".to_string(),
        format!("- 员工名称: {}", employee.name),
        format!("- employee_id: {}", employee.employee_id),
        format!("- role_id: {}", employee.role_id),
        format!(
            "- primary_skill_id: {}",
            if session_skill_id.trim().is_empty() {
                "builtin-general"
            } else {
                session_skill_id.trim()
            }
        ),
    ];
    if !employee.default_work_dir.trim().is_empty() {
        sections.push(format!("- 工作目录: {}", employee.default_work_dir.trim()));
    }
    if !employee.persona.trim().is_empty() {
        sections.push(format!("- 员工人设: {}", employee.persona.trim()));
    }
    sections.push(
        "执行要求:\n- 聚焦当前分配步骤\n- 优先直接用自然语言给出结论，只有在当前步骤明确需要读取文件、编辑文件、执行命令或抓取网页时才使用工具\n- 先给结论，再给关键依据或产出\n- 不要输出“模拟结果”或“占位结果”措辞".to_string(),
    );
    if !profile_markdown.is_empty() {
        sections.push(format!("员工资料:\n{profile_markdown}"));
    }
    (
        sections.join("\n"),
        Some(default_group_step_allowed_tools()),
        skill_config.max_iterations.unwrap_or(8),
    )
}

fn build_group_step_user_prompt(
    run_id: &str,
    step_id: &str,
    user_goal: &str,
    step_input: &str,
    employee: &AgentEmployee,
) -> String {
    let effective_input = if step_input.trim().is_empty() {
        user_goal.trim()
    } else {
        step_input.trim()
    };
    format!(
        "你正在执行多员工团队中的 execute 步骤。\n- run_id: {run_id}\n- step_id: {step_id}\n- 当前负责人: {} ({})\n- 用户总目标: {}\n- 当前步骤要求: {}\n\n请直接给出你的执行结果。如果信息不足，先指出缺口，再给最合理的下一步。",
        employee.name,
        employee.employee_id,
        user_goal.trim(),
        effective_input,
    )
}

fn extract_assistant_text(messages: &[Value]) -> String {
    messages
        .iter()
        .rev()
        .find_map(|message| {
            if message["role"].as_str() != Some("assistant") {
                return None;
            }
            if let Some(content) = message["content"].as_str() {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
            message["content"].as_array().map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|block| {
                        if block["type"].as_str() == Some("text") {
                            block["text"].as_str().map(str::trim).map(str::to_string)
                        } else {
                            None
                        }
                    })
                    .filter(|text| !text.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        })
        .unwrap_or_default()
}

async fn execute_group_step_in_employee_context_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    step_id: &str,
    session_id: &str,
    assignee_employee_id: &str,
    user_goal: &str,
    step_input: &str,
) -> Result<String, String> {
    let session_row = sqlx::query(
        "SELECT skill_id, model_id, COALESCE(work_dir, '')
         FROM sessions
         WHERE id = ?",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "group step session not found".to_string())?;

    let session_skill_id: String = session_row.try_get(0).map_err(|e| e.to_string())?;
    let model_id: String = session_row.try_get(1).map_err(|e| e.to_string())?;
    let work_dir: String = session_row.try_get(2).map_err(|e| e.to_string())?;

    let employee = list_agent_employees_with_pool(pool)
        .await?
        .into_iter()
        .find(|item| {
            item.employee_id.eq_ignore_ascii_case(assignee_employee_id)
                || item.role_id.eq_ignore_ascii_case(assignee_employee_id)
                || item.id.eq_ignore_ascii_case(assignee_employee_id)
        })
        .ok_or_else(|| "assignee employee not found".to_string())?;

    let model_row = sqlx::query(
        "SELECT api_format, base_url, model_name, api_key
         FROM model_configs
         WHERE id = ?",
    )
    .bind(&model_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "model config not found".to_string())?;
    let api_format: String = model_row.try_get(0).map_err(|e| e.to_string())?;
    let base_url: String = model_row.try_get(1).map_err(|e| e.to_string())?;
    let model_name: String = model_row.try_get(2).map_err(|e| e.to_string())?;
    let api_key: String = model_row.try_get(3).map_err(|e| e.to_string())?;

    let (system_prompt, allowed_tools, max_iterations) =
        build_group_step_system_prompt(&employee, &session_skill_id);
    let user_prompt =
        build_group_step_user_prompt(run_id, step_id, user_goal, step_input, &employee);

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at)
         VALUES (?, ?, 'user', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(&user_prompt)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    let history_rows = sqlx::query_as::<_, (String, String)>(
        "SELECT role, content FROM messages WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let messages: Vec<Value> = history_rows
        .into_iter()
        .map(|(role, content)| {
            let normalized_content = if role == "assistant" {
                extract_assistant_text_content(&content)
            } else {
                content
            };
            json!({ "role": role, "content": normalized_content })
        })
        .collect();

    let registry = Arc::new(ToolRegistry::with_standard_tools());
    let memory_root = if work_dir.trim().is_empty() {
        std::env::temp_dir().join("workclaw-group-run-memory")
    } else {
        PathBuf::from(work_dir.trim())
            .join("openclaw")
            .join(employee.employee_id.trim())
            .join("memory")
    };
    let memory_dir = memory_root.join(if session_skill_id.trim().is_empty() {
        "builtin-general"
    } else {
        session_skill_id.trim()
    });
    std::fs::create_dir_all(&memory_dir).map_err(|e| e.to_string())?;
    registry.register(Arc::new(MemoryTool::new(memory_dir)));
    registry.register(Arc::new(EmployeeManageTool::new(pool.clone())));

    let executor = AgentExecutor::with_max_iterations(Arc::clone(&registry), max_iterations);
    let final_messages = match executor
        .execute_turn(
            &api_format,
            &base_url,
            &api_key,
            &model_name,
            &system_prompt,
            messages,
            |_| {},
            None,
            None,
            allowed_tools.as_deref(),
            PermissionMode::Unrestricted,
            None,
            if work_dir.trim().is_empty() {
                None
            } else {
                Some(work_dir.clone())
            },
            Some(max_iterations),
            None,
            None,
            None,
        )
        .await
    {
        Ok(final_messages) => final_messages,
        Err(error) => {
            let error_text = error.to_string();
            if !error_text.contains("达到最大迭代次数") {
                return Err(error_text);
            }

            let fallback_output = build_group_step_iteration_fallback_output(
                &employee,
                user_goal,
                step_input,
                &error_text,
            );
            let finished_at = chrono::Utc::now().to_rfc3339();
            sqlx::query(
                "INSERT INTO messages (id, session_id, role, content, created_at)
                 VALUES (?, ?, 'assistant', ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(session_id)
            .bind(&fallback_output)
            .bind(&finished_at)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
            return Ok(fallback_output);
        }
    };

    let assistant_output = extract_assistant_text(&final_messages);
    if assistant_output.trim().is_empty() {
        return Err("employee step execution returned empty assistant output".to_string());
    }

    let finished_at = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at)
         VALUES (?, ?, 'assistant', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(&assistant_output)
    .bind(&finished_at)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(assistant_output)
}

async fn maybe_finalize_group_run_with_pool(pool: &SqlitePool, run_id: &str) -> Result<(), String> {
    let blocking_counts = sqlx::query(
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
    let execute_blocking = blocking_counts
        .try_get::<Option<i64>, _>("execute_blocking")
        .map_err(|e| e.to_string())?
        .unwrap_or(0);
    let review_blocking = blocking_counts
        .try_get::<Option<i64>, _>("review_blocking")
        .map_err(|e| e.to_string())?
        .unwrap_or(0);
    if execute_blocking > 0 || review_blocking > 0 {
        return Ok(());
    }

    let run_row = sqlx::query(
        "SELECT session_id, user_goal, state
         FROM group_runs
         WHERE id = ?",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "group run not found".to_string())?;
    let session_id: String = run_row.try_get(0).map_err(|e| e.to_string())?;
    let user_goal: String = run_row.try_get(1).map_err(|e| e.to_string())?;
    let state: String = run_row.try_get(2).map_err(|e| e.to_string())?;
    if state == "done" {
        return Ok(());
    }

    let execute_rows = sqlx::query(
        "SELECT assignee_employee_id, output
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute'
         ORDER BY round_no ASC, finished_at ASC, id ASC",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut summary_lines = vec![
        format!("计划：围绕“{}”的团队执行已完成。", user_goal.trim()),
        "执行：".to_string(),
    ];
    for row in execute_rows {
        let assignee_employee_id: String = row.try_get(0).map_err(|e| e.to_string())?;
        let output: String = row.try_get(1).map_err(|e| e.to_string())?;
        summary_lines.push(format!("- {}: {}", assignee_employee_id, output.trim()));
    }
    summary_lines.push("汇报：团队协作已完成，可继续进入人工复核或直接对外回复。".to_string());
    let final_report = summary_lines.join("\n");

    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at)
         VALUES (?, ?, 'assistant', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&session_id)
    .bind(&final_report)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
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
    .bind(&now)
    .bind(run_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
         VALUES (?, ?, '', 'run_completed', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(run_id)
    .bind(
        serde_json::json!({
            "state": "done",
            "phase": "finalize",
            "summary": final_report,
        })
        .to_string(),
    )
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn get_group_run_session_id_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<String, String> {
    sqlx::query_as::<_, (String,)>("SELECT session_id FROM group_runs WHERE id = ?")
        .bind(run_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .map(|(session_id,)| session_id)
        .ok_or_else(|| "group run not found".to_string())
}

async fn get_employee_group_run_snapshot_by_run_id_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<EmployeeGroupRunSnapshot, String> {
    let session_id = get_group_run_session_id_with_pool(pool, run_id).await?;
    get_employee_group_run_snapshot_with_pool(pool, &session_id)
        .await?
        .ok_or_else(|| "group run snapshot not found".to_string())
}

async fn get_group_run_reviewer_employee_id_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<String>, String> {
    let run_row = sqlx::query(
        "SELECT r.group_id, COALESCE(g.review_mode, 'none')
         FROM group_runs r
         INNER JOIN employee_groups g ON g.id = r.group_id
         WHERE r.id = ?",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "group run not found".to_string())?;
    let group_id: String = run_row.try_get(0).map_err(|e| e.to_string())?;
    let review_mode: String = run_row.try_get(1).map_err(|e| e.to_string())?;
    if review_mode.eq_ignore_ascii_case("none") {
        return Ok(None);
    }

    let reviewer = sqlx::query_as::<_, (String,)>(
        "SELECT to_employee_id
         FROM employee_group_rules
         WHERE group_id = ? AND relation_type = 'review'
         ORDER BY priority DESC, created_at ASC
         LIMIT 1",
    )
    .bind(&group_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .map(|(employee_id,)| employee_id.trim().to_string())
    .filter(|employee_id| !employee_id.is_empty());

    Ok(reviewer)
}

async fn advance_pending_plan_revision_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<bool, String> {
    let pending_plan_row = sqlx::query(
        "SELECT id, assignee_employee_id, COALESCE(input, ''), COALESCE(input_summary, '')
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'plan' AND status = 'pending'
         ORDER BY round_no DESC, id DESC
         LIMIT 1",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    let Some(pending_plan_row) = pending_plan_row else {
        return Ok(false);
    };

    let step_id: String = pending_plan_row.try_get(0).map_err(|e| e.to_string())?;
    let assignee_employee_id: String = pending_plan_row.try_get(1).map_err(|e| e.to_string())?;
    let step_input: String = pending_plan_row.try_get(2).map_err(|e| e.to_string())?;
    let revision_comment: String = pending_plan_row.try_get(3).map_err(|e| e.to_string())?;
    let reviewer_employee_id = get_group_run_reviewer_employee_id_with_pool(pool, run_id).await?;
    let now = chrono::Utc::now().to_rfc3339();
    let revision_output = if revision_comment.trim().is_empty() {
        "已重新整理计划，等待下一阶段推进".to_string()
    } else {
        format!("已根据审议意见修订计划：{}", revision_comment.trim())
    };

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE group_run_steps
         SET status = 'completed',
             output = ?,
             output_summary = ?,
             review_status = ?,
             started_at = CASE
               WHEN TRIM(started_at) = '' THEN ?
               ELSE started_at
             END,
             finished_at = ?
         WHERE id = ?",
    )
    .bind(&revision_output)
    .bind(&revision_output)
    .bind(if reviewer_employee_id.is_some() {
        "pending"
    } else {
        "not_required"
    })
    .bind(&now)
    .bind(&now)
    .bind(&step_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
         VALUES (?, ?, ?, 'step_completed', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(run_id)
    .bind(&step_id)
    .bind(
        serde_json::json!({
            "phase": "plan",
            "step_type": "plan",
            "assignee_employee_id": assignee_employee_id,
            "status": "completed",
            "revision_comment": revision_comment,
        })
        .to_string(),
    )
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    if let Some(reviewer_employee_id) = reviewer_employee_id {
        let review_step_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO group_run_steps (
                id, run_id, round_no, parent_step_id, assignee_employee_id, phase, step_type, step_kind,
                input, input_summary, output, output_summary, status, requires_review, review_status,
                attempt_no, session_id, visibility, started_at, finished_at
             ) VALUES (?, ?, ?, ?, ?, 'review', 'review', 'review', ?, ?, '等待审核计划', '', 'pending', 0, 'pending', 0, '', 'internal', '', '')",
        )
        .bind(&review_step_id)
        .bind(run_id)
        .bind(0_i64)
        .bind(&step_id)
        .bind(&reviewer_employee_id)
        .bind(&step_input)
        .bind(&revision_output)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        sqlx::query(
            "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
             VALUES (?, ?, ?, 'step_created', ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(run_id)
        .bind(&review_step_id)
        .bind(
            serde_json::json!({
                "phase": "review",
                "step_type": "review",
                "assignee_employee_id": reviewer_employee_id,
                "status": "pending",
            })
            .to_string(),
        )
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    sqlx::query(
        "UPDATE group_runs
         SET state = 'planning',
             current_phase = 'plan',
             waiting_for_employee_id = '',
             status_reason = '',
             updated_at = ?
         WHERE id = ?",
    )
    .bind(&now)
    .bind(run_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(true)
}

pub async fn continue_employee_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<EmployeeGroupRunSnapshot, String> {
    let normalized_run_id = run_id.trim();
    if normalized_run_id.is_empty() {
        return Err("run_id is required".to_string());
    }

    let run_row = sqlx::query(
        "SELECT state, COALESCE(current_phase, 'plan')
         FROM group_runs
         WHERE id = ?",
    )
    .bind(normalized_run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "group run not found".to_string())?;
    let state: String = run_row.try_get(0).map_err(|e| e.to_string())?;
    let current_phase: String = run_row.try_get(1).map_err(|e| e.to_string())?;

    if state == "paused" {
        return Err("group run is paused".to_string());
    }
    if state == "cancelled" || state == "done" {
        return get_employee_group_run_snapshot_by_run_id_with_pool(pool, normalized_run_id).await;
    }

    let _ = advance_pending_plan_revision_with_pool(pool, normalized_run_id).await?;

    if let Some(review_row) = sqlx::query(
        "SELECT id, assignee_employee_id
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'review' AND status IN ('pending', 'running', 'blocked')
         ORDER BY round_no DESC, id DESC
         LIMIT 1",
    )
    .bind(normalized_run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    {
        let review_step_id: String = review_row.try_get(0).map_err(|e| e.to_string())?;
        let reviewer_employee_id: String = review_row.try_get(1).map_err(|e| e.to_string())?;
        let default_reason = format!("等待{}审议", reviewer_employee_id.trim());
        let review_requested_exists = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*)
             FROM group_run_events
             WHERE run_id = ? AND step_id = ? AND event_type = 'review_requested'",
        )
        .bind(normalized_run_id)
        .bind(&review_step_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?
        .0;

        let now = chrono::Utc::now().to_rfc3339();
        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
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
        .bind(&reviewer_employee_id)
        .bind(&default_reason)
        .bind(&now)
        .bind(normalized_run_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        if review_requested_exists == 0 {
            sqlx::query(
                "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
                 VALUES (?, ?, ?, 'review_requested', ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(normalized_run_id)
            .bind(&review_step_id)
            .bind(
                serde_json::json!({
                    "assignee_employee_id": reviewer_employee_id,
                    "phase": "review",
                })
                .to_string(),
            )
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        }
        tx.commit().await.map_err(|e| e.to_string())?;
        return get_employee_group_run_snapshot_by_run_id_with_pool(pool, normalized_run_id).await;
    }

    let pending_execute_steps = sqlx::query_as::<_, (String,)>(
        "SELECT id
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute' AND status = 'pending'
         ORDER BY round_no ASC, id ASC",
    )
    .bind(normalized_run_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if pending_execute_steps.is_empty() && current_phase == "review" {
        return get_employee_group_run_snapshot_by_run_id_with_pool(pool, normalized_run_id).await;
    }

    for (step_id,) in pending_execute_steps {
        run_group_step_with_pool(pool, &step_id).await?;
    }
    maybe_finalize_group_run_with_pool(pool, normalized_run_id).await?;
    get_employee_group_run_snapshot_by_run_id_with_pool(pool, normalized_run_id).await
}

pub async fn run_group_step_with_pool(
    pool: &SqlitePool,
    step_id: &str,
) -> Result<GroupStepExecutionResult, String> {
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
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "group run step not found".to_string())?;

    let step_id: String = row.try_get(0).map_err(|e| e.to_string())?;
    let run_id: String = row.try_get(1).map_err(|e| e.to_string())?;
    let assignee_employee_id: String = row.try_get(2).map_err(|e| e.to_string())?;
    let dispatch_source_employee_id: String = row.try_get(3).map_err(|e| e.to_string())?;
    let step_type: String = row.try_get(4).map_err(|e| e.to_string())?;
    let existing_session_id: String = row.try_get(5).map_err(|e| e.to_string())?;
    let step_input: String = row.try_get(6).map_err(|e| e.to_string())?;
    let user_goal: String = row.try_get(7).map_err(|e| e.to_string())?;

    if step_type != "execute" {
        return Err("only execute steps can be run".to_string());
    }

    let now = chrono::Utc::now().to_rfc3339();
    let session_id = if existing_session_id.trim().is_empty() {
        ensure_group_step_session_with_pool(pool, &run_id, &assignee_employee_id, &now).await?
    } else {
        existing_session_id
    };

    let mut dispatch_tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE group_run_steps
         SET status = 'running',
             session_id = ?,
             started_at = CASE WHEN TRIM(started_at) = '' THEN ? ELSE started_at END,
             phase = CASE WHEN TRIM(phase) = '' THEN 'execute' ELSE phase END
         WHERE id = ?",
    )
    .bind(&session_id)
    .bind(&now)
    .bind(&step_id)
    .execute(&mut *dispatch_tx)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
         VALUES (?, ?, ?, 'step_dispatched', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&run_id)
    .bind(&step_id)
    .bind(
        serde_json::json!({
            "step_id": step_id,
            "session_id": session_id,
            "assignee_employee_id": assignee_employee_id,
            "dispatch_source_employee_id": dispatch_source_employee_id,
        })
        .to_string(),
    )
    .bind(&now)
    .execute(&mut *dispatch_tx)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE group_runs
         SET state = 'executing',
             current_phase = 'execute',
             waiting_for_employee_id = ?,
             status_reason = '',
             updated_at = ?
         WHERE id = ?",
    )
    .bind(&assignee_employee_id)
    .bind(&now)
    .bind(&run_id)
    .execute(&mut *dispatch_tx)
    .await
    .map_err(|e| e.to_string())?;
    dispatch_tx.commit().await.map_err(|e| e.to_string())?;

    let execution = execute_group_step_in_employee_context_with_pool(
        pool,
        &run_id,
        &step_id,
        &session_id,
        &assignee_employee_id,
        &user_goal,
        &step_input,
    )
    .await;

    let now = chrono::Utc::now().to_rfc3339();
    let output = match execution {
        Ok(output) => output,
        Err(error) => {
            let failed_summary = error.chars().take(120).collect::<String>();
            let mut failed_tx = pool.begin().await.map_err(|e| e.to_string())?;
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
            .bind(&error)
            .bind(&failed_summary)
            .bind(&session_id)
            .bind(&now)
            .bind(&step_id)
            .execute(&mut *failed_tx)
            .await
            .map_err(|e| e.to_string())?;
            sqlx::query(
                "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
                 VALUES (?, ?, ?, 'step_failed', ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&run_id)
            .bind(&step_id)
            .bind(
                serde_json::json!({
                    "step_id": step_id,
                    "session_id": session_id,
                    "status": "failed",
                    "error": error,
                    "assignee_employee_id": assignee_employee_id,
                    "dispatch_source_employee_id": dispatch_source_employee_id,
                })
                .to_string(),
            )
            .bind(&now)
            .execute(&mut *failed_tx)
            .await
            .map_err(|e| e.to_string())?;
            sqlx::query(
                "UPDATE group_runs
                 SET state = 'failed',
                     current_phase = 'execute',
                     waiting_for_employee_id = ?,
                     status_reason = ?,
                     updated_at = ?
                 WHERE id = ?",
            )
            .bind(&assignee_employee_id)
            .bind(&error)
            .bind(&now)
            .bind(&run_id)
            .execute(&mut *failed_tx)
            .await
            .map_err(|e| e.to_string())?;
            failed_tx.commit().await.map_err(|e| e.to_string())?;
            return Err(error);
        }
    };
    let output_summary = output.chars().take(120).collect::<String>();

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
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
    .bind(&output)
    .bind(&output_summary)
    .bind(&session_id)
    .bind(&now)
    .bind(&step_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
         VALUES (?, ?, ?, 'step_completed', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&run_id)
    .bind(&step_id)
    .bind(
        serde_json::json!({
            "step_id": step_id,
            "session_id": session_id,
            "status": "completed",
            "output_summary": output_summary,
            "assignee_employee_id": assignee_employee_id,
            "dispatch_source_employee_id": dispatch_source_employee_id,
        })
        .to_string(),
    )
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query(
        "UPDATE group_runs
         SET state = 'executing',
             current_phase = 'execute',
             status_reason = '',
             waiting_for_employee_id = '',
             updated_at = ?
         WHERE id = ?",
    )
    .bind(&now)
    .bind(&run_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;
    maybe_finalize_group_run_with_pool(pool, &run_id).await?;

    Ok(GroupStepExecutionResult {
        step_id,
        run_id,
        assignee_employee_id,
        session_id,
        status: "completed".to_string(),
        output,
    })
}

pub async fn review_group_run_step_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    action: &str,
    comment: &str,
) -> Result<(), String> {
    let normalized_action = action.trim().to_lowercase();
    if normalized_action != "approve" && normalized_action != "reject" {
        return Err("review action must be approve or reject".to_string());
    }

    let run_row = sqlx::query(
        "SELECT COALESCE(main_employee_id, ''), COALESCE(review_round, 0)
         FROM group_runs
         WHERE id = ?",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "group run not found".to_string())?;
    let main_employee_id: String = run_row.try_get(0).map_err(|e| e.to_string())?;
    let review_round: i64 = run_row.try_get(1).map_err(|e| e.to_string())?;

    let review_step_id = sqlx::query_as::<_, (String,)>(
        "SELECT id
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'review'
         ORDER BY round_no DESC, id DESC
         LIMIT 1",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .map(|(id,)| id)
    .ok_or_else(|| "review step not found".to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let review_status = if normalized_action == "approve" {
        "approved"
    } else {
        "rejected"
    };
    sqlx::query(
        "UPDATE group_run_steps
         SET status = 'completed',
             output = ?,
             output_summary = ?,
             review_status = ?,
             finished_at = ?
         WHERE id = ?",
    )
    .bind(comment)
    .bind(comment)
    .bind(review_status)
    .bind(&now)
    .bind(&review_step_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    if normalized_action == "reject" {
        let next_review_round = review_round + 1;
        let (revision_input, revision_assignee_employee_id) =
            sqlx::query_as::<_, (String, String)>(
                "SELECT COALESCE(input, ''), COALESCE(assignee_employee_id, '')
             FROM group_run_steps
             WHERE run_id = ? AND step_type = 'plan'
             ORDER BY round_no DESC, id DESC
             LIMIT 1",
            )
            .bind(run_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| (String::new(), main_employee_id.clone()));
        let revision_assignee_employee_id = if revision_assignee_employee_id.trim().is_empty() {
            main_employee_id.clone()
        } else {
            revision_assignee_employee_id.trim().to_lowercase()
        };
        let revision_step_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO group_run_steps (
                id, run_id, round_no, parent_step_id, assignee_employee_id, phase, step_type, step_kind,
                input, input_summary, output, output_summary, status, requires_review, review_status,
                attempt_no, session_id, visibility, started_at, finished_at
             ) VALUES (?, ?, ?, ?, ?, 'plan', 'plan', 'plan', ?, ?, '', '', 'pending', 1, 'pending', ?, '', 'internal', '', '')",
        )
        .bind(&revision_step_id)
        .bind(run_id)
        .bind(0_i64)
        .bind(&review_step_id)
        .bind(&revision_assignee_employee_id)
        .bind(&revision_input)
        .bind(comment)
        .bind(next_review_round)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        sqlx::query(
            "UPDATE group_runs
             SET state = 'planning',
                 current_phase = 'plan',
                 review_round = ?,
                 status_reason = ?,
                 waiting_for_employee_id = ?,
                 updated_at = ?
             WHERE id = ?",
        )
        .bind(next_review_round)
        .bind(comment)
        .bind(&revision_assignee_employee_id)
        .bind(&now)
        .bind(run_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        for (step_id, event_type, payload_json) in [
            (
                review_step_id.as_str(),
                "review_rejected",
                serde_json::json!({
                    "reason": comment,
                    "review_round": next_review_round,
                })
                .to_string(),
            ),
            (
                revision_step_id.as_str(),
                "step_created",
                serde_json::json!({
                    "phase": "plan",
                    "step_type": "plan",
                    "status": "pending",
                })
                .to_string(),
            ),
        ] {
            sqlx::query(
                "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(run_id)
            .bind(step_id)
            .bind(event_type)
            .bind(payload_json)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        }
    } else {
        sqlx::query(
            "UPDATE group_runs
             SET state = 'planning',
                 current_phase = 'execute',
                 status_reason = '',
                 waiting_for_employee_id = '',
                 updated_at = ?
             WHERE id = ?",
        )
        .bind(&now)
        .bind(run_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        sqlx::query(
            "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
             VALUES (?, ?, ?, 'review_passed', ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(run_id)
        .bind(&review_step_id)
        .bind(
            serde_json::json!({
                "comment": comment,
            })
            .to_string(),
        )
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn pause_employee_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    reason: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let result = sqlx::query(
        "UPDATE group_runs
         SET state = 'paused',
             status_reason = ?,
             updated_at = ?
         WHERE id = ? AND state NOT IN ('done', 'failed', 'cancelled', 'paused')",
    )
    .bind(reason.trim())
    .bind(&now)
    .bind(run_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if result.rows_affected() == 0 {
        return Err("group run is not pausable".to_string());
    }
    sqlx::query(
        "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
         VALUES (?, ?, '', 'run_paused', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(run_id)
    .bind(
        serde_json::json!({
            "reason": reason.trim(),
        })
        .to_string(),
    )
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn resume_employee_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(), String> {
    let run_row = sqlx::query(
        "SELECT state, COALESCE(current_phase, 'plan')
         FROM group_runs
         WHERE id = ?",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "group run not found".to_string())?;
    let state: String = run_row.try_get(0).map_err(|e| e.to_string())?;
    let current_phase: String = run_row.try_get(1).map_err(|e| e.to_string())?;
    if state != "paused" {
        return Err("group run is not paused".to_string());
    }

    let resumed_state = match current_phase.as_str() {
        "execute" => "executing",
        "review" => "waiting_review",
        _ => "planning",
    };
    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE group_runs
         SET state = ?,
             status_reason = '',
             updated_at = ?
         WHERE id = ?",
    )
    .bind(resumed_state)
    .bind(&now)
    .bind(run_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
         VALUES (?, ?, '', 'run_resumed', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(run_id)
    .bind(
        serde_json::json!({
            "state": resumed_state,
            "phase": current_phase,
        })
        .to_string(),
    )
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn reassign_group_run_step_with_pool(
    pool: &SqlitePool,
    step_id: &str,
    assignee_employee_id: &str,
) -> Result<(), String> {
    let new_assignee = assignee_employee_id.trim().to_lowercase();
    if new_assignee.is_empty() {
        return Err("assignee_employee_id is required".to_string());
    }

    let step_row = sqlx::query(
        "SELECT run_id, status, step_type, COALESCE(dispatch_source_employee_id, ''), COALESCE(assignee_employee_id, ''),
                COALESCE(output_summary, ''), COALESCE(output, '')
         FROM group_run_steps
         WHERE id = ?",
    )
    .bind(step_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "group run step not found".to_string())?;
    let run_id: String = step_row.try_get(0).map_err(|e| e.to_string())?;
    let status: String = step_row.try_get(1).map_err(|e| e.to_string())?;
    let step_type: String = step_row.try_get(2).map_err(|e| e.to_string())?;
    let dispatch_source_employee_id: String = step_row.try_get(3).map_err(|e| e.to_string())?;
    let previous_assignee_employee_id: String = step_row.try_get(4).map_err(|e| e.to_string())?;
    let previous_output_summary: String = step_row.try_get(5).map_err(|e| e.to_string())?;
    let previous_output: String = step_row.try_get(6).map_err(|e| e.to_string())?;
    if step_type != "execute" {
        return Err("only execute steps can be reassigned".to_string());
    }
    if status != "failed" && status != "pending" {
        return Err("only failed or pending steps can be reassigned".to_string());
    }

    let employee_exists = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM agent_employees
         WHERE lower(employee_id) = lower(?) OR lower(role_id) = lower(?)
         LIMIT 1",
    )
    .bind(&new_assignee)
    .bind(&new_assignee)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    if employee_exists.is_none() {
        return Err("target employee not found".to_string());
    }
    let (eligible_targets, has_execute_rules) = load_execute_reassignment_targets_with_pool(
        pool,
        &run_id,
        Some(dispatch_source_employee_id.as_str()),
    )
    .await?;
    if has_execute_rules
        && !eligible_targets
            .iter()
            .any(|candidate| candidate == &new_assignee)
    {
        return Err("target employee is not eligible for execute reassignment".to_string());
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
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
    .bind(&new_assignee)
    .bind(step_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    let remaining_failed_assignees = sqlx::query_as::<_, (String,)>(
        "SELECT assignee_employee_id
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute' AND status = 'failed'
         ORDER BY round_no ASC, id ASC",
    )
    .bind(&run_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if remaining_failed_assignees.is_empty() {
        sqlx::query(
            "UPDATE group_runs
             SET state = 'executing',
                 current_phase = 'execute',
                 waiting_for_employee_id = ?,
                 status_reason = '',
                 updated_at = ?
             WHERE id = ?",
        )
        .bind(&new_assignee)
        .bind(&now)
        .bind(&run_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    } else {
        let waiting_for_employee_id = remaining_failed_assignees[0].0.clone();
        let status_reason = format!(
            "{}执行失败",
            remaining_failed_assignees
                .iter()
                .map(|(assignee,)| assignee.as_str())
                .collect::<Vec<_>>()
                .join("、")
        );
        sqlx::query(
            "UPDATE group_runs
             SET state = 'failed',
                 current_phase = 'execute',
                 waiting_for_employee_id = ?,
                 status_reason = ?,
                 updated_at = ?
             WHERE id = ?",
        )
        .bind(&waiting_for_employee_id)
        .bind(&status_reason)
        .bind(&now)
        .bind(&run_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    sqlx::query(
        "INSERT INTO group_run_events (id, run_id, step_id, event_type, payload_json, created_at)
         VALUES (?, ?, ?, 'step_reassigned', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&run_id)
    .bind(step_id)
    .bind(
        serde_json::json!({
            "assignee_employee_id": new_assignee,
            "dispatch_source_employee_id": dispatch_source_employee_id,
            "previous_assignee_employee_id": previous_assignee_employee_id,
            "previous_output_summary": if previous_output_summary.trim().is_empty() {
                previous_output.chars().take(120).collect::<String>()
            } else {
                previous_output_summary
            },
        })
        .to_string(),
    )
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_employee_group_run_snapshot_with_pool(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<EmployeeGroupRunSnapshot>, String> {
    let run_row = sqlx::query(
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

    let Some(run_row) = run_row else {
        return Ok(None);
    };
    let run_id: String = run_row.try_get("id").map_err(|e| e.to_string())?;
    let group_id: String = run_row.try_get("group_id").map_err(|e| e.to_string())?;
    let run_session_id: String = run_row.try_get("session_id").map_err(|e| e.to_string())?;
    let state: String = run_row.try_get("state").map_err(|e| e.to_string())?;
    let current_round: i64 = run_row
        .try_get("current_round")
        .map_err(|e| e.to_string())?;
    let user_goal: String = run_row.try_get("user_goal").map_err(|e| e.to_string())?;
    let current_phase: String = run_row.try_get(6).map_err(|e| e.to_string())?;
    let review_round: i64 = run_row.try_get(7).map_err(|e| e.to_string())?;
    let status_reason: String = run_row.try_get(8).map_err(|e| e.to_string())?;
    let waiting_for_employee_id: String = run_row.try_get(9).map_err(|e| e.to_string())?;
    let waiting_for_user = run_row.try_get::<i64, _>(10).map_err(|e| e.to_string())? != 0;

    let step_rows = sqlx::query(
        "SELECT id, round_no, step_type, assignee_employee_id,
                COALESCE(dispatch_source_employee_id, ''), COALESCE(session_id, ''),
                COALESCE(attempt_no, 1), status, COALESCE(output_summary, ''), output
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
            step_type: row.try_get("step_type").map_err(|e| e.to_string())?,
            assignee_employee_id: row
                .try_get("assignee_employee_id")
                .map_err(|e| e.to_string())?,
            dispatch_source_employee_id: row.try_get(4).map_err(|e| e.to_string())?,
            session_id: row.try_get(5).map_err(|e| e.to_string())?,
            attempt_no: row.try_get(6).map_err(|e| e.to_string())?,
            status: row.try_get(7).map_err(|e| e.to_string())?,
            output_summary: row.try_get(8).map_err(|e| e.to_string())?,
            output: row.try_get(9).map_err(|e| e.to_string())?,
        });
    }
    let event_rows = sqlx::query(
        "SELECT id, COALESCE(step_id, ''), event_type, COALESCE(payload_json, '{}'), created_at
         FROM group_run_events
         WHERE run_id = ?
         ORDER BY created_at ASC, id ASC",
    )
    .bind(&run_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let mut events = Vec::with_capacity(event_rows.len());
    for row in event_rows {
        events.push(EmployeeGroupRunEvent {
            id: row.try_get("id").map_err(|e| e.to_string())?,
            step_id: row.try_get(1).map_err(|e| e.to_string())?,
            event_type: row.try_get("event_type").map_err(|e| e.to_string())?,
            payload_json: row.try_get(3).map_err(|e| e.to_string())?,
            created_at: row.try_get("created_at").map_err(|e| e.to_string())?,
        });
    }
    let completed = steps.iter().filter(|s| s.status == "completed").count();
    let final_report = sqlx::query_as::<_, (String,)>(
        "SELECT content
         FROM messages
         WHERE session_id = ? AND role = 'assistant'
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
    )
    .bind(&run_session_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .map(|(content,)| content)
    .filter(|content| !content.trim().is_empty())
    .unwrap_or_else(|| {
        format!(
            "计划：围绕“{}”共 {} 步。\n执行：已完成 {} 步。\n汇报：当前状态={}",
            user_goal,
            steps.len(),
            completed,
            state
        )
    });
    Ok(Some(EmployeeGroupRunSnapshot {
        run_id,
        group_id,
        session_id: run_session_id,
        state,
        current_round,
        current_phase,
        review_round,
        status_reason,
        waiting_for_employee_id,
        waiting_for_user,
        final_report,
        steps,
        events,
    }))
}

pub async fn cancel_employee_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(), String> {
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
    normalized_scopes
        .iter()
        .any(|scope| scope == "app" || scope == normalized_event_channel)
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
        .filter(|e| e.enabled && employee_scope_matches_event(e, event))
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

    if let Some(text) = event
        .text
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
    {
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

async fn resolve_team_entry_employee_for_event_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Option<AgentEmployee>, String> {
    let bindings = list_im_routing_bindings_with_pool(pool).await?;
    let matched_binding = bindings.into_iter().find(|binding| {
        !binding.team_id.trim().is_empty() && im_binding_matches_event(binding, event)
    });
    let Some(binding) = matched_binding else {
        return Ok(None);
    };

    let group_row = sqlx::query(
        "SELECT COALESCE(entry_employee_id, ''), coordinator_employee_id
         FROM employee_groups
         WHERE id = ?",
    )
    .bind(binding.team_id.trim())
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    let Some(group_row) = group_row else {
        return Ok(None);
    };
    let preferred_employee_id = {
        let entry_employee_id: String = group_row.try_get(0).map_err(|e| e.to_string())?;
        if entry_employee_id.trim().is_empty() {
            group_row
                .try_get::<String, _>(1)
                .map_err(|e| e.to_string())?
        } else {
            entry_employee_id
        }
    };

    Ok(list_agent_employees_with_pool(pool)
        .await?
        .into_iter()
        .find(|employee| {
            employee.enabled
                && employee_scope_matches_event(employee, event)
                && (employee
                    .employee_id
                    .eq_ignore_ascii_case(preferred_employee_id.trim())
                    || employee
                        .role_id
                        .eq_ignore_ascii_case(preferred_employee_id.trim())
                    || employee
                        .id
                        .eq_ignore_ascii_case(preferred_employee_id.trim()))
        }))
}

pub async fn maybe_handle_team_entry_session_message_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    user_message: &str,
) -> Result<Option<EmployeeGroupRunResult>, String> {
    let normalized_session_id = session_id.trim();
    if normalized_session_id.is_empty() {
        return Ok(None);
    }
    let normalized_user_message = user_message.trim();
    if normalized_user_message.is_empty() {
        return Ok(None);
    }

    let session_row = sqlx::query(
        "SELECT COALESCE(session_mode, 'general'), COALESCE(team_id, '')
         FROM sessions
         WHERE id = ?",
    )
    .bind(normalized_session_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    let Some(session_row) = session_row else {
        return Ok(None);
    };
    let session_mode: String = session_row.try_get(0).map_err(|e| e.to_string())?;
    let team_id: String = session_row.try_get(1).map_err(|e| e.to_string())?;
    if !session_mode.trim().eq_ignore_ascii_case("team_entry") || team_id.trim().is_empty() {
        return Ok(None);
    }

    let Some(group) = list_employee_groups_with_pool(pool)
        .await?
        .into_iter()
        .find(|group| group.id.eq_ignore_ascii_case(team_id.trim()))
    else {
        return Ok(None);
    };

    let result = start_employee_group_run_internal_with_pool(
        pool,
        StartEmployeeGroupRunInput {
            group_id: group.id,
            user_goal: normalized_user_message.to_string(),
            execution_window: default_group_execution_window(),
            timeout_employee_ids: Vec::new(),
            max_retry_per_step: default_group_max_retry(),
        },
        Some(normalized_session_id),
        false,
    )
    .await?;

    Ok(Some(result))
}

pub async fn ensure_employee_sessions_for_event_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<EnsuredEmployeeSession>, String> {
    let employees = if let Some(team_entry_employee) =
        resolve_team_entry_employee_for_event_with_pool(pool, event).await?
    {
        vec![team_entry_employee]
    } else {
        resolve_target_employees_for_event(pool, event).await?
    };
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
        let route_session_key = build_route_session_key(event, &employee);
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
                     VALUES (?, ?, ?, ?, ?, 'standard', ?, ?)",
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
            let by_route = sqlx::query_as::<_, (String,)>(
                "SELECT ts.session_id
                 FROM im_thread_sessions ts
                 INNER JOIN sessions s ON s.id = ts.session_id
                 WHERE ts.employee_id = ? AND ts.route_session_key = ?
                 ORDER BY ts.updated_at DESC
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
                     VALUES (?, ?, ?, ?, ?, 'standard', ?, ?)",
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
pub async fn create_employee_team(
    input: CreateEmployeeTeamInput,
    db: State<'_, DbState>,
) -> Result<String, String> {
    create_employee_team_with_pool(&db.0, input).await
}

#[tauri::command]
pub async fn clone_employee_group_template(
    input: CloneEmployeeGroupTemplateInput,
    db: State<'_, DbState>,
) -> Result<String, String> {
    clone_employee_group_template_with_pool(&db.0, input).await
}

#[tauri::command]
pub async fn list_employee_groups(db: State<'_, DbState>) -> Result<Vec<EmployeeGroup>, String> {
    list_employee_groups_with_pool(&db.0).await
}

#[tauri::command]
pub async fn list_employee_group_runs(
    limit: Option<i64>,
    db: State<'_, DbState>,
) -> Result<Vec<EmployeeGroupRunSummary>, String> {
    list_employee_group_runs_with_pool(&db.0, limit).await
}

#[tauri::command]
pub async fn list_employee_group_rules(
    group_id: String,
    db: State<'_, DbState>,
) -> Result<Vec<EmployeeGroupRule>, String> {
    list_employee_group_rules_with_pool(&db.0, group_id.trim()).await
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
pub async fn continue_employee_group_run(
    run_id: String,
    db: State<'_, DbState>,
) -> Result<EmployeeGroupRunSnapshot, String> {
    continue_employee_group_run_with_pool(&db.0, run_id.trim()).await
}

#[tauri::command]
pub async fn run_group_step(
    step_id: String,
    db: State<'_, DbState>,
) -> Result<GroupStepExecutionResult, String> {
    run_group_step_with_pool(&db.0, step_id.trim()).await
}

#[tauri::command]
pub async fn get_employee_group_run_snapshot(
    session_id: String,
    db: State<'_, DbState>,
) -> Result<Option<EmployeeGroupRunSnapshot>, String> {
    get_employee_group_run_snapshot_with_pool(&db.0, session_id.trim()).await
}

#[tauri::command]
pub async fn cancel_employee_group_run(
    run_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
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
pub async fn review_group_run_step(
    run_id: String,
    action: String,
    comment: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    review_group_run_step_with_pool(&db.0, run_id.trim(), action.trim(), comment.trim()).await
}

#[tauri::command]
pub async fn pause_employee_group_run(
    run_id: String,
    reason: Option<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    pause_employee_group_run_with_pool(&db.0, run_id.trim(), reason.as_deref().unwrap_or("")).await
}

#[tauri::command]
pub async fn resume_employee_group_run(
    run_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    resume_employee_group_run_with_pool(&db.0, run_id.trim()).await
}

#[tauri::command]
pub async fn reassign_group_run_step(
    step_id: String,
    assignee_employee_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    reassign_group_run_step_with_pool(&db.0, step_id.trim(), assignee_employee_id.trim()).await
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
            "enabled_scopes": ["app"],
            "enabled": true,
            "is_default": false,
            "skill_ids": []
        });
        let parsed: UpsertAgentEmployeeInput =
            serde_json::from_value(payload).expect("deserialize upsert input");
        assert_eq!(parsed.routing_priority, 100);
    }

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
