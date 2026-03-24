use runtime_executor_core::split_error_code_and_message;
use runtime_lib::agent::event_bridge::build_skill_route_event;
use runtime_lib::commands::chat::SkillRouteEvent;
use serde_json::Value;

#[test]
fn skill_route_event_payload_has_required_fields() {
    let evt = SkillRouteEvent {
        session_id: "s1".to_string(),
        route_run_id: "r1".to_string(),
        node_id: "n1".to_string(),
        parent_node_id: Some("root".to_string()),
        skill_name: "using-superpowers".to_string(),
        depth: 1,
        status: "routing".to_string(),
        duration_ms: None,
        error_code: None,
        error_message: None,
    };

    let v: Value = serde_json::to_value(evt).expect("event should serialize");
    assert_eq!(v["session_id"], "s1");
    assert_eq!(v["route_run_id"], "r1");
    assert_eq!(v["node_id"], "n1");
    assert_eq!(v["parent_node_id"], "root");
    assert_eq!(v["skill_name"], "using-superpowers");
    assert_eq!(v["depth"], 1);
    assert_eq!(v["status"], "routing");
}

#[test]
fn split_error_code_parses_prefixed_errors() {
    let (code, msg) = split_error_code_and_message("SKILL_NOT_FOUND: missing child");
    assert_eq!(code, "SKILL_NOT_FOUND");
    assert_eq!(msg, "missing child");

    let (code2, msg2) = split_error_code_and_message("plain text error");
    assert_eq!(code2, "SKILL_EXECUTION_ERROR");
    assert_eq!(msg2, "plain text error");
}

#[test]
fn skill_route_lifecycle_statuses_are_serialized() {
    let statuses = ["routing", "executing", "completed", "failed"];
    for status in statuses {
        let v = build_skill_route_event(
            "s1",
            "r1",
            "n1",
            None,
            "child-skill",
            1,
            status,
            Some(12),
            None,
            None,
        );
        assert_eq!(v["status"], status);
        assert_eq!(v["route_run_id"], "r1");
        assert_eq!(v["skill_name"], "child-skill");
    }
}
