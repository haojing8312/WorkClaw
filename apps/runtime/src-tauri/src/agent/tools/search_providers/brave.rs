/// Brave Search API Provider
///
/// 文档：https://api.search.brave.com/app/documentation/web-search/get-started
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::time::Instant;

use super::{SearchItem, SearchParams, SearchProvider, SearchResponse};

/// Brave Search Provider
pub struct BraveSearch {
    /// API 基础 URL，默认 https://api.search.brave.com
    pub base_url: String,
    /// Brave Search API 密钥
    pub api_key: String,
}

impl BraveSearch {
    /// 创建 BraveSearch 实例
    ///
    /// - `base_url`：为空时使用默认地址，末尾 `/` 会自动去除
    /// - `api_key`：Brave API 密钥
    pub fn new(base_url: &str, api_key: &str) -> Self {
        let url = if base_url.is_empty() {
            "https://api.search.brave.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        Self {
            base_url: url,
            api_key: api_key.to_string(),
        }
    }
}

impl SearchProvider for BraveSearch {
    fn name(&self) -> &str {
        "brave"
    }

    fn display_name(&self) -> &str {
        "Brave Search"
    }

    fn search(&self, params: &SearchParams) -> Result<SearchResponse> {
        let start = Instant::now();

        // 构造查询 URL
        let encoded_query = urlencoding::encode(&params.query).into_owned();
        let mut url = format!(
            "{}/res/v1/web/search?q={}&count={}",
            self.base_url, encoded_query, params.count
        );

        // 可选时效性过滤参数
        if let Some(ref freshness) = params.freshness {
            url.push_str(&format!("&freshness={}", urlencoding::encode(freshness)));
        }

        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        let response = client
            .get(&url)
            .header("X-Subscription-Token", &self.api_key)
            .header("Accept", "application/json")
            .send()?;

        // 错误码映射为友好错误信息
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

        let body: Value = response.json()?;

        // 调试日志：打印实际响应格式
        eprintln!(
            "[brave] 响应体: {}",
            serde_json::to_string_pretty(&body).unwrap_or_else(|_| "无法序列化".to_string())
        );

        let items = parse_brave_response(&body);

        // 调试日志：打印解析结果数量
        eprintln!("[brave] 解析到 {} 条搜索结果", items.len());

        Ok(SearchResponse {
            query: params.query.clone(),
            provider: "brave".to_string(),
            items,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }
}

/// 从 Brave Search API 响应中提取搜索结果
///
/// 响应结构：`{ web: { results: [{ title, url, description }] } }`
fn parse_brave_response(json: &Value) -> Vec<SearchItem> {
    let results = json
        .get("web")
        .and_then(|w| w.get("results"))
        .and_then(|r| r.as_array());

    match results {
        Some(arr) => arr
            .iter()
            .filter_map(|item| {
                // title 和 url 为必要字段，缺少则跳过
                let title = item.get("title")?.as_str()?.to_string();
                let url = item.get("url")?.as_str()?.to_string();
                let snippet = item
                    .get("description")
                    .and_then(|d| d.as_str())
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
        // 标准响应：包含 web.results 数组
        let json = json!({
            "web": {
                "results": [
                    {
                        "title": "Rust 官方网站",
                        "url": "https://www.rust-lang.org",
                        "description": "Rust 是一门系统编程语言"
                    },
                    {
                        "title": "Rust 文档",
                        "url": "https://doc.rust-lang.org",
                        "description": "Rust 官方文档"
                    }
                ]
            }
        });

        let items = parse_brave_response(&json);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Rust 官方网站");
        assert_eq!(items[0].url, "https://www.rust-lang.org");
        assert_eq!(items[0].snippet, "Rust 是一门系统编程语言");
        assert_eq!(items[1].title, "Rust 文档");
    }

    #[test]
    fn test_parse_missing_description() {
        // description 字段缺失时，snippet 应为空字符串
        let json = json!({
            "web": {
                "results": [
                    {
                        "title": "无摘要页面",
                        "url": "https://example.com"
                    }
                ]
            }
        });

        let items = parse_brave_response(&json);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].snippet, "");
    }

    #[test]
    fn test_parse_empty_results() {
        // results 为空数组
        let json = json!({
            "web": {
                "results": []
            }
        });

        let items = parse_brave_response(&json);
        assert_eq!(items.len(), 0, "空 results 应返回空列表");
    }

    #[test]
    fn test_parse_no_web_key() {
        // 响应中没有 web 字段
        let json = json!({ "type": "search", "query": {} });
        let items = parse_brave_response(&json);
        assert_eq!(items.len(), 0, "缺少 web 字段时应返回空列表");
    }

    #[test]
    fn test_parse_no_results_key() {
        // web 存在但没有 results 字段
        let json = json!({ "web": { "count": 0 } });
        let items = parse_brave_response(&json);
        assert_eq!(items.len(), 0, "缺少 results 字段时应返回空列表");
    }

    #[test]
    fn test_new_trims_trailing_slash() {
        // 构造时应去除末尾 /
        let provider = BraveSearch::new("https://api.search.brave.com/", "key");
        assert_eq!(provider.base_url, "https://api.search.brave.com");
    }

    #[test]
    fn test_new_uses_default_url() {
        // base_url 为空时使用默认地址
        let provider = BraveSearch::new("", "key");
        assert_eq!(provider.base_url, "https://api.search.brave.com");
    }
}
