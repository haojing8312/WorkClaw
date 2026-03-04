use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::RwLock;
use uuid::Uuid;

/// 单个任务条目
#[derive(Clone, Debug)]
struct TodoItem {
    id: String,
    content: String,
    status: String,
    priority: String,
}

/// TodoWrite 工具：管理 Agent 执行过程中的任务列表
///
/// 采用 Claude Code 的批量写入模式：每次调用传入完整的 todos 数组，
/// 替换整个列表。这样创建 N 个任务只需 1 次工具调用，而非 N 次。
pub struct TodoWriteTool {
    items: RwLock<Vec<TodoItem>>,
}

impl TodoWriteTool {
    pub fn new() -> Self {
        Self {
            items: RwLock::new(Vec::new()),
        }
    }
}

impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "todo_write"
    }

    fn description(&self) -> &str {
        "管理任务列表。传入完整的 todos 数组，替换整个列表。每个任务包含 id、content、status、priority 字段。一次调用即可创建/更新多个任务。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "完整的任务列表，替换当前所有任务。一次传入所有任务，避免多次调用。",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {
                                "type": "string",
                                "description": "任务 ID，新任务可留空自动生成"
                            },
                            "content": {
                                "type": "string",
                                "description": "任务内容描述"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"],
                                "description": "任务状态"
                            },
                            "priority": {
                                "type": "string",
                                "enum": ["high", "medium", "low"],
                                "description": "优先级"
                            }
                        },
                        "required": ["content", "status"]
                    }
                }
            },
            "required": ["todos"]
        })
    }

    fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let todos = input["todos"]
            .as_array()
            .ok_or_else(|| anyhow!("缺少 todos 数组参数"))?;

        let new_items: Vec<TodoItem> = todos
            .iter()
            .map(|t| {
                let id = t["id"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                let content = t["content"].as_str().unwrap_or("(无内容)").to_string();
                let status = t["status"].as_str().unwrap_or("pending").to_string();
                let priority = t["priority"].as_str().unwrap_or("medium").to_string();
                TodoItem {
                    id,
                    content,
                    status,
                    priority,
                }
            })
            .collect();

        let count = new_items.len();
        *self.items.write().unwrap() = new_items;

        // 返回当前列表状态
        let items = self.items.read().unwrap();
        if items.is_empty() {
            return Ok("任务列表已清空".to_string());
        }
        let list: Vec<String> = items
            .iter()
            .map(|item| {
                format!(
                    "- [{}][{}] {} (ID: {})",
                    item.status, item.priority, item.content, item.id
                )
            })
            .collect();
        Ok(format!(
            "已更新任务列表（共 {} 项）:\n{}",
            count,
            list.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_create() {
        let tool = TodoWriteTool::new();

        // 一次创建多个任务
        let result = tool
            .execute(
                json!({
                    "todos": [
                        {"content": "任务一", "status": "pending", "priority": "high"},
                        {"content": "任务二", "status": "pending", "priority": "medium"},
                        {"content": "任务三", "status": "pending", "priority": "low"}
                    ]
                }),
                &ToolContext::default(),
            )
            .unwrap();
        assert!(result.contains("共 3 项"));
        assert!(result.contains("任务一"));
        assert!(result.contains("任务二"));
        assert!(result.contains("任务三"));
    }

    #[test]
    fn test_replace_list() {
        let tool = TodoWriteTool::new();

        // 创建初始列表
        tool.execute(
            json!({
                "todos": [
                    {"content": "旧任务", "status": "pending"}
                ]
            }),
            &ToolContext::default(),
        )
        .unwrap();

        // 替换为新列表
        let result = tool
            .execute(
                json!({
                    "todos": [
                        {"content": "新任务A", "status": "in_progress", "priority": "high"},
                        {"content": "新任务B", "status": "pending"}
                    ]
                }),
                &ToolContext::default(),
            )
            .unwrap();
        assert!(result.contains("共 2 项"));
        assert!(result.contains("新任务A"));
        assert!(result.contains("新任务B"));
        assert!(!result.contains("旧任务"));
    }

    #[test]
    fn test_update_status() {
        let tool = TodoWriteTool::new();

        // 创建任务
        let result = tool
            .execute(
                json!({
                    "todos": [
                        {"id": "task-1", "content": "测试任务", "status": "pending"}
                    ]
                }),
                &ToolContext::default(),
            )
            .unwrap();
        assert!(result.contains("pending"));

        // 更新状态（重新提交完整列表）
        let result = tool
            .execute(
                json!({
                    "todos": [
                        {"id": "task-1", "content": "测试任务", "status": "completed"}
                    ]
                }),
                &ToolContext::default(),
            )
            .unwrap();
        assert!(result.contains("completed"));
    }

    #[test]
    fn test_empty_list() {
        let tool = TodoWriteTool::new();

        // 传入空数组清空列表
        let result = tool
            .execute(json!({"todos": []}), &ToolContext::default())
            .unwrap();
        assert!(result.contains("已清空"));
    }

    #[test]
    fn test_auto_generate_id() {
        let tool = TodoWriteTool::new();

        // 不提供 id，应自动生成
        let result = tool
            .execute(
                json!({
                    "todos": [
                        {"content": "自动 ID 任务", "status": "pending"}
                    ]
                }),
                &ToolContext::default(),
            )
            .unwrap();
        assert!(result.contains("ID:"));
        assert!(result.contains("自动 ID 任务"));
    }

    #[test]
    fn test_missing_todos() {
        let tool = TodoWriteTool::new();
        let result = tool.execute(json!({}), &ToolContext::default());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("缺少 todos 数组参数"));
    }
}
