#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ImReplyChunkPlan {
    pub index: usize,
    pub text: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ImReplyDeliveryPlan {
    pub logical_reply_id: String,
    pub session_id: String,
    pub channel: String,
    pub thread_id: String,
    pub chunks: Vec<ImReplyChunkPlan>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImReplyLifecyclePhase {
    ReplyStarted,
    ProcessingStarted,
    AskUserRequested,
    AskUserAnswered,
    ApprovalRequested,
    ApprovalResolved,
    InterruptRequested,
    Resumed,
    Failed,
    Stopped,
    ToolChunkQueued,
    BlockChunkQueued,
    FinalChunkQueued,
    WaitForIdle,
    IdleReached,
    FullyComplete,
    DispatchIdle,
    ProcessingStopped,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImReplyLifecycleEvent {
    pub logical_reply_id: String,
    pub phase: ImReplyLifecyclePhase,
    pub channel: String,
    pub account_id: Option<String>,
    pub thread_id: Option<String>,
    pub chat_id: Option<String>,
    pub message_id: Option<String>,
    pub queued_counts: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum ImReplyDeliveryState {
    Completed,
    Failed,
    FailedPartial,
}

#[cfg(test)]
mod tests {
    use super::{ImReplyDeliveryState, ImReplyLifecycleEvent, ImReplyLifecyclePhase};

    #[test]
    fn delivery_state_round_trips() {
        let value = ImReplyDeliveryState::FailedPartial;
        let json = serde_json::to_string(&value).expect("serialize state");
        let parsed: ImReplyDeliveryState = serde_json::from_str(&json).expect("deserialize state");
        assert_eq!(parsed, value);
    }

    #[test]
    fn lifecycle_event_round_trips() {
        let value = ImReplyLifecycleEvent {
            logical_reply_id: "reply-1".to_string(),
            phase: ImReplyLifecyclePhase::WaitForIdle,
            channel: "feishu".to_string(),
            account_id: Some("default".to_string()),
            thread_id: Some("oc_chat_1".to_string()),
            chat_id: None,
            message_id: Some("om_1".to_string()),
            queued_counts: Some(serde_json::json!({ "final": 1 })),
        };
        let json = serde_json::to_string(&value).expect("serialize lifecycle");
        let parsed: ImReplyLifecycleEvent =
            serde_json::from_str(&json).expect("deserialize lifecycle");
        assert_eq!(parsed, value);
    }
}
