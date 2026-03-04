use crate::agent::types::{Tool, ToolContext};
use anyhow::Result;
use serde_json::{json, Value};

/// 手动上下文压缩工具
///
/// Agent 可主动调用此工具触发上下文压缩。
/// 实际压缩由 executor 在下一轮迭代检测标志后执行。
///
/// # 示例
///
/// ```rust
/// use runtime_lib::agent::tools::CompactTool;
/// use runtime_lib::agent::types::{Tool, ToolContext};
/// use serde_json::json;
///
/// let tool = CompactTool::new();
/// assert_eq!(tool.name(), "compact");
///
/// let ctx = ToolContext::default();
/// // 不带重点方向调用
/// let result = tool.execute(json!({}), &ctx).unwrap();
/// assert!(result.contains("下一轮迭代"));
///
/// // 带重点方向调用
/// let result = tool.execute(json!({"focus": "TypeScript 相关变更"}), &ctx).unwrap();
/// assert!(result.contains("TypeScript 相关变更"));
/// ```
pub struct CompactTool;

impl CompactTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CompactTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for CompactTool {
    fn name(&self) -> &str {
        "compact"
    }

    fn description(&self) -> &str {
        "手动触发对话上下文压缩。当对话过长时使用此工具来压缩历史消息，保留关键信息。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "focus": {
                    "type": "string",
                    "description": "压缩时的重点关注方向（可选，如 '重点保留 TypeScript 相关变更'）"
                }
            }
        })
    }

    fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let focus = input["focus"].as_str().unwrap_or("");
        if focus.is_empty() {
            Ok("已请求上下文压缩。将在下一轮迭代执行。".to_string())
        } else {
            Ok(format!(
                "已请求上下文压缩（重点: {}）。将在下一轮迭代执行。",
                focus
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_compact_tool_name_and_description() {
        let tool = CompactTool::new();
        assert_eq!(tool.name(), "compact");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_compact_tool_input_schema() {
        let tool = CompactTool::new();
        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["focus"].is_object());
    }

    #[test]
    fn test_execute_without_focus() {
        let tool = CompactTool::new();
        let result = tool.execute(json!({}), &ToolContext::default()).unwrap();
        assert!(result.contains("下一轮迭代"));
        assert!(!result.contains("重点"));
    }

    #[test]
    fn test_execute_with_focus() {
        let tool = CompactTool::new();
        let result = tool
            .execute(
                json!({"focus": "TypeScript 相关变更"}),
                &ToolContext::default(),
            )
            .unwrap();
        assert!(result.contains("TypeScript 相关变更"));
        assert!(result.contains("下一轮迭代"));
    }

    #[test]
    fn test_execute_with_empty_focus() {
        let tool = CompactTool::new();
        // 空字符串 focus 等同于未提供
        let result = tool
            .execute(json!({"focus": ""}), &ToolContext::default())
            .unwrap();
        assert!(!result.contains("重点"));
    }

    #[test]
    fn test_default_impl() {
        let tool = CompactTool::default();
        assert_eq!(tool.name(), "compact");
    }
}
