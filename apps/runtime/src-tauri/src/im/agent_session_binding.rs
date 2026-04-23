#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentConversationBinding {
    pub conversation_id: String,
    pub channel: String,
    pub account_id: String,
    pub agent_id: String,
    pub session_key: String,
    pub session_id: String,
    pub base_conversation_id: String,
    pub parent_conversation_candidates: Vec<String>,
    pub scope: String,
    pub peer_kind: String,
    pub peer_id: String,
    pub topic_id: String,
    pub sender_id: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentConversationBindingUpsert<'a> {
    pub conversation_id: &'a str,
    pub channel: &'a str,
    pub account_id: &'a str,
    pub agent_id: &'a str,
    pub session_key: &'a str,
    pub session_id: &'a str,
    pub base_conversation_id: &'a str,
    pub parent_conversation_candidates: &'a [String],
    pub scope: &'a str,
    pub peer_kind: &'a str,
    pub peer_id: &'a str,
    pub topic_id: &'a str,
    pub sender_id: &'a str,
    pub created_at: &'a str,
    pub updated_at: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelDeliveryRoute {
    pub session_key: String,
    pub channel: String,
    pub account_id: String,
    pub conversation_id: String,
    pub reply_target: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelDeliveryRouteUpsert<'a> {
    pub session_key: &'a str,
    pub channel: &'a str,
    pub account_id: &'a str,
    pub conversation_id: &'a str,
    pub reply_target: &'a str,
    pub updated_at: &'a str,
}
