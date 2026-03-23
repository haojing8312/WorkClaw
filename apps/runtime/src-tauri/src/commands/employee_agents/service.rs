use super::repo::{
    clear_default_employee_flag, count_feishu_bindings_for_agent, delete_agent_employee_record,
    delete_displaced_default_feishu_bindings, delete_displaced_scoped_feishu_bindings,
    delete_feishu_bindings_for_agent, find_displaced_default_feishu_agent_ids,
    find_displaced_scoped_feishu_agent_ids, find_employee_db_id_by_employee_id,
    find_latest_thread_session_id, find_recent_route_session_id, find_thread_session_record,
    find_group_run_state, get_employee_association_row, get_employee_group_entry_row,
    insert_feishu_binding, insert_group_run_event, insert_inbound_event_link, insert_session_seed,
    list_agent_employee_rows, list_agent_scope_rows, list_failed_group_run_steps,
    list_skill_ids_for_employee, mark_group_run_done_after_retry, pause_group_run,
    replace_employee_skill_bindings, resume_group_run, update_employee_enabled_scopes,
    update_session_employee_id, upsert_agent_employee_record, upsert_thread_session_link,
    cancel_group_run, complete_failed_group_run_step, InboundEventLinkInput,
    InsertFeishuBindingInput, SessionSeedInput, ThreadSessionLinkInput,
    UpsertAgentEmployeeRecordInput,
};
use super::{
    AgentEmployee, EmployeeInboundDispatchSession, EnsuredEmployeeSession,
    SaveFeishuEmployeeAssociationInput,
    UpsertAgentEmployeeInput,
};
use crate::commands::im_routing::{list_im_routing_bindings_with_pool, ImRoutingBinding};
use crate::commands::models::resolve_default_model_id_with_pool;
use crate::commands::runtime_preferences::resolve_default_work_dir_with_pool;
use crate::im::types::ImEvent;
use serde_json::Value;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::path::PathBuf;
use uuid::Uuid;

pub(super) fn normalize_enabled_scopes_for_storage(enabled_scopes: &[String]) -> Vec<String> {
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

pub(super) fn resolve_employee_agent_id(
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

pub(super) async fn list_agent_employees_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<AgentEmployee>, String> {
    let rows = list_agent_employee_rows(pool).await?;
    let mut result = Vec::with_capacity(rows.len());

    for row in rows {
        let skill_ids = list_skill_ids_for_employee(pool, &row.id).await?;
        let enabled_scopes = serde_json::from_str::<Vec<String>>(&row.enabled_scopes_json)
            .unwrap_or_else(|_| vec!["app".to_string()]);
        let employee_id = if row.employee_id.trim().is_empty() {
            row.role_id.clone()
        } else {
            row.employee_id
        };

        result.push(AgentEmployee {
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
        });
    }

    Ok(result)
}

pub(super) async fn upsert_agent_employee_with_pool(
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
        let employee_dir = PathBuf::from(base)
            .join("employees")
            .join(&employee_id);
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

pub(super) async fn delete_agent_employee_with_pool(
    pool: &SqlitePool,
    employee_id: &str,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    delete_agent_employee_record(&mut tx, employee_id).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn clear_feishu_scope_for_agent_if_unbound(
    tx: &mut Transaction<'_, Sqlite>,
    agent_id: &str,
    now: &str,
) -> Result<(), String> {
    let normalized_agent_id = agent_id.trim();
    if normalized_agent_id.is_empty() {
        return Ok(());
    }

    if count_feishu_bindings_for_agent(tx, normalized_agent_id).await? > 0 {
        return Ok(());
    }

    let employee_rows = list_agent_scope_rows(tx).await?;
    for (employee_db_id, employee_id, role_id, openclaw_agent_id, enabled_scopes_json) in employee_rows {
        let resolved_agent_id =
            resolve_employee_agent_id(&employee_id, &role_id, &openclaw_agent_id);
        if !resolved_agent_id.eq_ignore_ascii_case(normalized_agent_id) {
            continue;
        }

        let existing_scopes = serde_json::from_str::<Vec<String>>(&enabled_scopes_json)
            .unwrap_or_else(|_| vec!["app".to_string()]);
        let next_scopes = existing_scopes
            .into_iter()
            .filter(|scope| scope.trim().to_lowercase() != "feishu")
            .collect::<Vec<_>>();
        let next_scopes = normalize_enabled_scopes_for_storage(&next_scopes);
        let next_scopes_json = serde_json::to_string(&next_scopes).map_err(|e| e.to_string())?;
        update_employee_enabled_scopes(tx, &employee_db_id, &next_scopes_json, now).await?;
    }

    Ok(())
}

pub(super) async fn save_feishu_employee_association_with_pool(
    pool: &SqlitePool,
    input: SaveFeishuEmployeeAssociationInput,
) -> Result<(), String> {
    let employee_db_id = input.employee_db_id.trim();
    if employee_db_id.is_empty() {
        return Err("employee_db_id is required".to_string());
    }

    let mode = input.mode.trim().to_lowercase();
    if mode != "default" && mode != "scoped" {
        return Err("mode must be default or scoped".to_string());
    }

    let peer_kind = input.peer_kind.trim().to_lowercase();
    if mode == "scoped" && !matches!(peer_kind.as_str(), "group" | "channel" | "direct") {
        return Err("peer_kind must be group, channel, or direct".to_string());
    }
    if mode == "scoped" && input.peer_id.trim().is_empty() {
        return Err("peer_id is required for scoped feishu association".to_string());
    }

    let employee_row = get_employee_association_row(pool, employee_db_id)
        .await?
        .ok_or_else(|| "employee not found".to_string())?;

    let existing_scopes = serde_json::from_str::<Vec<String>>(&employee_row.enabled_scopes_json)
        .unwrap_or_else(|_| vec!["app".to_string()]);
    let agent_id = resolve_employee_agent_id(
        &employee_row.employee_id,
        &employee_row.role_id,
        &employee_row.openclaw_agent_id,
    );
    if agent_id.is_empty() {
        return Err("employee is missing agent identity".to_string());
    }

    let mut next_scopes = existing_scopes;
    if input.enabled {
        next_scopes.push("feishu".to_string());
    } else {
        next_scopes.retain(|scope| scope.trim().to_lowercase() != "feishu");
    }
    let next_scopes = normalize_enabled_scopes_for_storage(&next_scopes);
    let next_scopes_json = serde_json::to_string(&next_scopes).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let scoped_peer_id = input.peer_id.trim().to_string();

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    update_employee_enabled_scopes(&mut tx, employee_db_id, &next_scopes_json, &now).await?;
    delete_feishu_bindings_for_agent(&mut tx, &agent_id).await?;

    let displaced_agent_ids = if input.enabled {
        if mode == "default" {
            let ids = find_displaced_default_feishu_agent_ids(&mut tx, &agent_id).await?;
            delete_displaced_default_feishu_bindings(&mut tx, &agent_id).await?;
            ids
        } else {
            let ids =
                find_displaced_scoped_feishu_agent_ids(&mut tx, &agent_id, &peer_kind, &scoped_peer_id)
                    .await?;
            delete_displaced_scoped_feishu_bindings(&mut tx, &agent_id, &peer_kind, &scoped_peer_id)
                .await?;
            ids
        }
    } else {
        Vec::new()
    };

    if input.enabled {
        let binding_id = Uuid::new_v4().to_string();
        let binding_peer_kind = if mode == "default" {
            "group"
        } else {
            peer_kind.as_str()
        };
        let binding_peer_id = if mode == "default" {
            ""
        } else {
            scoped_peer_id.as_str()
        };
        let connector_meta_json =
            serde_json::to_string(&serde_json::json!({ "connector_id": "feishu" }))
                .map_err(|e| e.to_string())?;

        insert_feishu_binding(
            &mut tx,
            &InsertFeishuBindingInput {
                id: &binding_id,
                agent_id: &agent_id,
                peer_kind: binding_peer_kind,
                peer_id: binding_peer_id,
                connector_meta_json: &connector_meta_json,
                priority: input.priority,
                now: &now,
            },
        )
        .await?;
    }

    for displaced_agent_id in displaced_agent_ids {
        clear_feishu_scope_for_agent_if_unbound(&mut tx, &displaced_agent_id, &now).await?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn resolve_target_employees_for_event(
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
        .filter(|employee| employee.enabled && super::employee_scope_matches_event(employee, event))
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

pub(super) async fn resolve_team_entry_employee_for_event_with_pool(
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
                && super::employee_scope_matches_event(employee, event)
                && (employee
                    .employee_id
                    .eq_ignore_ascii_case(preferred_employee_id.trim())
                    || employee
                        .role_id
                        .eq_ignore_ascii_case(preferred_employee_id.trim())
                    || employee.id.eq_ignore_ascii_case(preferred_employee_id.trim()))
        }))
}

pub(super) async fn ensure_employee_sessions_for_event_with_pool(
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

    let default_model_id = resolve_default_model_id_with_pool(pool)
        .await?
        .ok_or_else(|| "no model config found".to_string())?;

    let mut shared_thread_session_id = find_latest_thread_session_id(pool, &event.thread_id).await?;
    let mut results = Vec::with_capacity(employees.len());

    for employee in employees {
        let route_session_key = super::build_route_session_key(event, &employee);
        let existing = find_thread_session_record(pool, &event.thread_id, &employee.id).await?;

        let (session_id, created) = if let Some(existing) = existing {
            if existing.session_exists {
                (existing.session_id, false)
            } else if let Some(shared_session_id) = shared_thread_session_id.clone() {
                let now = chrono::Utc::now().to_rfc3339();
                upsert_thread_session_link(
                    pool,
                    &ThreadSessionLinkInput {
                        thread_id: &event.thread_id,
                        employee_db_id: &employee.id,
                        session_id: &shared_session_id,
                        route_session_key: &route_session_key,
                        created_at: &now,
                        updated_at: &now,
                    },
                )
                .await?;
                (shared_session_id, false)
            } else {
                create_employee_route_session(
                    pool,
                    event,
                    &employee,
                    &default_model_id,
                    &route_session_key,
                )
                .await?
            }
        } else if let Some(shared_session_id) = shared_thread_session_id.clone() {
            let now = chrono::Utc::now().to_rfc3339();
            upsert_thread_session_link(
                pool,
                &ThreadSessionLinkInput {
                    thread_id: &event.thread_id,
                    employee_db_id: &employee.id,
                    session_id: &shared_session_id,
                    route_session_key: &route_session_key,
                    created_at: &now,
                    updated_at: &now,
                },
            )
            .await?;
            (shared_session_id, false)
        } else if let Some(session_id) =
            find_recent_route_session_id(pool, &employee.id, &route_session_key).await?
        {
            let now = chrono::Utc::now().to_rfc3339();
            upsert_thread_session_link(
                pool,
                &ThreadSessionLinkInput {
                    thread_id: &event.thread_id,
                    employee_db_id: &employee.id,
                    session_id: &session_id,
                    route_session_key: &route_session_key,
                    created_at: &now,
                    updated_at: &now,
                },
            )
            .await?;
            (session_id, false)
        } else {
            create_employee_route_session(
                pool,
                event,
                &employee,
                &default_model_id,
                &route_session_key,
            )
            .await?
        };

        if shared_thread_session_id.is_none() {
            shared_thread_session_id = Some(session_id.clone());
        }

        let _ = update_session_employee_id(pool, &session_id, employee.employee_id.trim()).await;

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

pub(super) async fn link_inbound_event_to_session_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
    employee_db_id: &str,
    session_id: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let link_id = Uuid::new_v4().to_string();
    insert_inbound_event_link(
        pool,
        &InboundEventLinkInput {
            id: &link_id,
            thread_id: &event.thread_id,
            session_id,
            employee_db_id,
            im_event_id: event.event_id.as_deref().unwrap_or_default(),
            im_message_id: event.message_id.as_deref().unwrap_or_default(),
            created_at: &now,
        },
    )
    .await
}

pub(super) async fn bridge_inbound_event_to_employee_sessions_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
    route_decision: Option<&Value>,
) -> Result<Vec<EmployeeInboundDispatchSession>, String> {
    let employee_sessions = ensure_employee_sessions_for_event_with_pool(pool, event).await?;
    let prompt = event
        .text
        .clone()
        .unwrap_or_else(|| "请继续基于当前上下文推进".to_string());
    let message_id = event.message_id.clone().unwrap_or_default();

    let mut bridged = Vec::with_capacity(employee_sessions.len());
    for session in employee_sessions {
        let _ =
            link_inbound_event_to_session_with_pool(pool, event, &session.employee_id, &session.session_id)
                .await;
        bridged.push(EmployeeInboundDispatchSession {
            session_id: session.session_id.clone(),
            thread_id: event.thread_id.clone(),
            employee_id: session.employee_id,
            role_id: session.role_id.clone(),
            employee_name: session.employee_name,
            route_agent_id: route_decision
                .and_then(|value| value.get("agentId"))
                .and_then(Value::as_str)
                .unwrap_or(&session.role_id)
                .to_string(),
            route_session_key: route_decision
                .and_then(|value| value.get("sessionKey"))
                .and_then(Value::as_str)
                .unwrap_or(&session.session_id)
                .to_string(),
            matched_by: route_decision
                .and_then(|value| value.get("matchedBy"))
                .and_then(Value::as_str)
                .unwrap_or("default")
                .to_string(),
            prompt: prompt.clone(),
            message_id: message_id.clone(),
        });
    }

    Ok(bridged)
}

pub(super) async fn pause_employee_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    reason: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let affected = pause_group_run(&mut tx, run_id, reason.trim(), &now).await?;
    if affected == 0 {
        return Err("group run is not pausable".to_string());
    }
    insert_group_run_event(
        &mut tx,
        run_id,
        "",
        "run_paused",
        &serde_json::json!({ "reason": reason.trim() }).to_string(),
        &now,
    )
    .await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn resume_employee_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(), String> {
    let run_row = find_group_run_state(pool, run_id)
        .await?
        .ok_or_else(|| "group run not found".to_string())?;
    if run_row.state != "paused" {
        return Err("group run is not paused".to_string());
    }

    let resumed_state = match run_row.current_phase.as_str() {
        "execute" => "executing",
        "review" => "waiting_review",
        _ => "planning",
    };
    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    resume_group_run(&mut tx, run_id, resumed_state, &now).await?;
    insert_group_run_event(
        &mut tx,
        run_id,
        "",
        "run_resumed",
        &serde_json::json!({
            "state": resumed_state,
            "phase": run_row.current_phase,
        })
        .to_string(),
        &now,
    )
    .await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn cancel_employee_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    cancel_group_run(pool, run_id, &now).await
}

pub(super) async fn retry_employee_group_run_failed_steps_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(), String> {
    let failed_rows = list_failed_group_run_steps(pool, run_id).await?;
    if failed_rows.is_empty() {
        return Err("no failed steps to retry".to_string());
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    for row in failed_rows {
        let retried_output = if row.output.trim().is_empty() {
            "重试后完成".to_string()
        } else {
            format!("{}\n重试后完成", row.output)
        };
        complete_failed_group_run_step(&mut tx, &row.step_id, &retried_output, &now).await?;
    }
    mark_group_run_done_after_retry(&mut tx, run_id, &now).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn create_employee_route_session(
    pool: &SqlitePool,
    event: &ImEvent,
    employee: &AgentEmployee,
    default_model_id: &str,
    route_session_key: &str,
) -> Result<(String, bool), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let session_id = Uuid::new_v4().to_string();
    let skill_id = if employee.primary_skill_id.trim().is_empty() {
        "builtin-general".to_string()
    } else {
        employee.primary_skill_id.clone()
    };

    insert_session_seed(
        pool,
        &SessionSeedInput {
            id: &session_id,
            skill_id: &skill_id,
            title: &format!("IM:{}@{}", employee.name, event.thread_id),
            created_at: &now,
            model_id: default_model_id,
            work_dir: employee.default_work_dir.trim(),
            employee_id: employee.employee_id.trim(),
        },
    )
    .await?;

    upsert_thread_session_link(
        pool,
        &ThreadSessionLinkInput {
            thread_id: &event.thread_id,
            employee_db_id: &employee.id,
            session_id: &session_id,
            route_session_key,
            created_at: &now,
            updated_at: &now,
        },
    )
    .await?;

    Ok((session_id, true))
}

fn im_binding_matches_event(binding: &ImRoutingBinding, event: &ImEvent) -> bool {
    super::im_binding_matches_event(binding, event)
}
