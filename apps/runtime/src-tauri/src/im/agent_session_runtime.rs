use crate::commands::employee_agents::repo::{
    find_conversation_session_record, find_thread_session_record, insert_inbound_event_link,
    insert_session_seed, update_session_employee_id, upsert_conversation_session_link,
    upsert_thread_session_link, ConversationSessionLinkInput, InboundEventLinkInput,
    SessionSeedInput, ThreadSessionLinkInput,
};
use crate::commands::employee_agents::AgentEmployee;
use crate::commands::employee_agents::{
    build_route_session_key, ensure_employee_sessions_for_event_with_pool,
};
use crate::im::types::ImEvent;
use serde_json::Value;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnsuredAgentSession {
    pub agent_id: String,
    pub role_id: String,
    pub agent_name: String,
    pub session_id: String,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentInboundDispatchSession {
    pub session_id: String,
    pub thread_id: String,
    pub agent_id: String,
    pub role_id: String,
    pub agent_name: String,
    pub route_agent_id: String,
    pub route_session_key: String,
    pub matched_by: String,
    pub prompt: String,
    pub message_id: String,
}

fn event_conversation_key(event: &ImEvent) -> &str {
    event
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| event.thread_id.trim())
}

fn event_conversation_matches_record(event: &ImEvent, record_conversation_id: &str) -> bool {
    let normalized_record = record_conversation_id.trim();
    normalized_record.is_empty() || normalized_record == event_conversation_key(event)
}

fn event_peer_kind(event: &ImEvent) -> &'static str {
    match event.chat_type.as_deref().map(str::trim) {
        Some("p2p") | Some("direct") => "direct",
        _ => "group",
    }
}

fn event_account_id(event: &ImEvent) -> &str {
    event
        .account_id
        .as_deref()
        .or(event.tenant_id.as_deref())
        .unwrap_or_default()
}

fn event_topic_id(event: &ImEvent) -> &str {
    if matches!(
        event.conversation_scope.as_deref(),
        Some("topic") | Some("topic_sender")
    ) {
        if let Some(conversation_id) = event.conversation_id.as_deref() {
            if let Some((_, topic_id)) = conversation_id.split_once(":topic:") {
                return topic_id.split(":sender:").next().unwrap_or("");
            }
        }
    }
    ""
}

async fn write_session_links(
    pool: &SqlitePool,
    event: &ImEvent,
    employee_db_id: &str,
    session_id: &str,
    route_session_key: &str,
    created_at: &str,
    updated_at: &str,
) -> Result<(), String> {
    let parent_conversation_candidates_json =
        serde_json::to_string(&event.parent_conversation_candidates)
            .unwrap_or_else(|_| "[]".to_string());
    let conversation_id = event_conversation_key(event);
    let base_conversation_id = event.base_conversation_id.as_deref().unwrap_or_default();
    let account_id = event_account_id(event);
    let peer_kind = event_peer_kind(event);
    let peer_id = event.thread_id.as_str();
    let topic_id = event_topic_id(event);
    let sender_id = event.sender_id.as_deref().unwrap_or_default();
    let scope = event.conversation_scope.as_deref().unwrap_or_default();

    upsert_conversation_session_link(
        pool,
        &ConversationSessionLinkInput {
            conversation_id,
            employee_db_id,
            thread_id: &event.thread_id,
            session_id,
            route_session_key,
            created_at,
            updated_at,
            channel: &event.channel,
            account_id,
            base_conversation_id,
            parent_conversation_candidates_json: &parent_conversation_candidates_json,
            scope,
            peer_kind,
            peer_id,
            topic_id,
            sender_id,
        },
    )
    .await?;

    upsert_thread_session_link(
        pool,
        &ThreadSessionLinkInput {
            thread_id: &event.thread_id,
            employee_db_id,
            session_id,
            route_session_key,
            created_at,
            updated_at,
            channel: &event.channel,
            account_id,
            conversation_id,
            base_conversation_id,
            parent_conversation_candidates_json: &parent_conversation_candidates_json,
            scope,
            peer_kind,
            peer_id,
            topic_id,
            sender_id,
        },
    )
    .await?;

    Ok(())
}

async fn create_agent_route_session(
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

    write_session_links(
        pool,
        event,
        &employee.id,
        &session_id,
        route_session_key,
        &now,
        &now,
    )
    .await?;

    Ok((session_id, true))
}

pub async fn ensure_agent_session_binding_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
    employee: &AgentEmployee,
    default_model_id: &str,
    route_session_key: &str,
) -> Result<EnsuredAgentSession, String> {
    let existing_conversation =
        find_conversation_session_record(pool, event_conversation_key(event), &employee.id).await?;
    let existing_thread = find_thread_session_record(pool, &event.thread_id, &employee.id).await?;

    let (session_id, created) = if let Some(existing) = existing_conversation {
        if existing.session_exists {
            let now = chrono::Utc::now().to_rfc3339();
            write_session_links(
                pool,
                event,
                &employee.id,
                &existing.session_id,
                route_session_key,
                &now,
                &now,
            )
            .await?;
            (existing.session_id, false)
        } else {
            create_agent_route_session(pool, event, employee, default_model_id, route_session_key)
                .await?
        }
    } else if let Some(existing) = existing_thread {
        if existing.session_exists
            && event_conversation_matches_record(event, &existing.conversation_id)
        {
            let now = chrono::Utc::now().to_rfc3339();
            write_session_links(
                pool,
                event,
                &employee.id,
                &existing.session_id,
                route_session_key,
                &now,
                &now,
            )
            .await?;
            (existing.session_id, false)
        } else {
            create_agent_route_session(pool, event, employee, default_model_id, route_session_key)
                .await?
        }
    } else {
        create_agent_route_session(pool, event, employee, default_model_id, route_session_key)
            .await?
    };

    let _ = update_session_employee_id(pool, &session_id, employee.employee_id.trim()).await;

    Ok(EnsuredAgentSession {
        agent_id: employee.agent_id(),
        role_id: employee.role_id.clone(),
        agent_name: employee.name.clone(),
        session_id,
        created,
    })
}

pub async fn link_inbound_event_to_agent_session_with_pool(
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

pub async fn build_agent_session_dispatches_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
    agent_sessions: Vec<EnsuredAgentSession>,
    route_decision: Option<&Value>,
) -> Result<Vec<AgentInboundDispatchSession>, String> {
    let prompt = event
        .text
        .clone()
        .unwrap_or_else(|| "请继续基于当前上下文推进".to_string());
    let message_id = event.message_id.clone().unwrap_or_default();

    let mut bridged = Vec::with_capacity(agent_sessions.len());
    for session in agent_sessions {
        let _ = link_inbound_event_to_agent_session_with_pool(
            pool,
            event,
            &session.agent_id,
            &session.session_id,
        )
        .await;
        bridged.push(AgentInboundDispatchSession {
            session_id: session.session_id.clone(),
            thread_id: event.thread_id.clone(),
            agent_id: session.agent_id,
            role_id: session.role_id.clone(),
            agent_name: session.agent_name,
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

pub async fn resolve_agent_session_dispatches_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
    route_decision: Option<&Value>,
) -> Result<Vec<AgentInboundDispatchSession>, String> {
    let employee_sessions = ensure_employee_sessions_for_event_with_pool(pool, event).await?;
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
    build_agent_session_dispatches_with_pool(pool, event, agent_sessions, route_decision).await
}

pub async fn ensure_agent_sessions_for_event_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<(), String> {
    ensure_employee_sessions_for_event_with_pool(pool, event)
        .await
        .map(|_| ())
}

pub async fn list_ensured_agent_sessions_for_event_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Vec<EnsuredAgentSession>, String> {
    let employee_sessions = ensure_employee_sessions_for_event_with_pool(pool, event).await?;
    Ok(employee_sessions
        .into_iter()
        .map(|session| EnsuredAgentSession {
            agent_id: session.employee_id,
            role_id: session.role_id,
            agent_name: session.employee_name,
            session_id: session.session_id,
            created: session.created,
        })
        .collect())
}

pub fn build_agent_route_session_key(event: &ImEvent, employee: &AgentEmployee) -> String {
    build_route_session_key(event, employee)
}
