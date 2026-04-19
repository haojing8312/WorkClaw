use runtime_policy::{
    classify_action_risk, narrow_allowed_tools, normalize_tool_name, tool_permission_decision,
    ActionRisk, PermissionMode, ToolPermissionAction, ToolPermissionDecision,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

fn setup_work_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("runtime_policy_{}", name));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn default_mode_is_accept_edits_variant() {
    assert_eq!(PermissionMode::default(), PermissionMode::AcceptEdits);
}

#[test]
fn normalize_tool_name_aliases() {
    assert_eq!(normalize_tool_name("ReadFile"), "read_file");
    assert_eq!(normalize_tool_name("read-file"), "read_file");
    assert_eq!(normalize_tool_name("todoWrite"), "todo_write");
}

#[test]
fn narrow_allowed_tools_intersection() {
    let parent = vec![
        "read_file".to_string(),
        "glob".to_string(),
        "bash".to_string(),
    ];
    let child = vec!["ReadFile".to_string(), "web_search".to_string()];
    let narrowed = narrow_allowed_tools(Some(&parent), Some(&child));
    assert_eq!(narrowed, vec!["read_file".to_string()]);
}

#[test]
fn standard_mode_requires_confirmation_for_dangerous_commands() {
    let input = json!({
        "command": "rm -rf ./dist"
    });

    assert!(PermissionMode::AcceptEdits.needs_confirmation("bash", &input, None));
}

#[test]
fn standard_mode_allows_non_submit_browser_clicks() {
    let input = json!({
        "selector": ".menu-toggle"
    });

    assert!(!PermissionMode::AcceptEdits.needs_confirmation("browser_click", &input, None));
}

#[test]
fn classifier_marks_browser_submit_as_critical() {
    let input = json!({
        "kind": "click",
        "selector": ".publish-button"
    });

    assert_eq!(
        classify_action_risk("browser_act", &input, None),
        ActionRisk::Critical
    );
}

#[test]
fn out_of_workspace_write_is_critical() {
    let input = json!({
        "path": "C:\\Users\\alice\\Desktop\\main.ts",
        "content": "hello"
    });
    assert_eq!(
        classify_action_risk("write_file", &input, Some(Path::new("E:\\workspace\\proj"))),
        ActionRisk::Critical
    );
}

#[cfg(windows)]
#[test]
fn nested_absolute_write_inside_canonicalized_workspace_is_not_critical() {
    let work_dir = setup_work_dir("absolute_nested");
    let canonical_work_dir = work_dir.canonicalize().unwrap();
    let target = work_dir
        .join("公众号文章")
        .join("20251120-WorkClaw企业版介绍")
        .join("brief.md");
    let input = json!({
        "path": target.to_str().unwrap(),
        "content": "# brief"
    });

    assert_eq!(
        classify_action_risk("write_file", &input, Some(canonical_work_dir.as_path())),
        ActionRisk::Normal
    );

    fs::remove_dir_all(&work_dir).unwrap();
}

#[test]
fn decision_allows_read_only_file_access() {
    let decision = tool_permission_decision(
        PermissionMode::AcceptEdits,
        "read_file",
        &json!({
            "path": "README.md"
        }),
        None,
    );

    assert_eq!(decision.action, ToolPermissionAction::Allow);
    assert!(decision.reason.is_none());
    assert!(decision.fingerprint.is_none());
}

#[test]
fn decision_asks_for_destructive_bash() {
    let decision = tool_permission_decision(
        PermissionMode::AcceptEdits,
        "bash",
        &json!({
            "command": "rm -rf ./dist"
        }),
        None,
    );

    assert_eq!(decision.action, ToolPermissionAction::Ask);
    assert!(decision
        .reason
        .as_deref()
        .unwrap_or_default()
        .contains("bash"));
    assert!(decision.fingerprint.is_some());
}

#[test]
fn decision_asks_for_file_delete() {
    let decision = tool_permission_decision(
        PermissionMode::AcceptEdits,
        "file_delete",
        &json!({
            "path": "dist",
            "recursive": true
        }),
        None,
    );

    assert_eq!(decision.action, ToolPermissionAction::Ask);
}

#[test]
fn deny_decision_constructor_preserves_reason() {
    let decision = ToolPermissionDecision::deny("policy blocked");

    assert_eq!(decision.action, ToolPermissionAction::Deny);
    assert_eq!(decision.reason.as_deref(), Some("policy blocked"));
    assert!(decision.fingerprint.is_none());
}
