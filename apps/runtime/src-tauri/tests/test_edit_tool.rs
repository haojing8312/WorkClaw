use runtime_lib::agent::{EditTool, Tool, ToolContext, ToolRegistry};
use serde_json::json;
use std::fs;
use std::sync::Arc;

#[test]
fn test_edit_replace_single() {
    let tool = EditTool;
    let ctx = ToolContext::default();
    let path = "test_edit_single.txt";
    fs::write(path, "Hello, World!\nGoodbye, World!").unwrap();

    let input = json!({"path": path, "old_string": "Hello", "new_string": "Hi"});
    let result = tool.execute(input, &ctx).unwrap();
    assert!(result.contains("成功替换"));

    let content = fs::read_to_string(path).unwrap();
    assert_eq!(content, "Hi, World!\nGoodbye, World!");
    fs::remove_file(path).unwrap();
}

#[test]
fn test_edit_not_found() {
    let tool = EditTool;
    let ctx = ToolContext::default();
    let path = "test_edit_notfound.txt";
    fs::write(path, "Hello, World!").unwrap();

    let input = json!({"path": path, "old_string": "NONEXISTENT", "new_string": "replacement"});
    let result = tool.execute(input, &ctx);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("未找到"));
    fs::remove_file(path).unwrap();
}

#[test]
fn test_edit_not_unique() {
    let tool = EditTool;
    let ctx = ToolContext::default();
    let path = "test_edit_notunique.txt";
    fs::write(path, "aaa bbb aaa").unwrap();

    let input = json!({"path": path, "old_string": "aaa", "new_string": "ccc"});
    let result = tool.execute(input, &ctx);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("不唯一"));
    fs::remove_file(path).unwrap();
}

#[test]
fn test_edit_replace_all() {
    let tool = EditTool;
    let ctx = ToolContext::default();
    let path = "test_edit_replaceall.txt";
    fs::write(path, "aaa bbb aaa").unwrap();

    let input =
        json!({"path": path, "old_string": "aaa", "new_string": "ccc", "replace_all": true});
    let result = tool.execute(input, &ctx).unwrap();
    assert!(result.contains("2"));

    let content = fs::read_to_string(path).unwrap();
    assert_eq!(content, "ccc bbb ccc");
    fs::remove_file(path).unwrap();
}

#[test]
fn test_edit_missing_params() {
    let tool = EditTool;
    let ctx = ToolContext::default();
    let result = tool.execute(json!({}), &ctx);
    assert!(result.is_err());
}

#[test]
fn test_edit_registered() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(EditTool));
    assert!(registry.get("edit").is_some());
}
