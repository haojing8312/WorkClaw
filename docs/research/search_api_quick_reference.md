# 搜索引擎 API 快速参考

## 响应格式速查表

### 字段命名对比

```
用户需求      |  Brave          |  Tavily        |  Bocha         |  SerpAPI
-------------|-----------------|-----------------|-----------------|-----------
标题          | title           | title           | name            | title
链接          | url             | url             | url             | link
摘要/描述     | description     | content         | snippet         | snippet
来源          | profile.name    | ❌              | siteName        | displayed_link
图标          | profile.img     | favicon         | siteIcon        | favicon
时间戳        | ❌              | ❌              | datePublished   | date
评分          | ❌              | score (0-1)     | ❌              | ❌
位置          | ❌              | ❌              | ❌              | position
```

---

## API 端点速查

### Brave Search API
```
方法: GET
端点: https://api.search.brave.com/res/v1/web/search
认证: Query Parameter "api_key" 或 Header "Authorization: Token YOUR_KEY"
典型响应时间: ~200ms
```

### Tavily API
```
方法: POST
端点: https://api.tavily.com/search
认证: Header "Authorization: Bearer tvly-YOUR_KEY"
请求体: JSON
典型响应时间: ~500ms
```

### 博查搜索 API
```
方法: POST
端点: https://api.bochaai.com/v1/web-search
认证: Header "Authorization: Bearer YOUR_KEY"
请求体: JSON
典型响应时间: <1000ms (国内最快)
```

### SerpAPI
```
方法: GET
端点: https://serpapi.com/search
认证: Query Parameter "api_key"
典型响应时间: ~1000ms
```

---

## 结果数组位置

```
API          | 结果数组位置
-------------|-----------------------------------
Brave        | response.web.results[]
Tavily       | response.results[]
Bocha        | response.webPages.value[]
SerpAPI      | response.organic_results[]
```

---

## JSON 响应示例

### Brave Search API
```json
{
  "web": {
    "results": [
      {
        "title": "Example",
        "url": "https://example.com",
        "description": "Description..."
      }
    ]
  },
  "query": {
    "original": "query",
    "more_results_available": true
  }
}
```

### Tavily API
```json
{
  "query": "query",
  "answer": "AI生成的回答",
  "results": [
    {
      "title": "Example",
      "url": "https://example.com",
      "content": "Content...",
      "score": 0.95
    }
  ],
  "response_time": 0.5,
  "request_id": "uuid"
}
```

### 博查搜索 API
```json
{
  "webPages": {
    "totalEstimatedMatches": 1000000,
    "value": [
      {
        "name": "Example",
        "url": "https://example.com",
        "snippet": "Snippet...",
        "summary": "Summary...",
        "datePublished": "2025-02-24T10:00:00Z"
      }
    ]
  }
}
```

### SerpAPI
```json
{
  "search_metadata": {
    "status": "Success",
    "id": "uuid"
  },
  "organic_results": [
    {
      "position": 1,
      "title": "Example",
      "link": "https://example.com",
      "snippet": "Snippet..."
    }
  ]
}
```

---

## 常见参数速查

### Brave Search API
| 参数 | 类型 | 范围 | 默认值 |
|------|------|------|--------|
| q | string | - | 必需 |
| count | integer | 1-20 | 10 |
| offset | integer | 0+ | 0 |
| text_decorations | boolean | - | false |
| spellcheck | boolean | - | true |

### Tavily API
| 参数 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| query | string | 搜索词 | 必需 |
| max_results | integer | 1-20 | 10 |
| search_depth | string | basic/advanced | basic |
| topic | string | general/news | general |
| include_images | boolean | - | false |
| include_raw_content | boolean | - | false |
| include_answer | boolean | - | true |

### 博查搜索 API
| 参数 | 类型 | 说明 | 可选值 |
|------|------|------|--------|
| query | string | 搜索词 | 必需 |
| count | integer | 返回数量 | 1-50 |
| freshness | string | 时间范围 | oneDay/oneWeek/oneMonth/oneYear |
| summary | boolean | 返回摘要 | true/false |
| include_domains | array | 限制域名 | - |
| exclude_domains | array | 排除域名 | - |

### SerpAPI
| 参数 | 类型 | 范围 | 默认值 |
|------|------|------|--------|
| q | string | - | 必需 |
| num | integer | 1-100 | 10 |
| start | integer | 0+ | 0 |
| google_domain | string | - | google.com |
| gl | string | 国家代码 | - |
| hl | string | 语言代码 | - |

---

## 错误处理速查

### Brave Search
```
401 Unauthorized    → API Key 无效
403 Forbidden       → 超过订阅限制
429 Too Many Req    → 速率限制
```

### Tavily
```
400 Bad Request     → 请求格式错误
401 Unauthorized    → 无效的 API Key
429 Rate Limited    → 超过额度
使用 request_id 追踪问题
```

### 博查搜索
```
400 Bad Request     → 参数错误
401 Unauthorized    → 无效的 API Key
支持 include/exclude domains 过滤
```

### SerpAPI
```
验证 search_metadata.status
"Success"          → 成功
"Processing"       → 处理中
"Error"            → 出错
```

---

## 性能对比

| 指标 | Brave | Tavily | Bocha | SerpAPI |
|------|-------|--------|-------|---------|
| 平均响应时间 | ~200ms | ~500ms | <1000ms | ~1000ms |
| 最大结果数 | 20 | 20 | 50 | 100 |
| 国内可用性 | ✅ | ✅ | ✅✅ | ❌ |
| 价格（美元/月） | $10+ | 免费-$49 | 免费-¥299 | 免费-$99 |
| 免费额度 | ❌ | 1000/月 | 100/月 | 100/月 |

---

## 选择决策树

```
需要什么功能?
│
├─ 相关性评分 ──→ 使用 Tavily (score 字段)
│
├─ AI 直接回答 ──→ 使用 Tavily 或 Bocha (answer/summary)
│
├─ 最快响应速度 ──→ 使用 Bocha (<1s)
│
├─ 国内用户优化 ──→ 使用 Bocha (国内内容丰富)
│
├─ 隐私至上 ──────→ 使用 Brave (无追踪)
│
├─ 最完整元数据 ──→ 使用 SerpAPI (位置、日期等)
│
└─ 开源/自托管 ──→ 无官方选项, 考虑 Meilisearch/Typesense
```

---

## 集成检查清单

- [ ] API Key 已获取并安全存储
- [ ] 请求头/参数格式正确
- [ ] 实现了错误处理
- [ ] 实现了速率限制
- [ ] 实现了结果缓存
- [ ] 实现了重试机制（指数退避）
- [ ] 响应格式已转换为标准 SearchResult
- [ ] 单元测试已覆盖各 Provider
- [ ] 性能测试完成
- [ ] 生产环境密钥已加密存储

---

## 常见集成模式

### Pattern 1: 单一 Provider
```rust
let provider = BraveSearchProvider::new(api_key);
let results = provider.search("query", 10).await?;
```

### Pattern 2: Provider 选择
```rust
let provider = match preferred_engine {
    "brave" => create_provider(SearchProviderType::Brave, api_key),
    "tavily" => create_provider(SearchProviderType::Tavily, api_key),
    _ => create_provider(SearchProviderType::Bocha, api_key),
};
```

### Pattern 3: 多 Provider 并行
```rust
let results = futures::future::join_all(vec![
    provider1.search(query, 5),
    provider2.search(query, 5),
    provider3.search(query, 5),
]).await;

// 合并去重
let combined = merge_and_deduplicate(results);
```

### Pattern 4: 带缓存和 Fallback
```rust
// 先查缓存
if let Some(cached) = cache.get(&query) {
    return Ok(cached);
}

// 尝试主 Provider
let result = primary_provider.search(query, 10).await
    .or_else(|_| fallback_provider.search(query, 10).await)?;

// 缓存结果
cache.set(&query, &result, Duration::from_secs(3600));
Ok(result)
```

---

## 数据字段映射（WorkClaw 标准）

```typescript
interface SearchResult {
  // 必需字段
  title: string;           // 结果标题
  url: string;            // 结果 URL
  snippet: string;        // 摘要文本

  // 可选增强字段
  source?: string;        // 来源名称
  favicon?: string;       // 来源图标 URL
  timestamp?: string;     // 发布时间 (ISO 8601)
  score?: number;         // 相关性评分 (0-1)

  // 内部使用
  provider: string;       // 来源 API (brave/tavily/bocha/serpapi)
  raw?: Record<string, any>; // 原始 API 响应 (调试用)
}
```

---

## 推荐使用组合

### 小型应用（单引擎）
```
推荐: 博查搜索 (Bocha)
原因: 国内优化, 快速响应, 免费额度充足
```

### 中等应用（双引擎）
```
主引擎: 博查搜索 (国内)
备用引擎: Tavily (国外)
理由: 地域覆盖, Tavily 有相关性评分
```

### 大型应用（多引擎）
```
US/EU 用户: Brave (隐私) + SerpAPI (完整)
中国用户: Bocha (国内) + Tavily (综合)
策略: 根据用户地域路由, 并行查询去重合并
```

---

*最后更新: 2025-02-24*
