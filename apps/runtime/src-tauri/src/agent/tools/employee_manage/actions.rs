use super::support::{
    default_employee_work_dir, dedupe_skill_ids, normalize_employee_id, parse_optional_bool,
    parse_optional_string, parse_profile_answers, parse_string_array, resolve_employee,
    DEFAULT_PRIMARY_SKILL_ID,
};
use crate::commands::agent_profile::{apply_agent_profile_with_pool, AgentProfilePayload};
use crate::commands::employee_agents::{
    list_agent_employees_with_pool, upsert_agent_employee_with_pool, UpsertAgentEmployeeInput,
};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::HashSet;

pub(crate) async fn list_skills(pool: SqlitePool) -> std::result::Result<Value, String> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, manifest, COALESCE(source_type, 'encrypted') FROM installed_skills ORDER BY installed_at DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut items = Vec::new();
    for (id, manifest_json, source_type) in rows {
        let Ok(manifest) = serde_json::from_str::<skillpack_rs::SkillManifest>(&manifest_json)
        else {
            continue;
        };
        items.push(json!({
            "id": id,
            "name": manifest.name,
            "description": manifest.description,
            "source_type": source_type,
            "tags": manifest.tags,
        }));
    }
    Ok(json!({
        "action": "list_skills",
        "items": items,
    }))
}

pub(crate) async fn list_employees(pool: SqlitePool) -> std::result::Result<Value, String> {
    let employees = list_agent_employees_with_pool(&pool).await?;
    Ok(json!({
        "action": "list_employees",
        "items": employees,
    }))
}

pub(crate) async fn create_employee(
    pool: SqlitePool,
    input: Value,
) -> std::result::Result<Value, String> {
    let name = input["name"].as_str().unwrap_or("").trim();
    if name.is_empty() {
        return Err("create_employee 缺少 name 参数".to_string());
    }

    let requested_employee_id = input["employee_id"].as_str().unwrap_or("").trim();
    let generated = normalize_employee_id(name);
    let employee_id = if requested_employee_id.is_empty() {
        generated
    } else {
        normalize_employee_id(requested_employee_id)
    };

    let mut skill_ids = parse_string_array(&input, "skill_ids");
    let requested_primary_skill_id = input["primary_skill_id"]
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string);
    let primary_skill_id = requested_primary_skill_id
        .or_else(|| skill_ids.first().cloned())
        .unwrap_or_else(|| DEFAULT_PRIMARY_SKILL_ID.to_string());
    if !skill_ids
        .iter()
        .any(|id| id.eq_ignore_ascii_case(primary_skill_id.as_str()))
    {
        skill_ids.insert(0, primary_skill_id.clone());
    }
    let skill_ids = dedupe_skill_ids(skill_ids);

    let default_work_dir = input["default_work_dir"]
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| default_employee_work_dir(&employee_id));

    let upsert_input = UpsertAgentEmployeeInput {
        id: input["id"]
            .as_str()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string),
        employee_id: employee_id.clone(),
        name: name.to_string(),
        role_id: employee_id.clone(),
        persona: input["persona"].as_str().unwrap_or("").trim().to_string(),
        feishu_open_id: input["feishu_open_id"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string(),
        feishu_app_id: input["feishu_app_id"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string(),
        feishu_app_secret: input["feishu_app_secret"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string(),
        primary_skill_id,
        default_work_dir,
        openclaw_agent_id: employee_id,
        routing_priority: 100,
        enabled_scopes: {
            let scopes = parse_string_array(&input, "enabled_scopes");
            if scopes.is_empty() {
                vec!["app".to_string()]
            } else {
                scopes
            }
        },
        enabled: input["enabled"].as_bool().unwrap_or(true),
        is_default: input["is_default"].as_bool().unwrap_or(false),
        skill_ids,
    };

    let created_id = upsert_agent_employee_with_pool(&pool, upsert_input).await?;
    let employees = list_agent_employees_with_pool(&pool).await?;
    let created = employees
        .into_iter()
        .find(|item| item.id == created_id)
        .ok_or_else(|| "创建成功但未找到员工记录".to_string())?;

    let auto_apply_profile = input["auto_apply_profile"].as_bool().unwrap_or(true);
    let profile = if auto_apply_profile {
        let payload = AgentProfilePayload {
            employee_db_id: created_id.clone(),
            answers: parse_profile_answers(&input),
        };
        match apply_agent_profile_with_pool(&pool, payload).await {
            Ok(result) => json!({
                "applied": true,
                "files": result.files,
            }),
            Err(error) => json!({
                "applied": false,
                "error": error,
            }),
        }
    } else {
        json!({
            "applied": false,
            "skipped": true,
        })
    };

    Ok(json!({
        "action": "create_employee",
        "ok": true,
        "employee": created,
        "profile": profile,
    }))
}

pub(crate) async fn update_employee(
    pool: SqlitePool,
    input: Value,
) -> std::result::Result<Value, String> {
    let existing = resolve_employee(&pool, &input, "update_employee").await?;

    let name = parse_optional_string(&input, "name").unwrap_or(existing.name.clone());
    if name.trim().is_empty() {
        return Err("update_employee name 不能为空".to_string());
    }
    let persona = parse_optional_string(&input, "persona").unwrap_or(existing.persona.clone());
    let feishu_open_id =
        parse_optional_string(&input, "feishu_open_id").unwrap_or(existing.feishu_open_id.clone());
    let feishu_app_id =
        parse_optional_string(&input, "feishu_app_id").unwrap_or(existing.feishu_app_id);
    let feishu_app_secret = parse_optional_string(&input, "feishu_app_secret")
        .unwrap_or(existing.feishu_app_secret);
    let default_work_dir = parse_optional_string(&input, "default_work_dir")
        .unwrap_or(existing.default_work_dir.clone());
    let enabled = parse_optional_bool(&input, "enabled").unwrap_or(existing.enabled);
    let is_default = parse_optional_bool(&input, "is_default").unwrap_or(existing.is_default);
    let enabled_scopes = if input
        .as_object()
        .is_some_and(|obj| obj.contains_key("enabled_scopes"))
    {
        parse_string_array(&input, "enabled_scopes")
    } else {
        existing.enabled_scopes.clone()
    };

    let mut skill_ids = if input
        .as_object()
        .is_some_and(|obj| obj.contains_key("skill_ids"))
    {
        parse_string_array(&input, "skill_ids")
    } else {
        existing.skill_ids.clone()
    };
    if input
        .as_object()
        .is_some_and(|obj| obj.contains_key("add_skill_ids"))
    {
        skill_ids.extend(parse_string_array(&input, "add_skill_ids"));
    }
    if input
        .as_object()
        .is_some_and(|obj| obj.contains_key("remove_skill_ids"))
    {
        let remove_set = parse_string_array(&input, "remove_skill_ids")
            .into_iter()
            .map(|id| id.to_lowercase())
            .collect::<HashSet<_>>();
        skill_ids.retain(|id| !remove_set.contains(&id.to_lowercase()));
    }
    let mut skill_ids = dedupe_skill_ids(skill_ids);

    let requested_primary_skill_id = input["primary_skill_id"]
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string);
    let mut primary_skill_id = requested_primary_skill_id
        .clone()
        .or_else(|| {
            let current = existing.primary_skill_id.trim();
            if current.is_empty() {
                None
            } else {
                Some(current.to_string())
            }
        })
        .or_else(|| skill_ids.first().cloned())
        .unwrap_or_else(|| DEFAULT_PRIMARY_SKILL_ID.to_string());

    if skill_ids.is_empty() {
        skill_ids.push(primary_skill_id.clone());
    }
    if !skill_ids
        .iter()
        .any(|id| id.eq_ignore_ascii_case(primary_skill_id.as_str()))
    {
        if requested_primary_skill_id.is_some() {
            skill_ids.insert(0, primary_skill_id.clone());
        } else {
            primary_skill_id = skill_ids
                .first()
                .cloned()
                .unwrap_or_else(|| DEFAULT_PRIMARY_SKILL_ID.to_string());
        }
    }
    if primary_skill_id.trim().is_empty() {
        primary_skill_id = skill_ids
            .first()
            .cloned()
            .unwrap_or_else(|| DEFAULT_PRIMARY_SKILL_ID.to_string());
    }
    if skill_ids.is_empty() {
        skill_ids.push(primary_skill_id.clone());
    }

    let upsert_input = UpsertAgentEmployeeInput {
        id: Some(existing.id.clone()),
        employee_id: existing.employee_id.clone(),
        name,
        role_id: existing.role_id.clone(),
        persona,
        feishu_open_id,
        feishu_app_id,
        feishu_app_secret,
        primary_skill_id,
        default_work_dir,
        openclaw_agent_id: existing.openclaw_agent_id.clone(),
        routing_priority: 100,
        enabled_scopes,
        enabled,
        is_default,
        skill_ids,
    };

    let updated_id = upsert_agent_employee_with_pool(&pool, upsert_input).await?;
    let employees = list_agent_employees_with_pool(&pool).await?;
    let updated = employees
        .into_iter()
        .find(|item| item.id == updated_id)
        .ok_or_else(|| "更新成功但未找到员工记录".to_string())?;

    let profile_answers = parse_profile_answers(&input);
    let should_apply_profile = input["auto_apply_profile"]
        .as_bool()
        .unwrap_or(!profile_answers.is_empty());
    let profile = if should_apply_profile {
        let payload = AgentProfilePayload {
            employee_db_id: updated_id,
            answers: profile_answers,
        };
        match apply_agent_profile_with_pool(&pool, payload).await {
            Ok(result) => json!({
                "applied": true,
                "files": result.files,
            }),
            Err(error) => json!({
                "applied": false,
                "error": error,
            }),
        }
    } else {
        json!({
            "applied": false,
            "skipped": true,
        })
    };

    Ok(json!({
        "action": "update_employee",
        "ok": true,
        "employee": updated,
        "profile": profile,
    }))
}

pub(crate) async fn apply_profile(
    pool: SqlitePool,
    input: Value,
) -> std::result::Result<Value, String> {
    let employee_db_id = input["employee_db_id"]
        .as_str()
        .map(str::trim)
        .unwrap_or("");
    let employee_id = input["employee_id"].as_str().map(str::trim).unwrap_or("");

    let resolved_db_id = if !employee_db_id.is_empty() {
        employee_db_id.to_string()
    } else if !employee_id.is_empty() {
        let employees = list_agent_employees_with_pool(&pool).await?;
        let matched = employees
            .into_iter()
            .find(|item| {
                item.id.eq_ignore_ascii_case(employee_id)
                    || item.employee_id.eq_ignore_ascii_case(employee_id)
                    || item.role_id.eq_ignore_ascii_case(employee_id)
            })
            .ok_or_else(|| "apply_profile 未找到对应员工".to_string())?;
        matched.id
    } else {
        return Err("apply_profile 缺少 employee_db_id 或 employee_id 参数".to_string());
    };

    let payload = AgentProfilePayload {
        employee_db_id: resolved_db_id.clone(),
        answers: parse_profile_answers(&input),
    };
    let result = apply_agent_profile_with_pool(&pool, payload).await?;
    Ok(json!({
        "action": "apply_profile",
        "ok": true,
        "employee_db_id": resolved_db_id,
        "files": result.files,
    }))
}
