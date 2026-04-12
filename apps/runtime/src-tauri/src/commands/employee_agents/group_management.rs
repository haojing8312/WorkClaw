use super::{
    group_rule_matches_relation_types, normalize_member_employee_ids,
    CloneEmployeeGroupTemplateInput, CreateEmployeeGroupInput, CreateEmployeeTeamInput,
    CreateEmployeeTeamRuleInput, EmployeeGroup, EmployeeGroupRule, EmployeeGroupRunSummary,
};
use crate::employee_runtime_adapter::team_topology::resolve_executor_employee_ids;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

pub(crate) async fn create_employee_group_with_pool(
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

    let mut seen_execute_targets = std::collections::HashSet::new();
    let execute_targets = resolve_executor_employee_ids(
        coordinator_employee_id,
        member_employee_ids,
        planner_employee_id,
        if reviewer_employee_id.trim().is_empty() {
            None
        } else {
            Some(reviewer_employee_id)
        },
    );
    for execute_target in execute_targets {
        if execute_target == coordinator_employee_id {
            continue;
        }
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

pub(crate) async fn create_employee_team_with_pool(
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

pub(crate) async fn clone_employee_group_template_with_pool(
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
        let required = row.try_get::<i64, _>("required").map_err(|e| e.to_string())?;
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

pub(crate) async fn list_employee_groups_with_pool(
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

pub(crate) async fn list_employee_group_runs_with_pool(
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

pub(crate) async fn list_employee_group_rules_with_pool(
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
            required: row.try_get::<i64, _>("required").map_err(|e| e.to_string())? != 0,
            priority: row.try_get("priority").map_err(|e| e.to_string())?,
            created_at: row.try_get("created_at").map_err(|e| e.to_string())?,
        });
    }

    Ok(rules)
}

pub(crate) async fn delete_employee_group_with_pool(
    pool: &SqlitePool,
    group_id: &str,
) -> Result<(), String> {
    sqlx::query(
        "DELETE FROM group_run_steps WHERE run_id IN (SELECT id FROM group_runs WHERE group_id = ?)",
    )
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

#[cfg(test)]
mod tests {
    use super::normalize_team_mode;

    #[test]
    fn normalize_team_mode_rejects_unknown_mode() {
        let err = normalize_team_mode("weird", &["none", "soft"], "none", "review_mode")
            .expect_err("invalid mode should fail");
        assert_eq!(err, "invalid review_mode");
    }
}
