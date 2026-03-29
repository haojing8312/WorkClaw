use crate::agent::runtime::RuntimeTranscript;
use serde_json::Value;

pub(crate) fn reconstruct_history_messages(
    history: &[(String, String, Option<String>)],
    api_format: &str,
) -> Vec<Value> {
    RuntimeTranscript::reconstruct_history_messages(history, api_format)
}

pub(crate) fn build_assistant_content_from_final_messages(
    final_messages: &[Value],
    reconstructed_history_len: usize,
) -> (String, bool, String) {
    RuntimeTranscript::build_assistant_content_from_final_messages(
        final_messages,
        reconstructed_history_len,
    )
}

pub(crate) fn build_assistant_content_with_stream_fallback(
    final_messages: &[Value],
    reconstructed_history_len: usize,
    streamed_text: &str,
) -> (String, bool, String) {
    RuntimeTranscript::build_assistant_content_with_stream_fallback(
        final_messages,
        reconstructed_history_len,
        streamed_text,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        build_assistant_content_from_final_messages, build_assistant_content_with_stream_fallback,
        reconstruct_history_messages,
    };
    use crate::agent::runtime::RuntimeTranscript;
    use serde_json::{json, Value};

    #[test]
    fn stream_fallback_restores_empty_text_response() {
        let final_messages = vec![json!({
            "role": "assistant",
            "content": ""
        })];

        let (final_text, has_tool_calls, content) =
            build_assistant_content_with_stream_fallback(&final_messages, 0, "你好，我在。");

        assert_eq!(final_text, "你好，我在。");
        assert!(!has_tool_calls);
        assert_eq!(content, "你好，我在。");
    }

    #[test]
    fn stream_fallback_preserves_tool_calls_when_text_missing() {
        let final_messages = vec![
            json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [
                    {
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "search",
                            "arguments": "{\"q\":\"minimax\"}"
                        }
                    }
                ]
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call-1",
                "content": "ok"
            }),
        ];

        let (_, has_tool_calls_before, content_before) =
            build_assistant_content_from_final_messages(&final_messages, 0);
        assert!(has_tool_calls_before);

        let (final_text, has_tool_calls, content) =
            build_assistant_content_with_stream_fallback(&final_messages, 0, "我查到了结果");

        assert_eq!(final_text, "我查到了结果");
        assert!(has_tool_calls);

        let parsed: Value = serde_json::from_str(&content).expect("structured content");
        assert_eq!(parsed["text"].as_str(), Some("我查到了结果"));
        assert_eq!(parsed["items"].as_array().map(|items| items.len()), Some(1));
        assert_eq!(
            parsed["items"][0]["toolCall"]["name"].as_str(),
            Some("search")
        );

        let parsed_before: Value =
            serde_json::from_str(&content_before).expect("structured content before fallback");
        assert_eq!(parsed_before["text"].as_str(), Some(""));
    }

    #[test]
    fn build_assistant_content_from_final_messages_does_not_duplicate_text_when_tool_calls_exist() {
        let final_messages = vec![json!({
            "role": "assistant",
            "content": "让我先检查正确的目录路径。",
            "tool_calls": [
                {
                    "id": "call-1",
                    "type": "function",
                    "function": {
                        "name": "list_dir",
                        "arguments": "{\"path\":\".\"}"
                    }
                }
            ]
        })];

        let (final_text, has_tool_calls, content) =
            build_assistant_content_from_final_messages(&final_messages, 0);

        assert_eq!(final_text, "让我先检查正确的目录路径。");
        assert!(has_tool_calls);

        let parsed: Value = serde_json::from_str(&content).expect("structured content");
        let items = parsed["items"].as_array().expect("items array");

        assert_eq!(parsed["text"].as_str(), Some("让我先检查正确的目录路径。"));
        assert_eq!(
            items
                .iter()
                .filter(|item| item["type"].as_str() == Some("text"))
                .count(),
            1
        );
        assert_eq!(
            items[0]["content"].as_str(),
            Some("让我先检查正确的目录路径。")
        );
        assert_eq!(items[1]["toolCall"]["name"].as_str(), Some("list_dir"));
    }

    #[test]
    fn reconstruct_history_messages_restores_user_multimodal_parts() {
        let history = vec![(
            "user".to_string(),
            "[图片 1 张] [文本文件 1 个]".to_string(),
            Some(
                serde_json::to_string(&vec![
                    json!({
                        "type": "text",
                        "text": "请分析这些附件"
                    }),
                    json!({
                        "type": "image",
                        "name": "screen.png",
                        "mimeType": "image/png",
                        "data": "data:image/png;base64,aGVsbG8="
                    }),
                    json!({
                        "type": "file_text",
                        "name": "debug.ts",
                        "mimeType": "text/plain",
                        "text": "console.log('hi')"
                    }),
                ])
                .expect("serialize parts"),
            ),
        )];

        let messages = reconstruct_history_messages(&history, "openai");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"].as_str(), Some("user"));
        let content = messages[0]["content"].as_array().expect("content array");
        assert_eq!(content[0]["type"].as_str(), Some("text"));
        assert!(content[0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("请分析这些附件"));
        assert!(content[0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("debug.ts"));
        assert_eq!(content[1]["type"].as_str(), Some("image_url"));
    }

    #[test]
    fn reconstruct_history_messages_restores_structured_assistant_text_without_replaying_json() {
        let history = vec![(
            "assistant".to_string(),
            serde_json::json!({
                "text": "我是 WorkClaw 助手。",
                "reasoning": {
                    "status": "completed",
                    "content": "先自我介绍"
                }
            })
            .to_string(),
            None,
        )];

        let messages = reconstruct_history_messages(&history, "openai");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"].as_str(), Some("assistant"));
        assert_eq!(messages[0]["content"].as_str(), Some("我是 WorkClaw 助手。"));
    }

    #[test]
    fn runtime_transcript_round_trip_preserves_tool_call_output_pairs() {
        let final_messages = vec![
            json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [
                    {
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"path\":\"README.md\"}"
                        }
                    }
                ]
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call-1",
                "content": "{\"summary\":\"done\"}"
            }),
        ];

        let (_, has_tool_calls, content) =
            RuntimeTranscript::build_assistant_content_from_final_messages(&final_messages, 0);
        assert!(has_tool_calls);

        let parsed: Value = serde_json::from_str(&content).expect("structured transcript");
        let reconstructed = RuntimeTranscript::reconstruct_llm_messages(&parsed, "openai");

        assert!(!reconstructed.is_empty());
        assert_eq!(reconstructed[0]["role"].as_str(), Some("assistant"));
        assert_eq!(reconstructed[1]["role"].as_str(), Some("tool"));
        assert_eq!(
            reconstructed[1]["content"].as_str(),
            Some("{\"summary\":\"done\"}")
        );
    }

    #[test]
    fn runtime_transcript_round_trip_preserves_tool_call_output_pairs_for_anthropic() {
        let final_messages = vec![
            json!({
                "role": "assistant",
                "content": [
                    {
                        "type": "text",
                        "text": "先检查目录。"
                    },
                    {
                        "type": "tool_use",
                        "id": "call-1",
                        "name": "list_dir",
                        "input": {"path": "."}
                    }
                ]
            }),
            json!({
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "call-1",
                        "content": "{\"summary\":\"ok\"}"
                    }
                ]
            }),
        ];

        let (_, has_tool_calls, content) =
            RuntimeTranscript::build_assistant_content_from_final_messages(&final_messages, 0);
        assert!(has_tool_calls);

        let parsed: Value = serde_json::from_str(&content).expect("structured transcript");
        let reconstructed = RuntimeTranscript::reconstruct_llm_messages(&parsed, "anthropic");

        assert_eq!(reconstructed.len(), 2);
        assert_eq!(reconstructed[0]["role"].as_str(), Some("assistant"));
        assert_eq!(
            reconstructed[0]["content"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(
            reconstructed[0]["content"][1]["type"].as_str(),
            Some("tool_use")
        );
        assert_eq!(reconstructed[1]["role"].as_str(), Some("user"));
        assert_eq!(
            reconstructed[1]["content"][0]["type"].as_str(),
            Some("tool_result")
        );
        assert_eq!(
            reconstructed[1]["content"][0]["tool_use_id"].as_str(),
            Some("call-1")
        );
        assert_eq!(
            reconstructed[1]["content"][0]["content"].as_str(),
            Some("{\"summary\":\"ok\"}")
        );
    }
}
