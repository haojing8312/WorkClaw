use super::repo::{
    clear_default_employee_flag, count_feishu_bindings_for_agent, delete_agent_employee_record,
    delete_displaced_default_feishu_bindings, delete_displaced_scoped_feishu_bindings,
    delete_feishu_bindings_for_agent, find_displaced_default_feishu_agent_ids,
    find_displaced_scoped_feishu_agent_ids, find_employee_db_id_by_employee_id,
    find_employee_session_seed_row, find_existing_session_skill_id, find_group_step_session_row,
    find_latest_thread_session_id, find_model_config_row, find_recent_group_step_session_id,
    find_recent_route_session_id, find_thread_session_record,
    find_group_run_execute_step_context, find_group_run_finalize_state, find_group_run_snapshot_row,
    find_group_run_state, find_latest_assistant_message_content, find_pending_review_step,
    get_employee_association_row, get_employee_group_entry_row, get_group_run_session_id,
    insert_feishu_binding, insert_group_run_assistant_message, insert_group_run_event,
    insert_inbound_event_link, insert_session_message, insert_session_seed,
    list_agent_employee_rows,
    list_agent_scope_rows, list_failed_execute_assignees, list_failed_group_run_steps,
    list_group_run_event_snapshot_rows, list_group_run_execute_outputs,
    list_group_run_step_snapshot_rows, list_pending_execute_step_ids, list_session_message_rows,
    list_skill_ids_for_employee, load_group_run_blocking_counts,
    mark_group_run_done_after_retry, mark_group_run_executing,
    mark_group_run_failed, mark_group_run_finalized, mark_group_run_step_completed,
    mark_group_run_step_dispatched, mark_group_run_step_failed, mark_group_run_waiting_review,
    pause_group_run, replace_employee_skill_bindings, reset_group_run_step_for_reassignment,
    resume_group_run, review_requested_event_exists, update_employee_enabled_scopes,
    update_group_run_after_reassignment, update_session_employee_id, upsert_agent_employee_record,
    upsert_thread_session_link, cancel_group_run, clear_group_run_execute_waiting_state,
    complete_failed_group_run_step, employee_exists_for_reassignment,
    find_group_run_step_reassign_row, GroupRunEventSnapshotRow, GroupRunStepSnapshotRow,
    InboundEventLinkInput, InsertFeishuBindingInput, SessionSeedInput, ThreadSessionLinkInput,
    UpsertAgentEmployeeRecordInput,
};
use super::{
    AgentEmployee, EmployeeInboundDispatchSession, EnsuredEmployeeSession,
    SaveFeishuEmployeeAssociationInput,
    UpsertAgentEmployeeInput,
};
use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::{parse_run_stop_reason, RunStopReasonKind};
use crate::agent::tools::{EmployeeManageTool, MemoryTool};
use crate::agent::{AgentExecutor, ToolRegistry};
use crate::commands::chat_runtime_io::extract_assistant_text_content;
use crate::commands::im_routing::{list_im_routing_bindings_with_pool, ImRoutingBinding};
use crate::commands::models::resolve_default_model_id_with_pool;
use crate::commands::runtime_preferences::resolve_default_work_dir_with_pool;
use crate::im::types::ImEvent;
use serde_json::Value;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::path::PathBuf;
use std::sync::Arc;
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

pub(super) async fn reassign_group_run_step_with_pool(
    pool: &SqlitePool,
    step_id: &str,
    assignee_employee_id: &str,
) -> Result<(), String> {
    let new_assignee = assignee_employee_id.trim().to_lowercase();
    if new_assignee.is_empty() {
        return Err("assignee_employee_id is required".to_string());
    }

    let step_row = find_group_run_step_reassign_row(pool, step_id)
        .await?
        .ok_or_else(|| "group run step not found".to_string())?;
    if step_row.step_type != "execute" {
        return Err("only execute steps can be reassigned".to_string());
    }
    if step_row.status != "failed" && step_row.status != "pending" {
        return Err("only failed or pending steps can be reassigned".to_string());
    }

    if !employee_exists_for_reassignment(pool, &new_assignee).await? {
        return Err("target employee not found".to_string());
    }

    let (eligible_targets, has_execute_rules) = super::load_execute_reassignment_targets_with_pool(
        pool,
        &step_row.run_id,
        Some(step_row.dispatch_source_employee_id.as_str()),
    )
    .await?;
    if has_execute_rules && !eligible_targets.iter().any(|candidate| candidate == &new_assignee) {
        return Err("target employee is not eligible for execute reassignment".to_string());
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    reset_group_run_step_for_reassignment(&mut tx, step_id, &new_assignee).await?;

    let remaining_failed_assignees = list_failed_execute_assignees(&mut tx, &step_row.run_id).await?;
    if remaining_failed_assignees.is_empty() {
        update_group_run_after_reassignment(
            &mut tx,
            &step_row.run_id,
            "executing",
            &new_assignee,
            "",
            &now,
        )
        .await?;
    } else {
        let waiting_for_employee_id = remaining_failed_assignees[0].clone();
        let status_reason = format!("{}执行失败", remaining_failed_assignees.join("、"));
        update_group_run_after_reassignment(
            &mut tx,
            &step_row.run_id,
            "failed",
            &waiting_for_employee_id,
            &status_reason,
            &now,
        )
        .await?;
    }

    let previous_output_summary = if step_row.previous_output_summary.trim().is_empty() {
        step_row.previous_output.chars().take(120).collect::<String>()
    } else {
        step_row.previous_output_summary
    };
    insert_group_run_event(
        &mut tx,
        &step_row.run_id,
        step_id,
        "step_reassigned",
        &serde_json::json!({
            "assignee_employee_id": new_assignee,
            "dispatch_source_employee_id": step_row.dispatch_source_employee_id,
            "previous_assignee_employee_id": step_row.previous_assignee_employee_id,
            "previous_output_summary": previous_output_summary,
        })
        .to_string(),
        &now,
    )
    .await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn load_group_run_execute_step_context(
    pool: &SqlitePool,
    step_id: &str,
) -> Result<
    (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    ),
    String,
> {
    let row = find_group_run_execute_step_context(pool, step_id)
        .await?
        .ok_or_else(|| "group run step not found".to_string())?;
    if row.step_type != "execute" {
        return Err("only execute steps can be run".to_string());
    }
    Ok((
        row.step_id,
        row.run_id,
        row.assignee_employee_id,
        row.dispatch_source_employee_id,
        row.existing_session_id,
        row.step_input,
        row.user_goal,
        row.step_type,
    ))
}

pub(super) async fn mark_group_run_step_dispatched_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    step_id: &str,
    session_id: &str,
    assignee_employee_id: &str,
    dispatch_source_employee_id: &str,
    now: &str,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    mark_group_run_step_dispatched(&mut tx, step_id, session_id, now).await?;
    insert_group_run_event(
        &mut tx,
        run_id,
        step_id,
        "step_dispatched",
        &serde_json::json!({
            "step_id": step_id,
            "session_id": session_id,
            "assignee_employee_id": assignee_employee_id,
            "dispatch_source_employee_id": dispatch_source_employee_id,
        })
        .to_string(),
        now,
    )
    .await?;
    mark_group_run_executing(&mut tx, run_id, assignee_employee_id, now).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn mark_group_run_step_failed_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    step_id: &str,
    session_id: &str,
    assignee_employee_id: &str,
    dispatch_source_employee_id: &str,
    error: &str,
    now: &str,
) -> Result<(), String> {
    let failed_summary = error.chars().take(120).collect::<String>();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    mark_group_run_step_failed(&mut tx, step_id, error, &failed_summary, session_id, now).await?;
    insert_group_run_event(
        &mut tx,
        run_id,
        step_id,
        "step_failed",
        &serde_json::json!({
            "step_id": step_id,
            "session_id": session_id,
            "status": "failed",
            "error": error,
            "assignee_employee_id": assignee_employee_id,
            "dispatch_source_employee_id": dispatch_source_employee_id,
        })
        .to_string(),
        now,
    )
    .await?;
    mark_group_run_failed(&mut tx, run_id, assignee_employee_id, error, now).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn mark_group_run_step_completed_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    step_id: &str,
    session_id: &str,
    assignee_employee_id: &str,
    dispatch_source_employee_id: &str,
    output: &str,
    now: &str,
) -> Result<(), String> {
    let output_summary = output.chars().take(120).collect::<String>();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    mark_group_run_step_completed(&mut tx, step_id, output, &output_summary, session_id, now).await?;
    insert_group_run_event(
        &mut tx,
        run_id,
        step_id,
        "step_completed",
        &serde_json::json!({
            "step_id": step_id,
            "session_id": session_id,
            "status": "completed",
            "output_summary": output_summary,
            "assignee_employee_id": assignee_employee_id,
            "dispatch_source_employee_id": dispatch_source_employee_id,
        })
        .to_string(),
        now,
    )
    .await?;
    clear_group_run_execute_waiting_state(&mut tx, run_id, now).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn load_group_run_continue_state(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(String, String), String> {
    let normalized_run_id = run_id.trim();
    if normalized_run_id.is_empty() {
        return Err("run_id is required".to_string());
    }
    let run_row = find_group_run_state(pool, normalized_run_id)
        .await?
        .ok_or_else(|| "group run not found".to_string())?;
    Ok((run_row.state, run_row.current_phase))
}

pub(super) async fn maybe_mark_group_run_waiting_review(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<String>, String> {
    let Some(review_row) = find_pending_review_step(pool, run_id).await? else {
        return Ok(None);
    };

    let review_requested_exists = review_requested_event_exists(pool, run_id, &review_row.step_id).await?;
    let default_reason = format!("等待{}审议", review_row.assignee_employee_id.trim());
    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    mark_group_run_waiting_review(
        &mut tx,
        run_id,
        &review_row.assignee_employee_id,
        &default_reason,
        &now,
    )
    .await?;
    if !review_requested_exists {
        insert_group_run_event(
            &mut tx,
            run_id,
            &review_row.step_id,
            "review_requested",
            &serde_json::json!({
                "assignee_employee_id": review_row.assignee_employee_id,
                "phase": "review",
            })
            .to_string(),
            &now,
        )
        .await?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(Some(review_row.assignee_employee_id))
}

pub(super) async fn list_pending_execute_steps_for_continue(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<String>, String> {
    list_pending_execute_step_ids(pool, run_id).await
}

pub(super) async fn maybe_finalize_group_run_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<(), String> {
    let (execute_blocking, review_blocking) = load_group_run_blocking_counts(pool, run_id).await?;
    if execute_blocking > 0 || review_blocking > 0 {
        return Ok(());
    }

    let run_row = find_group_run_finalize_state(pool, run_id)
        .await?
        .ok_or_else(|| "group run not found".to_string())?;
    if run_row.state == "done" {
        return Ok(());
    }

    let execute_rows = list_group_run_execute_outputs(pool, run_id).await?;
    let mut summary_lines = vec![
        format!("计划：围绕“{}”的团队执行已完成。", run_row.user_goal.trim()),
        "执行：".to_string(),
    ];
    for (assignee_employee_id, output) in execute_rows {
        summary_lines.push(format!("- {}: {}", assignee_employee_id, output.trim()));
    }
    summary_lines.push("汇报：团队协作已完成，可继续进入人工复核或直接对外回复。".to_string());
    let final_report = summary_lines.join("\n");

    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    insert_group_run_assistant_message(&mut tx, &run_row.session_id, &final_report, &now).await?;
    mark_group_run_finalized(&mut tx, run_id, &now).await?;
    insert_group_run_event(
        &mut tx,
        run_id,
        "",
        "run_completed",
        &serde_json::json!({
            "state": "done",
            "phase": "finalize",
            "summary": final_report,
        })
        .to_string(),
        &now,
    )
    .await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) async fn execute_group_step_in_employee_context_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    step_id: &str,
    session_id: &str,
    assignee_employee_id: &str,
    user_goal: &str,
    step_input: &str,
) -> Result<String, String> {
    let session_row = find_group_step_session_row(pool, session_id)
        .await?
        .ok_or_else(|| "group step session not found".to_string())?;

    let employee = list_agent_employees_with_pool(pool)
        .await?
        .into_iter()
        .find(|item| {
            item.employee_id.eq_ignore_ascii_case(assignee_employee_id)
                || item.role_id.eq_ignore_ascii_case(assignee_employee_id)
                || item.id.eq_ignore_ascii_case(assignee_employee_id)
        })
        .ok_or_else(|| "assignee employee not found".to_string())?;

    let model_row = find_model_config_row(pool, &session_row.model_id)
        .await?
        .ok_or_else(|| "model config not found".to_string())?;

    let (system_prompt, allowed_tools, max_iterations) =
        super::build_group_step_system_prompt(&employee, &session_row.skill_id);
    let user_prompt =
        super::build_group_step_user_prompt(run_id, step_id, user_goal, step_input, &employee);

    let now = chrono::Utc::now().to_rfc3339();
    insert_session_message(pool, session_id, "user", &user_prompt, &now).await?;

    let messages: Vec<Value> = list_session_message_rows(pool, session_id)
        .await?
        .into_iter()
        .map(|row| {
            let normalized_content = if row.role == "assistant" {
                extract_assistant_text_content(&row.content)
            } else {
                row.content
            };
            serde_json::json!({ "role": row.role, "content": normalized_content })
        })
        .collect();

    let registry = Arc::new(ToolRegistry::with_standard_tools());
    let memory_root = if session_row.work_dir.trim().is_empty() {
        std::env::temp_dir().join("workclaw-group-run-memory")
    } else {
        PathBuf::from(session_row.work_dir.trim())
            .join("openclaw")
            .join(employee.employee_id.trim())
            .join("memory")
    };
    let memory_dir = memory_root.join(if session_row.skill_id.trim().is_empty() {
        "builtin-general"
    } else {
        session_row.skill_id.trim()
    });
    std::fs::create_dir_all(&memory_dir).map_err(|e| e.to_string())?;
    registry.register(Arc::new(MemoryTool::new(memory_dir)));
    registry.register(Arc::new(EmployeeManageTool::new(pool.clone())));

    let executor = AgentExecutor::with_max_iterations(Arc::clone(&registry), max_iterations);
    let final_messages = match executor
        .execute_turn(
            &model_row.api_format,
            &model_row.base_url,
            &model_row.api_key,
            &model_row.model_name,
            &system_prompt,
            messages,
            |_| {},
            None,
            None,
            allowed_tools.as_deref(),
            PermissionMode::Unrestricted,
            None,
            if session_row.work_dir.trim().is_empty() {
                None
            } else {
                Some(session_row.work_dir.clone())
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
            let stop_reason = match parse_run_stop_reason(&error_text) {
                Some(reason) => reason,
                None => return Err(error_text),
            };
            if stop_reason.kind != RunStopReasonKind::MaxTurns {
                return Err(error_text);
            }

            let fallback_output = super::build_group_step_iteration_fallback_output(
                &employee,
                user_goal,
                step_input,
                stop_reason
                    .detail
                    .as_deref()
                    .unwrap_or(stop_reason.message.as_str()),
            );
            let finished_at = chrono::Utc::now().to_rfc3339();
            insert_session_message(pool, session_id, "assistant", &fallback_output, &finished_at)
                .await?;
            return Ok(fallback_output);
        }
    };

    let assistant_output = super::extract_assistant_text(&final_messages);
    if assistant_output.trim().is_empty() {
        return Err("employee step execution returned empty assistant output".to_string());
    }

    let finished_at = chrono::Utc::now().to_rfc3339();
    insert_session_message(pool, session_id, "assistant", &assistant_output, &finished_at).await?;

    Ok(assistant_output)
}

pub(super) async fn ensure_group_run_session_with_pool(
    pool: &SqlitePool,
    coordinator_employee_id: &str,
    group_name: &str,
    now: &str,
    preferred_session_id: Option<&str>,
) -> Result<(String, String), String> {
    let employee_row = find_employee_session_seed_row(pool, coordinator_employee_id)
        .await?
        .ok_or_else(|| "coordinator employee not found".to_string())?;

    let session_skill_id = if employee_row.primary_skill_id.trim().is_empty() {
        "builtin-general".to_string()
    } else {
        employee_row.primary_skill_id.trim().to_string()
    };

    if let Some(existing_session_id) = preferred_session_id
        .map(str::trim)
        .filter(|session_id| !session_id.is_empty())
    {
        let existing_skill_id = find_existing_session_skill_id(pool, existing_session_id)
            .await?
            .ok_or_else(|| "preferred group run session not found".to_string())?;
        let existing_skill_id = if existing_skill_id.trim().is_empty() {
            session_skill_id.clone()
        } else {
            existing_skill_id.trim().to_string()
        };
        return Ok((existing_session_id.to_string(), existing_skill_id));
    }

    let model_id = resolve_default_model_id_with_pool(pool)
        .await?
        .ok_or_else(|| "model config not found".to_string())?;

    let session_id = Uuid::new_v4().to_string();
    insert_session_seed(
        pool,
        &SessionSeedInput {
            id: &session_id,
            skill_id: &session_skill_id,
            title: &format!("群组协作：{}", group_name.trim()),
            created_at: now,
            model_id: &model_id,
            work_dir: &employee_row.default_work_dir,
            employee_id: coordinator_employee_id,
        },
    )
    .await?;

    Ok((session_id, session_skill_id))
}

pub(super) async fn append_group_run_assistant_message_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    content: &str,
) -> Result<(), String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    let now = chrono::Utc::now().to_rfc3339();
    insert_session_message(pool, session_id, "assistant", trimmed, &now).await
}

pub(super) async fn ensure_group_step_session_with_pool(
    pool: &SqlitePool,
    run_id: &str,
    assignee_employee_id: &str,
    now: &str,
) -> Result<String, String> {
    if let Some(session_id) = find_recent_group_step_session_id(pool, run_id, assignee_employee_id).await? {
        return Ok(session_id);
    }

    let employee_row = find_employee_session_seed_row(pool, assignee_employee_id)
        .await?
        .ok_or_else(|| "assignee employee not found".to_string())?;

    let session_skill_id = if employee_row.primary_skill_id.trim().is_empty() {
        "builtin-general".to_string()
    } else {
        employee_row.primary_skill_id.trim().to_string()
    };

    let model_id = resolve_default_model_id_with_pool(pool)
        .await?
        .ok_or_else(|| "model config not found".to_string())?;

    let session_id = Uuid::new_v4().to_string();
    insert_session_seed(
        pool,
        &SessionSeedInput {
            id: &session_id,
            skill_id: &session_skill_id,
            title: &format!("群组执行:{}@{}", run_id, assignee_employee_id),
            created_at: now,
            model_id: &model_id,
            work_dir: &employee_row.default_work_dir,
            employee_id: assignee_employee_id,
        },
    )
    .await?;

    Ok(session_id)
}

pub(super) async fn get_group_run_session_id_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<String, String> {
    get_group_run_session_id(pool, run_id)
        .await?
        .ok_or_else(|| "group run not found".to_string())
}

pub(super) async fn get_employee_group_run_snapshot_by_run_id_with_pool(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<super::EmployeeGroupRunSnapshot, String> {
    let session_id = get_group_run_session_id_with_pool(pool, run_id).await?;
    get_employee_group_run_snapshot_with_pool(pool, &session_id)
        .await?
        .ok_or_else(|| "group run snapshot not found".to_string())
}

pub(super) async fn get_employee_group_run_snapshot_with_pool(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<super::EmployeeGroupRunSnapshot>, String> {
    let Some(run_row) = find_group_run_snapshot_row(pool, session_id).await? else {
        return Ok(None);
    };

    let steps = list_group_run_step_snapshot_rows(pool, &run_row.run_id)
        .await?
        .into_iter()
        .map(map_group_run_step_snapshot)
        .collect::<Vec<_>>();
    let events = list_group_run_event_snapshot_rows(pool, &run_row.run_id)
        .await?
        .into_iter()
        .map(map_group_run_event_snapshot)
        .collect::<Vec<_>>();
    let completed = steps.iter().filter(|step| step.status == "completed").count();
    let final_report = find_latest_assistant_message_content(pool, &run_row.session_id)
        .await?
        .filter(|content| !content.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "计划：围绕“{}”共 {} 步。\n执行：已完成 {} 步。\n汇报：当前状态={}",
                run_row.user_goal,
                steps.len(),
                completed,
                run_row.state
            )
        });

    Ok(Some(super::EmployeeGroupRunSnapshot {
        run_id: run_row.run_id,
        group_id: run_row.group_id,
        session_id: run_row.session_id,
        state: run_row.state,
        current_round: run_row.current_round,
        current_phase: run_row.current_phase,
        review_round: run_row.review_round,
        status_reason: run_row.status_reason,
        waiting_for_employee_id: run_row.waiting_for_employee_id,
        waiting_for_user: run_row.waiting_for_user,
        final_report,
        steps,
        events,
    }))
}

fn map_group_run_step_snapshot(
    row: GroupRunStepSnapshotRow,
) -> super::EmployeeGroupRunStep {
    super::EmployeeGroupRunStep {
        id: row.id,
        round_no: row.round_no,
        step_type: row.step_type,
        assignee_employee_id: row.assignee_employee_id,
        dispatch_source_employee_id: row.dispatch_source_employee_id,
        session_id: row.session_id,
        attempt_no: row.attempt_no,
        status: row.status,
        output_summary: row.output_summary,
        output: row.output,
    }
}

fn map_group_run_event_snapshot(
    row: GroupRunEventSnapshotRow,
) -> super::EmployeeGroupRunEvent {
    super::EmployeeGroupRunEvent {
        id: row.id,
        step_id: row.step_id,
        event_type: row.event_type,
        payload_json: row.payload_json,
        created_at: row.created_at,
    }
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
