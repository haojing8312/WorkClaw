use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

/// 文件/目录移动（重命名）工具
pub struct FileMoveTool;

impl Tool for FileMoveTool {
    fn name(&self) -> &str {
        "file_move"
    }

    fn description(&self) -> &str {
        "移动或重命名文件/目录。将 source 移动到 destination，如果目标父目录不存在会自动创建。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "源文件或目录路径"
                },
                "destination": {
                    "type": "string",
                    "description": "目标路径（新位置或新名称）"
                }
            },
            "required": ["source", "destination"]
        })
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let source = input["source"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 source 参数"))?;
        let destination = input["destination"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 destination 参数"))?;

        // 检查源路径和目标路径是否在工作目录范围内
        let src_path = ctx.check_path(source)?;
        let dst_path = ctx.check_path(destination)?;

        // 确认源文件/目录存在
        if !src_path.exists() {
            return Err(anyhow!("源路径不存在: {}", source));
        }

        // 确保目标的父目录存在
        if let Some(parent) = dst_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| anyhow!("创建目标父目录失败: {}", e))?;
        }

        // 执行移动/重命名
        std::fs::rename(&src_path, &dst_path).map_err(|e| anyhow!("移动失败: {}", e))?;

        Ok(format!("已移动 {} → {}", source, destination))
    }
}
