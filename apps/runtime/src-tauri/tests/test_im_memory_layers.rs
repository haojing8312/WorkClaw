use runtime_lib::im::memory::{capture_entry, recall_context, MemoryEntry};

#[test]
fn capture_writes_session_and_long_term_with_gate() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let root = tmp.path();

    let confirmed = MemoryEntry {
        category: "fact".to_string(),
        content: "客户预算在 80-120 万".to_string(),
        confirmed: true,
        source_msg_id: "msg-1".to_string(),
        author_role: "presales".to_string(),
        confidence: 0.9,
    };
    let r1 = capture_entry(root, "thread-1", "presales", &confirmed).expect("capture confirmed");
    assert!(r1.session_written);
    assert!(r1.long_term_written);

    let unconfirmed = MemoryEntry {
        category: "risk".to_string(),
        content: "可能存在第三方系统不可控".to_string(),
        confirmed: false,
        source_msg_id: "msg-2".to_string(),
        author_role: "architect".to_string(),
        confidence: 0.5,
    };
    let r2 = capture_entry(root, "thread-1", "architect", &unconfirmed).expect("capture unconfirmed");
    assert!(r2.session_written);
    assert!(!r2.long_term_written);

    let recalled = recall_context(root, "thread-1", "presales").expect("recall context");
    assert!(recalled.contains("客户预算在 80-120 万"));
    assert!(recalled.contains("msg-1"));
}

