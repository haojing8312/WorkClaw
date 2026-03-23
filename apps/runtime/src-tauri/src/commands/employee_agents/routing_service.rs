use super::super::repo::get_employee_group_entry_row;
use super::super::AgentEmployee;
use super::list_agent_employees_with_pool;
use crate::commands::im_routing::{list_im_routing_bindings_with_pool, ImRoutingBinding};
use crate::im::types::ImEvent;
use sqlx::SqlitePool;

fn text_mentioned(text_lower: &str, alias: &str) -> bool {
    let normalized = alias.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }
    text_lower.contains(&format!("@{}", normalized))
}

pub(crate) async fn resolve_target_employees_for_event(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<AgentEmployee>, String> {
    let all_enabled = list_agent_employees_with_pool(pool)
        .await?
        .into_iter()
        .filter(|employee| employee.enabled && super::super::employee_scope_matches_event(employee, event))
        .collect::<Vec<_>>();

    if let Some(role_id) = event
        .role_id
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        let targeted = all_enabled
            .iter()
            .filter(|employee| {
                employee.feishu_open_id == role_id
                    || employee.role_id == role_id
                    || employee.employee_id == role_id
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
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        let text_lower = text.to_lowercase();
        let targeted = all_enabled
            .iter()
            .filter(|employee| {
                text_mentioned(&text_lower, &employee.name)
                    || text_mentioned(&text_lower, &employee.employee_id)
                    || text_mentioned(&text_lower, &employee.role_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        if !targeted.is_empty() {
            return Ok(vec![targeted[0].clone()]);
        }
    }

    let defaults = all_enabled
        .iter()
        .filter(|employee| employee.is_default)
        .cloned()
        .collect::<Vec<_>>();
    if !defaults.is_empty() {
        return Ok(vec![defaults[0].clone()]);
    }

    Ok(all_enabled.iter().take(1).cloned().collect())
}

pub(crate) async fn resolve_team_entry_employee_for_event_with_pool(
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

    let Some(group_row) = get_employee_group_entry_row(pool, binding.team_id.trim()).await? else {
        return Ok(None);
    };

    let preferred_employee_id = if group_row.entry_employee_id.trim().is_empty() {
        group_row.coordinator_employee_id
    } else {
        group_row.entry_employee_id
    };

    Ok(list_agent_employees_with_pool(pool)
        .await?
        .into_iter()
        .find(|employee| {
            employee.enabled
                && super::super::employee_scope_matches_event(employee, event)
                && (employee
                    .employee_id
                    .eq_ignore_ascii_case(preferred_employee_id.trim())
                    || employee
                        .role_id
                        .eq_ignore_ascii_case(preferred_employee_id.trim())
                    || employee.id.eq_ignore_ascii_case(preferred_employee_id.trim()))
        }))
}

fn im_binding_matches_event(binding: &ImRoutingBinding, event: &ImEvent) -> bool {
    super::super::im_binding_matches_event(binding, event)
}
