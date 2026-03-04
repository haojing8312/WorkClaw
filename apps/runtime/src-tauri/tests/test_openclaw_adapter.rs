use runtime_lib::im::openclaw_adapter::build_openclaw_outbound_message;

#[test]
fn build_openclaw_outbound_message_has_expected_shape() {
    let payload =
        build_openclaw_outbound_message("thread-1", "architect", "架构师", "结论\n建议承接");

    assert_eq!(payload["thread_id"], "thread-1");
    assert_eq!(payload["sender"]["type"], "role");
    assert_eq!(payload["sender"]["id"], "architect");
    assert_eq!(payload["message"]["content_type"], "text/markdown");
}
