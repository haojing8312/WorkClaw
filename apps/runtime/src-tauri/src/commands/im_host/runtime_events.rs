pub(crate) fn runtime_event_name(value: &serde_json::Value) -> Option<&str> {
    value.get("event").and_then(|entry| entry.as_str())
}

pub(crate) fn parse_runtime_event<T>(
    value: &serde_json::Value,
    expected_event: &str,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let event = runtime_event_name(value).unwrap_or_default();
    if event != expected_event {
        return Err(format!("unexpected runtime event: {event}"));
    }
    serde_json::from_value::<T>(value.clone())
        .map_err(|error| format!("invalid {expected_event} event: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{parse_runtime_event, runtime_event_name};

    #[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
    struct SampleEvent {
        event: String,
        value: String,
    }

    #[test]
    fn runtime_event_name_reads_event_field() {
        let value = serde_json::json!({ "event": "send_result" });
        assert_eq!(runtime_event_name(&value), Some("send_result"));
    }

    #[test]
    fn parse_runtime_event_validates_expected_event() {
        let value = serde_json::json!({
            "event": "send_result",
            "value": "ok"
        });
        let parsed: SampleEvent =
            parse_runtime_event(&value, "send_result").expect("parse runtime event");
        assert_eq!(parsed.value, "ok");
    }
}
