/// 多 Provider 网络搜索模块
///
/// 提供统一的 SearchProvider trait，支持 Brave、Tavily、秘塔、博查、SerpApi 等搜索引擎。
pub mod bocha;
pub mod brave;
pub mod cache;
pub mod metaso;
pub mod serpapi;
pub mod tavily;

use anyhow::Result;

/// 搜索 Provider 统一 trait
pub trait SearchProvider: Send + Sync {
    /// Provider 内部标识符（小写，用于缓存键等）
    fn name(&self) -> &str;
    /// Provider 显示名称（用于日志、UI）
    fn display_name(&self) -> &str;
    /// 执行搜索，返回结构化结果
    fn search(&self, params: &SearchParams) -> Result<SearchResponse>;
}

/// 搜索请求参数
pub struct SearchParams {
    /// 搜索关键词
    pub query: String,
    /// 期望返回的结果数量
    pub count: usize,
    /// 时效性过滤（如 "day"、"week"、"month"），None 表示不限
    pub freshness: Option<String>,
}

/// 搜索响应
pub struct SearchResponse {
    /// 原始查询词
    pub query: String,
    /// 实际使用的 Provider 名称
    pub provider: String,
    /// 搜索结果列表
    pub items: Vec<SearchItem>,
    /// 请求耗时（毫秒）
    pub elapsed_ms: u64,
}

/// 单条搜索结果
#[derive(Clone, Debug)]
pub struct SearchItem {
    /// 结果标题
    pub title: String,
    /// 结果 URL
    pub url: String,
    /// 摘要文本
    pub snippet: String,
}

/// 根据 `api_format` 字段创建对应的 SearchProvider 实例
///
/// # 参数
/// - `api_format`：Provider 类型标识，如 `"search_brave"`
/// - `base_url`：API 基础 URL（部分 Provider 可为空，使用默认值）
/// - `api_key`：API 密钥
/// - `model_name`：部分 Provider 需要的附加参数（如 SerpApi 的搜索引擎类型）
pub fn create_provider(
    api_format: &str,
    base_url: &str,
    api_key: &str,
    model_name: &str,
) -> Result<Box<dyn SearchProvider>> {
    match api_format {
        "search_brave" => Ok(Box::new(brave::BraveSearch::new(base_url, api_key))),
        "search_tavily" => Ok(Box::new(tavily::TavilySearch::new(base_url, api_key))),
        "search_metaso" => Ok(Box::new(metaso::MetasoSearch::new(base_url, api_key))),
        "search_bocha" => Ok(Box::new(bocha::BochaSearch::new(base_url, api_key))),
        "search_serpapi" => Ok(Box::new(serpapi::SerpApiSearch::new(
            base_url, api_key, model_name,
        ))),
        _ => anyhow::bail!("未知的搜索 Provider: {}", api_format),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_provider_unknown() {
        let result = create_provider("search_unknown", "", "key", "");
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(
            err_msg.contains("未知的搜索 Provider"),
            "错误信息: {}",
            err_msg
        );
    }

    #[test]
    fn test_create_all_known_providers() {
        // 确认所有已知 Provider 可以正常实例化
        let formats = [
            "search_brave",
            "search_tavily",
            "search_metaso",
            "search_bocha",
            "search_serpapi",
        ];
        for fmt in &formats {
            let result = create_provider(fmt, "https://example.com", "test_key", "google");
            assert!(result.is_ok(), "Provider {} 创建失败", fmt);
        }
    }
}
