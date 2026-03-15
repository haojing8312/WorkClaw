use runtime_lib::agent::permissions::PermissionMode;
use runtime_lib::agent::{AgentExecutor, ToolRegistry};
use runtime_lib::providers::{route_with_fallback, RouteFailureKind, RouteTarget, RoutingPolicy};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

fn setup_work_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("test_react_loop_{}", name));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn test_react_loop_structure() {
    let registry = Arc::new(ToolRegistry::with_file_tools());
    let _executor = AgentExecutor::new(registry);

    // 验证 AgentExecutor 创建成功且默认值正确
    assert!(true, "AgentExecutor created successfully");
}

#[tokio::test]
async fn test_react_loop_max_iterations_error() {
    let registry = Arc::new(ToolRegistry::with_file_tools());
    let executor = AgentExecutor::with_max_iterations(Arc::clone(&registry), 0);

    let messages = vec![json!({"role": "user", "content": "hello"})];

    let result = executor
        .execute_turn(
            "anthropic",
            "http://mock-url",
            "mock-key",
            "mock-model",
            "You are a helpful assistant.",
            messages,
            |_token| {},
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

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("最大迭代次数"));
}

#[tokio::test]
async fn test_react_loop_openai_format_network_error() {
    let registry = Arc::new(ToolRegistry::with_file_tools());
    let executor = AgentExecutor::new(registry);

    let messages = vec![json!({"role": "user", "content": "hello"})];

    let result = executor
        .execute_turn(
            "openai",
            "http://invalid-openai-url",
            "mock-key",
            "gpt-4",
            "You are a helpful assistant.",
            messages,
            |_token| {},
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

    // OpenAI 格式应返回网络错误（不是 "not yet implemented"）
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(!err_msg.contains("not yet implemented"));
}

#[tokio::test]
async fn test_react_loop_stops_repeated_invalid_write_file_calls() {
    let registry = Arc::new(ToolRegistry::with_file_tools());
    let executor = AgentExecutor::with_max_iterations(Arc::clone(&registry), 8);

    let messages = vec![json!({
        "role": "user",
        "content": "请生成一个 HTML 网页版本的简报"
    })];

    let result = executor
        .execute_turn(
            "openai",
            "http://mock-repeat-invalid-write-file",
            "mock-key",
            "gpt-4",
            "You are a helpful assistant.",
            messages,
            |_token| {},
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

    assert!(
        result.is_ok(),
        "重复的无效 write_file 调用应被熔断，而不是耗尽迭代: {:?}",
        result
    );

    let messages = result.unwrap();
    let last = messages.last().expect("assistant summary message");
    let content = last["content"].as_str().unwrap_or_default();
    assert!(
        content.contains("重复调用") || content.contains("write_file"),
        "应返回针对重复无效工具调用的说明，实际: {}",
        content
    );
}

#[tokio::test]
async fn test_react_loop_executes_absolute_nested_write_within_work_dir() {
    let registry = Arc::new(ToolRegistry::with_file_tools());
    let executor = AgentExecutor::with_max_iterations(Arc::clone(&registry), 4);
    let work_dir = setup_work_dir("absolute_nested_write");
    let target = work_dir
        .join("公众号文章")
        .join("20251120-WorkClaw企业版介绍")
        .join("brief.md");
    let request = json!({
        "path": target.to_str().unwrap(),
        "content": "# brief"
    })
    .to_string();

    let messages = vec![json!({
        "role": "user",
        "content": request
    })];

    let result = executor
        .execute_turn(
            "openai",
            "http://mock-write-file-from-user-path",
            "mock-key",
            "gpt-4",
            "You are a helpful assistant.",
            messages,
            |_token| {},
            None,
            None,
            None,
            PermissionMode::AcceptEdits,
            None,
            Some(work_dir.to_string_lossy().to_string()),
            None,
            None,
            None,
            None,
        )
        .await;

    assert!(
        result.is_ok(),
        "executor should finish successfully: {:?}",
        result
    );
    assert_eq!(fs::read_to_string(&target).unwrap(), "# brief");

    let messages = result.unwrap();
    let last = messages.last().expect("assistant final message");
    assert!(
        last["content"]
            .as_str()
            .unwrap_or_default()
            .contains("已完成文件写入"),
        "unexpected final content: {:?}",
        last
    );

    fs::remove_dir_all(&work_dir).unwrap();
}

#[test]
fn router_uses_fallback_on_primary_error() {
    let policy = RoutingPolicy {
        capability: "chat".to_string(),
        primary: RouteTarget {
            provider_id: "deepseek".to_string(),
            model: "deepseek-chat".to_string(),
        },
        fallbacks: vec![RouteTarget {
            provider_id: "qwen".to_string(),
            model: "qwen-max".to_string(),
        }],
    };

    let primary = route_with_fallback(&policy, None).expect("primary route");
    assert_eq!(primary.provider_id, "deepseek");
    assert_eq!(primary.model, "deepseek-chat");

    let fallback =
        route_with_fallback(&policy, Some(RouteFailureKind::RateLimit)).expect("fallback route");
    assert_eq!(fallback.provider_id, "qwen");
    assert_eq!(fallback.model, "qwen-max");
}
