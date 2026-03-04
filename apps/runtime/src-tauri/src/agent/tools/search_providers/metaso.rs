/// 秘塔搜索（Metaso）Provider
///
/// 文档：https://metaso.cn/
/// 支持两种响应格式，解析时自动适配。
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::time::Instant;

use super::{SearchItem, SearchParams, SearchProvider, SearchResponse};

/// 秘塔搜索 Provider
pub struct MetasoSearch {
    /// API 基础 URL，默认 https://metaso.cn
    pub base_url: String,
    /// 秘塔搜索 API 密钥
    pub api_key: String,
}

impl MetasoSearch {
    /// 创建 MetasoSearch 实例
    ///
    /// - `base_url`：为空时使用默认地址，末尾 `/` 会自动去除
    /// - `api_key`：秘塔搜索 API 密钥
    pub fn new(base_url: &str, api_key: &str) -> Self {
        // 使用官方默认地址 https://metaso.cn
        let url = if base_url.is_empty() {
            "https://metaso.cn".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        Self {
            base_url: url,
            api_key: api_key.to_string(),
        }
    }
}

impl SearchProvider for MetasoSearch {
    fn name(&self) -> &str {
        "metaso"
    }

    fn display_name(&self) -> &str {
        "秘塔搜索"
    }

    fn search(&self, params: &SearchParams) -> Result<SearchResponse> {
        let start = Instant::now();

        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        // 使用官方 API 参数格式
        let body = json!({
            "q": params.query,
            "scope": "webpage",
            "includeSummary": false,
            "size": params.count.to_string(),
            "includeRawContent": false,
            "conciseSnippet": false
        });

        // 使用官方 API 端点：/api/v1/search
        let url = format!("{}/api/v1/search", self.base_url);

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
            "[metaso] 响应体: {}",
            serde_json::to_string_pretty(&resp_body).unwrap_or_else(|_| "无法序列化".to_string())
        );

        let items = parse_metaso_response(&resp_body);

        // 调试日志：打印解析结果数量
        eprintln!("[metaso] 解析到 {} 条搜索结果", items.len());

        Ok(SearchResponse {
            query: params.query.clone(),
            provider: "metaso".to_string(),
            items,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }
}

/// 从秘塔搜索 API 响应中提取搜索结果
///
/// 官方 API 响应格式：`{ webpages: [{ title, link, snippet, ... }] }`
fn parse_metaso_response(json: &Value) -> Vec<SearchItem> {
    // 官方格式：webpages 数组
    if let Some(webpages_arr) = json.get("webpages").and_then(|w| w.as_array()) {
        return webpages_arr
            .iter()
            .filter_map(|item| {
                let title = item.get("title")?.as_str()?.to_string();
                // 注意：秘塔使用 "link" 而不是 "url"
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
            .collect();
    }

    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_official_response() {
        // 官方 API 格式：webpages 数组，使用 link 和 snippet 字段
        let json = json!({
            "credits": 3,
            "total": 2,
            "webpages": [
                {
                    "title": "OpenClaw GitHub",
                    "link": "https://github.com/openclaw/openclaw",
                    "snippet": "Open-source AI agent platform",
                    "date": "2026-02-24",
                    "position": 1,
                    "score": "high"
                },
                {
                    "title": "秘塔搜索文档",
                    "link": "https://metaso.cn/docs",
                    "snippet": "API 接口文档说明"
                }
            ]
        });

        let items = parse_metaso_response(&json);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "OpenClaw GitHub");
        assert_eq!(items[0].url, "https://github.com/openclaw/openclaw");
        assert_eq!(items[0].snippet, "Open-source AI agent platform");
        assert_eq!(items[1].title, "秘塔搜索文档");
        assert_eq!(items[1].url, "https://metaso.cn/docs");
        assert_eq!(items[1].snippet, "API 接口文档说明");
    }

    #[test]
    fn test_parse_empty_webpages() {
        // webpages 为空数组
        let json = json!({ "webpages": [], "total": 0 });
        let items = parse_metaso_response(&json);
        assert_eq!(items.len(), 0, "空 webpages 应返回空列表");
    }

    #[test]
    fn test_parse_no_webpages_field() {
        // 响应中没有 webpages 字段
        let json = json!({ "status": "ok", "query": "rust" });
        let items = parse_metaso_response(&json);
        assert_eq!(items.len(), 0, "未知格式应返回空列表");
    }

    #[test]
    fn test_parse_missing_snippet() {
        // title 和 link 存在，但 snippet 缺失
        let json = json!({
            "webpages": [
                {
                    "title": "无摘要页面",
                    "link": "https://example.com"
                }
            ]
        });

        let items = parse_metaso_response(&json);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].snippet, "");
    }

    #[test]
    fn test_new_trims_trailing_slash() {
        let provider = MetasoSearch::new("https://metaso.cn/", "key");
        assert_eq!(provider.base_url, "https://metaso.cn");
    }

    #[test]
    fn test_new_uses_default_url() {
        let provider = MetasoSearch::new("", "key");
        assert_eq!(provider.base_url, "https://metaso.cn");
    }
}
