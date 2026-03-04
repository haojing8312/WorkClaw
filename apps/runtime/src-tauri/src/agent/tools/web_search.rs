use crate::agent::tools::search_providers::{
    cache::SearchCache, SearchItem, SearchParams, SearchProvider,
};
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::Arc;

/// 输出最大字符数，超出时截断
const MAX_OUTPUT_CHARS: usize = 30_000;

/// Web 搜索工具 — 通过可插拔的 SearchProvider 执行搜索，支持结果缓存
pub struct WebSearchTool {
    /// 搜索 Provider 实例
    provider: Box<dyn SearchProvider>,
    /// 搜索结果缓存（Arc 共享，支持多工具实例复用同一缓存）
    cache: Arc<SearchCache>,
}

impl WebSearchTool {
    /// 使用指定的搜索 Provider 和缓存创建工具实例
    pub fn with_provider(provider: Box<dyn SearchProvider>, cache: Arc<SearchCache>) -> Self {
        Self { provider, cache }
    }
}

impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "搜索互联网获取最新信息。返回网页标题、URL 和摘要。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "搜索关键词"
                },
                "count": {
                    "type": "integer",
                    "description": "返回结果数量 (1-10，默认 5)",
                    "default": 5
                },
                "freshness": {
                    "type": "string",
                    "description": "结果新鲜度: day/week/month/year",
                    "enum": ["day", "week", "month", "year"]
                }
            },
            "required": ["query"]
        })
    }

    fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let query = input["query"].as_str().ok_or(anyhow!("缺少 query 参数"))?;
        if query.trim().is_empty() {
            return Err(anyhow!("query 不能为空"));
        }

        let count = input["count"].as_i64().unwrap_or(5).clamp(1, 10) as usize;
        let freshness = input["freshness"].as_str().map(String::from);

        // 带时效性过滤的请求不使用缓存，确保结果实时性
        if freshness.is_none() {
            if let Some(cached_items) = self.cache.get(self.provider.name(), query, count) {
                let output = format_results(&cached_items, self.provider.display_name());
                return Ok(truncate_output(&output));
            }
        }

        let params = SearchParams {
            query: query.to_string(),
            count,
            freshness,
        };
        let response = self.provider.search(&params)?;

        if response.items.is_empty() {
            return Ok("未找到搜索结果".to_string());
        }

        // 将结果写入缓存（freshness 请求跳过缓存写入）
        if params.freshness.is_none() {
            self.cache
                .put(self.provider.name(), query, count, response.items.clone());
        }

        let output = format_results(&response.items, self.provider.display_name());
        Ok(truncate_output(&output))
    }
}

/// 将搜索结果列表格式化为可读文本
fn format_results(items: &[SearchItem], provider_name: &str) -> String {
    let mut output = format!("[搜索结果 - 来自 {}]\n\n", provider_name);
    for (i, item) in items.iter().enumerate() {
        if item.snippet.is_empty() {
            output.push_str(&format!("{}. {}\n   {}\n\n", i + 1, item.title, item.url));
        } else {
            output.push_str(&format!(
                "{}. {}\n   {}\n   {}\n\n",
                i + 1,
                item.title,
                item.url,
                item.snippet
            ));
        }
    }
    output
}

/// 截断超长输出，防止超出 LLM 上下文限制
fn truncate_output(output: &str) -> String {
    if output.len() > MAX_OUTPUT_CHARS {
        // 按字节截断，添加截断提示
        let truncated = &output[..MAX_OUTPUT_CHARS];
        format!("{}\n\n... (结果已截断)", truncated)
    } else {
        output.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tools::search_providers::{
        SearchItem, SearchParams, SearchProvider, SearchResponse,
    };
    use std::time::Duration;

    /// 模拟搜索 Provider，始终返回固定结果
    struct MockProvider;

    impl SearchProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }
        fn display_name(&self) -> &str {
            "Mock Search"
        }
        fn search(&self, params: &SearchParams) -> anyhow::Result<SearchResponse> {
            Ok(SearchResponse {
                query: params.query.clone(),
                provider: "mock".to_string(),
                items: vec![SearchItem {
                    title: "Mock Result".to_string(),
                    url: "https://example.com".to_string(),
                    snippet: "A mock result".to_string(),
                }],
                elapsed_ms: 10,
            })
        }
    }

    #[test]
    fn test_web_search_with_provider() {
        let cache = Arc::new(SearchCache::new(Duration::from_secs(60), 10));
        let tool = WebSearchTool::with_provider(Box::new(MockProvider), cache);
        let result = tool.execute(json!({"query": "test"}), &ToolContext::default());
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Mock Result"));
        assert!(output.contains("https://example.com"));
        assert!(output.contains("[搜索结果 - 来自 Mock Search]"));
    }

    #[test]
    fn test_web_search_missing_query() {
        let cache = Arc::new(SearchCache::new(Duration::from_secs(60), 10));
        let tool = WebSearchTool::with_provider(Box::new(MockProvider), cache);
        let result = tool.execute(json!({}), &ToolContext::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_web_search_empty_query() {
        let cache = Arc::new(SearchCache::new(Duration::from_secs(60), 10));
        let tool = WebSearchTool::with_provider(Box::new(MockProvider), cache);
        let result = tool.execute(json!({"query": "  "}), &ToolContext::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_web_search_uses_cache() {
        let cache = Arc::new(SearchCache::new(Duration::from_secs(60), 10));
        let tool = WebSearchTool::with_provider(Box::new(MockProvider), cache.clone());
        // 第一次调用触发实际搜索并写入缓存
        let _ = tool.execute(json!({"query": "cached test"}), &ToolContext::default());
        // 缓存中应存在对应条目
        assert!(cache.get("mock", "cached test", 5).is_some());
    }

    #[test]
    fn test_web_search_freshness_param() {
        let cache = Arc::new(SearchCache::new(Duration::from_secs(60), 10));
        let tool = WebSearchTool::with_provider(Box::new(MockProvider), cache);
        let result = tool.execute(
            json!({"query": "news", "freshness": "day"}),
            &ToolContext::default(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_web_search_result_truncation() {
        /// 返回超长摘要的 Provider，用于测试截断逻辑
        struct LongProvider;
        impl SearchProvider for LongProvider {
            fn name(&self) -> &str {
                "long"
            }
            fn display_name(&self) -> &str {
                "Long"
            }
            fn search(&self, params: &SearchParams) -> anyhow::Result<SearchResponse> {
                Ok(SearchResponse {
                    query: params.query.clone(),
                    provider: "long".to_string(),
                    items: vec![SearchItem {
                        title: "Long".to_string(),
                        url: "https://example.com".to_string(),
                        // 生成远超 MAX_OUTPUT_CHARS 的摘要
                        snippet: "x".repeat(40_000),
                    }],
                    elapsed_ms: 1,
                })
            }
        }

        let cache = Arc::new(SearchCache::new(Duration::from_secs(60), 10));
        let tool = WebSearchTool::with_provider(Box::new(LongProvider), cache);
        let result = tool
            .execute(json!({"query": "long"}), &ToolContext::default())
            .unwrap();
        // 输出长度不应超过截断上限加上截断提示的少量字节
        assert!(result.len() <= 30_100);
        assert!(result.contains("... (结果已截断)"));
    }
}
