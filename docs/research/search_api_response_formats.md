# 搜索引擎 API 响应格式对比

本文档总结了主流搜索引擎 API 的响应格式，用于在 WorkClaw 中统一标准化的搜索结果格式。

## 1. Brave Search API

### 官方文档链接
- 主站：https://brave.com/search/api/
- 文档：https://api-dashboard.search.brave.com/app/documentation/web-search/responses

### 响应格式

#### 顶级响应结构
```json
{
  "type": "search",
  "web": {
    "type": "search",
    "results": [
      // 搜索结果数组
    ]
  },
  "query": {
    "original": "搜索查询词",
    "spellcheck_off": false,
    "show_strict_warning": false,
    "more_results_available": true
  }
}
```

#### 单个搜索结果字段
| 字段名 | 类型 | 说明 |
|--------|------|------|
| `title` | string | 结果标题 |
| `url` | string | 结果链接 |
| `description` | string | 片段/摘要 |
| `is_source_local` | boolean | 是否本地结果 |
| `is_source_both` | boolean | 是否混合结果 |
| `profile.name` | string | 来源名称 |
| `profile.url` | string | 来源URL |
| `profile.long_name` | string | 来源完整名称 |
| `profile.img` | string | 来源图标 |
| `extra_snippets` | string[] | 额外的摘要片段（可选） |
| `age` | string | 内容年龄（新闻结果） |
| `page_age` | string | 页面年龄时间戳（新闻结果） |

#### 访问其他结果类型
```python
# 通过属性访问不同类型的结果
search_results.web_results      # 网页结果
search_results.news_results     # 新闻结果
search_results.video_results    # 视频结果
```

### 关键特性
- 支持 `extra_snippets`：最多 5 个额外摘要片段，增加上下文
- 新闻结果包含时间戳信息
- 支持 `more_results_available` 字段指示是否有更多结果

---

## 2. Tavily API

### 官方文档链接
- 主站：https://docs.tavily.com/
- 搜索端点：https://docs.tavily.com/documentation/api-reference/endpoint/search

### 响应格式

#### 完整响应结构
```json
{
  "query": "搜索查询词",
  "answer": "对查询的简短回答",
  "images": [
    "图片URL 1",
    "图片URL 2"
  ],
  "results": [
    {
      "title": "结果标题",
      "url": "https://example.com",
      "content": "内容片段",
      "raw_content": "原始HTML内容（可选）",
      "score": 0.95,
      "favicon": "https://example.com/favicon.png"
    }
  ],
  "response_time": "1.67",
  "auto_parameters": {
    "topic": "general",
    "search_depth": "basic"
  },
  "usage": {
    "credits": 1
  },
  "request_id": "123e4567-e89b-12d3-a456-426614174111"
}
```

#### 单个搜索结果字段
| 字段名 | 类型 | 说明 |
|--------|------|------|
| `title` | string | 结果标题 |
| `url` | string | 结果链接 |
| `content` | string | 内容片段 |
| `raw_content` | string | 原始HTML内容（可选） |
| `score` | number | 相关性评分（0-1） |
| `favicon` | string | 来源图标URL |

#### 响应的其他字段
| 字段名 | 说明 |
|--------|------|
| `answer` | API 生成的简短回答 |
| `images` | 相关图片 URL 数组 |
| `response_time` | 响应时间（毫秒） |
| `auto_parameters` | 自动推断的参数 |
| `usage` | 使用的额度信息 |
| `request_id` | 请求ID（用于追踪） |

### 关键特性
- 提供 `answer` 字段：AI 生成的直接回答
- `score` 字段表示相关性评分
- 支持原始 HTML 内容提取
- 包含使用额度信息

---

## 3. 博查搜索 (Bocha) API

### 官方平台
- 官网：https://open.bochaai.com/

### 响应格式

#### 完整响应结构
```json
{
  "_type": "SearchResponse",
  "queryContext": {
    "originalQuery": "搜索查询词"
  },
  "webPages": {
    "webSearchUrl": "https://bochaai.com/search?q=...",
    "totalEstimatedMatches": 606721,
    "value": [
      {
        "id": "https://api.bochaai.com/v1/#WebPages.0",
        "name": "结果标题",
        "url": "https://example.com",
        "siteName": "网站名称",
        "siteIcon": "https://th.bochaai.com/favicon?domain_url=...",
        "snippet": "摘要片段",
        "summary": "详细摘要",
        "datePublished": "2024-07-22T00:00:00+08:00"
      }
    ]
  }
}
```

#### 单个搜索结果字段
| 字段名 | 类型 | 说明 |
|--------|------|------|
| `id` | string | 结果唯一标识 |
| `name` | string | 结果标题 |
| `url` | string | 结果链接 |
| `siteName` | string | 网站名称 |
| `siteIcon` | string | 网站图标URL |
| `snippet` | string | 简短摘要 |
| `summary` | string | 详细摘要 |
| `datePublished` | string | 发布时间（ISO 8601格式） |

### 关键特性
- 同时提供 `snippet`（简短）和 `summary`（详细）
- 包含 `datePublished` 时间戳
- `totalEstimatedMatches` 显示总结果数
- 字段命名：`name` 代替 `title`，`url` 保持一致

---

## 4. SerpAPI

### 官方文档链接
- 主站：https://serpapi.com/search-api
- 有机结果 API：https://serpapi.com/organic-results

### 响应格式

#### 顶级响应结构
```json
{
  "search_metadata": {
    "id": "搜索ID",
    "status": "Success",
    "json_endpoint": "https://serpapi.com/search.json?...",
    "created_at": "2025-02-24T10:00:00Z",
    "processed_at": "2025-02-24T10:00:01Z",
    "google_url": "https://www.google.com/search?q=...",
    "total_time_taken": 1.23
  },
  "search_parameters": {
    "q": "搜索查询词",
    "engine": "google",
    "location": "United States",
    "google_domain": "google.com"
  },
  "search_information": {
    "organic_results_state": "Results for exact search",
    "total_results": 1234567
  },
  "organic_results": [
    // 搜索结果数组
  ],
  "related_searches": [...],
  "related_questions": [...]
}
```

#### 单个有机搜索结果字段
| 字段名 | 类型 | 说明 |
|--------|------|------|
| `position` | number | 结果位置（1 开始） |
| `title` | string | 结果标题 |
| `link` | string | 结果链接 |
| `displayed_link` | string | 显示的链接（格式化） |
| `snippet` | string | 结果摘要 |
| `redirect_link` | string | 重定向链接 |
| `favicon` | string | 网站图标URL |
| `about_page_link` | string | "关于此页面"链接 |
| `cached_page_link` | string | 缓存页面链接 |
| `related_pages_link` | string | 相关页面链接 |
| `date` | string | 发布日期 |
| `author` | string | 作者 |
| `extensions` | string[] | 扩展信息 |
| `reviews` | number | 评论数 |
| `ratings` | number | 评分 |
| `carousel` | object[] | 轮播数据（可选） |

#### 访问不同的结果类型
```python
results['organic_results']      # 有机搜索结果
results['shopping_results']     # 购物结果
results['knowledge_graph']      # 知识图谱
results['local_results']        # 本地结果
results['related_searches']     # 相关搜索
results['related_questions']    # 相关问题
```

### 关键特性
- `position` 字段明确指示结果排序位置
- 包含丰富的元数据：`displayed_link`, `favicon`, `about_page_link` 等
- `snippet` 字段名一致性强
- 支持各种增强元素：轮播、扩展、评分等

---

## 字段映射与标准化

### 为了在 WorkClaw 中统一标准，推荐的字段映射如下：

```typescript
// WorkClaw 标准搜索结果格式
interface StandardSearchResult {
  title: string;              // 结果标题
  url: string;               // 结果链接
  snippet: string;           // 结果摘要/描述
  source?: string;           // 来源名称
  favicon?: string;          // 来源图标
  timestamp?: string;        // 发布时间（ISO 8601）
  score?: number;            // 相关性评分（0-1）
  raw?: Record<string, any>; // 原始API响应（用于调试）
}
```

### 映射规则

| 字段 | Brave | Tavily | Bocha | SerpAPI |
|------|-------|--------|-------|---------|
| **title** | `title` | `title` | `name` | `title` |
| **url** | `url` | `url` | `url` | `link` |
| **snippet** | `description` | `content` | `snippet` | `snippet` |
| **source** | `profile.name` | ❌ | `siteName` | `displayed_link` |
| **favicon** | `profile.img` | `favicon` | `siteIcon` | `favicon` |
| **timestamp** | ❌ | ❌ | `datePublished` | `date` |
| **score** | ❌ | `score` | ❌ | ❌ |

### 备注
- **❌** 表示该 API 不提供此字段
- Brave 的 `description` 字段有时包含 HTML 标签，需要清理
- Tavily 的 `score` 字段对于排序最有帮助
- SerpAPI 的 `position` 字段可用于验证排序

---

## 实现建议

### 1. Provider 适配层
在 `apps/runtime/src-tauri/src/adapters/search.rs` 中创建统一的适配层：

```rust
pub trait SearchProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>>;
}

pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: Option<String>,
    pub favicon: Option<String>,
    pub timestamp: Option<String>,
    pub score: Option<f32>,
}

// 为每个 API 实现转换
impl From<BraveResult> for SearchResult { /* ... */ }
impl From<TavilyResult> for SearchResult { /* ... */ }
impl From<BochaResult> for SearchResult { /* ... */ }
impl From<SerpApiResult> for SearchResult { /* ... */ }
```

### 2. 错误处理
各 API 的错误处理方式不同：
- **Brave**：Status 200 OK，需要检查 HTTP 状态码
- **Tavily**：包含 `request_id` 用于调试
- **Bocha**：支持自定义参数过滤
- **SerpAPI**：`search_metadata.status` 字段指示成功/失败

### 3. 速率限制
- **Brave**：根据订阅计划限制
- **Tavily**：使用 `usage.credits` 追踪
- **Bocha**：国内优化，默认 <1 秒响应
- **SerpAPI**：根据计划分配请求额度

### 4. 缓存策略
推荐缓存键格式：`{provider}:{query}:{timestamp}`
- 缓存时间：1 小时（或根据 `datePublished` 调整）
- 缓存大小：由应用配置决定

---

## 相关资源

### 官方文档
- [Brave Search API](https://brave.com/search/api/)
- [Tavily 文档](https://docs.tavily.com/)
- [博查 AI 平台](https://open.bochaai.com/)
- [SerpAPI 文档](https://serpapi.com/search-api)

### GitHub 参考实现
- [Brave Search MCP Server](https://github.com/brave/brave-search-mcp-server)
- [Tavily Python SDK](https://github.com/tavily-ai/tavily-python)
- [Bocha Search MCP](https://github.com/BochaAI/bocha-search-mcp)
- [SerpAPI Python](https://github.com/serpapi/google-search-results-python)

### 相关 WorkClaw 代码
- Provider 预设配置：`docs/plans/2026-02-19-llm-adapter-provider-presets-design.md`
- 搜索工具注册：`apps/runtime/src-tauri/src/agent/tools/`

---

*本文档最后更新：2025-02-24*
*覆盖的 API 版本：*
- *Brave Search API - 最新*
- *Tavily API - 最新*
- *Bocha Web Search API - 最新*
- *SerpAPI - 最新（Google 搜索）*
