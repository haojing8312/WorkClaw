use runtime_policy::{
    classify_action_risk, narrow_allowed_tools, normalize_tool_name, ActionRisk, PermissionMode,
};
use serde_json::json;
use std::path::Path;

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
