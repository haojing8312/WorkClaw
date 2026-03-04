/// SerpApi 搜索 Provider
///
/// 文档：https://serpapi.com/search-api
/// 支持多种搜索引擎（google、bing、baidu 等），通过 engine 参数指定。
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::time::Instant;

use super::{SearchItem, SearchParams, SearchProvider, SearchResponse};

/// SerpApi 搜索 Provider
pub struct SerpApiSearch {
    /// API 基础 URL，默认 https://serpapi.com
    pub base_url: String,
    /// SerpApi API 密钥
    pub api_key: String,
    /// 搜索引擎类型（如 "google"、"bing"、"baidu"），空字符串时默认为 "google"
    pub engine: String,
}

impl SerpApiSearch {
    /// 创建 SerpApiSearch 实例
    ///
    /// - `base_url`：为空时使用默认地址，末尾 `/` 会自动去除
    /// - `api_key`：SerpApi API 密钥
    /// - `engine`：搜索引擎类型，为空时默认使用 "google"
    pub fn new(base_url: &str, api_key: &str, engine: &str) -> Self {
        let url = if base_url.is_empty() {
            "https://serpapi.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        let eng = if engine.is_empty() {
            "google".to_string()
        } else {
            engine.to_string()
        };
        Self {
            base_url: url,
            api_key: api_key.to_string(),
            engine: eng,
        }
    }
}

impl SearchProvider for SerpApiSearch {
    fn name(&self) -> &str {
        "serpapi"
    }

    fn display_name(&self) -> &str {
        "SerpApi"
    }

    fn search(&self, params: &SearchParams) -> Result<SearchResponse> {
        let start = Instant::now();

        // SerpApi 使用 GET 请求，API 密钥放在查询字符串中
        let encoded_query = urlencoding::encode(&params.query).into_owned();
        let url = format!(
            "{}/search.json?engine={}&q={}&num={}&api_key={}",
            self.base_url,
            urlencoding::encode(&self.engine),
            encoded_query,
            params.count,
            urlencoding::encode(&self.api_key)
        );

        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        let response = client.get(&url).send()?;

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
            "[serpapi] 响应体: {}",
            serde_json::to_string_pretty(&resp_body).unwrap_or_else(|_| "无法序列化".to_string())
        );

        let items = parse_serpapi_response(&resp_body);

        // 调试日志：打印解析结果数量
        eprintln!("[serpapi] 解析到 {} 条搜索结果", items.len());

        Ok(SearchResponse {
            query: params.query.clone(),
            provider: "serpapi".to_string(),
            items,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }
}

/// 从 SerpApi 响应中提取有机搜索结果
///
/// 响应结构：`{ organic_results: [{ title, link, snippet }] }`
fn parse_serpapi_response(json: &Value) -> Vec<SearchItem> {
    let results = json.get("organic_results").and_then(|r| r.as_array());

    match results {
        Some(arr) => arr
            .iter()
            .filter_map(|item| {
                let title = item.get("title")?.as_str()?.to_string();
                // SerpApi 使用 link 字段而非 url
                let url = item.get("link")?.as_str()?.to_string();
                let snippet = item
                    .get("snippet")
                    .and_then(|s| s.as_str())
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
        // 标准响应：包含 organic_results 数组
        let json = json!({
            "organic_results": [
                {
                    "title": "Rust 官网",
                    "link": "https://www.rust-lang.org",
                    "snippet": "Rust 系统编程语言"
                },
                {
                    "title": "Rust Book",
                    "link": "https://doc.rust-lang.org/book",
                    "snippet": "The Rust Programming Language 书籍"
                }
            ],
            "search_metadata": {
                "status": "Success"
            }
        });

        let items = parse_serpapi_response(&json);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Rust 官网");
        assert_eq!(items[0].url, "https://www.rust-lang.org");
        assert_eq!(items[0].snippet, "Rust 系统编程语言");
        assert_eq!(items[1].title, "Rust Book");
        assert_eq!(items[1].url, "https://doc.rust-lang.org/book");
    }

    #[test]
    fn test_parse_missing_snippet() {
        // snippet 字段缺失时应返回空字符串
        let json = json!({
            "organic_results": [
                {
                    "title": "无摘要页面",
                    "link": "https://example.com"
                }
            ]
        });

        let items = parse_serpapi_response(&json);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].snippet, "");
    }

    #[test]
    fn test_parse_empty_response() {
        // organic_results 为空数组
        let json = json!({ "organic_results": [] });
        let items = parse_serpapi_response(&json);
        assert_eq!(items.len(), 0, "空 organic_results 应返回空列表");
    }

    #[test]
    fn test_parse_no_organic_results_key() {
        // 缺少 organic_results 字段（如仅有 knowledge_graph）
        let json = json!({
            "knowledge_graph": { "title": "Rust" },
            "search_metadata": { "status": "Success" }
        });
        let items = parse_serpapi_response(&json);
        assert_eq!(items.len(), 0, "缺少 organic_results 字段时应返回空列表");
    }

    #[test]
    fn test_default_engine() {
        // engine 为空时应默认为 "google"
        let provider = SerpApiSearch::new("", "key", "");
        assert_eq!(provider.engine, "google");
    }

    #[test]
    fn test_custom_engine() {
        // 指定 engine 时应保留原值
        let provider = SerpApiSearch::new("", "key", "bing");
        assert_eq!(provider.engine, "bing");
    }

    #[test]
    fn test_new_trims_trailing_slash() {
        let provider = SerpApiSearch::new("https://serpapi.com/", "key", "google");
        assert_eq!(provider.base_url, "https://serpapi.com");
    }

    #[test]
    fn test_new_uses_default_url() {
        let provider = SerpApiSearch::new("", "key", "google");
        assert_eq!(provider.base_url, "https://serpapi.com");
    }
}
