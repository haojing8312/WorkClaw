use runtime_lib::im::types::{ImEvent, ImEventType};

#[test]
fn im_event_parses_minimal_message_created() {
    let raw = r#"{"event_type":"message.created","thread_id":"t1","message_id":"m1","text":"hello"}"#;
    let evt: ImEvent = serde_json::from_str(raw).expect("should parse im event");
    assert_eq!(evt.event_type, ImEventType::MessageCreated);
    assert_eq!(evt.thread_id, "t1");
    assert_eq!(evt.message_id.as_deref(), Some("m1"));
    assert_eq!(evt.text.as_deref(), Some("hello"));
}

