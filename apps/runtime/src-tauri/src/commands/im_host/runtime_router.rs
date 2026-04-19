use super::runtime_events::runtime_event_name;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImRuntimeEventRoute {
    SendResult,
    CommandError,
    ReplyLifecycle,
    PairingRequest,
    DispatchRequest,
    Other(String),
}

pub(crate) fn route_runtime_event(value: &serde_json::Value) -> Option<ImRuntimeEventRoute> {
    let event = runtime_event_name(value)?;
    Some(match event {
        "send_result" => ImRuntimeEventRoute::SendResult,
        "command_error" => ImRuntimeEventRoute::CommandError,
        "reply_lifecycle" => ImRuntimeEventRoute::ReplyLifecycle,
        "pairing_request" => ImRuntimeEventRoute::PairingRequest,
        "dispatch_request" => ImRuntimeEventRoute::DispatchRequest,
        other => ImRuntimeEventRoute::Other(other.to_string()),
    })
}

#[cfg(test)]
pub(crate) fn dispatch_runtime_route<
    R,
    FSend,
    FCommandError,
    FReplyLifecycle,
    FPairing,
    FDispatch,
    FOther,
>(
    route: Option<&ImRuntimeEventRoute>,
    on_send_result: FSend,
    on_command_error: FCommandError,
    on_reply_lifecycle: FReplyLifecycle,
    on_pairing_request: FPairing,
    on_dispatch_request: FDispatch,
    on_other: FOther,
) -> R
where
    FSend: FnOnce() -> R,
    FCommandError: FnOnce() -> R,
    FReplyLifecycle: FnOnce() -> R,
    FPairing: FnOnce() -> R,
    FDispatch: FnOnce() -> R,
    FOther: FnOnce() -> R,
{
    match route {
        Some(ImRuntimeEventRoute::SendResult) => on_send_result(),
        Some(ImRuntimeEventRoute::CommandError) => on_command_error(),
        Some(ImRuntimeEventRoute::ReplyLifecycle) => on_reply_lifecycle(),
        Some(ImRuntimeEventRoute::PairingRequest) => on_pairing_request(),
        Some(ImRuntimeEventRoute::DispatchRequest) => on_dispatch_request(),
        _ => on_other(),
    }
}

#[cfg(test)]
mod tests {
    use super::{dispatch_runtime_route, route_runtime_event, ImRuntimeEventRoute};

    #[test]
    fn routes_known_runtime_events() {
        let value = serde_json::json!({ "event": "send_result" });
        assert_eq!(
            route_runtime_event(&value),
            Some(ImRuntimeEventRoute::SendResult)
        );
    }

    #[test]
    fn preserves_unknown_runtime_event_names() {
        let value = serde_json::json!({ "event": "custom_event" });
        assert_eq!(
            route_runtime_event(&value),
            Some(ImRuntimeEventRoute::Other("custom_event".to_string()))
        );
    }

    #[test]
    fn dispatch_runtime_route_selects_matching_branch() {
        let route = Some(ImRuntimeEventRoute::DispatchRequest);
        let result = dispatch_runtime_route(
            route.as_ref(),
            || "send",
            || "command_error",
            || "reply",
            || "pairing",
            || "dispatch",
            || "other",
        );
        assert_eq!(result, "dispatch");
    }
}
