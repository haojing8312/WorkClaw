use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Local};
use serde_json::{json, Value};

/// 获取文件或目录的元信息（大小、类型、修改时间、只读状态）
pub struct FileStatTool;

impl Tool for FileStatTool {
    fn name(&self) -> &str {
        "file_stat"
    }

    fn description(&self) -> &str {
        "获取文件或目录的元信息，包括类型、大小、修改时间和只读状态。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要查询的文件或目录路径（相对或绝对）"
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;

        let checked = ctx.check_path(path)?;

        let metadata =
            std::fs::metadata(&checked).map_err(|e| anyhow!("获取文件元信息失败: {}", e))?;

        // 判断文件类型
        let file_type = if metadata.is_file() {
            "file"
        } else if metadata.is_dir() {
            "directory"
        } else {
            "symlink"
        };

        // 获取文件大小（字节）
        let size = metadata.len();

        // 获取修改时间并格式化
        let modified = metadata
            .modified()
            .map(|t| {
                let dt: DateTime<Local> = t.into();
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            })
            .unwrap_or_else(|_| "unknown".to_string());

        // 检查是否只读
        let readonly = metadata.permissions().readonly();

        let result = json!({
            "type": file_type,
            "size": size,
            "modified": modified,
            "readonly": readonly,
        });

        Ok(serde_json::to_string_pretty(&result)?)
    }
}
