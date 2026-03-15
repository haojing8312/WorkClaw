use runtime_lib::agent::permissions::PermissionMode;
use runtime_lib::agent::{AgentExecutor, ToolRegistry};
use serde_json::json;
use std::sync::Arc;

/// 验证 execute_turn 对 OpenAI 格式的行为
///
/// OpenAI 分支已接入 chat_stream_with_tools，
/// 使用无效 URL 时应返回网络错误（而非 "not yet implemented"）。
#[tokio::test]
async fn test_openai_tool_calling_executor_branch() {
    let registry = Arc::new(ToolRegistry::with_file_tools());
    let executor = AgentExecutor::new(registry);

    let messages = vec![json!({"role": "user", "content": "hello"})];

    let result = executor
        .execute_turn(
            "openai",
            "http://invalid-openai-mock-url",
            "mock-key",
            "gpt-4",
            "You are a helpful assistant.",
            messages,
            |_| {},
            None,
            None,
            None,
            PermissionMode::Unrestricted,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await;

    // 使用无效 URL 应返回网络错误
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        !err_msg.contains("not yet implemented"),
        "OpenAI tool calling 应该已实现，但得到: {}",
        err_msg
    );
}

/// 集成测试：需要真实 OpenAI 兼容 API 端点才能通过
/// 运行方式：OPENAI_API_KEY=xxx cargo test test_openai_tool_calling_real -- --ignored
#[tokio::test]
#[ignore]
async fn test_openai_tool_calling_real() {
    let registry = Arc::new(ToolRegistry::with_file_tools());
    let executor = AgentExecutor::new(registry);

    let messages = vec![json!({"role": "user", "content": "Read the file test.txt"})];

    let result = executor
        .execute_turn(
            "openai",
            "https://api.openai.com/v1",
            &std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            "gpt-4",
            "You are a helpful assistant with file tools.",
            messages,
            |token| {
                eprint!("{:?}", token);
            },
            None,
            None,
            None,
            PermissionMode::Unrestricted,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await;

    assert!(result.is_ok(), "OpenAI tool calling 失败: {:?}", result);
}
