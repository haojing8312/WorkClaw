use runtime_lib::im::feishu_adapter::{build_feishu_markdown_message, build_feishu_text_message};

#[test]
fn build_feishu_text_message_has_expected_shape() {
    let payload = build_feishu_text_message("chat-1", "hello");
    assert_eq!(payload["receive_id"], "chat-1");
    assert_eq!(payload["msg_type"], "text");
    let content = payload["content"].as_str().unwrap_or_default();
    assert!(content.contains("hello"));
}

#[test]
fn build_feishu_markdown_message_has_expected_shape() {
    let payload = build_feishu_markdown_message("chat-1", "结论\\n建议承接");
    assert_eq!(payload["receive_id"], "chat-1");
    assert_eq!(payload["msg_type"], "post");
    let content = payload["content"].as_str().unwrap_or_default();
    assert!(content.contains("智能体协作更新"));
}

