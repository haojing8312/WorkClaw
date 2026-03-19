use runtime_lib::agent::{ReadFileTool, Tool, ToolContext, ToolRegistry};
use serde_json::json;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

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

    let parsed: serde_json::Value = serde_json::from_str(&result).expect("valid json payload");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["tool"], "read_file");
    assert!(parsed["summary"].as_str().is_some_and(|value| !value.is_empty()));
    assert_eq!(parsed["details"]["path"], test_path);
    assert_eq!(parsed["details"]["content"], "Hello, World!");
    assert_eq!(parsed["details"]["line_count"], 1);
    assert_eq!(parsed["details"]["truncated"], false);
    assert!(
        parsed["details"]["absolute_path"]
            .as_str()
            .is_some_and(|value| Path::new(value).is_absolute() && value.ends_with(test_path))
    );

    // Cleanup
    fs::remove_file(test_path).unwrap();
}

#[test]
fn test_read_file_docx_returns_structured_failure() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool));

    let tool = registry.get("read_file");
    assert!(tool.is_some(), "read_file tool should be registered");

    let dir = tempdir().expect("create temp dir");
    let test_path = dir.path().join("report.docx");
    fs::write(&test_path, "fake docx payload").unwrap();

    let ctx = ToolContext {
        work_dir: Some(PathBuf::from(dir.path())),
        ..Default::default()
    };
    let input = json!({"path": test_path.to_str().expect("utf8 path")});

    let result = tool.unwrap().execute(input, &ctx).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).expect("valid json payload");

    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["tool"], "read_file");
    assert_eq!(parsed["error_code"], "UNSUPPORTED_RAW_FILE_READ");
    assert!(
        parsed["error_message"]
            .as_str()
            .is_some_and(|value| value.contains("raw-read"))
    );
    assert!(
        parsed["details"]["read_mode"]
            .as_str()
            .is_some_and(|value| value == "binary_or_office")
    );
    assert_eq!(parsed["details"]["path"], test_path.to_str().expect("utf8 path"));
    assert_eq!(
        parsed["details"]["absolute_path"],
        test_path.to_str().expect("utf8 path")
    );
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
