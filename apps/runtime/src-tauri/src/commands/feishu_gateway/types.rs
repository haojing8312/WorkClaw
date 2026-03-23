use crate::im::types::ImEvent;
use sqlx::FromRow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FeishuInboundGateDecision {
    Allow,
    Reject { reason: &'static str },
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct FeishuGateAccountConfig {
    pub(crate) dm_policy: Option<String>,
    pub(crate) group_policy: Option<String>,
    pub(crate) require_mention: Option<bool>,
    pub(crate) allow_from: Vec<String>,
    pub(crate) group_allow_from: Vec<String>,
    pub(crate) groups: std::collections::HashMap<String, FeishuGateGroupConfig>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct FeishuGateGroupConfig {
    pub(crate) enabled: Option<bool>,
    pub(crate) group_policy: Option<String>,
    pub(crate) require_mention: Option<bool>,
    pub(crate) allow_from: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuGatewayResult {
    pub accepted: bool,
    pub deduped: bool,
    pub challenge: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ImRouteDecisionEvent {
    pub session_id: String,
    pub thread_id: String,
    pub agent_id: String,
    pub session_key: String,
    pub matched_by: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuGatewaySettings {
    pub app_id: String,
    pub app_secret: String,
    pub ingress_token: String,
    pub encrypt_key: String,
    pub sidecar_base_url: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, FromRow)]
pub struct FeishuPairingRequestRecord {
    pub id: String,
    pub channel: String,
    pub account_id: String,
    pub sender_id: String,
    pub chat_id: String,
    pub code: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub resolved_at: Option<String>,
    pub resolved_by_user: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedFeishuPayload {
    Challenge(String),
    Event(ImEvent),
}
