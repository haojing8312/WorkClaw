use runtime_lib::agent::{ReadFileTool, Tool, ToolContext, ToolRegistry};
use serde_json::json;
use std::fs;
use std::sync::Arc;

#[test]
fn test_read_file_success() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool));

    let tool = registry.get("read_file");
    assert!(tool.is_some(), "read_file tool should be registered");

    // Create test file
    let test_path = "test_read_file_success.txt";
    fs::write(test_path, "Hello, World!").unwrap();

    // Execute tool
    let input = json!({"path": test_path});
    let ctx = ToolContext::default();
    let result = tool.unwrap().execute(input, &ctx).unwrap();

    assert_eq!(result, "Hello, World!");

    // Cleanup
    fs::remove_file(test_path).unwrap();
}

#[test]
fn test_read_file_missing_path() {
    let tool = ReadFileTool;
    let ctx = ToolContext::default();
    let input = json!({});
    let result = tool.execute(input, &ctx);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("缺少 path 参数"));
}

#[test]
fn test_read_file_not_found() {
    let tool = ReadFileTool;
    let ctx = ToolContext::default();
    let input = json!({"path": "nonexistent_file_xyz.txt"});
    let result = tool.execute(input, &ctx);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("读取文件失败"));
}
