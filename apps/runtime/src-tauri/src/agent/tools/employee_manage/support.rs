use crate::commands::agent_profile::AgentProfileAnswerInput;
use crate::commands::employee_agents::{list_agent_employees_with_pool, AgentEmployee};
use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

pub(crate) const DEFAULT_PRIMARY_SKILL_ID: &str = "builtin-general";

pub(crate) fn parse_string_array(input: &Value, field: &str) -> Vec<String> {
    input[field]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn parse_profile_answers(input: &Value) -> Vec<AgentProfileAnswerInput> {
    input["profile_answers"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let key = item["key"].as_str().map(str::trim).unwrap_or("");
                    if key.is_empty() {
                        return None;
                    }
                    let question = item["question"]
                        .as_str()
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .unwrap_or(key)
                        .to_string();
                    let answer = item["answer"]
                        .as_str()
                        .map(str::trim)
                        .unwrap_or("")
                        .to_string();
                    Some(AgentProfileAnswerInput {
                        key: key.to_string(),
                        question,
                        answer,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn parse_optional_string(input: &Value, field: &str) -> Option<String> {
    input
        .as_object()
        .and_then(|obj| obj.get(field))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .map(ToString::to_string)
}

pub(crate) fn parse_optional_bool(input: &Value, field: &str) -> Option<bool> {
    input
        .as_object()
        .and_then(|obj| obj.get(field))
        .and_then(|v| v.as_bool())
}

pub(crate) fn dedupe_skill_ids(skill_ids: Vec<String>) -> Vec<String> {
    let mut seen_skill_ids = HashSet::new();
    skill_ids
        .into_iter()
        .filter(|id| seen_skill_ids.insert(id.to_lowercase()))
        .collect::<Vec<_>>()
}

pub(crate) async fn resolve_employee(
    pool: &SqlitePool,
    input: &Value,
    action: &str,
) -> std::result::Result<AgentEmployee, String> {
    let employee_db_id = input["employee_db_id"]
        .as_str()
        .map(str::trim)
        .unwrap_or("");
    let employee_id = input["employee_id"].as_str().map(str::trim).unwrap_or("");
    let employees = list_agent_employees_with_pool(pool).await?;

    if !employee_db_id.is_empty() {
        return employees
            .into_iter()
            .find(|item| item.id.eq_ignore_ascii_case(employee_db_id))
            .ok_or_else(|| format!("{action} 未找到对应员工"));
    }

    if !employee_id.is_empty() {
        return employees
            .into_iter()
            .find(|item| {
                item.id.eq_ignore_ascii_case(employee_id)
                    || item.employee_id.eq_ignore_ascii_case(employee_id)
                    || item.role_id.eq_ignore_ascii_case(employee_id)
            })
            .ok_or_else(|| format!("{action} 未找到对应员工"));
    }

    Err(format!("{action} 缺少 employee_db_id 或 employee_id 参数"))
}

pub(crate) fn normalize_employee_id(raw: &str) -> String {
    let mut out = String::new();
    let mut last_sep = false;
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_sep = false;
        } else if (ch == '-' || ch == '_' || ch == ' ') && !last_sep {
            out.push('_');
            last_sep = true;
        }
    }
    let normalized = out.trim_matches('_').to_string();
    if normalized.is_empty() {
        let id = Uuid::new_v4().to_string();
        format!("employee_{}", &id[..8])
    } else {
        normalized
    }
}

pub(crate) fn default_employee_work_dir(employee_id: &str) -> String {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join("WorkClaw")
        .join("workspace")
        .join("employees")
        .join(employee_id)
        .to_string_lossy()
        .to_string()
}
