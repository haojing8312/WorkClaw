use runtime_lib::agent::{PathAccessPolicy, Tool, ToolContext, WriteFileTool};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

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
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["tool"], "write_file");
    assert!(parsed["summary"].as_str().unwrap().contains("成功写入"));
    assert_eq!(parsed["details"]["path"], test_path);
    assert_eq!(parsed["details"]["bytes_written"], 12);

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
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["details"]["path"], test_path);
    assert_eq!(parsed["details"]["bytes_written"], 14);

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
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("缺少 content 参数")
    );
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
        path_access: PathAccessPolicy::WorkspaceOnly,
        allowed_tools: None,
        session_id: None,
        task_temp_dir: None,
        execution_caps: None,
        file_task_caps: None,
    };

    let input = json!({
        "path": target.to_str().unwrap(),
        "content": "# brief"
    });

    let result = tool.execute(input, &ctx).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["details"]["bytes_written"], 7);
    assert_eq!(fs::read_to_string(&target).unwrap(), "# brief");

    fs::remove_dir_all(&work_dir).unwrap();
}

#[test]
fn test_write_file_allows_absolute_path_outside_work_dir_in_full_access() {
    let tool = WriteFileTool;
    let work_dir = tempdir().expect("create work dir");
    let outside_dir = tempdir().expect("create outside dir");
    let target = outside_dir.path().join("artifact.md");
    let ctx = ToolContext {
        work_dir: Some(work_dir.path().to_path_buf()),
        path_access: PathAccessPolicy::FullAccessWithSensitiveGuards,
        allowed_tools: None,
        session_id: None,
        task_temp_dir: None,
        execution_caps: None,
        file_task_caps: None,
    };

    let input = json!({
        "path": target.to_str().unwrap(),
        "content": "# outside"
    });

    let result = tool.execute(input, &ctx).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["details"]["bytes_written"], 9);
    assert_eq!(fs::read_to_string(&target).unwrap(), "# outside");
}
