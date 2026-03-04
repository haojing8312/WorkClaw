use runtime_lib::agent::executor::{micro_compact, trim_messages};
use serde_json::json;

#[test]
fn test_trim_under_budget() {
    let messages = vec![
        json!({"role": "user", "content": "hello"}),
        json!({"role": "assistant", "content": "hi"}),
    ];
    let trimmed = trim_messages(&messages, 10_000);
    assert_eq!(trimmed.len(), 2);
}

#[test]
fn test_trim_over_budget() {
    let long_text = "x".repeat(5000);
    let messages = vec![
        json!({"role": "user", "content": &long_text}),
        json!({"role": "assistant", "content": &long_text}),
        json!({"role": "user", "content": &long_text}),
        json!({"role": "assistant", "content": &long_text}),
        json!({"role": "user", "content": "latest question"}),
    ];
    // 预算 3000 tokens ≈ 12000 字符，5 条消息总约 20000+ 字符
    let trimmed = trim_messages(&messages, 3_000);
    assert!(trimmed.len() < 5);
    // 最后一条消息必须保留
    let last = trimmed.last().unwrap();
    assert_eq!(last["content"].as_str().unwrap(), "latest question");
    // 存在裁剪标记
    let has_marker = trimmed.iter().any(|m| {
        m["content"]
            .as_str()
            .map_or(false, |c| c.contains("已省略"))
    });
    assert!(has_marker);
}

#[test]
fn test_trim_preserves_first_and_last() {
    let text = "x".repeat(5000);
    let messages = vec![
        json!({"role": "user", "content": &text}),
        json!({"role": "assistant", "content": &text}),
        json!({"role": "user", "content": &text}),
        json!({"role": "assistant", "content": &text}),
        json!({"role": "user", "content": "final"}),
    ];
    let trimmed = trim_messages(&messages, 2_000);
    assert_eq!(trimmed.first().unwrap()["content"].as_str().unwrap(), &text);
    assert_eq!(
        trimmed.last().unwrap()["content"].as_str().unwrap(),
        "final"
    );
}

#[test]
fn test_trim_two_messages_never_trimmed() {
    let long = "x".repeat(100_000);
    let messages = vec![
        json!({"role": "user", "content": &long}),
        json!({"role": "assistant", "content": &long}),
    ];
    // 即使超预算，只有 2 条消息也不裁剪
    let trimmed = trim_messages(&messages, 100);
    assert_eq!(trimmed.len(), 2);
}

#[test]
fn test_micro_compact_replaces_old_tool_results() {
    let messages = vec![
        json!({"role": "user", "content": "start"}),
        json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": "1", "content": "long output 1 long output 1 long output 1"}]}),
        json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": "2", "content": "long output 2 long output 2 long output 2"}]}),
        json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": "3", "content": "long output 3"}]}),
        json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": "4", "content": "long output 4"}]}),
        json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": "5", "content": "recent output"}]}),
        json!({"role": "assistant", "content": "done"}),
    ];

    let result = micro_compact(&messages, 3);
    // 旧的 tool_result（id 1、2）应替换为 [已执行]
    let r1 = serde_json::to_string(&result[1]).unwrap();
    assert!(
        r1.contains("[已执行]"),
        "Old tool result 1 should be replaced"
    );
    let r2 = serde_json::to_string(&result[2]).unwrap();
    assert!(
        r2.contains("[已执行]"),
        "Old tool result 2 should be replaced"
    );
    // 近期的（id 3、4、5）应保留原始内容
    let r5 = serde_json::to_string(&result[5]).unwrap();
    assert!(
        r5.contains("recent output"),
        "Recent tool result should be preserved"
    );
}

#[test]
fn test_micro_compact_few_messages_no_change() {
    let messages = vec![
        json!({"role": "user", "content": "hello"}),
        json!({"role": "assistant", "content": "hi"}),
    ];
    let result = micro_compact(&messages, 3);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0]["content"].as_str().unwrap(), "hello");
}

#[test]
fn test_micro_compact_openai_tool_role() {
    let messages = vec![
        json!({"role": "user", "content": "start"}),
        json!({"role": "tool", "tool_call_id": "c1", "content": "old output 1"}),
        json!({"role": "tool", "tool_call_id": "c2", "content": "old output 2"}),
        json!({"role": "tool", "tool_call_id": "c3", "content": "recent output"}),
        json!({"role": "assistant", "content": "done"}),
    ];

    let result = micro_compact(&messages, 1);
    // 只保留最后 1 条 tool result（c3），其余替换为 [已执行]
    assert_eq!(result[1]["content"].as_str().unwrap(), "[已执行]");
    assert_eq!(result[2]["content"].as_str().unwrap(), "[已执行]");
    assert_eq!(result[3]["content"].as_str().unwrap(), "recent output");
}
