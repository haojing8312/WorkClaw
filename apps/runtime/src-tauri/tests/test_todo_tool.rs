use runtime_lib::agent::tools::TodoWriteTool;
use runtime_lib::agent::types::{Tool, ToolContext};
use serde_json::json;

#[test]
fn test_todo_replace_and_list() {
    let tool = TodoWriteTool::new();
    let ctx = ToolContext::default();

    let result = tool
        .execute(json!({
            "todos": [
                {"id": "t1", "content": "实现 Edit 工具", "status": "pending", "priority": "high"},
                {"id": "t2", "content": "补充测试", "status": "in_progress", "priority": "medium"}
            ]
        }), &ctx)
        .unwrap();
    assert!(result.contains("共 2 项"));
    assert!(result.contains("实现 Edit 工具"));
    assert!(result.contains("补充测试"));
    assert!(result.contains("pending"));
    assert!(result.contains("in_progress"));
}

#[test]
fn test_todo_update_status() {
    let tool = TodoWriteTool::new();
    let ctx = ToolContext::default();

    tool.execute(
        json!({
            "todos": [
                {"id": "task-1", "content": "Test task", "status": "pending", "priority": "medium"}
            ]
        }),
        &ctx,
    )
    .unwrap();

    let result = tool
        .execute(json!({
            "todos": [
                {"id": "task-1", "content": "Test task", "status": "in_progress", "priority": "medium"}
            ]
        }), &ctx)
        .unwrap();
    assert!(result.contains("in_progress"));
}

#[test]
fn test_todo_delete() {
    let tool = TodoWriteTool::new();
    let ctx = ToolContext::default();

    tool.execute(
        json!({
            "todos": [
                {"id": "task-del", "content": "Will delete", "status": "pending", "priority": "low"}
            ]
        }),
        &ctx,
    )
    .unwrap();

    let result = tool
        .execute(
            json!({
                "todos": []
            }),
            &ctx,
        )
        .unwrap();
    assert!(result.contains("已清空"));
}

#[test]
fn test_todo_missing_action() {
    let tool = TodoWriteTool::new();
    let ctx = ToolContext::default();
    let result = tool.execute(json!({}), &ctx);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("缺少 todos 数组参数"));
}

#[test]
fn test_todo_empty_list() {
    let tool = TodoWriteTool::new();
    let ctx = ToolContext::default();
    let result = tool.execute(json!({"todos": []}), &ctx).unwrap();
    assert!(result.contains("已清空"));
}

#[test]
fn test_todo_delete_nonexistent() {
    let tool = TodoWriteTool::new();
    let ctx = ToolContext::default();
    let result = tool.execute(json!({"todos": []}), &ctx).unwrap();
    assert!(result.contains("已清空"));
}
