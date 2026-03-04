use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

pub struct FileCopyTool;

/// 递归复制目录，返回复制的文件数量
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<usize> {
    fs::create_dir_all(dst).map_err(|e| anyhow!("创建目标目录失败: {}", e))?;

    let mut count = 0;
    for entry in fs::read_dir(src).map_err(|e| anyhow!("读取源目录失败: {}", e))? {
        let entry = entry.map_err(|e| anyhow!("读取目录条目失败: {}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            count += copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)
                .map_err(|e| anyhow!("复制文件 {} 失败: {}", src_path.display(), e))?;
            count += 1;
        }
    }
    Ok(count)
}

impl Tool for FileCopyTool {
    fn name(&self) -> &str {
        "file_copy"
    }

    fn description(&self) -> &str {
        "复制文件或目录到目标路径。支持递归复制目录。"
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
                    "description": "目标文件或目录路径"
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

        let src = ctx.check_path(source)?;
        let dst = ctx.check_path(destination)?;

        if !src.exists() {
            anyhow::bail!("源路径不存在: {}", source);
        }

        if src.is_dir() {
            let count = copy_dir_recursive(&src, &dst)?;
            Ok(format!(
                "已复制目录 {} → {}（{} 个文件）",
                source, destination, count
            ))
        } else {
            // 确保目标父目录存在
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).map_err(|e| anyhow!("创建目标父目录失败: {}", e))?;
            }
            fs::copy(&src, &dst).map_err(|e| anyhow!("复制文件失败: {}", e))?;
            Ok(format!("已复制文件 {} → {}", source, destination))
        }
    }
}
