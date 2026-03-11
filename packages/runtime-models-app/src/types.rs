use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutingSettings {
    pub max_call_depth: usize,
    pub node_timeout_seconds: u64,
    pub retry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderPluginInfo {
    pub key: String,
    pub display_name: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub api_format: String,
    pub base_url: String,
    pub model_name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderConfig {
    pub id: String,
    pub provider_key: String,
    pub display_name: String,
    pub protocol_type: String,
    pub base_url: String,
    pub auth_type: String,
    pub api_key_encrypted: String,
    pub org_id: String,
    pub extra_json: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderConnectionInfo {
    pub provider_id: String,
    pub protocol_type: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderHealthInfo {
    pub provider_id: String,
    pub ok: bool,
    pub protocol_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityRoutingPolicy {
    pub capability: String,
    pub primary_provider_id: String,
    pub primary_model: String,
    pub fallback_chain_json: String,
    pub timeout_ms: i64,
    pub retry_count: i64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatRoutingPolicy {
    pub primary_provider_id: String,
    pub primary_model: String,
    pub fallback_chain_json: String,
    pub timeout_ms: i64,
    pub retry_count: i64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelCatalogCacheEntry {
    pub model_id: String,
    pub fetched_at: String,
    pub ttl_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteAttemptLog {
    pub session_id: String,
    pub capability: String,
    pub api_format: String,
    pub model_name: String,
    pub attempt_index: i64,
    pub retry_index: i64,
    pub error_kind: String,
    pub success: bool,
    pub error_message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteAttemptStat {
    pub capability: String,
    pub error_kind: String,
    pub success: bool,
    pub count: i64,
}
