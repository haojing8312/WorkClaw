use runtime_lib::agent::tools::MemoryTool;
use runtime_lib::agent::types::{Tool, ToolContext};
use serde_json::json;

/// 创建临时目录并返回 MemoryTool 实例
fn create_test_memory() -> (MemoryTool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let tool = MemoryTool::new(dir.path().to_path_buf());
    (tool, dir)
}

#[test]
fn test_memory_write_and_read() {
    let (tool, _dir) = create_test_memory();
    let ctx = ToolContext::default();

    // 写入内容
    let write_result = tool
        .execute(
            json!({
                "action": "write",
                "key": "test",
                "content": "Hello Memory"
            }),
            &ctx,
        )
        .unwrap();
    assert!(write_result.contains("已写入"));

    // 读回并验证内容一致
    let read_result = tool
        .execute(
            json!({
                "action": "read",
                "key": "test"
            }),
            &ctx,
        )
        .unwrap();
    assert_eq!(read_result, "Hello Memory");
}

#[test]
fn test_memory_list() {
    let (tool, _dir) = create_test_memory();
    let ctx = ToolContext::default();

    // 未写入时应返回空提示
    let result = tool.execute(json!({"action": "list"}), &ctx).unwrap();
    assert!(result.contains("内存为空"));

    // 写入两个键后列表应包含两个键名
    tool.execute(json!({"action": "write", "key": "a", "content": "1"}), &ctx)
        .unwrap();
    tool.execute(json!({"action": "write", "key": "b", "content": "2"}), &ctx)
        .unwrap();
    let result = tool.execute(json!({"action": "list"}), &ctx).unwrap();
    assert!(result.contains("a"));
    assert!(result.contains("b"));
}

#[test]
fn test_memory_delete() {
    let (tool, _dir) = create_test_memory();
    let ctx = ToolContext::default();

    // 写入后删除
    tool.execute(
        json!({"action": "write", "key": "del", "content": "x"}),
        &ctx,
    )
    .unwrap();
    let result = tool
        .execute(json!({"action": "delete", "key": "del"}), &ctx)
        .unwrap();
    assert!(result.contains("已删除"));

    // 删除后读取应返回不存在提示
    let read_result = tool
        .execute(json!({"action": "read", "key": "del"}), &ctx)
        .unwrap();
    assert!(read_result.contains("不存在"));
}

#[test]
fn test_memory_read_nonexistent() {
    let (tool, _dir) = create_test_memory();
    let ctx = ToolContext::default();

    // 读取不存在的键应返回友好提示而非 error
    let result = tool
        .execute(json!({"action": "read", "key": "nope"}), &ctx)
        .unwrap();
    assert!(result.contains("不存在"));
}

#[test]
fn test_memory_missing_action() {
    let (tool, _dir) = create_test_memory();
    let ctx = ToolContext::default();

    // 缺少 action 参数应返回错误
    let result = tool.execute(json!({}), &ctx);
    assert!(result.is_err());
}

#[test]
fn test_memory_overwrite() {
    let (tool, _dir) = create_test_memory();
    let ctx = ToolContext::default();

    // 同一个键多次写入，应以最新内容为准
    tool.execute(
        json!({"action": "write", "key": "k", "content": "first"}),
        &ctx,
    )
    .unwrap();
    tool.execute(
        json!({"action": "write", "key": "k", "content": "second"}),
        &ctx,
    )
    .unwrap();
    let result = tool
        .execute(json!({"action": "read", "key": "k"}), &ctx)
        .unwrap();
    assert_eq!(result, "second");
}

#[test]
fn test_memory_delete_nonexistent() {
    let (tool, _dir) = create_test_memory();
    let ctx = ToolContext::default();

    // 删除不存在的键应返回友好提示而非 error
    let result = tool
        .execute(json!({"action": "delete", "key": "ghost"}), &ctx)
        .unwrap();
    assert!(result.contains("不存在"));
}

#[test]
fn test_memory_unknown_action() {
    let (tool, _dir) = create_test_memory();
    let ctx = ToolContext::default();

    // 未知操作应返回错误
    let result = tool.execute(json!({"action": "explode"}), &ctx);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("未知操作"));
}

#[test]
fn test_memory_list_sorted() {
    let (tool, _dir) = create_test_memory();
    let ctx = ToolContext::default();

    // 写入乱序键，列表结果应按字母排序
    tool.execute(json!({"action": "write", "key": "c", "content": "3"}), &ctx)
        .unwrap();
    tool.execute(json!({"action": "write", "key": "a", "content": "1"}), &ctx)
        .unwrap();
    tool.execute(json!({"action": "write", "key": "b", "content": "2"}), &ctx)
        .unwrap();

    let result = tool.execute(json!({"action": "list"}), &ctx).unwrap();
    // 验证 a 在 b 前面，b 在 c 前面
    let pos_a = result.find('a').unwrap();
    let pos_b = result.find('b').unwrap();
    let pos_c = result.find('c').unwrap();
    assert!(pos_a < pos_b);
    assert!(pos_b < pos_c);
}
