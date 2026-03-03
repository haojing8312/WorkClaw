# 搜索引擎 API 集成实现指南

本文档提供了在 WorkClaw 中集成各搜索引擎 API 的实现指南和最佳实践。

## 快速参考表

### API 能力对比

| 能力 | Brave | Tavily | Bocha | SerpAPI |
|------|-------|--------|-------|---------|
| **基础网页搜索** | ✅ | ✅ | ✅ | ✅ |
| **新闻搜索** | ✅ | ❌ | ✅ | ✅ |
| **视频搜索** | ✅ | ❌ | ❌ | ✅ |
| **AI 生成摘要** | ✅ | ✅ | ✅ | ❌ |
| **相关性评分** | ❌ | ✅ | ❌ | ❌ |
| **原始 HTML** | ❌ | ✅ | ❌ | ❌ |
| **图片结果** | ✅ | ✅ | ✅ | ✅ |
| **本地结果** | ✅ | ❌ | ❌ | ✅ |
| **响应时间** | 快 | 快 | 最快(<1s) | 中等 |
| **国内可用** | ✅ | ✅ | ✅✅ | ❌ |

## 1. Brave Search API 集成

### 初始化

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct BraveSearchRequest {
    q: String,
    count: Option<u32>,
    offset: Option<u32>,
    result_filter: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BraveSearchResponse {
    web: Option<WebResults>,
    news: Option<NewsResults>,
    videos: Option<VideoResults>,
    query: QueryInfo,
}

#[derive(Debug, Deserialize)]
struct WebResults {
    results: Vec<WebResult>,
}

#[derive(Debug, Deserialize)]
struct WebResult {
    title: String,
    url: String,
    description: String,
    #[serde(default)]
    extra_snippets: Vec<String>,
    #[serde(default)]
    profile: Option<Profile>,
}

#[derive(Debug, Deserialize)]
struct Profile {
    name: Option<String>,
    url: Option<String>,
    long_name: Option<String>,
    img: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QueryInfo {
    original: String,
    #[serde(default)]
    more_results_available: bool,
}

pub struct BraveSearchProvider {
    client: Client,
    api_key: String,
}

impl BraveSearchProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    pub async fn search(&self, query: &str, count: u32) -> Result<Vec<SearchResult>> {
        let url = "https://api.search.brave.com/res/v1/web/search";

        let response = self.client
            .get(url)
            .query(&[("q", query), ("count", &count.to_string())])
            .header("Authorization", format!("Token {}", self.api_key))
            .send()
            .await?;

        let brave_response: BraveSearchResponse = response.json().await?;

        Ok(brave_response
            .web
            .unwrap_or_default()
            .results
            .into_iter()
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.description,
                source: r.profile.as_ref().and_then(|p| p.name.clone()),
                favicon: r.profile.and_then(|p| p.img),
                timestamp: None,
                score: None,
                raw: None,
            })
            .collect())
    }
}
```

### 字段说明

| 参数 | 必需 | 说明 | 默认值 |
|------|------|------|--------|
| `q` | ✅ | 搜索查询 | - |
| `count` | ❌ | 返回结果数（1-20） | 10 |
| `offset` | ❌ | 结果偏移量 | 0 |
| `result_filter` | ❌ | 结果类型过滤 | - |
| `text_decorations` | ❌ | 启用文本装饰 | false |
| `spellcheck` | ❌ | 拼写检查 | true |
| `goggles_id` | ❌ | Goggles ID | - |

### 错误处理

```rust
// Brave 的所有错误都通过 HTTP 状态码表示
match response.status() {
    StatusCode::OK => Ok(data),
    StatusCode::UNAUTHORIZED => Err("Invalid API key"),
    StatusCode::FORBIDDEN => Err("Plan limit exceeded"),
    StatusCode::NOT_FOUND => Err("Endpoint not found"),
    StatusCode::TOO_MANY_REQUESTS => Err("Rate limited"),
    _ => Err("Unknown error"),
}
```

---

## 2. Tavily API 集成

### 初始化

```rust
#[derive(Debug, Serialize)]
struct TavilySearchRequest {
    api_key: String,
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_images: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_raw_content: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_answer: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    search_depth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TavilySearchResponse {
    pub query: String,
    pub answer: Option<String>,
    pub images: Vec<String>,
    pub results: Vec<TavilyResult>,
    pub response_time: f32,
    pub usage: Usage,
    pub request_id: String,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
    pub title: String,
    pub url: String,
    pub content: String,
    #[serde(default)]
    pub raw_content: Option<String>,
    pub score: f32,
    #[serde(default)]
    pub favicon: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    credits: u32,
}

pub struct TavilySearchProvider {
    client: Client,
    api_key: String,
}

impl TavilySearchProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    pub async fn search(&self, query: &str, max_results: u32) -> Result<Vec<SearchResult>> {
        let url = "https://api.tavily.com/search";

        let request = TavilySearchRequest {
            api_key: self.api_key.clone(),
            query: query.to_string(),
            include_images: Some(true),
            include_raw_content: Some(false),
            include_answer: Some(true),
            max_results: Some(max_results),
            search_depth: Some("advanced".to_string()),
            topic: Some("general".to_string()),
        };

        let response = self.client
            .post(url)
            .header("Authorization", format!("Bearer tvly-{}", self.api_key))
            .json(&request)
            .send()
            .await?;

        let tavily_response: TavilySearchResponse = response.json().await?;

        // 按 score 排序（高到低）
        let mut results: Vec<SearchResult> = tavily_response
            .results
            .into_iter()
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.content,
                source: None,
                favicon: r.favicon,
                timestamp: None,
                score: Some(r.score),
                raw: None,
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        Ok(results)
    }
}
```

### 字段说明

| 参数 | 必需 | 说明 | 默认值 |
|------|------|------|--------|
| `api_key` | ✅ | API 密钥 | - |
| `query` | ✅ | 搜索查询 | - |
| `include_images` | ❌ | 包含图片 | false |
| `include_raw_content` | ❌ | 包含原始HTML | false |
| `include_answer` | ❌ | 包含AI回答 | true |
| `max_results` | ❌ | 最大结果数 | 10 |
| `search_depth` | ❌ | "basic" 或 "advanced" | "basic" |
| `topic` | ❌ | "general" 或 "news" | "general" |

### 优势

- **相关性评分**：每个结果都有 0-1 的 score，便于排序
- **AI 摘要**：`answer` 字段提供直接回答
- **原始内容**：可选的 `raw_content` 用于深度处理
- **图片结果**：额外的图片搜索

### 错误处理

```rust
// Tavily 错误通过 request_id 追踪
if response.status().is_success() {
    let data = response.json::<TavilySearchResponse>().await?;
    eprintln!("Request ID for debugging: {}", data.request_id);
    Ok(data.results)
} else {
    match response.status().as_u16() {
        400 => Err("Invalid request"),
        401 => Err("Invalid API key"),
        429 => Err("Rate limited - too many requests"),
        _ => Err("Server error"),
    }
}
```

---

## 3. 博查搜索 (Bocha) API 集成

### 初始化

```rust
#[derive(Debug, Serialize)]
struct BochaSearchRequest {
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    freshness: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclude_domains: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct BochaSearchResponse {
    #[serde(rename = "_type")]
    _type: String,
    #[serde(rename = "queryContext")]
    query_context: QueryContext,
    #[serde(rename = "webPages")]
    web_pages: WebPages,
}

#[derive(Debug, Deserialize)]
struct QueryContext {
    #[serde(rename = "originalQuery")]
    original_query: String,
}

#[derive(Debug, Deserialize)]
struct WebPages {
    #[serde(rename = "webSearchUrl")]
    web_search_url: String,
    #[serde(rename = "totalEstimatedMatches")]
    total_estimated_matches: u64,
    value: Vec<BochaResult>,
}

#[derive(Debug, Deserialize)]
struct BochaResult {
    id: String,
    name: String,
    url: String,
    #[serde(rename = "siteName")]
    site_name: String,
    #[serde(rename = "siteIcon")]
    site_icon: String,
    snippet: String,
    summary: String,
    #[serde(rename = "datePublished")]
    date_published: String,
}

pub struct BochaSearchProvider {
    client: Client,
    api_key: String,
}

impl BochaSearchProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    pub async fn search(&self, query: &str, count: u32) -> Result<Vec<SearchResult>> {
        let url = "https://api.bochaai.com/v1/web-search";

        let request = BochaSearchRequest {
            query: query.to_string(),
            freshness: Some("oneMonth".to_string()),
            summary: Some(true),
            count: Some(count),
            include_domains: None,
            exclude_domains: None,
        };

        let response = self.client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Bocha API error: {}", response.status()).into());
        }

        let bocha_response: BochaSearchResponse = response.json().await?;

        Ok(bocha_response
            .web_pages
            .value
            .into_iter()
            .map(|r| SearchResult {
                title: r.name,
                url: r.url,
                snippet: r.summary.or(r.snippet).unwrap_or_default(),
                source: Some(r.site_name),
                favicon: Some(r.site_icon),
                timestamp: Some(r.date_published),
                score: None,
                raw: None,
            })
            .collect())
    }
}
```

### 字段说明

| 参数 | 必需 | 说明 | 可选值 |
|------|------|------|--------|
| `query` | ✅ | 搜索查询 | - |
| `freshness` | ❌ | 时间范围 | oneDay, oneWeek, oneMonth, oneYear |
| `summary` | ❌ | 返回详细摘要 | true/false |
| `count` | ❌ | 返回结果数 | 1-50 |
| `include_domains` | ❌ | 限制域名 | 域名数组 |
| `exclude_domains` | ❌ | 排除域名 | 域名数组 |

### 优势

- **国内优化**：平均响应 <1 秒
- **两级摘要**：同时提供 snippet（简短）和 summary（详细）
- **时间戳**：ISO 8601 格式的 `datePublished`
- **双重过滤**：支持 include_domains 和 exclude_domains

### 特殊处理

```rust
// 博查返回的日期需要解析
use chrono::DateTime;

let date = DateTime::parse_from_rfc3339(&result.date_published)?;
let formatted = date.format("%Y-%m-%d").to_string();
```

---

## 4. SerpAPI 集成

### 初始化

```rust
#[derive(Debug, Serialize)]
struct SerpApiRequest {
    q: String,
    engine: String,
    api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    num: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    google_domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hl: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SerpApiResponse {
    pub search_metadata: SearchMetadata,
    pub search_parameters: SearchParameters,
    pub search_information: SearchInformation,
    #[serde(default)]
    pub organic_results: Vec<OrganicResult>,
    #[serde(default)]
    pub related_searches: Vec<RelatedSearch>,
    #[serde(default)]
    pub related_questions: Vec<RelatedQuestion>,
}

#[derive(Debug, Deserialize)]
struct SearchMetadata {
    pub id: String,
    pub status: String,
    pub json_endpoint: String,
    pub created_at: String,
    pub processed_at: String,
    pub google_url: String,
    pub total_time_taken: f32,
}

#[derive(Debug, Deserialize)]
struct SearchParameters {
    pub q: String,
    pub engine: String,
}

#[derive(Debug, Deserialize)]
struct SearchInformation {
    pub organic_results_state: String,
    pub total_results: String,
}

#[derive(Debug, Deserialize)]
struct OrganicResult {
    pub position: u32,
    pub title: String,
    pub link: String,
    #[serde(default)]
    pub displayed_link: String,
    #[serde(default)]
    pub snippet: String,
    #[serde(default)]
    pub favicon: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RelatedSearch {
    pub query: String,
}

#[derive(Debug, Deserialize)]
struct RelatedQuestion {
    pub question: String,
}

pub struct SerpApiProvider {
    client: Client,
    api_key: String,
}

impl SerpApiProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    pub async fn search(&self, query: &str, num: u32) -> Result<Vec<SearchResult>> {
        let url = "https://serpapi.com/search";

        let params = [
            ("q", query),
            ("engine", "google"),
            ("api_key", &self.api_key),
            ("num", &num.to_string()),
        ];

        let response = self.client
            .get(url)
            .query(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("SerpAPI error: {}", response.status()).into());
        }

        let serp_response: SerpApiResponse = response.json().await?;

        // 检查搜索状态
        if serp_response.search_metadata.status != "Success" {
            return Err("Search failed".into());
        }

        Ok(serp_response
            .organic_results
            .into_iter()
            .map(|r| SearchResult {
                title: r.title,
                url: r.link,
                snippet: r.snippet,
                source: Some(r.displayed_link),
                favicon: r.favicon,
                timestamp: r.date,
                score: None,
                raw: None,
            })
            .collect())
    }
}
```

### 字段说明

| 参数 | 必需 | 说明 | 默认值 |
|------|------|------|--------|
| `q` | ✅ | 搜索查询 | - |
| `engine` | ✅ | 搜索引擎 | google |
| `api_key` | ✅ | API 密钥 | - |
| `num` | ❌ | 返回结果数（1-100） | 10 |
| `start` | ❌ | 结果起始位置 | 0 |
| `google_domain` | ❌ | Google 域名 | google.com |
| `gl` | ❌ | 国家代码 | - |
| `hl` | ❌ | 语言代码 | - |

### 关键特性

- **position 字段**：明确的结果排序位置
- **metadata**：详细的请求元数据（处理时间、搜索ID等）
- **多种结果类型**：除了有机结果，还有相关搜索、相关问题
- **displayed_link**：格式化的可显示链接

### 错误处理

```rust
// 通过 search_metadata.status 检查结果
match &response.search_metadata.status[..] {
    "Success" => Ok(response.organic_results),
    "Error" => {
        // 解析具体错误
        Err(format!("SerpAPI search error: {:?}", response))
    }
    _ => Err("Unknown status".into()),
}
```

---

## 统一的搜索 Provider 模式

```rust
// 统一接口
#[async_trait]
pub trait SearchProvider: Send + Sync {
    async fn search(&self, query: &str, limit: u32) -> Result<Vec<SearchResult>>;
    fn name(&self) -> &str;
}

// 标准化的搜索结果
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: Option<String>,
    pub favicon: Option<String>,
    pub timestamp: Option<String>,
    pub score: Option<f32>,
    pub raw: Option<serde_json::Value>,
}

impl SearchResult {
    /// 格式化为 Markdown
    pub fn to_markdown(&self) -> String {
        let mut md = format!("## [{}]({})\n\n{}\n", self.title, self.url, self.snippet);

        if let Some(source) = &self.source {
            md.push_str(&format!("_来源: {}_ ", source));
        }

        if let Some(timestamp) = &self.timestamp {
            md.push_str(&format!("| _发布: {}_ ", timestamp));
        }

        if let Some(score) = self.score {
            md.push_str(&format!("| _相关度: {:.0}%_ ", score * 100.0));
        }

        md.push('\n');
        md
    }
}

// 工厂模式选择 provider
pub enum SearchProviderType {
    Brave,
    Tavily,
    Bocha,
    SerpApi,
}

pub fn create_provider(provider_type: SearchProviderType, api_key: String) -> Box<dyn SearchProvider> {
    match provider_type {
        SearchProviderType::Brave => Box::new(BraveSearchProvider::new(api_key)),
        SearchProviderType::Tavily => Box::new(TavilySearchProvider::new(api_key)),
        SearchProviderType::Bocha => Box::new(BochaSearchProvider::new(api_key)),
        SearchProviderType::SerpApi => Box::new(SerpApiProvider::new(api_key)),
    }
}
```

---

## 最佳实践

### 1. 缓存策略
```rust
// 缓存键格式
let cache_key = format!("search:{}:{}", provider_name, query);

// 缓存有效期（基于时间戳）
let is_stale = if let Some(timestamp) = &result.timestamp {
    let pub_date = DateTime::parse_from_rfc3339(timestamp)?;
    let age = chrono::Utc::now().signed_duration_since(pub_date);
    age.num_days() > 7  // 7 天以上的结果标记为旧
} else {
    false
};
```

### 2. 速率限制
```rust
// 使用令牌桶限流
use governor::{Quota, RateLimiter};

let limiter = RateLimiter::direct(Quota::per_second(10)); // 10 req/s

limiter.until_ready().await;
provider.search(query, limit).await?;
```

### 3. 重试机制
```rust
async fn search_with_retry(
    provider: &dyn SearchProvider,
    query: &str,
    limit: u32,
    max_retries: u32,
) -> Result<Vec<SearchResult>> {
    let mut retries = 0;
    loop {
        match provider.search(query, limit).await {
            Ok(results) => return Ok(results),
            Err(e) if retries < max_retries => {
                retries += 1;
                tokio::time::sleep(Duration::from_millis(100 * 2_u64.pow(retries))).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}
```

### 4. 多 Provider 并行查询
```rust
async fn search_multiple(
    providers: &[Box<dyn SearchProvider>],
    query: &str,
    limit: u32,
) -> Vec<SearchResult> {
    let futures = providers.iter().map(|p| p.search(query, limit));

    let results = futures::future::join_all(futures).await;

    // 合并结果，去重
    let mut combined = Vec::new();
    for result_list in results {
        if let Ok(items) = result_list {
            combined.extend(items);
        }
    }

    // 按 URL 去重
    combined.sort_by(|a, b| a.url.cmp(&b.url));
    combined.dedup_by(|a, b| a.url == b.url);

    combined
}
```

---

## 常见问题

### Q: 如何选择最合适的搜索引擎？

**A:** 根据以下条件选择：

- **需要相关性评分**：使用 Tavily（有 score 字段）
- **需要国内优化**：使用 Bocha（<1s 响应，国内内容丰富）
- **需要全面搜索**：使用 SerpAPI（支持多种结果类型）
- **需要隐私保护**：使用 Brave（隐私友好）

### Q: 如何处理不同 API 的字段差异？

**A:** 使用本文档提供的映射表和标准化 SearchResult 结构，在适配器层进行转换。

### Q: 如何处理速率限制？

**A:**
1. 检查各 API 的速率限制文档
2. 使用令牌桶算法实现限流
3. 实现指数退避重试策略
4. 缓存搜索结果避免重复查询

### Q: 如何在应用中同时使用多个搜索引擎？

**A:**
1. 在数据库中存储多个 API 密钥
2. 实现 SearchProvider 工厂模式
3. 根据用户偏好或查询特征选择 provider
4. 可选：并行查询多个 provider，合并结果去重

---

## 相关链接

- [Brave Search API 官方文档](https://brave.com/search/api/)
- [Tavily API 官方文档](https://docs.tavily.com/)
- [博查搜索平台](https://open.bochaai.com/)
- [SerpAPI 官方文档](https://serpapi.com/search-api)
- [WorkClaw 搜索 API 响应格式对比](./search_api_response_formats.md)

---

*本文档最后更新：2025-02-24*
