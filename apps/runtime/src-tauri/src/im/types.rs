use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImEvent {
    pub event_type: ImEventType,
    pub thread_id: String,
    #[serde(default)]
    pub event_id: Option<String>,
    #[serde(default)]
    pub message_id: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub role_id: Option<String>,
    #[serde(default)]
    pub tenant_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImEventType {
    #[serde(rename = "message.created")]
    MessageCreated,
    #[serde(rename = "mention.role")]
    MentionRole,
    #[serde(rename = "command.pause")]
    CommandPause,
    #[serde(rename = "command.resume")]
    CommandResume,
    #[serde(rename = "human.override")]
    HumanOverride,
}

