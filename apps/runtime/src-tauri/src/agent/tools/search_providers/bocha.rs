/// 博查（Bocha）搜索 Provider
///
/// 文档：https://open.bochaai.com/
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::time::Instant;

use super::{SearchItem, SearchParams, SearchProvider, SearchResponse};

/// 博查搜索 Provider
pub struct BochaSearch {
    /// API 基础 URL，默认 https://api.bochaai.com
    pub base_url: String,
    /// 博查搜索 API 密钥
    pub api_key: String,
}

impl BochaSearch {
    /// 创建 BochaSearch 实例
    ///
    /// - `base_url`：为空时使用默认地址，末尾 `/` 会自动去除
    /// - `api_key`：博查搜索 API 密钥
    pub fn new(base_url: &str, api_key: &str) -> Self {
        let url = if base_url.is_empty() {
            "https://api.bochaai.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        Self {
            base_url: url,
            api_key: api_key.to_string(),
        }
    }
}

impl SearchProvider for BochaSearch {
    fn name(&self) -> &str {
        "bocha"
    }

    fn display_name(&self) -> &str {
        "博查搜索"
    }

    fn search(&self, params: &SearchParams) -> Result<SearchResponse> {
        let start = Instant::now();

        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        // 构造请求体，可选时效性过滤
        let mut body = json!({
            "query": params.query,
            "count": params.count
        });

        if let Some(ref freshness) = params.freshness {
            body["freshness"] = json!(freshness);
        }

        let url = format!("{}/v1/web-search", self.base_url);

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
            "[bocha] 响应体: {}",
            serde_json::to_string_pretty(&resp_body).unwrap_or_else(|_| "无法序列化".to_string())
        );

        let items = parse_bocha_response(&resp_body);

        // 调试日志：打印解析结果数量
        eprintln!("[bocha] 解析到 {} 条搜索结果", items.len());

        Ok(SearchResponse {
            query: params.query.clone(),
            provider: "bocha".to_string(),
            items,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }
}

/// 从博查搜索 API 响应中提取搜索结果
///
/// 响应结构：`{ data: { webPages: { value: [{ name, url, snippet }] } } }`
fn parse_bocha_response(json: &Value) -> Vec<SearchItem> {
    let value_arr = json
        .get("data")
        .and_then(|d| d.get("webPages"))
        .and_then(|w| w.get("value"))
        .and_then(|v| v.as_array());

    match value_arr {
        Some(arr) => arr
            .iter()
            .filter_map(|item| {
                // 博查使用 name 作为标题字段
                let title = item.get("name")?.as_str()?.to_string();
                let url = item.get("url")?.as_str()?.to_string();
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
        // 标准响应：包含 data.webPages.value 数组
        let json = json!({
            "data": {
                "webPages": {
                    "value": [
                        {
                            "name": "Rust 编程语言",
                            "url": "https://www.rust-lang.org",
                            "snippet": "Rust 是一门注重安全和性能的系统编程语言"
                        },
                        {
                            "name": "crates.io",
                            "url": "https://crates.io",
                            "snippet": "Rust 包注册中心"
                        }
                    ]
                }
            }
        });

        let items = parse_bocha_response(&json);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Rust 编程语言");
        assert_eq!(items[0].url, "https://www.rust-lang.org");
        assert_eq!(items[0].snippet, "Rust 是一门注重安全和性能的系统编程语言");
        assert_eq!(items[1].title, "crates.io");
    }

    #[test]
    fn test_parse_missing_snippet() {
        // snippet 字段缺失时应返回空字符串
        let json = json!({
            "data": {
                "webPages": {
                    "value": [
                        {
                            "name": "无摘要页面",
                            "url": "https://example.com"
                        }
                    ]
                }
            }
        });

        let items = parse_bocha_response(&json);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].snippet, "");
    }

    #[test]
    fn test_parse_empty_response() {
        // value 为空数组
        let json = json!({
            "data": {
                "webPages": {
                    "value": []
                }
            }
        });

        let items = parse_bocha_response(&json);
        assert_eq!(items.len(), 0, "空 value 数组应返回空列表");
    }

    #[test]
    fn test_parse_no_data_key() {
        // 缺少 data 字段
        let json = json!({ "status": "ok" });
        let items = parse_bocha_response(&json);
        assert_eq!(items.len(), 0, "缺少 data 字段时应返回空列表");
    }

    #[test]
    fn test_parse_no_web_pages_key() {
        // data 存在但缺少 webPages
        let json = json!({ "data": { "count": 0 } });
        let items = parse_bocha_response(&json);
        assert_eq!(items.len(), 0, "缺少 webPages 字段时应返回空列表");
    }

    #[test]
    fn test_new_trims_trailing_slash() {
        let provider = BochaSearch::new("https://api.bochaai.com/", "key");
        assert_eq!(provider.base_url, "https://api.bochaai.com");
    }

    #[test]
    fn test_new_uses_default_url() {
        let provider = BochaSearch::new("", "key");
        assert_eq!(provider.base_url, "https://api.bochaai.com");
    }
}
