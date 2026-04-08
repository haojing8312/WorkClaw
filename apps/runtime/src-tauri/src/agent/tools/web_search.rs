use crate::agent::tools::search_providers::{
    cache::SearchCache, SearchItem, SearchParams, SearchProvider,
};
use crate::agent::tool_manifest::{ToolCategory, ToolMetadata};
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use serde_json::{json, Value};
use std::sync::Arc;

/// 输出最大字符数，超出时截断
const MAX_OUTPUT_CHARS: usize = 30_000;

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedSearchRequest {
    query: String,
    freshness: Option<String>,
}

fn replace_all(query: &str, replacements: &[(&str, String)]) -> String {
    replacements
        .iter()
        .fold(query.to_string(), |acc, (needle, value)| acc.replace(needle, value))
}

fn start_of_week(reference_date: NaiveDate) -> NaiveDate {
    let days_from_monday = match reference_date.weekday() {
        Weekday::Mon => 0,
        Weekday::Tue => 1,
        Weekday::Wed => 2,
        Weekday::Thu => 3,
        Weekday::Fri => 4,
        Weekday::Sat => 5,
        Weekday::Sun => 6,
    };
    reference_date - Duration::days(days_from_monday)
}

fn normalize_relative_date_query(query: &str, reference_date: NaiveDate) -> NormalizedSearchRequest {
    let tomorrow = reference_date + Duration::days(1);
    let yesterday = reference_date - Duration::days(1);
    let week_start = start_of_week(reference_date);
    let week_end = week_start + Duration::days(6);
    let month_label = format!("{}年{}月", reference_date.year(), reference_date.month());
    let week_label = format!(
        "{} 至 {}",
        week_start.format("%Y-%m-%d"),
        week_end.format("%Y-%m-%d")
    );
    let replacements = [
        ("今天", reference_date.format("%Y-%m-%d").to_string()),
        ("明天", tomorrow.format("%Y-%m-%d").to_string()),
        ("昨天", yesterday.format("%Y-%m-%d").to_string()),
        ("昨日", yesterday.format("%Y-%m-%d").to_string()),
        ("这个月", month_label.clone()),
        ("本月", month_label.clone()),
        ("这周", week_label.clone()),
        ("本周", week_label.clone()),
    ];
    let normalized_query = replace_all(query, &replacements);

    let inferred_freshness = if query.contains("今天")
        || query.contains("昨天")
        || query.contains("昨日")
    {
        Some("day".to_string())
    } else if query.contains("这周") || query.contains("本周") {
        Some("week".to_string())
    } else if query.contains("这个月") || query.contains("本月") {
        Some("month".to_string())
    } else {
        None
    };

    NormalizedSearchRequest {
        query: normalized_query,
        freshness: inferred_freshness,
    }
}

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

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            category: ToolCategory::Search,
            read_only: true,
            concurrency_safe: true,
            open_world: true,
            ..ToolMetadata::default()
        }
    }

    fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let query = input["query"].as_str().ok_or(anyhow!("缺少 query 参数"))?;
        if query.trim().is_empty() {
            return Err(anyhow!("query 不能为空"));
        }

        let count = input["count"].as_i64().unwrap_or(5).clamp(1, 10) as usize;
        let normalized = normalize_relative_date_query(query, Local::now().date_naive());
        let query = normalized.query;
        let freshness = input["freshness"]
            .as_str()
            .map(String::from)
            .or(normalized.freshness);

        // 带时效性过滤的请求不使用缓存，确保结果实时性
        if freshness.is_none() {
            if let Some(cached_items) = self.cache.get(self.provider.name(), &query, count) {
                let output = format_results(&cached_items, self.provider.display_name());
                return Ok(truncate_output(&output));
            }
        }

        let params = SearchParams {
            query: query.clone(),
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
                .put(self.provider.name(), &query, count, response.items.clone());
        }

        let output = format_results(&response.items, self.provider.display_name());
        Ok(truncate_output(&output))
    }
}

/// 将搜索结果列表格式化为可读文本
fn format_results(items: &[SearchItem], provider_name: &str) -> String {
    let mut output = format!("已使用搜索引擎：{}\n[搜索结果 - 来自 {}]\n\n", provider_name, provider_name);
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
    use chrono::NaiveDate;
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

    #[test]
    fn test_normalize_relative_date_query_for_today() {
        let normalized = normalize_relative_date_query(
            "帮我搜一下今天的 AI 新闻，并给我一个简报",
            NaiveDate::from_ymd_opt(2026, 3, 20).expect("valid date"),
        );

        assert_eq!(normalized.freshness.as_deref(), Some("day"));
        assert!(normalized.query.contains("2026-03-20"));
        assert!(!normalized.query.contains("今天"));
    }

    #[test]
    fn test_normalize_relative_date_query_for_this_month() {
        let normalized = normalize_relative_date_query(
            "整理一下这个月的 AI 融资新闻",
            NaiveDate::from_ymd_opt(2026, 3, 20).expect("valid date"),
        );

        assert_eq!(normalized.freshness.as_deref(), Some("month"));
        assert!(normalized.query.contains("2026年3月"));
        assert!(!normalized.query.contains("这个月"));
    }
}
