use crate::agent::file_task_preflight::preflight_file_task;
use crate::agent::types::{Tool, ToolContext};
use crate::agent::tools::tool_result;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

pub struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "读取文件内容。返回结构化结果，其中 details.content 包含完整文本。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要读取的文件路径（相对或绝对）"
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;

        let preflight = preflight_file_task(ctx, path)?;
        if matches!(preflight.read_mode.as_deref(), Some("binary_or_office")) {
            let absolute_path = preflight
                .resolved_path
                .as_ref()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_default();
            return tool_result::failure(
                self.name(),
                format!("不支持直接读取二进制或 Office 文件: {}", path),
                "UNSUPPORTED_RAW_FILE_READ",
                "unsupported raw-read for binary_or_office file",
                json!({
                    "path": path,
                    "absolute_path": absolute_path,
                    "read_mode": preflight.read_mode,
                    "reason": preflight.reason,
                    "truncated": false,
                }),
            );
        }

        let checked = preflight
            .resolved_path
            .clone()
            .ok_or_else(|| anyhow!("读取文件失败: 预检未返回规范化路径"))?;
        let content =
            std::fs::read_to_string(&checked).map_err(|e| anyhow!("读取文件失败: {}", e))?;
        let line_count = content.lines().count().max(1);

        tool_result::success(
            self.name(),
            format!("已读取文件 {}", path),
            json!({
                "path": path,
                "absolute_path": checked.to_string_lossy().to_string(),
                "content": content,
                "line_count": line_count,
                "truncated": false,
                "read_mode": preflight.read_mode,
            }),
        )
    }
}
