use runtime_lib::im::runtime_bridge::{
    build_im_role_dispatch_request, build_im_role_event_payload, build_runtime_task_payload,
    normalize_stream_token, RoleTaskRequest,
};

#[tokio::test]
async fn bridge_dispatches_role_task_and_receives_stream_events() {
    let req = RoleTaskRequest {
        role_id: "architect".to_string(),
        role_name: "架构师".to_string(),
        prompt: "评估该商机的技术可行性".to_string(),
        agent_type: "plan".to_string(),
    };

    let payload = build_runtime_task_payload(&req);
    assert_eq!(payload["agent_type"], "plan");
    assert_eq!(payload["role_id"], "architect");
    assert!(payload["prompt"]
        .as_str()
        .unwrap_or_default()
        .contains("架构师"));

    let stream_raw = serde_json::json!({
        "session_id": "s1",
        "token": "阶段结论：可承接",
        "done": false,
        "sub_agent": true
    });
    let event =
        normalize_stream_token("architect", "架构师", &stream_raw).expect("event should normalize");
    assert_eq!(event.role_id, "architect");
    assert_eq!(event.role_name, "架构师");
    assert_eq!(event.token, "阶段结论：可承接");
    assert!(!event.done);
    assert!(event.sub_agent);

    let timeline = build_im_role_event_payload(
        "session-1",
        "thread-1",
        "architect",
        "架构师",
        "running",
        "正在评估技术可行性",
        Some(1200),
    );
    assert_eq!(timeline.session_id, "session-1");
    assert_eq!(timeline.thread_id, "thread-1");
    assert_eq!(timeline.status, "running");
    assert_eq!(timeline.source_channel, "app");

    let dispatch = build_im_role_dispatch_request(
        "session-1",
        "thread-1",
        "architect",
        "架构师",
        "请继续评审",
        "plan",
    );
    assert_eq!(dispatch.source_channel, "app");
}
