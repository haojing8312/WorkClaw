use runtime_lib::agent::runtime::RuntimeTranscript;
use serde_json::json;

#[test]
fn anthropic_sanitizer_synthesizes_missing_tool_results_for_replay() {
    let messages = vec![json!({
        "role": "assistant",
        "content": [
            {
                "type": "tool_use",
                "id": "call-1",
                "name": "read_file",
                "input": {"path": "README.md"}
            }
        ]
    })];

    let repaired = RuntimeTranscript::sanitize_reconstructed_messages(messages, "anthropic");

    assert_eq!(repaired.len(), 2);
    assert_eq!(repaired[0]["role"].as_str(), Some("assistant"));
    assert_eq!(repaired[1]["role"].as_str(), Some("user"));
    assert_eq!(
        repaired[1]["content"][0]["type"].as_str(),
        Some("tool_result")
    );
    assert_eq!(
        repaired[1]["content"][0]["tool_use_id"].as_str(),
        Some("call-1")
    );
    assert_eq!(
        repaired[1]["content"][0]["content"].as_str(),
        Some("[已执行]")
    );
}

#[test]
fn anthropic_sanitizer_appends_only_missing_tool_results_to_partial_user_message() {
    let messages = vec![
        json!({
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    "id": "call-1",
                    "name": "read_file",
                    "input": {"path": "README.md"}
                },
                {
                    "type": "tool_use",
                    "id": "call-2",
                    "name": "grep",
                    "input": {"pattern": "TODO"}
                }
            ]
        }),
        json!({
            "role": "user",
            "content": [
                {
                    "type": "tool_result",
                    "tool_use_id": "call-1",
                    "content": "read ok"
                }
            ]
        }),
    ];

    let repaired = RuntimeTranscript::sanitize_reconstructed_messages(messages, "anthropic");

    assert_eq!(repaired.len(), 2);
    let results = repaired[1]["content"].as_array().expect("tool results");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["tool_use_id"].as_str(), Some("call-1"));
    assert_eq!(results[0]["content"].as_str(), Some("read ok"));
    assert_eq!(results[1]["tool_use_id"].as_str(), Some("call-2"));
    assert_eq!(results[1]["content"].as_str(), Some("[已执行]"));
}
