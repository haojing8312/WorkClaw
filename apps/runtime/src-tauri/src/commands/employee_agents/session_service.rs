use super::super::{EmployeeInboundDispatchSession, EnsuredEmployeeSession};
use super::{
    resolve_agent_employee_for_agent_id_with_pool, resolve_target_employees_for_event,
    resolve_team_entry_employee_for_event_with_pool,
};
use crate::commands::models::resolve_default_model_id_with_pool;
use crate::im::types::ImEvent;
use crate::im::{
    build_agent_session_dispatches_with_pool, ensure_agent_session_binding_with_pool,
    link_inbound_event_to_agent_session_with_pool, EnsuredAgentSession,
};
use serde_json::Value;
use sqlx::SqlitePool;

async fn resolve_dispatch_employees_for_event_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
    route_decision: Option<&Value>,
) -> Result<Vec<super::super::AgentEmployee>, String> {
    if let Some(route_agent_id) = route_decision
        .and_then(|value| value.get("agentId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(employee) =
            resolve_agent_employee_for_agent_id_with_pool(pool, route_agent_id).await?
        {
            return Ok(vec![employee]);
        }
    }

    if let Some(team_entry_employee) =
        resolve_team_entry_employee_for_event_with_pool(pool, event).await?
    {
        return Ok(vec![team_entry_employee]);
    }

    resolve_target_employees_for_event(pool, event).await
}

async fn ensure_employee_sessions_for_dispatch_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
    route_decision: Option<&Value>,
) -> Result<Vec<EnsuredEmployeeSession>, String> {
    let employees =
        resolve_dispatch_employees_for_event_with_pool(pool, event, route_decision).await?;
    if employees.is_empty() {
        return Ok(Vec::new());
    }

    let default_model_id = resolve_default_model_id_with_pool(pool)
        .await?
        .ok_or_else(|| "no model config found".to_string())?;

    let mut results = Vec::with_capacity(employees.len());
    for employee in employees {
        let route_session_key = super::super::build_route_session_key(event, &employee);
        let ensured = ensure_agent_session_binding_with_pool(
            pool,
            event,
            &employee,
            &default_model_id,
            &route_session_key,
        )
        .await?;
        results.push(EnsuredEmployeeSession::from(ensured));
    }

    Ok(results)
}

pub(crate) async fn ensure_employee_sessions_for_event_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<EnsuredEmployeeSession>, String> {
    ensure_employee_sessions_for_dispatch_with_pool(pool, event, None).await
}

pub(crate) async fn link_inbound_event_to_session_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
    employee_db_id: &str,
    session_id: &str,
) -> Result<(), String> {
    link_inbound_event_to_agent_session_with_pool(pool, event, employee_db_id, session_id).await
}

pub(crate) async fn bridge_inbound_event_to_employee_sessions_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
    route_decision: Option<&Value>,
) -> Result<Vec<EmployeeInboundDispatchSession>, String> {
    let employee_sessions =
        ensure_employee_sessions_for_dispatch_with_pool(pool, event, route_decision).await?;
    let agent_sessions = employee_sessions
        .into_iter()
        .map(|session| EnsuredAgentSession {
            agent_id: session.employee_id,
            role_id: session.role_id,
            agent_name: session.employee_name,
            session_id: session.session_id,
            created: session.created,
        })
        .collect::<Vec<_>>();

    let dispatches =
        build_agent_session_dispatches_with_pool(pool, event, agent_sessions, route_decision)
            .await?;
    Ok(dispatches
        .into_iter()
        .map(EmployeeInboundDispatchSession::from)
        .collect())
}
