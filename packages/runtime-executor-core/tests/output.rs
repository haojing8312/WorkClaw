use runtime_executor_core::{
    split_error_code_and_message, truncate_tool_output, update_tool_failure_streak,
    ToolFailureStreak,
};
use serde_json::json;

#[test]
fn truncate_long_output() {
    let long_output = "x".repeat(40_000);
    let truncated = truncate_tool_output(&long_output, 30_000);
    assert!(truncated.len() < 31_100);
    assert!(truncated.contains("[输出已截断"));
    assert!(truncated.contains("40000"));
}

#[test]
fn truncate_structured_output_preserves_summary_and_shape() {
    let huge_details = "x".repeat(40_000);
    let structured = json!({
        "ok": true,
        "tool": "write_file",
        "summary": "成功写入 12 字节到 report.html",
        "data": {
            "path": "report.html",
            "content": huge_details.clone(),
        },
        "artifacts": [],
        "details": {
            "path": "report.html",
            "content": huge_details,
        }
    })
    .to_string();

    let truncated = truncate_tool_output(&structured, 30_000);
    let parsed: serde_json::Value = serde_json::from_str(&truncated).expect("structured json");
    assert_eq!(parsed["summary"], "成功写入 12 字节到 report.html");
    assert_eq!(parsed["details"]["truncated"], true);
}

#[test]
fn split_error_code_parses_prefixed_errors() {
    let (code, msg) = split_error_code_and_message("SKILL_NOT_FOUND: missing child");
    assert_eq!(code, "SKILL_NOT_FOUND");
    assert_eq!(msg, "missing child");
}

#[test]
fn split_error_code_parses_structured_errors() {
    let text = json!({
        "ok": false,
        "tool": "write_file",
        "summary": "写入失败",
        "data": {},
        "error": {
            "code": "MISSING_PATH",
            "message": "缺少 path 参数"
        },
        "artifacts": [],
        "error_code": "MISSING_PATH",
        "error_message": "缺少 path 参数",
        "details": {}
    })
    .to_string();

    let (code, msg) = split_error_code_and_message(&text);
    assert_eq!(code, "MISSING_PATH");
    assert_eq!(msg, "缺少 path 参数");
}

#[test]
fn repeated_failure_streak_trips_after_threshold() {
    let mut streak: Option<ToolFailureStreak> = None;
    let input = json!({"path": "a.txt"});

    assert!(update_tool_failure_streak(&mut streak, "write_file", &input, "boom").is_none());
    assert!(update_tool_failure_streak(&mut streak, "write_file", &input, "boom").is_none());
    let summary = update_tool_failure_streak(&mut streak, "write_file", &input, "boom");
    assert!(summary.is_some());
}

#[test]
fn repeated_failure_streak_normalizes_structured_errors() {
    let mut streak: Option<ToolFailureStreak> = None;
    let input = json!({"path": "a.txt"});
    let structured_error = json!({
        "ok": false,
        "tool": "write_file",
        "summary": "写入失败",
        "data": {},
        "error": {
            "code": "MISSING_PATH",
            "message": "缺少 path 参数"
        },
        "artifacts": [],
        "error_code": "MISSING_PATH",
        "error_message": "缺少 path 参数",
        "details": {}
    })
    .to_string();

    assert!(
        update_tool_failure_streak(&mut streak, "write_file", &input, &structured_error).is_none()
    );
    assert!(
        update_tool_failure_streak(&mut streak, "write_file", &input, &structured_error).is_none()
    );
    let summary = update_tool_failure_streak(&mut streak, "write_file", &input, &structured_error);
    assert!(summary.is_some());
    assert!(summary.unwrap().contains("缺少 path 参数"));
}

#[test]
fn structured_envelope_shape_exposes_data_error_and_artifacts() {
    let output = json!({
        "ok": false,
        "tool": "read_file",
        "summary": "读取失败",
        "data": {
            "path": "README.md"
        },
        "error": {
            "code": "NOT_FOUND",
            "message": "文件不存在"
        },
        "artifacts": [
            {
                "kind": "log",
                "path": "logs/read.txt"
            }
        ],
        "details": {
            "path": "README.md"
        },
        "error_code": "NOT_FOUND",
        "error_message": "文件不存在"
    })
    .to_string();

    let parsed: serde_json::Value = serde_json::from_str(&output).expect("structured json");
    assert_eq!(parsed["tool"], "read_file");
    assert_eq!(parsed["summary"], "读取失败");
    assert_eq!(parsed["data"]["path"], "README.md");
    assert_eq!(parsed["error"]["code"], "NOT_FOUND");
    assert_eq!(parsed["error"]["message"], "文件不存在");
    assert_eq!(parsed["artifacts"][0]["kind"], "log");
    assert_eq!(parsed["details"]["path"], "README.md");
    assert_eq!(parsed["error_code"], "NOT_FOUND");
    assert_eq!(parsed["error_message"], "文件不存在");
}
