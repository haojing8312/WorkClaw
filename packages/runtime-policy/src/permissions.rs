use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionRisk {
    Normal,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    Unrestricted,
}

impl PermissionMode {
    pub fn needs_confirmation(
        &self,
        tool_name: &str,
        input: &Value,
        work_dir: Option<&Path>,
    ) -> bool {
        match self {
            Self::Unrestricted => false,
            Self::AcceptEdits | Self::Default => {
                classify_action_risk(tool_name, input, work_dir) == ActionRisk::Critical
            }
        }
    }
}

impl Default for PermissionMode {
    fn default() -> Self {
        Self::AcceptEdits
    }
}

pub fn classify_action_risk(tool_name: &str, input: &Value, work_dir: Option<&Path>) -> ActionRisk {
    match normalize_tool_name(tool_name).as_str() {
        "file_delete" => ActionRisk::Critical,
        "write_file" => classify_write_risk(input, work_dir),
        "edit" => classify_edit_risk(input, work_dir),
        "bash" => classify_bash_risk(input),
        "browser_click" => classify_browser_click_risk(input),
        "browser_type" => classify_browser_type_risk(input),
        "browser_press_key" => classify_browser_press_risk(input),
        "browser_evaluate" => ActionRisk::Critical,
        "browser_act" => classify_browser_act_risk(input),
        "browser_navigate" | "browser_launch" | "browser_scroll" | "browser_hover"
        | "browser_screenshot" | "browser_get_dom" | "browser_wait_for" | "browser_go_back"
        | "browser_go_forward" | "browser_reload" | "browser_get_state" | "browser_snapshot"
        | "read_file" | "glob" | "grep" | "list_dir" | "file_stat" | "todo_write"
        | "web_search" | "web_fetch" => ActionRisk::Normal,
        _ => ActionRisk::Normal,
    }
}

fn classify_write_risk(input: &Value, work_dir: Option<&Path>) -> ActionRisk {
    let path = extract_path(input);
    let content = input
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if is_path_outside_work_dir(path, work_dir) || content.is_empty() || is_critical_path(path) {
        ActionRisk::Critical
    } else {
        ActionRisk::Normal
    }
}

fn classify_edit_risk(input: &Value, work_dir: Option<&Path>) -> ActionRisk {
    let path = extract_path(input);
    let new_string = input
        .get("new_string")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if is_path_outside_work_dir(path, work_dir) || new_string.is_empty() || is_critical_path(path) {
        ActionRisk::Critical
    } else {
        ActionRisk::Normal
    }
}

fn classify_bash_risk(input: &Value) -> ActionRisk {
    let command = input
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if [
        "rm -rf",
        "rm -r ",
        "del /f",
        "del /s",
        "rmdir /s",
        "remove-item -recurse",
        "remove-item -force",
        "git clean -fd",
        "git clean -fdx",
        "format ",
        "mkfs",
        "shutdown",
        "reboot",
    ]
    .iter()
    .any(|needle| command.contains(needle))
    {
        ActionRisk::Critical
    } else {
        ActionRisk::Normal
    }
}

fn classify_browser_click_risk(input: &Value) -> ActionRisk {
    let selector = input
        .get("selector")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if contains_browser_commit_keyword(selector) {
        ActionRisk::Critical
    } else {
        ActionRisk::Normal
    }
}

fn classify_browser_type_risk(input: &Value) -> ActionRisk {
    let selector = input
        .get("selector")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let text = input
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let submit = input
        .get("submit")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if submit || contains_browser_commit_keyword(selector) || contains_browser_commit_keyword(text)
    {
        ActionRisk::Critical
    } else {
        ActionRisk::Normal
    }
}

fn classify_browser_press_risk(input: &Value) -> ActionRisk {
    let key = input
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if key == "enter" {
        ActionRisk::Critical
    } else {
        ActionRisk::Normal
    }
}

fn classify_browser_act_risk(input: &Value) -> ActionRisk {
    let kind = input
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(kind.as_str(), "evaluate") {
        return ActionRisk::Critical;
    }
    if matches!(kind.as_str(), "type" | "fill")
        && input
            .get("submit")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return ActionRisk::Critical;
    }
    let risky_text = [
        input
            .get("selector")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        input.get("ref").and_then(Value::as_str).unwrap_or_default(),
        input
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        input.get("key").and_then(Value::as_str).unwrap_or_default(),
        input.get("fn").and_then(Value::as_str).unwrap_or_default(),
    ]
    .join(" ");
    if contains_browser_commit_keyword(&risky_text) {
        ActionRisk::Critical
    } else {
        ActionRisk::Normal
    }
}

fn extract_path(input: &Value) -> &str {
    input
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn normalize_for_scope_check(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return path.canonicalize().ok();
    }

    let existing_ancestor = path.ancestors().find(|ancestor| ancestor.exists())?;
    let mut normalized = existing_ancestor.canonicalize().ok()?;
    let remainder = path.strip_prefix(existing_ancestor).ok()?;

    for component in remainder.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
            Component::Prefix(_) | Component::RootDir => {}
        }
    }

    Some(normalized)
}

fn is_path_outside_work_dir(path: &str, work_dir: Option<&Path>) -> bool {
    let Some(work_dir) = work_dir else {
        return false;
    };
    if path.trim().is_empty() {
        return false;
    }
    let candidate = Path::new(path);
    if !candidate.is_absolute() {
        return false;
    }

    match (
        normalize_for_scope_check(candidate),
        normalize_for_scope_check(work_dir),
    ) {
        (Some(candidate), Some(work_dir)) => !candidate.starts_with(&work_dir),
        _ => !candidate.starts_with(work_dir),
    }
}

fn is_critical_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [
        ".env",
        "settings.json",
        "config.toml",
        "config.json",
        "secrets",
        "credential",
        "token",
        "key",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn contains_browser_commit_keyword(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "submit", "send", "publish", "delete", "remove", "confirm", "sync", "pay", "提交", "发送",
        "发布", "删除", "移除", "确认", "同步", "支付",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub fn normalize_tool_name(name: &str) -> String {
    let raw = name.trim().to_ascii_lowercase().replace('-', "_");
    match raw.as_str() {
        "readfile" => "read_file".to_string(),
        "writefile" => "write_file".to_string(),
        "listdir" => "list_dir".to_string(),
        "bashoutput" => "bash_output".to_string(),
        "bashkill" => "bash_kill".to_string(),
        "websearch" => "web_search".to_string(),
        "webfetch" => "web_fetch".to_string(),
        "todowrite" => "todo_write".to_string(),
        other => other.to_string(),
    }
}

pub fn narrow_allowed_tools(
    parent_allowed: Option<&[String]>,
    child_allowed: Option<&[String]>,
) -> Vec<String> {
    let parent_norm: Option<Vec<String>> = parent_allowed.map(|tools| {
        tools
            .iter()
            .map(|t| normalize_tool_name(t))
            .collect::<Vec<_>>()
    });

    let child_norm: Option<HashSet<String>> = child_allowed.map(|tools| {
        tools
            .iter()
            .map(|t| normalize_tool_name(t))
            .collect::<HashSet<_>>()
    });

    match (parent_norm, child_norm) {
        (Some(parent), Some(child)) => parent.into_iter().filter(|t| child.contains(t)).collect(),
        (Some(parent), None) => parent,
        (None, Some(child)) => child.into_iter().collect(),
        (None, None) => vec![],
    }
}
