use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatPermissionMode {
    AcceptEdits,
    Unrestricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelRouteErrorKind {
    Auth,
    RateLimit,
    Timeout,
    Network,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingSettingsSnapshot {
    pub max_call_depth: usize,
    pub node_timeout_seconds: u64,
    pub retry_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatRoutingSnapshot {
    pub primary_provider_id: String,
    pub primary_model: String,
    pub fallback_chain_json: String,
    pub timeout_ms: i64,
    pub retry_count: i64,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatRoutePolicySnapshot {
    pub primary_provider_id: String,
    pub primary_model: String,
    pub fallback_chain_json: String,
    pub retry_count: i64,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConnectionSnapshot {
    pub provider_id: String,
    pub protocol_type: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionModelSnapshot {
    pub model_id: String,
    pub api_format: String,
    pub base_url: String,
    pub model_name: String,
    pub api_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatEmployeeSnapshot {
    pub id: String,
    pub employee_id: String,
    pub name: String,
    pub role_id: String,
    pub feishu_open_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedRouteCandidate {
    pub protocol_type: String,
    pub base_url: String,
    pub model_name: String,
    pub api_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedRouteCandidates {
    pub candidates: Vec<PreparedRouteCandidate>,
    pub retry_count_per_candidate: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatPreparationRequest {
    pub user_message: String,
    pub permission_mode: Option<String>,
    pub session_mode: Option<String>,
    pub team_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatExecutionPreparationRequest {
    pub user_message: String,
    pub session_id: Option<String>,
    pub permission_mode: Option<String>,
    pub session_mode: Option<String>,
    pub team_id: Option<String>,
    pub employee_id: Option<String>,
    pub requested_capability: Option<String>,
    pub work_dir: Option<String>,
    pub imported_mcp_server_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCreationRequest {
    pub permission_mode: Option<String>,
    pub session_mode: Option<String>,
    pub team_id: Option<String>,
    pub title: Option<String>,
    pub work_dir: Option<String>,
    pub employee_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedSessionCreation {
    pub permission_mode_storage: String,
    pub session_mode_storage: String,
    pub normalized_team_id: String,
    pub normalized_title: String,
    pub normalized_work_dir: String,
    pub normalized_employee_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatExecutionContext {
    pub session_id: String,
    pub session_mode_storage: String,
    pub normalized_team_id: String,
    pub employee_id: String,
    pub work_dir: String,
    pub imported_mcp_server_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionExecutionContextSnapshot {
    pub session_id: String,
    pub session_mode: String,
    pub team_id: String,
    pub employee_id: String,
    pub work_dir: String,
    pub imported_mcp_server_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatExecutionGuidance {
    pub effective_work_dir: String,
    pub imported_mcp_guidance: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreparedChatExecutionAssembly {
    pub chat_preparation: PreparedChatExecution,
    pub execution_context: ChatExecutionContext,
    pub execution_guidance: ChatExecutionGuidance,
    pub route_decisions: PreparedRouteCandidates,
    pub employee_collaboration_guidance: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreparedChatExecution {
    pub capability: String,
    pub permission_mode_storage: String,
    pub session_mode_storage: String,
    pub normalized_team_id: String,
    pub permission_label: String,
    pub max_call_depth: usize,
    pub node_timeout_seconds: u64,
    pub retry_count: usize,
    pub primary_provider_id: Option<String>,
    pub primary_model: Option<String>,
    pub fallback_targets: Vec<(String, String)>,
    pub default_model_id: Option<String>,
    pub default_usable_model_id: Option<String>,
    pub execution_context: ChatExecutionContext,
}

impl Default for PreparedChatExecution {
    fn default() -> Self {
        Self {
            capability: "chat".to_string(),
            permission_mode_storage: "standard".to_string(),
            session_mode_storage: "general".to_string(),
            normalized_team_id: String::new(),
            permission_label: "标准模式".to_string(),
            max_call_depth: 4,
            node_timeout_seconds: 60,
            retry_count: 0,
            primary_provider_id: None,
            primary_model: None,
            fallback_targets: Vec::new(),
            default_model_id: None,
            default_usable_model_id: None,
            execution_context: ChatExecutionContext {
                session_id: String::new(),
                session_mode_storage: "general".to_string(),
                normalized_team_id: String::new(),
                employee_id: String::new(),
                work_dir: String::new(),
                imported_mcp_server_ids: Vec::new(),
            },
        }
    }
}
