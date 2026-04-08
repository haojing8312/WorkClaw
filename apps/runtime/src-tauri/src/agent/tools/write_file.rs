use crate::agent::tool_manifest::{ToolCategory, ToolMetadata};
use crate::agent::tools::tool_result;
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

pub struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "写入内容到文件。如果文件不存在会创建，已存在会覆盖。返回结构化结果，其中 details 包含路径和写入字节数。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要写入的文件路径"
                },
                "content": {
                    "type": "string",
                    "description": "要写入的文本内容"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            category: ToolCategory::File,
            destructive: true,
            requires_approval: true,
            ..ToolMetadata::default()
        }
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;
        let content = input["content"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 content 参数"))?;

        let checked = ctx.check_path(path)?;

        // 确保父目录存在
        if let Some(parent) = checked.parent() {
            std::fs::create_dir_all(parent).map_err(|e| anyhow!("创建目录失败: {}", e))?;
        }

        std::fs::write(&checked, content).map_err(|e| anyhow!("写入文件失败: {}", e))?;

        tool_result::success(
            self.name(),
            format!("成功写入 {} 字节到 {}", content.len(), path),
            json!({
                "path": path,
                "absolute_path": checked.to_string_lossy().to_string(),
                "bytes_written": content.len(),
            }),
        )
    }
}
