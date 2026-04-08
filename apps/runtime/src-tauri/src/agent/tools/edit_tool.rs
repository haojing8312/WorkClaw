use crate::agent::tool_manifest::{ToolCategory, ToolMetadata};
use crate::agent::tools::tool_result;
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;

pub struct EditTool;

impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "在文件中精确替换文本。查找 old_string 并替换为 new_string。默认要求 old_string 在文件中唯一。返回结构化结果，其中 details 包含替换次数和路径。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "文件路径" },
                "old_string": { "type": "string", "description": "要替换的原始文本" },
                "new_string": { "type": "string", "description": "替换后的文本" },
                "replace_all": { "type": "boolean", "description": "是否替换所有匹配（默认 false，要求唯一匹配）" }
            },
            "required": ["path", "old_string", "new_string"]
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
        let old_string = input["old_string"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 old_string 参数"))?;
        let new_string = input["new_string"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 new_string 参数"))?;
        let replace_all = input["replace_all"].as_bool().unwrap_or(false);

        let checked = ctx.check_path(path)?;
        let content = fs::read_to_string(&checked).map_err(|e| anyhow!("读取文件失败: {}", e))?;
        let count = content.matches(old_string).count();

        if count == 0 {
            return Err(anyhow!(
                "未找到匹配文本: \"{}\"，文件: {}",
                old_string,
                checked.display()
            ));
        }
        if !replace_all && count > 1 {
            return Err(anyhow!(
                "匹配不唯一（找到 {} 处），文件: {}。请提供更多上下文或使用 replace_all",
                count,
                checked.display()
            ));
        }

        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };
        fs::write(&checked, &new_content).map_err(|e| anyhow!("写入文件失败: {}", e))?;

        tool_result::success(
            self.name(),
            format!("成功替换 {} 处，文件: {}", count, path),
            json!({
                "path": path,
                "absolute_path": checked.to_string_lossy().to_string(),
                "replacements": count,
                "replace_all": replace_all,
                "old_string": old_string,
                "new_string": new_string,
            }),
        )
    }
}
