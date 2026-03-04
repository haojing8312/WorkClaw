use runtime_lib::im::orchestrator::{resolve_next_action, OrchestratorAction};
use runtime_lib::im::types::{ImEvent, ImEventType};

#[test]
fn human_override_preempts_auto_turn() {
    let events = vec![
        ImEvent {
            event_type: ImEventType::MessageCreated,
            thread_id: "t1".to_string(),
            event_id: Some("evt-auto".to_string()),
            message_id: Some("m-auto".to_string()),
            text: Some("auto turn".to_string()),
            role_id: None,
            tenant_id: None,
        },
        ImEvent {
            event_type: ImEventType::HumanOverride,
            thread_id: "t1".to_string(),
            event_id: Some("evt-override".to_string()),
            message_id: Some("m-override".to_string()),
            text: Some("stop and follow this".to_string()),
            role_id: None,
            tenant_id: None,
        },
    ];

    let next = resolve_next_action(&events);
    assert_eq!(next, OrchestratorAction::ApplyOverride);
}
