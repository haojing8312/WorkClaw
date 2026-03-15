use runtime_lib::agent::{Tool, ToolContext, WriteFileTool};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

fn setup_work_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("test_write_file_{}", name));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_write_file_success() {
    let tool = WriteFileTool;
    let ctx = ToolContext::default();
    let test_path = "test_write_output.txt";

    let input = json!({
        "path": test_path,
        "content": "Test content"
    });

    let result = tool.execute(input, &ctx).unwrap();
    assert!(result.contains("成功写入"));
    assert!(result.contains(test_path));

    // Verify file was written
    let content = fs::read_to_string(test_path).unwrap();
    assert_eq!(content, "Test content");

    // Cleanup
    fs::remove_file(test_path).unwrap();
}

#[test]
fn test_write_file_creates_parent_dirs() {
    let tool = WriteFileTool;
    let ctx = ToolContext::default();
    let test_path = "test_write_dir/nested/file.txt";

    let input = json!({
        "path": test_path,
        "content": "Nested content"
    });

    let result = tool.execute(input, &ctx).unwrap();
    assert!(result.contains("成功写入"));

    // Verify file was written
    let content = fs::read_to_string(test_path).unwrap();
    assert_eq!(content, "Nested content");

    // Cleanup
    fs::remove_dir_all("test_write_dir").unwrap();
}

#[test]
fn test_write_file_missing_params() {
    let tool = WriteFileTool;
    let ctx = ToolContext::default();

    let input = json!({"path": "test.txt"});
    let result = tool.execute(input, &ctx);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("缺少 content 参数"));
}

#[test]
fn test_write_file_allows_absolute_nested_path_within_work_dir() {
    let tool = WriteFileTool;
    let work_dir = setup_work_dir("absolute_nested");
    let target = work_dir
        .join("公众号文章")
        .join("20251120-WorkClaw企业版介绍")
        .join("brief.md");
    let ctx = ToolContext {
        work_dir: Some(work_dir.clone()),
        allowed_tools: None,
    };

    let input = json!({
        "path": target.to_str().unwrap(),
        "content": "# brief"
    });

    let result = tool.execute(input, &ctx).unwrap();
    assert!(result.contains("成功写入"));
    assert_eq!(fs::read_to_string(&target).unwrap(), "# brief");

    fs::remove_dir_all(&work_dir).unwrap();
}
