use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Agent 工具执行权限模式
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PermissionMode {
    /// 默认：Write/Edit/Bash 需要用户确认
    Default,
    /// 接受编辑：Write/Edit 自动通过，Bash 仍需确认
    AcceptEdits,
    /// 无限制：所有工具自动通过
    Unrestricted,
}

impl PermissionMode {
    /// 判断指定工具是否需要用户确认
    pub fn needs_confirmation(&self, tool_name: &str) -> bool {
        // 浏览器高风险动作：可能触发提交、外发、状态变更等副作用
        let browser_risky = matches!(
            tool_name,
            "browser_act"
                | "browser_click"
                | "browser_type"
                | "browser_press_key"
                | "browser_evaluate"
                | "browser_launch"
                | "browser_navigate"
        );

        match self {
            Self::Unrestricted => false,
            Self::AcceptEdits => matches!(tool_name, "bash") || browser_risky,
            Self::Default => matches!(tool_name, "write_file" | "edit" | "bash") || browser_risky,
        }
    }
}

impl Default for PermissionMode {
    fn default() -> Self {
        Self::Default
    }
}

/// 规范化工具名（兼容 ReadFile/read_file/read-file/readfile 等写法）
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

/// 计算子 Skill 可用工具集合：parent_allowed ∩ child_allowed
///
/// - 当 child_allowed 为空（未声明）时，沿用 parent_allowed
/// - 输出按 parent_allowed 顺序稳定返回，避免 UI 抖动
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
        (Some(parent), Some(child)) => parent
            .into_iter()
            .filter(|t| child.contains(t))
            .collect(),
        (Some(parent), None) => parent,
        (None, Some(child)) => child.into_iter().collect(),
        (None, None) => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mode_is_default_variant() {
        // 确认 Default::default() 返回 Default 变体
        assert_eq!(PermissionMode::default(), PermissionMode::Default);
    }

    #[test]
    fn test_normalize_tool_name_aliases() {
        assert_eq!(normalize_tool_name("ReadFile"), "read_file");
        assert_eq!(normalize_tool_name("read-file"), "read_file");
        assert_eq!(normalize_tool_name("todoWrite"), "todo_write");
    }

    #[test]
    fn test_narrow_allowed_tools_intersection() {
        let parent = vec!["read_file".to_string(), "glob".to_string(), "bash".to_string()];
        let child = vec!["ReadFile".to_string(), "web_search".to_string()];
        let narrowed = narrow_allowed_tools(Some(&parent), Some(&child));
        assert_eq!(narrowed, vec!["read_file".to_string()]);
    }

    #[test]
    fn test_narrow_allowed_tools_child_undefined_inherits_parent() {
        let parent = vec!["read_file".to_string(), "glob".to_string()];
        let narrowed = narrow_allowed_tools(Some(&parent), None);
        assert_eq!(narrowed, parent);
    }

    #[test]
    fn test_permission_mode_browser_risky_tools_need_confirmation() {
        // Default 与 AcceptEdits 下浏览器高风险工具都应要求确认
        assert!(PermissionMode::Default.needs_confirmation("browser_act"));
        assert!(PermissionMode::Default.needs_confirmation("browser_click"));
        assert!(PermissionMode::AcceptEdits.needs_confirmation("browser_act"));
        assert!(PermissionMode::AcceptEdits.needs_confirmation("browser_evaluate"));
        // Unrestricted 模式不需要确认
        assert!(!PermissionMode::Unrestricted.needs_confirmation("browser_act"));
    }
}
