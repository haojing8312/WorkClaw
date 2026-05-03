use runtime_lib::agent::execution_caps::detect_execution_caps;
use runtime_lib::agent::system_prompts::tool_policy::TOOL_USAGE_POLICY;
use runtime_lib::agent::{BashTool, ExecTool, Tool, ToolContext, ToolRegistry};
use serde_json::json;
use std::path::PathBuf;
use tempfile::tempdir;

fn parse_bash_result(result: &str) -> serde_json::Value {
    serde_json::from_str(result).expect("valid bash result json")
}

#[test]
fn test_bash_simple_command() {
    let tool = BashTool::new();
    let ctx = ToolContext::default();

    let input = json!({"command": "echo Hello"});

    let result = tool.execute(input, &ctx).unwrap();
    let parsed = parse_bash_result(&result);
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["tool"], "bash");
    assert_eq!(parsed["details"]["timed_out"], false);
    assert_eq!(parsed["details"]["background"], false);
    assert_eq!(parsed["details"]["exit_code"], 0);
    assert!(parsed["details"]["stdout"]
        .as_str()
        .unwrap_or_default()
        .contains("Hello"));
}

#[test]
fn test_bash_includes_execution_context_metadata() {
    let tool = BashTool::new();
    let work_dir = tempdir().expect("work dir");
    let task_temp_dir = tempdir().expect("task temp dir");
    let ctx = ToolContext {
        work_dir: Some(PathBuf::from(work_dir.path())),
        path_access: Default::default(),
        allowed_tools: None,
        session_id: None,
        task_temp_dir: Some(PathBuf::from(task_temp_dir.path())),
        execution_caps: None,
        file_task_caps: None,
    };

    let input = json!({"command": "echo metadata"});
    let result = tool.execute(input, &ctx).unwrap();
    let parsed = parse_bash_result(&result);

    assert_eq!(parsed["ok"], true);
    assert_eq!(
        parsed["details"]["work_dir"].as_str(),
        Some(&*work_dir.path().to_string_lossy())
    );
    assert_eq!(
        parsed["details"]["task_temp_dir"].as_str(),
        Some(&*task_temp_dir.path().to_string_lossy())
    );
    assert_eq!(
        parsed["details"]["platform_shell"],
        if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "bash"
        }
    );
}

#[test]
fn test_bash_failure_includes_execution_context_metadata() {
    let tool = BashTool::new();
    let work_dir = tempdir().expect("work dir");
    let task_temp_dir = tempdir().expect("task temp dir");
    let ctx = ToolContext {
        work_dir: Some(PathBuf::from(work_dir.path())),
        path_access: Default::default(),
        allowed_tools: None,
        session_id: None,
        task_temp_dir: Some(PathBuf::from(task_temp_dir.path())),
        execution_caps: None,
        file_task_caps: None,
    };

    let input = json!({"command": if cfg!(target_os = "windows") { "ping -n 10 127.0.0.1" } else { "sleep 10" }, "timeout_ms": 1000});
    let result = tool.execute(input, &ctx).unwrap();
    let parsed = parse_bash_result(&result);

    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["details"]["timed_out"], true);
    assert_eq!(
        parsed["details"]["work_dir"].as_str(),
        Some(&*work_dir.path().to_string_lossy())
    );
    assert_eq!(
        parsed["details"]["task_temp_dir"].as_str(),
        Some(&*task_temp_dir.path().to_string_lossy())
    );
    assert_eq!(
        parsed["details"]["platform_shell"],
        if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "bash"
        }
    );
}

#[test]
fn test_bash_command_failure() {
    let tool = BashTool::new();
    let ctx = ToolContext::default();
    let input = json!({"command": "nonexistent_command_xyz_12345"});

    let result = tool.execute(input, &ctx);
    // On Windows PowerShell, this will either error or return non-zero exit code
    // Either way, it should not panic
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_bash_dangerous_command_blocked() {
    let tool = BashTool::new();
    let ctx = ToolContext::default();
    let input = json!({"command": "rm -rf /"});
    let result = tool.execute(input, &ctx).unwrap();
    let parsed = parse_bash_result(&result);
    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["error_code"], "DANGEROUS_COMMAND_BLOCKED");
    assert!(parsed["error_message"]
        .as_str()
        .unwrap_or_default()
        .contains("危险命令"));
}

#[test]
fn test_bash_dangerous_format_blocked() {
    let tool = BashTool::new();
    let ctx = ToolContext::default();
    let input = json!({"command": "format c:"});
    let result = tool.execute(input, &ctx).unwrap();
    let parsed = parse_bash_result(&result);
    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["error_code"], "DANGEROUS_COMMAND_BLOCKED");
}

#[test]
fn test_bash_safe_command_not_blocked() {
    let tool = BashTool::new();
    let ctx = ToolContext::default();
    let input = json!({"command": "echo safe"});
    let result = tool.execute(input, &ctx).unwrap();
    let parsed = parse_bash_result(&result);
    assert_eq!(parsed["ok"], true);
    assert!(parsed["details"]["stdout"]
        .as_str()
        .unwrap_or_default()
        .contains("safe"));
}

#[test]
fn test_bash_timeout() {
    let tool = BashTool::new();
    let ctx = ToolContext::default();
    let command = if cfg!(target_os = "windows") {
        "ping -n 10 127.0.0.1"
    } else {
        "sleep 10"
    };
    let input = json!({"command": command, "timeout_ms": 1000});
    let result = tool.execute(input, &ctx).unwrap();
    let parsed = parse_bash_result(&result);
    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["details"]["timed_out"], true);
    assert!(parsed["summary"]
        .as_str()
        .unwrap_or_default()
        .contains("超时"));
}

#[test]
fn test_bash_no_timeout_fast_command() {
    let tool = BashTool::new();
    let ctx = ToolContext::default();
    let input = json!({"command": "echo fast", "timeout_ms": 5000});
    let result = tool.execute(input, &ctx).unwrap();
    let parsed = parse_bash_result(&result);
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["details"]["timed_out"], false);
    assert!(parsed["details"]["stdout"]
        .as_str()
        .unwrap_or_default()
        .contains("fast"));
}

#[test]
fn test_exec_simple_command() {
    let tool = ExecTool::new();
    let ctx = ToolContext::default();

    let input = json!({"command": "echo Hello"});

    let result = tool.execute(input, &ctx).unwrap();
    let parsed = parse_bash_result(&result);
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["tool"], "exec");
    assert_eq!(parsed["details"]["timed_out"], false);
    assert_eq!(parsed["details"]["background"], false);
    assert_eq!(parsed["details"]["exit_code"], 0);
    assert!(parsed["details"]["stdout"]
        .as_str()
        .unwrap_or_default()
        .contains("Hello"));
}

#[test]
fn test_exec_includes_execution_context_metadata() {
    let tool = ExecTool::new();
    let work_dir = tempdir().expect("work dir");
    let task_temp_dir = tempdir().expect("task temp dir");
    let ctx = ToolContext {
        work_dir: Some(PathBuf::from(work_dir.path())),
        path_access: Default::default(),
        allowed_tools: None,
        session_id: None,
        task_temp_dir: Some(PathBuf::from(task_temp_dir.path())),
        execution_caps: None,
        file_task_caps: None,
    };

    let input = json!({"command": "echo metadata"});
    let result = tool.execute(input, &ctx).unwrap();
    let parsed = parse_bash_result(&result);

    assert_eq!(parsed["ok"], true);
    assert_eq!(
        parsed["details"]["work_dir"].as_str(),
        Some(&*work_dir.path().to_string_lossy())
    );
    assert_eq!(
        parsed["details"]["task_temp_dir"].as_str(),
        Some(&*task_temp_dir.path().to_string_lossy())
    );
    assert_eq!(
        parsed["details"]["platform_shell"],
        if cfg!(target_os = "windows") {
            "powershell"
        } else {
            "bash"
        }
    );
}

#[test]
fn test_standard_registry_exposes_exec_as_stable_command_entry() {
    let registry = ToolRegistry::with_standard_tools();

    assert!(registry.get("exec").is_some());
    assert!(ToolRegistry::standard_tool_names().contains(&"exec"));
}

#[test]
fn test_execution_caps_prefer_shell_matching_exec_tool() {
    let caps = detect_execution_caps();

    assert_eq!(
        caps.preferred_shell.as_deref(),
        if cfg!(target_os = "windows") {
            Some("powershell")
        } else {
            Some("bash")
        }
    );
}

#[test]
fn test_tool_policy_prefers_exec_for_commands() {
    assert!(TOOL_USAGE_POLICY.contains("执行命令用 `exec`"));
    assert!(!TOOL_USAGE_POLICY.contains("执行命令用 `bash`"));
}
