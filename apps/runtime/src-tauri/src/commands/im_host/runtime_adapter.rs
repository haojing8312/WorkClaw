use super::runtime_router::ImRuntimeEventRoute;

pub(crate) trait ImRuntimeStdoutAdapter {
    fn handle_send_result(&mut self, value: &serde_json::Value) -> bool;
    fn handle_command_error(&mut self, value: &serde_json::Value) -> bool;
    fn handle_reply_lifecycle(&mut self, value: &serde_json::Value) -> bool;
    fn handle_pairing_request(&mut self, value: &serde_json::Value);
    fn handle_dispatch_request(&mut self, value: &serde_json::Value);
    fn handle_other(&mut self, value: &serde_json::Value);
}

pub(crate) fn dispatch_runtime_stdout_with_adapter<A: ImRuntimeStdoutAdapter>(
    adapter: &mut A,
    route: Option<&ImRuntimeEventRoute>,
    value: &serde_json::Value,
) {
    match route {
        Some(ImRuntimeEventRoute::SendResult) => {
            let _ = adapter.handle_send_result(value);
        }
        Some(ImRuntimeEventRoute::CommandError) => {
            let _ = adapter.handle_command_error(value);
        }
        Some(ImRuntimeEventRoute::ReplyLifecycle) => {
            let _ = adapter.handle_reply_lifecycle(value);
        }
        Some(ImRuntimeEventRoute::PairingRequest) => {
            adapter.handle_pairing_request(value);
        }
        Some(ImRuntimeEventRoute::DispatchRequest) => {
            adapter.handle_dispatch_request(value);
        }
        _ => {
            adapter.handle_other(value);
        }
    }
}

pub(crate) fn handle_runtime_stdout_line_with_adapter<A: ImRuntimeStdoutAdapter>(
    adapter: &mut A,
    trimmed: &str,
) -> bool {
    if trimmed.is_empty() {
        return false;
    }

    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return false;
    };

    let route = super::runtime_router::route_runtime_event(&value);
    dispatch_runtime_stdout_with_adapter(adapter, route.as_ref(), &value);
    true
}

#[cfg(test)]
mod tests {
    use super::{
        dispatch_runtime_stdout_with_adapter, handle_runtime_stdout_line_with_adapter,
        ImRuntimeStdoutAdapter, ImRuntimeEventRoute,
    };

    #[derive(Default)]
    struct RecordingAdapter {
        calls: Vec<&'static str>,
    }

    impl ImRuntimeStdoutAdapter for RecordingAdapter {
        fn handle_send_result(&mut self, _value: &serde_json::Value) -> bool {
            self.calls.push("send_result");
            true
        }

        fn handle_command_error(&mut self, _value: &serde_json::Value) -> bool {
            self.calls.push("command_error");
            true
        }

        fn handle_reply_lifecycle(&mut self, _value: &serde_json::Value) -> bool {
            self.calls.push("reply_lifecycle");
            true
        }

        fn handle_pairing_request(&mut self, _value: &serde_json::Value) {
            self.calls.push("pairing_request");
        }

        fn handle_dispatch_request(&mut self, _value: &serde_json::Value) {
            self.calls.push("dispatch_request");
        }

        fn handle_other(&mut self, _value: &serde_json::Value) {
            self.calls.push("other");
        }
    }

    #[test]
    fn dispatches_to_matching_adapter_method() {
        let mut adapter = RecordingAdapter::default();
        let value = serde_json::json!({ "event": "dispatch_request" });

        dispatch_runtime_stdout_with_adapter(
            &mut adapter,
            Some(&ImRuntimeEventRoute::DispatchRequest),
            &value,
        );

        assert_eq!(adapter.calls, vec!["dispatch_request"]);
    }

    #[test]
    fn handles_runtime_stdout_line_with_adapter() {
        let mut adapter = RecordingAdapter::default();
        let handled = handle_runtime_stdout_line_with_adapter(
            &mut adapter,
            r#"{"event":"command_error","error":"bad target"}"#,
        );

        assert!(handled);
        assert_eq!(adapter.calls, vec!["command_error"]);
    }
}
