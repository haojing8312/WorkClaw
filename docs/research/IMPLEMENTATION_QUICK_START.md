# Web Search & Fetch Tools: Quick Start Implementation Guide

**For WorkClaw Runtime**
**Version**: 1.0
**Date**: February 24, 2026

---

## TL;DR Decision Matrix

```
Need to search web?
├─ Single query → Use Tavily directly
├─ Multiple searches → Add SQLite cache
├─ Need content extract → Add Trafilatura
├─ Complex pages (JS) → Add Firecrawl
├─ Different results → Add Brave fallback
└─ Production scale → Add semantic cache + Redis
```

**Recommended Stack for WorkClaw**:
1. **Search**: Tavily (primary) + Brave (fallback)
2. **Extraction**: Trafilatura (free) + Firecrawl (for JS)
3. **Caching**: In-Memory → SQLite → Semantic
4. **Browser**: Playwright (for stateful browsing)

---

## Phase 1: Implement Basic Web Search (Week 1)

### Step 1: Add Tavily Integration (Rust Backend)

```rust
// apps/runtime/src-tauri/src/agent/tools/web_search.rs

use serde::{Deserialize, Serialize};
use anyhow::Result;
use reqwest::Client;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub content: String,
    pub relevance_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TavilyResponse {
    pub results: Vec<SearchResult>,
    pub answer: Option<String>,
    pub response_time: f32,
}

pub struct TavilySearchTool {
    api_key: String,
    client: Client,
}

impl TavilySearchTool {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }

    pub async fn search(&self, query: &str) -> Result<TavilyResponse> {
        let payload = serde_json::json!({
            "api_key": self.api_key,
            "query": query,
            "search_depth": "advanced",
            "max_results": 5,
            "include_answer": true,
            "include_raw_content": "markdown",
            "chunks_per_source": 2,
        });

        let response = self
            .client
            .post("https://api.tavily.com/search")
            .json(&payload)
            .send()
            .await?;

        let tavily_resp: TavilyResponse = response.json().await?;
        Ok(tavily_resp)
    }
}

// Implement Tool trait
use crate::agent::types::Tool;

impl Tool for TavilySearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for information using Tavily Search API. \
         Returns results with markdown content ready for LLM injection."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                }
            },
            "required": ["query"]
        })
    }

    fn execute(&self, input: serde_json::Value) -> Result<String> {
        let query = input.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing query"))?;

        // Run async function synchronously (requires tokio runtime)
        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.search(query))
        })?;

        // Format results for LLM
        let formatted = format_search_results(&response);
        Ok(formatted)
    }
}

fn format_search_results(response: &TavilyResponse) -> String {
    let mut output = String::new();

    if let Some(answer) = &response.answer {
        output.push_str("## Summary\n");
        output.push_str(answer);
        output.push_str("\n\n");
    }

    output.push_str("## Detailed Results\n\n");

    for (i, result) in response.results.iter().enumerate() {
        output.push_str(&format!("### Result {}: {}\n", i + 1, result.title));
        output.push_str(&format!("**URL**: {}\n", result.url));
        output.push_str(&format!("**Relevance**: {:.2}%\n\n", result.relevance_score * 100.0));
        output.push_str(&result.content);
        output.push_str("\n\n---\n\n");
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_schema() {
        let tool = TavilySearchTool::new("test_key".to_string());
        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
    }
}
```

### Step 2: Register Tool in Agent Executor

```rust
// apps/runtime/src-tauri/src/agent/registry.rs

use crate::agent::tools::web_search::TavilySearchTool;

pub fn register_tools(api_keys: &ApiKeys) -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    // Web search
    registry.register(
        Box::new(TavilySearchTool::new(
            api_keys.tavily_api_key.clone()
        ))
    );

    // File tools (existing)
    registry.register(Box::new(ReadFileTool));
    registry.register(Box::new(WriteFileTool));
    registry.register(Box::new(GlobTool));
    registry.register(Box::new(GrepTool));
    registry.register(Box::new(BashTool));

    registry
}
```

### Step 3: Add Tavily API Key to Settings

```typescript
// apps/runtime/src/components/SettingsView.tsx

export function SettingsView() {
  const [tavilyKey, setTavilyKey] = useState("");

  const saveTavilyKey = async () => {
    await invoke("set_api_key", {
      provider: "tavily",
      apiKey: tavilyKey,
    });
  };

  return (
    <div className="p-6">
      <h2 className="text-lg font-bold mb-4">API 配置</h2>

      <div className="mb-4">
        <label className="block text-sm font-medium mb-2">
          Tavily API 密钥
        </label>
        <input
          type="password"
          value={tavilyKey}
          onChange={(e) => setTavilyKey(e.target.value)}
          className="w-full px-3 py-2 border rounded"
          placeholder="your-tavily-api-key"
        />
        <button
          onClick={saveTavilyKey}
          className="mt-2 px-4 py-2 bg-blue-500 text-white rounded"
        >
          保存
        </button>
      </div>
    </div>
  );
}
```

### Step 4: Test in Agent Loop

```rust
// apps/runtime/src-tauri/tests/test_web_search.rs

#[tokio::test]
async fn test_tavily_search() {
    let api_key = std::env::var("TAVILY_API_KEY")
        .expect("TAVILY_API_KEY env var required");

    let tool = TavilySearchTool::new(api_key);

    let result = tool.search("latest AI agent frameworks 2026")
        .await
        .expect("Search failed");

    assert!(!result.results.is_empty());
    assert!(result.response_time > 0.0);

    println!("Got {} results in {:.2}s",
        result.results.len(),
        result.response_time
    );
}
```

---

## Phase 2: Add Caching Layer (Week 2)

### Step 1: Implement SQLite Cache

```rust
// apps/runtime/src-tauri/src/agent/cache.rs

use sqlx::{sqlite::SqlitePool, Row};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SearchCache {
    pool: SqlitePool,
    max_age_days: i64,
}

impl SearchCache {
    pub async fn new(db_path: &str, max_age_days: i64) -> Result<Self> {
        let pool = SqlitePool::connect(&format!("sqlite://{}", db_path))
            .await?;

        // Create table if not exists
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS search_cache (
                query_hash TEXT PRIMARY KEY,
                query TEXT NOT NULL,
                results JSON NOT NULL,
                created_at INTEGER NOT NULL
            )"
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool, max_age_days })
    }

    pub async fn get(&self, query: &str) -> Result<Option<String>> {
        let query_hash = format!("{:x}", md5::compute(query.as_bytes()));
        let now = current_timestamp();
        let cutoff = now - (self.max_age_days * 86400);

        let row = sqlx::query(
            "SELECT results FROM search_cache
             WHERE query_hash = ?1 AND created_at > ?2"
        )
        .bind(query_hash)
        .bind(cutoff)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.get::<String, _>("results")))
    }

    pub async fn set(&self, query: &str, results: &str) -> Result<()> {
        let query_hash = format!("{:x}", md5::compute(query.as_bytes()));
        let now = current_timestamp();

        sqlx::query(
            "INSERT OR REPLACE INTO search_cache
             (query_hash, query, results, created_at)
             VALUES (?1, ?2, ?3, ?4)"
        )
        .bind(query_hash)
        .bind(query)
        .bind(results)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
```

### Step 2: Wrap Search Tool with Cache

```rust
// apps/runtime/src-tauri/src/agent/tools/cached_web_search.rs

pub struct CachedWebSearchTool {
    inner: TavilySearchTool,
    cache: SearchCache,
}

impl CachedWebSearchTool {
    pub async fn new(
        api_key: String,
        cache: SearchCache,
    ) -> Self {
        Self {
            inner: TavilySearchTool::new(api_key),
            cache,
        }
    }

    pub async fn search_with_cache(&self, query: &str) -> Result<TavilyResponse> {
        // Try cache first
        if let Ok(Some(cached)) = self.cache.get(query).await {
            if let Ok(response) = serde_json::from_str::<TavilyResponse>(&cached) {
                println!("Cache hit for query: {}", query);
                return Ok(response);
            }
        }

        // Cache miss: search
        let response = self.inner.search(query).await?;

        // Store in cache
        let json = serde_json::to_string(&response)?;
        let _ = self.cache.set(query, &json).await;

        Ok(response)
    }
}

// Implement Tool trait
impl Tool for CachedWebSearchTool {
    fn name(&self) -> &str { "web_search" }

    fn description(&self) -> &str {
        "Search the web using Tavily (cached, 7-day TTL)"
    }

    fn input_schema(&self) -> serde_json::Value {
        self.inner.input_schema()
    }

    fn execute(&self, input: serde_json::Value) -> Result<String> {
        let query = input.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing query"))?;

        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(self.search_with_cache(query))
        })?;

        Ok(format_search_results(&response))
    }
}
```

---

## Phase 3: Add Fallback Provider (Week 2.5)

### Step 1: Implement Brave Search

```rust
// apps/runtime/src-tauri/src/agent/tools/brave_search.rs

pub struct BraveSearchTool {
    api_key: String,
    client: Client,
}

impl BraveSearchTool {
    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let response = self
            .client
            .get("https://api.search.brave.com/res/v1/web/search")
            .header("Authorization", format!("Token {}", self.api_key))
            .query(&[("q", query)])
            .send()
            .await?;

        let data: serde_json::Value = response.json().await?;

        let results = data["web"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .take(5)
            .map(|item| SearchResult {
                title: item["title"].as_str().unwrap_or("").to_string(),
                url: item["url"].as_str().unwrap_or("").to_string(),
                content: item["description"].as_str().unwrap_or("").to_string(),
                relevance_score: 0.8,
            })
            .collect();

        Ok(results)
    }
}
```

### Step 2: Implement Fallback Strategy

```rust
// apps/runtime/src-tauri/src/agent/tools/web_search_with_fallback.rs

pub struct WebSearchWithFallback {
    primary: CachedWebSearchTool,
    fallback: BraveSearchTool,
}

impl WebSearchWithFallback {
    pub async fn search(&self, query: &str) -> Result<TavilyResponse> {
        // Try primary (Tavily)
        match self.primary.search_with_cache(query).await {
            Ok(response) => {
                if !response.results.is_empty() {
                    return Ok(response);
                }
            }
            Err(e) => {
                eprintln!("Primary search failed: {}", e);
            }
        }

        // Fallback to Brave
        eprintln!("Falling back to Brave Search for: {}", query);
        let brave_results = self.fallback.search(query).await?;

        Ok(TavilyResponse {
            results: brave_results,
            answer: None,
            response_time: 0.0,
        })
    }
}

impl Tool for WebSearchWithFallback {
    fn name(&self) -> &str { "web_search" }

    fn description(&self) -> &str {
        "Search the web: try Tavily first, fallback to Brave"
    }

    fn input_schema(&self) -> serde_json::Value {
        self.primary.input_schema()
    }

    fn execute(&self, input: serde_json::Value) -> Result<String> {
        let query = input["query"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing query"))?;

        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(self.search(query))
        })?;

        Ok(format_search_results(&response))
    }
}
```

---

## Phase 4: Add Content Extraction (Week 3)

### Step 1: Add Trafilatura to Sidecar

```typescript
// apps/runtime/sidecar/src/routes/web.ts

import { Router } from "hono";
import trafilatura from "trafilatura";

const router = new Router();

router.post("/api/web/extract", async (c) => {
  const { url } = await c.req.json();

  try {
    // Fetch HTML
    const response = await fetch(url);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);

    const html = await response.text();

    // Extract with Trafilatura
    const markdown = trafilatura.extract(html, {
      output_format: "markdown",
      with_metadata: true,
      include_formatting: true,
      include_links: true,
      favor_precision: true,
    });

    return c.json({
      success: true,
      markdown,
      url,
      bytes: html.length,
    });
  } catch (error) {
    return c.json(
      {
        success: false,
        error: error.message,
      },
      { status: 400 }
    );
  }
});

export default router;
```

### Step 2: Call from Rust

```rust
// apps/runtime/src-tauri/src/sidecar/web_extract.rs

use reqwest::Client;

pub async fn extract_content(url: &str) -> Result<String> {
    let client = Client::new();
    let response = client
        .post("http://localhost:8765/api/web/extract")
        .json(&serde_json::json!({ "url": url }))
        .send()
        .await?;

    let data: serde_json::Value = response.json().await?;

    if data["success"].as_bool().unwrap_or(false) {
        Ok(data["markdown"].as_str().unwrap_or("").to_string())
    } else {
        Err(anyhow::anyhow!(
            "Extraction failed: {}",
            data["error"].as_str().unwrap_or("unknown error")
        ))
    }
}
```

---

## Phase 5: Add Playwright (Week 4)

### Step 1: Setup Playwright in Sidecar

```typescript
// apps/runtime/sidecar/src/browser.ts

import { chromium, Browser, Page } from "playwright";
import trafilatura from "trafilatura";

class BrowserManager {
  private browser: Browser | null = null;
  private pages: Map<string, Page> = new Map();

  async init() {
    this.browser = await chromium.launch({
      headless: true,
    });
  }

  async fetch(url: string, waitForSelector?: string): Promise<string> {
    if (!this.browser) throw new Error("Browser not initialized");

    const context = await this.browser.newContext({
      viewport: { width: 1280, height: 720 },
    });

    const page = await context.newPage();

    try {
      await page.goto(url, { waitUntil: "networkidle" });

      if (waitForSelector) {
        await page.waitForSelector(waitForSelector, { timeout: 5000 });
      }

      const html = await page.content();
      const markdown = trafilatura.extract(html, {
        output_format: "markdown",
      });

      return markdown;
    } finally {
      await context.close();
    }
  }

  async close() {
    if (this.browser) {
      await this.browser.close();
    }
  }
}

export const browserManager = new BrowserManager();
```

### Step 2: Add Playwright Route

```typescript
// apps/runtime/sidecar/src/routes/browser.ts

import { Router } from "hono";
import { browserManager } from "../browser";

const router = new Router();

router.post("/api/browser/fetch", async (c) => {
  const { url, waitForSelector } = await c.req.json();

  try {
    const markdown = await browserManager.fetch(url, waitForSelector);
    return c.json({
      success: true,
      markdown,
      url,
    });
  } catch (error) {
    return c.json(
      {
        success: false,
        error: error.message,
      },
      { status: 400 }
    );
  }
});

export default router;
```

---

## Environment Setup

### .env Configuration

```bash
# apps/runtime/.env

# Tavily Search API
TAVILY_API_KEY=your-tavily-api-key

# Brave Search API
BRAVE_SEARCH_API_KEY=your-brave-search-api-key

# Database
DATABASE_URL=sqlite:./workclaw.db

# Search Caching
SEARCH_CACHE_TTL_DAYS=7

# Feature flags
ENABLE_WEB_SEARCH=true
ENABLE_CONTENT_EXTRACTION=true
ENABLE_PLAYWRIGHT=true
```

---

## Testing Checklist

- [ ] Tavily API key configured and tested
- [ ] Web search returns results in markdown format
- [ ] Cache stores and retrieves results correctly
- [ ] Brave search works as fallback
- [ ] Content extraction returns markdown
- [ ] Token count < 3000 for 5 results
- [ ] Agent uses web search tool in ReAct loop
- [ ] Cost tracking dashboard shows API usage

---

## Costs Estimate

| Component | Cost | Notes |
|-----------|------|-------|
| Tavily | $25/month | 10K searches/month at basic pricing |
| Brave | $0/month | Using free tier (2K/month) |
| Trafilatura | $0 | Self-hosted |
| Playwright | $0 | Self-hosted in sidecar |
| Infrastructure | $20/month | Redis + SQLite |
| **Total** | **~$45/month** | For 10K searches + 1K extractions |

---

## Next Steps

1. **Week 1**: Implement Tavily + basic caching
2. **Week 2**: Add Brave fallback + SQLite cache
3. **Week 3**: Integrate Trafilatura
4. **Week 4**: Add Playwright for stateful browsing
5. **Week 5**: Semantic caching + cost optimization

---

**Created**: February 24, 2026
**Status**: Ready for Implementation
