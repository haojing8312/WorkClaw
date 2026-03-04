/// Tavily Search API Provider
///
/// 文档：https://docs.tavily.com/docs/tavily-api/rest_api
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::time::Instant;

use super::{SearchItem, SearchParams, SearchProvider, SearchResponse};

/// Tavily Search Provider
pub struct TavilySearch {
    /// API 基础 URL，默认 https://api.tavily.com
    pub base_url: String,
    /// Tavily API 密钥
    pub api_key: String,
}

impl TavilySearch {
    /// 创建 TavilySearch 实例
    ///
    /// - `base_url`：为空时使用默认地址，末尾 `/` 会自动去除
    /// - `api_key`：Tavily API 密钥
    pub fn new(base_url: &str, api_key: &str) -> Self {
        let url = if base_url.is_empty() {
            "https://api.tavily.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        Self {
            base_url: url,
            api_key: api_key.to_string(),
        }
    }
}

impl SearchProvider for TavilySearch {
    fn name(&self) -> &str {
        "tavily"
    }

    fn display_name(&self) -> &str {
        "Tavily Search"
    }

    fn search(&self, params: &SearchParams) -> Result<SearchResponse> {
        let start = Instant::now();

        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        // POST 请求体
        let body = json!({
            "query": params.query,
            "max_results": params.count,
            "search_depth": "basic",
            "include_answer": false
        });

        let url = format!("{}/search", self.base_url);

        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()?;

        // 错误码映射
        let status = response.status();
        if !status.is_success() {
            let code = status.as_u16();
            return Err(anyhow!(
                "{}",
                match code {
                    401 | 403 => "搜索配置错误：API 密钥无效或权限不足".to_string(),
                    429 => "搜索频率超限：请稍后重试".to_string(),
                    500..=599 => "搜索服务暂不可用：服务器内部错误".to_string(),
                    _ => format!("搜索请求失败，HTTP 状态码: {}", code),
                }
            ));
        }

        let resp_body: Value = response.json()?;

        // 调试日志：打印实际响应格式
        eprintln!(
            "[tavily] 响应体: {}",
            serde_json::to_string_pretty(&resp_body).unwrap_or_else(|_| "无法序列化".to_string())
        );

        let items = parse_tavily_response(&resp_body);

        // 调试日志：打印解析结果数量
        eprintln!("[tavily] 解析到 {} 条搜索结果", items.len());

        Ok(SearchResponse {
            query: params.query.clone(),
            provider: "tavily".to_string(),
            items,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }
}

/// 从 Tavily Search API 响应中提取搜索结果
///
/// 响应结构：`{ results: [{ title, url, content }] }`
fn parse_tavily_response(json: &Value) -> Vec<SearchItem> {
    let results = json.get("results").and_then(|r| r.as_array());

    match results {
        Some(arr) => arr
            .iter()
            .filter_map(|item| {
                let title = item.get("title")?.as_str()?.to_string();
                let url = item.get("url")?.as_str()?.to_string();
                // Tavily 使用 content 字段作为摘要
                let snippet = item
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();
                Some(SearchItem {
                    title,
                    url,
                    snippet,
                })
            })
            .collect(),
        None => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_normal_response() {
        // 标准响应：包含 results 数组
        let json = json!({
            "results": [
                {
                    "title": "Tokio 异步运行时",
                    "url": "https://tokio.rs",
                    "content": "Tokio 是 Rust 的异步运行时库"
                },
                {
                    "title": "Axum Web 框架",
                    "url": "https://docs.rs/axum",
                    "content": "基于 Tokio 的 Web 框架"
                }
            ],
            "answer": null
        });

        let items = parse_tavily_response(&json);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Tokio 异步运行时");
        assert_eq!(items[0].url, "https://tokio.rs");
        assert_eq!(items[0].snippet, "Tokio 是 Rust 的异步运行时库");
        assert_eq!(items[1].title, "Axum Web 框架");
    }

    #[test]
    fn test_parse_missing_content() {
        // content 字段缺失时，snippet 应为空字符串
        let json = json!({
            "results": [
                {
                    "title": "无内容页面",
                    "url": "https://example.com"
                }
            ]
        });

        let items = parse_tavily_response(&json);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].snippet, "");
    }

    #[test]
    fn test_parse_empty_response() {
        // results 为空数组
        let json = json!({ "results": [] });
        let items = parse_tavily_response(&json);
        assert_eq!(items.len(), 0, "空 results 应返回空列表");
    }

    #[test]
    fn test_parse_no_results_key() {
        // 响应中没有 results 字段
        let json = json!({ "answer": "some answer" });
        let items = parse_tavily_response(&json);
        assert_eq!(items.len(), 0, "缺少 results 字段时应返回空列表");
    }

    #[test]
    fn test_new_trims_trailing_slash() {
        let provider = TavilySearch::new("https://api.tavily.com/", "key");
        assert_eq!(provider.base_url, "https://api.tavily.com");
    }

    #[test]
    fn test_new_uses_default_url() {
        let provider = TavilySearch::new("", "key");
        assert_eq!(provider.base_url, "https://api.tavily.com");
    }
}
