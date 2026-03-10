use serde::{Deserialize, Serialize};

fn default_im_channel() -> String {
    "feishu".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImEvent {
    #[serde(default = "default_im_channel")]
    pub channel: String,
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
    pub account_id: Option<String>,
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
