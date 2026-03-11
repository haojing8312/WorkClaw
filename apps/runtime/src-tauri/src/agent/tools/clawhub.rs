use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::agent::executor::truncate_tool_output;
use crate::agent::types::{Tool, ToolContext};

const DEFAULT_CLAWHUB_BASE: &str = "https://www.clawhub.ai";
const OUTPUT_MAX_CHARS: usize = 30_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClawhubSkillSummary {
    name: String,
    slug: String,
    description: String,
    github_url: Option<String>,
    source_url: Option<String>,
    stars: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClawhubLibraryItem {
    slug: String,
    name: String,
    summary: String,
    stars: i64,
}

#[derive(Debug, Clone, Serialize)]
struct ClawhubSkillRecommendation {
    slug: String,
    name: String,
    description: String,
    stars: i64,
    score: f64,
    reason: String,
    github_url: Option<String>,
    source_url: Option<String>,
}

pub struct ClawhubSearchTool;
pub struct ClawhubRecommendTool;

fn clawhub_base_url() -> String {
    std::env::var("CLAWHUB_API_BASE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CLAWHUB_BASE.to_string())
}

fn tokenize_query(input: &str) -> Vec<String> {
    let mut out: Vec<String> = input
        .split(|c: char| !c.is_alphanumeric() && !('\u{4E00}'..='\u{9FFF}').contains(&c))
        .map(|s| s.trim().to_lowercase())
        .filter(|s| s.chars().count() >= 2)
        .collect();
    out.sort();
    out.dedup();
    out
}

fn calc_recommendation_score(query: &str, skill: &ClawhubSkillSummary) -> (f64, usize) {
    let q = query.to_lowercase();
    let name = skill.name.to_lowercase();
    let desc = skill.description.to_lowercase();
    let tokens = tokenize_query(query);

    let mut score = 0.0;
    let mut hits = 0usize;
    if !q.is_empty() && name.contains(&q) {
        score += 45.0;
        hits += 2;
    }
    if !q.is_empty() && desc.contains(&q) {
        score += 25.0;
        hits += 1;
    }
    for t in &tokens {
        if name.contains(t) {
            score += 12.0;
            hits += 1;
        } else if desc.contains(t) {
            score += 7.0;
            hits += 1;
        }
    }
    let pop = (skill.stars.max(0) as f64).sqrt().min(10.0);
    score += pop;
    (score, hits)
}

fn build_recommend_reason(hits: usize, stars: i64) -> String {
    if hits >= 3 && stars > 0 {
        format!("与需求关键词高度匹配，且有一定社区认可（{} stars）", stars)
    } else if hits >= 3 {
        "与需求关键词高度匹配，建议优先尝试".to_string()
    } else if hits >= 1 && stars > 0 {
        format!("与需求有较强相关性，并具备社区反馈（{} stars）", stars)
    } else {
        "与需求相关，适合作为备选方案".to_string()
    }
}

fn normalize_library_item(item: &Value) -> Option<ClawhubLibraryItem> {
    let slug = item.get("slug")?.as_str()?.to_string();
    let name = item
        .get("displayName")
        .and_then(|v| v.as_str())
        .or_else(|| item.get("name").and_then(|v| v.as_str()))
        .unwrap_or(&slug)
        .to_string();
    let summary = item
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let stars = item
        .get("stats")
        .and_then(|s| s.get("stars"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    Some(ClawhubLibraryItem {
        slug,
        name,
        summary,
        stars,
    })
}

fn normalize_search_skill_from_library_item(item: &Value) -> Option<ClawhubSkillSummary> {
    let slug = item.get("slug")?.as_str()?.to_string();
    let name = item
        .get("displayName")
        .and_then(|v| v.as_str())
        .or_else(|| item.get("name").and_then(|v| v.as_str()))
        .unwrap_or(&slug)
        .to_string();
    let description = item
        .get("summary")
        .and_then(|v| v.as_str())
        .or_else(|| item.get("description").and_then(|v| v.as_str()))
        .unwrap_or_default()
        .to_string();
    let github_url = item
        .get("github_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let source_url = item
        .get("source_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| Some(format!("{}/skills/{}", clawhub_base_url(), slug)));
    let stars = item
        .get("stats")
        .and_then(|s| s.get("stars"))
        .and_then(|v| v.as_i64())
        .or_else(|| item.get("stars").and_then(|v| v.as_i64()))
        .unwrap_or(0);

    Some(ClawhubSkillSummary {
        name,
        slug,
        description,
        github_url,
        source_url,
        stars,
    })
}

fn build_query_variants(query: &str) -> Vec<String> {
    let mut variants = vec![query.trim().to_string()];
    let mut english_hints: Vec<&str> = Vec::new();
    if query.contains("短视频") || query.contains("视频") {
        english_hints.push("video");
        english_hints.push("short video");
    }
    if query.contains("自动") || query.contains("批量") {
        english_hints.push("automation");
    }
    if query.contains("内容创作") || query.contains("文案") {
        english_hints.push("content creation");
    }
    if query.contains("小红书") {
        english_hints.push("xiaohongshu");
    }
    if query.contains("公众号") {
        english_hints.push("wechat");
    }
    for hint in english_hints {
        if !variants.iter().any(|v| v.eq_ignore_ascii_case(hint)) {
            variants.push(hint.to_string());
        }
    }
    variants
}

fn fetch_search_once(
    client: &reqwest::blocking::Client,
    query: &str,
    page: u32,
    limit: u32,
) -> Result<Vec<ClawhubSkillSummary>> {
    let base = clawhub_base_url();
    let url = format!(
        "{}/api/v1/skills?limit={}&sort={}&nonSuspicious=true&q={}",
        base,
        limit,
        if page > 1 { "updated" } else { "downloads" },
        urlencoding::encode(query)
    );

    let resp = client
        .get(url)
        .send()
        .map_err(|e| anyhow!("ClawHub 请求失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(anyhow!("ClawHub 搜索失败: HTTP {}", resp.status()));
    }
    let body: Value = resp
        .json()
        .map_err(|e| anyhow!("ClawHub 响应解析失败: {}", e))?;

    let items = body
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out: Vec<ClawhubSkillSummary> = items
        .iter()
        .filter_map(normalize_search_skill_from_library_item)
        .collect();
    out.sort_by(|a, b| b.stars.cmp(&a.stars).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

fn fetch_library_candidates(
    client: &reqwest::blocking::Client,
    query: &str,
    limit: u32,
) -> Result<Vec<ClawhubSkillSummary>> {
    let base = clawhub_base_url();
    let url = format!(
        "{}/api/v1/skills?limit={}&sort=downloads&nonSuspicious=true&q={}",
        base,
        200,
        urlencoding::encode(query)
    );
    let resp = client
        .get(url)
        .send()
        .map_err(|e| anyhow!("ClawHub 列表请求失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(anyhow!("ClawHub 列表加载失败: HTTP {}", resp.status()));
    }
    let body: Value = resp
        .json()
        .map_err(|e| anyhow!("ClawHub 列表解析失败: {}", e))?;
    let items = body
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let query_variants = build_query_variants(query);
    let mut scored: Vec<(f64, usize, ClawhubSkillSummary)> = items
        .iter()
        .filter_map(normalize_library_item)
        .map(|item| {
            let as_skill = ClawhubSkillSummary {
                name: item.name.clone(),
                slug: item.slug.clone(),
                description: item.summary.clone(),
                github_url: None,
                source_url: Some(format!("https://www.clawhub.ai/skills/{}", item.slug)),
                stars: item.stars,
            };
            let (score, hits) = query_variants
                .iter()
                .map(|q| calc_recommendation_score(q, &as_skill))
                .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or((0.0, 0));
            (score, hits, as_skill)
        })
        .collect();
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.2.stars.cmp(&a.2.stars))
            .then_with(|| a.2.name.cmp(&b.2.name))
    });

    let mut out: Vec<ClawhubSkillSummary> = scored
        .into_iter()
        .filter(|(_, hits, _)| *hits > 0)
        .map(|(_, _, s)| s)
        .collect();
    out.truncate(limit as usize);
    Ok(out)
}

fn search_skills(query: &str, page: u32, limit: u32) -> Result<Vec<ClawhubSkillSummary>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| anyhow!("创建 HTTP 客户端失败: {}", e))?;

    let mut merged: Vec<ClawhubSkillSummary> = Vec::new();
    for variant in build_query_variants(query) {
        let mut part = fetch_search_once(&client, &variant, page, limit)?;
        for item in part.drain(..) {
            if !merged.iter().any(|s| s.slug == item.slug) {
                merged.push(item);
            }
        }
        if merged.len() >= limit as usize {
            break;
        }
    }

    if merged.is_empty() {
        merged = fetch_library_candidates(&client, query, limit)?;
    }
    merged.sort_by(|a, b| b.stars.cmp(&a.stars).then_with(|| a.name.cmp(&b.name)));
    merged.truncate(limit as usize);
    Ok(merged)
}

impl Tool for ClawhubSearchTool {
    fn name(&self) -> &str {
        "clawhub_search"
    }

    fn description(&self) -> &str {
        "在 ClawHub 技能社区检索可安装技能。返回技能名、slug、描述、星标和仓库链接。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "检索关键词（例如：xhs、docx、飞书、多角色）"
                },
                "page": {
                    "type": "integer",
                    "description": "页码，默认 1",
                    "default": 1
                },
                "limit": {
                    "type": "integer",
                    "description": "返回数量，1-20，默认 10",
                    "default": 10
                }
            },
            "required": ["query"]
        })
    }

    fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let query = input["query"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 query 参数"))?
            .trim();
        if query.is_empty() {
            return Err(anyhow!("query 不能为空"));
        }
        let page = input["page"].as_u64().unwrap_or(1).clamp(1, 100) as u32;
        let limit = input["limit"].as_u64().unwrap_or(10).clamp(1, 20) as u32;
        let skills = search_skills(query, page, limit)?;

        let payload = json!({
            "source": "clawhub",
            "query": query,
            "items": skills.iter().map(|s| json!({
                "name": s.name,
                "slug": s.slug,
                "description": s.description,
                "stars": s.stars,
                "github_url": s.github_url,
                "source_url": s.source_url
            })).collect::<Vec<_>>()
        });
        let rendered =
            serde_json::to_string_pretty(&payload).map_err(|e| anyhow!("序列化结果失败: {}", e))?;
        Ok(truncate_tool_output(&rendered, OUTPUT_MAX_CHARS))
    }
}

impl Tool for ClawhubRecommendTool {
    fn name(&self) -> &str {
        "clawhub_recommend"
    }

    fn description(&self) -> &str {
        "基于关键词从 ClawHub 推荐最相关可安装技能。返回排序后的候选、匹配分和推荐理由。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "需求描述（越具体越好）"
                },
                "limit": {
                    "type": "integer",
                    "description": "推荐数量，1-10，默认 5",
                    "default": 5
                }
            },
            "required": ["query"]
        })
    }

    fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let query = input["query"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 query 参数"))?
            .trim();
        if query.is_empty() {
            return Err(anyhow!("query 不能为空"));
        }
        let limit = input["limit"].as_u64().unwrap_or(5).clamp(1, 10) as usize;
        let raw = search_skills(query, 1, 30)?;

        let mut recs: Vec<ClawhubSkillRecommendation> = raw
            .iter()
            .map(|skill| {
                let (score, hits) = calc_recommendation_score(query, skill);
                ClawhubSkillRecommendation {
                    slug: skill.slug.clone(),
                    name: skill.name.clone(),
                    description: skill.description.clone(),
                    stars: skill.stars,
                    score,
                    reason: build_recommend_reason(hits, skill.stars),
                    github_url: skill.github_url.clone(),
                    source_url: skill.source_url.clone(),
                }
            })
            .collect();
        recs.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.stars.cmp(&a.stars))
                .then_with(|| a.name.cmp(&b.name))
        });
        recs.truncate(limit);

        let payload = json!({
            "source": "clawhub",
            "query": query,
            "items": recs
        });
        let rendered =
            serde_json::to_string_pretty(&payload).map_err(|e| anyhow!("序列化结果失败: {}", e))?;
        Ok(truncate_tool_output(&rendered, OUTPUT_MAX_CHARS))
    }
}
