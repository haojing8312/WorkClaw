use super::super::repo::{
    find_latest_thread_session_id, find_recent_route_session_id, find_thread_session_record,
    insert_inbound_event_link, insert_session_seed, update_session_employee_id,
    upsert_thread_session_link, InboundEventLinkInput, SessionSeedInput, ThreadSessionLinkInput,
};
use super::super::{
    AgentEmployee, EmployeeInboundDispatchSession, EnsuredEmployeeSession,
};
use super::{
    resolve_target_employees_for_event, resolve_team_entry_employee_for_event_with_pool,
};
use crate::commands::models::resolve_default_model_id_with_pool;
use crate::im::types::ImEvent;
use serde_json::Value;
use sqlx::SqlitePool;
use uuid::Uuid;

pub(crate) async fn ensure_employee_sessions_for_event_with_pool(
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
        let route_session_key = super::super::build_route_session_key(event, &employee);
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

pub(crate) async fn link_inbound_event_to_session_with_pool(
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

pub(crate) async fn bridge_inbound_event_to_employee_sessions_with_pool(
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
