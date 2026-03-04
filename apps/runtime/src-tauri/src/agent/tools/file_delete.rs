use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

/// 安全删除文件或目录的工具
pub struct FileDeleteTool;

impl Tool for FileDeleteTool {
    fn name(&self) -> &str {
        "file_delete"
    }

    fn description(&self) -> &str {
        "删除文件或目录。对于非空目录，需要设置 recursive 为 true 才能删除。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要删除的文件或目录路径"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "是否递归删除非空目录，默认 false"
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;

        let recursive = input["recursive"].as_bool().unwrap_or(false);

        let checked = ctx.check_path(path)?;

        // 检查路径是否存在
        if !checked.exists() {
            return Err(anyhow!("路径不存在: {}", path));
        }

        if checked.is_file() {
            // 删除文件
            std::fs::remove_file(&checked).map_err(|e| anyhow!("删除文件失败: {}", e))?;
        } else if checked.is_dir() {
            // 尝试先删除空目录
            match std::fs::remove_dir(&checked) {
                Ok(_) => {} // 空目录删除成功
                Err(_) => {
                    // 目录非空，检查是否允许递归删除
                    if recursive {
                        std::fs::remove_dir_all(&checked)
                            .map_err(|e| anyhow!("递归删除目录失败: {}", e))?;
                    } else {
                        return Err(anyhow!(
                            "非空目录，请设置 recursive 为 true 以递归删除: {}",
                            path
                        ));
                    }
                }
            }
        } else {
            return Err(anyhow!("不支持的路径类型: {}", path));
        }

        Ok(format!("成功删除: {}", path))
    }
}
