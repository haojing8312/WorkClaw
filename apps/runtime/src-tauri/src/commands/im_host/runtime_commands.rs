use super::contract::ImReplyLifecyclePhase;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImRuntimeTextCommandPayload {
    pub request_id: String,
    pub command: String,
    pub account_id: String,
    pub target: String,
    pub thread_id: Option<String>,
    pub text: String,
    pub mode: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImRuntimeProcessingStopCommandPayload {
    pub request_id: String,
    pub command: String,
    pub account_id: String,
    pub message_id: String,
    pub logical_reply_id: Option<String>,
    pub final_state: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImRuntimeLifecycleEventCommandPayload {
    pub request_id: String,
    pub command: String,
    pub account_id: String,
    pub phase: ImReplyLifecyclePhase,
    pub logical_reply_id: Option<String>,
    pub thread_id: Option<String>,
    pub message_id: Option<String>,
}

pub(crate) fn build_runtime_text_command_payload(
    request_id: String,
    command: &str,
    account_id: String,
    target: String,
    thread_id: Option<String>,
    text: String,
    mode: String,
) -> ImRuntimeTextCommandPayload {
    ImRuntimeTextCommandPayload {
        request_id,
        command: command.to_string(),
        account_id,
        target,
        thread_id,
        text,
        mode,
    }
}

pub(crate) fn build_runtime_processing_stop_command_payload(
    request_id: String,
    command: &str,
    account_id: String,
    message_id: String,
    logical_reply_id: Option<String>,
    final_state: Option<String>,
) -> ImRuntimeProcessingStopCommandPayload {
    ImRuntimeProcessingStopCommandPayload {
        request_id,
        command: command.to_string(),
        account_id,
        message_id,
        logical_reply_id,
        final_state,
    }
}

pub(crate) fn build_runtime_lifecycle_event_command_payload(
    request_id: String,
    command: &str,
    account_id: String,
    phase: ImReplyLifecyclePhase,
    logical_reply_id: Option<String>,
    thread_id: Option<String>,
    message_id: Option<String>,
) -> ImRuntimeLifecycleEventCommandPayload {
    ImRuntimeLifecycleEventCommandPayload {
        request_id,
        command: command.to_string(),
        account_id,
        phase,
        logical_reply_id,
        thread_id,
        message_id,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_runtime_lifecycle_event_command_payload,
        build_runtime_processing_stop_command_payload, build_runtime_text_command_payload,
    };
    use crate::commands::im_host::ImReplyLifecyclePhase;

    #[test]
    fn builds_text_command_payload() {
        let payload = build_runtime_text_command_payload(
            "req-1".to_string(),
            "send_message",
            "default".to_string(),
            "oc_chat_1".to_string(),
            Some("oc_chat_1".to_string()),
            "hello".to_string(),
            "text".to_string(),
        );

        assert_eq!(payload.command, "send_message");
        assert_eq!(payload.target, "oc_chat_1");
    }

    #[test]
    fn builds_lifecycle_command_payload() {
        let payload = build_runtime_lifecycle_event_command_payload(
            "req-2".to_string(),
            "lifecycle_event",
            "default".to_string(),
            ImReplyLifecyclePhase::ProcessingStarted,
            Some("reply-1".to_string()),
            Some("oc_chat_1".to_string()),
            Some("om_1".to_string()),
        );

        assert_eq!(payload.command, "lifecycle_event");
        assert_eq!(payload.phase, ImReplyLifecyclePhase::ProcessingStarted);
    }

    #[test]
    fn builds_processing_stop_command_payload() {
        let payload = build_runtime_processing_stop_command_payload(
            "req-3".to_string(),
            "processing_stop",
            "default".to_string(),
            "om_1".to_string(),
            Some("reply-1".to_string()),
            Some("completed".to_string()),
        );

        assert_eq!(payload.command, "processing_stop");
        assert_eq!(payload.message_id, "om_1");
    }
}
