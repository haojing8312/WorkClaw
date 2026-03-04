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
        "在文件中精确替换文本。查找 old_string 并替换为 new_string。默认要求 old_string 在文件中唯一。"
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
            return Err(anyhow!("未找到匹配文本: \"{}\"", old_string));
        }
        if !replace_all && count > 1 {
            return Err(anyhow!(
                "匹配不唯一（找到 {} 处），请提供更多上下文或使用 replace_all",
                count
            ));
        }

        let new_content = content.replace(old_string, new_string);
        fs::write(&checked, &new_content).map_err(|e| anyhow!("写入文件失败: {}", e))?;

        Ok(format!("成功替换 {} 处，文件: {}", count, path))
    }
}
