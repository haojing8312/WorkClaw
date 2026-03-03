use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::agent::executor::truncate_tool_output;
use crate::agent::types::{Tool, ToolContext};

/// 获取指定 URL 的内容，自动清洗 HTML 标签
pub struct WebFetchTool;

impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "获取指定 URL 的网页内容，自动移除 HTML 标签并返回纯文本"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "要获取的 URL 地址"
                }
            },
            "required": ["url"]
        })
    }

    fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let url = input["url"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 url 参数"))?;

        // 构建带超时的 HTTP 客户端
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let resp = client
            .get(url)
            .header("User-Agent", "WorkClaw/1.0")
            .send()?;

        let status = resp.status();
        if !status.is_success() {
            return Err(anyhow!("HTTP 请求失败: {}", status));
        }

        let body = resp.text()?;
        // 清洗 HTML 标签，提取纯文本内容
        let cleaned = strip_html_tags(&body);

        Ok(truncate_tool_output(&cleaned, 30_000))
    }
}

/// 移除 HTML script/style 标签及所有 HTML 标签，压缩多余空行
///
/// 处理步骤：
/// 1. 移除 `<script>...</script>` 块
/// 2. 移除 `<style>...</style>` 块
/// 3. 移除所有剩余 HTML 标签
/// 4. 将连续三个及以上空行压缩为两个
pub fn strip_html_tags(html: &str) -> String {
    // 移除 script 标签及其内容（大小写不敏感，跨行匹配）
    let re_script = regex::Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
    // 移除 style 标签及其内容（大小写不敏感，跨行匹配）
    let re_style = regex::Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
    // 移除所有剩余 HTML 标签
    let re_tags = regex::Regex::new(r"<[^>]+>").unwrap();
    // 将三个及以上连续空行压缩为两个
    let re_lines = regex::Regex::new(r"\n{3,}").unwrap();

    let no_script = re_script.replace_all(html, "");
    let no_style = re_style.replace_all(&no_script, "");
    let text = re_tags.replace_all(&no_style, "");

    re_lines
        .replace_all(&text, "\n\n")
        .trim()
        .to_string()
}
