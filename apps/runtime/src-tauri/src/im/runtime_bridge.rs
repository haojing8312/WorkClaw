use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoleTaskRequest {
    pub role_id: String,
    pub role_name: String,
    pub prompt: String,
    #[serde(default = "default_agent_type")]
    pub agent_type: String,
}

fn default_agent_type() -> String {
    "general-purpose".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoleProgressEvent {
    pub role_id: String,
    pub role_name: String,
    pub token: String,
    pub done: bool,
    pub sub_agent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImRoleEventPayload {
    pub session_id: String,
    pub thread_id: String,
    pub role_id: String,
    pub role_name: String,
    pub status: String,
    pub summary: String,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImRoleDispatchRequest {
    pub session_id: String,
    pub thread_id: String,
    pub role_id: String,
    pub role_name: String,
    pub prompt: String,
    pub agent_type: String,
}

pub fn build_runtime_task_payload(req: &RoleTaskRequest) -> Value {
    json!({
        "prompt": format!("[{}] {}", req.role_name, req.prompt),
        "agent_type": req.agent_type,
        "role_id": req.role_id,
    })
}

pub fn normalize_stream_token(
    role_id: &str,
    role_name: &str,
    payload: &Value,
) -> Option<RoleProgressEvent> {
    let token = payload.get("token")?.as_str()?.to_string();
    let done = payload
        .get("done")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let sub_agent = payload
        .get("sub_agent")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Some(RoleProgressEvent {
        role_id: role_id.to_string(),
        role_name: role_name.to_string(),
        token,
        done,
        sub_agent,
    })
}

pub fn build_im_role_event_payload(
    session_id: &str,
    thread_id: &str,
    role_id: &str,
    role_name: &str,
    status: &str,
    summary: &str,
    duration_ms: Option<u64>,
) -> ImRoleEventPayload {
    ImRoleEventPayload {
        session_id: session_id.to_string(),
        thread_id: thread_id.to_string(),
        role_id: role_id.to_string(),
        role_name: role_name.to_string(),
        status: status.to_string(),
        summary: summary.to_string(),
        duration_ms,
    }
}

pub fn build_im_role_dispatch_request(
    session_id: &str,
    thread_id: &str,
    role_id: &str,
    role_name: &str,
    prompt: &str,
    agent_type: &str,
) -> ImRoleDispatchRequest {
    ImRoleDispatchRequest {
        session_id: session_id.to_string(),
        thread_id: thread_id.to_string(),
        role_id: role_id.to_string(),
        role_name: role_name.to_string(),
        prompt: prompt.to_string(),
        agent_type: agent_type.to_string(),
    }
}
