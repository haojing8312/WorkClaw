use chrono::Utc;
use serde_json::Value;

use super::types::{
    ClawhubLibraryItem, ClawhubLibraryResponse, ClawhubSkillDetail, ClawhubSkillSummary,
    SkillhubCatalogIndexRow,
};

const DEFAULT_CLAWHUB_BASE: &str = "https://www.clawhub.ai";
const DEFAULT_SKILLHUB_CATALOG_URL: &str =
    "https://cloudcache.tencentcs.com/qcloud/tea/app/data/skills.2d46363b.json?max_age=31536000";
const DEFAULT_SKILLHUB_DOWNLOAD_BASE: &str = "https://lightmake.site";
const CLAWHUB_FIRST_CURSOR: &str = "__first__";

pub(crate) fn clawhub_base_url() -> String {
    std::env::var("CLAWHUB_API_BASE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CLAWHUB_BASE.to_string())
}

pub(crate) fn prefer_proxy_download_for_github_archives() -> bool {
    let base = clawhub_base_url();
    !base.eq_ignore_ascii_case(DEFAULT_CLAWHUB_BASE)
}

pub(crate) fn skillhub_catalog_url() -> String {
    std::env::var("SKILLHUB_CATALOG_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SKILLHUB_CATALOG_URL.to_string())
}

pub(crate) fn skillhub_download_base_url() -> String {
    std::env::var("SKILLHUB_DOWNLOAD_BASE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SKILLHUB_DOWNLOAD_BASE.to_string())
}

pub(crate) fn build_skillhub_download_url(slug: &str) -> String {
    format!(
        "{}/api/v1/download?slug={}",
        skillhub_download_base_url().trim_end_matches('/'),
        urlencoding::encode(slug.trim())
    )
}

pub(crate) fn normalize_library_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(20).max(1).min(100)
}

pub(crate) fn normalize_library_sort(sort: Option<&str>) -> String {
    let clean = sort.unwrap_or("updated").trim();
    if clean.is_empty() {
        "updated".to_string()
    } else {
        clean.to_string()
    }
}

pub(crate) fn build_library_cache_key(cursor: Option<&str>, limit: u32, sort: &str) -> String {
    let normalized_cursor = cursor
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .unwrap_or(CLAWHUB_FIRST_CURSOR);
    format!(
        "clawhub:library:v1:sort={}:limit={}:cursor={}",
        sort, limit, normalized_cursor
    )
}

pub(crate) fn parse_skillhub_cursor(cursor: Option<&str>) -> usize {
    cursor
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != CLAWHUB_FIRST_CURSOR)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
}

pub(crate) fn build_detail_cache_key(slug: &str) -> String {
    format!("clawhub:detail:v1:slug={}", slug.trim())
}

pub(crate) fn sanitize_slug_stable(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "skill".to_string()
    } else {
        trimmed
    }
}

pub(crate) fn normalize_skillhub_tags(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        })
        .unwrap_or_default()
}

pub(crate) fn skillhub_text_fallback(
    item: &Value,
    primary_key: &str,
    secondary_key: &str,
) -> String {
    item.get(primary_key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            item.get(secondary_key)
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn normalize_skillhub_library_item(item: &Value) -> Option<ClawhubLibraryItem> {
    let slug = item.get("slug")?.as_str()?.to_string();
    let name = item
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&slug)
        .to_string();
    let summary = skillhub_text_fallback(item, "description_zh", "description");
    let source_url = item
        .get("homepage")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string());
    let tags = normalize_skillhub_tags(item.get("tags").unwrap_or(&Value::Null));
    let stars = item.get("stars").and_then(|v| v.as_i64()).unwrap_or(0);
    let downloads = item.get("downloads").and_then(|v| v.as_i64()).unwrap_or(0);

    Some(ClawhubLibraryItem {
        slug,
        name,
        summary,
        github_url: None,
        source_url,
        tags,
        stars,
        downloads,
    })
}

pub(crate) fn normalize_skillhub_search_skill(item: &Value) -> Option<ClawhubSkillSummary> {
    let slug = item.get("slug")?.as_str()?.to_string();
    let name = item
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&slug)
        .to_string();
    let description = skillhub_text_fallback(item, "description_zh", "description");
    let source_url = item
        .get("homepage")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string());
    let stars = item.get("stars").and_then(|v| v.as_i64()).unwrap_or(0);

    Some(ClawhubSkillSummary {
        name,
        slug,
        description,
        github_url: None,
        source_url,
        stars,
    })
}

pub(crate) fn normalize_library_item(item: &Value) -> Option<ClawhubLibraryItem> {
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
    let github_url = item
        .get("github_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let source_url = item
        .get("source_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| Some(format!("{}/skills/{}", clawhub_base_url(), slug)));

    let tags = item
        .get("tags")
        .and_then(|v| v.as_object())
        .map(|m| m.keys().cloned().collect::<Vec<String>>())
        .unwrap_or_default();

    let stars = item
        .get("stats")
        .and_then(|s| s.get("stars"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let downloads = item
        .get("stats")
        .and_then(|s| s.get("downloads"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    Some(ClawhubLibraryItem {
        slug,
        name,
        summary,
        github_url,
        source_url,
        tags,
        stars,
        downloads,
    })
}

pub(crate) fn normalize_search_skill_from_library_item(
    item: &Value,
) -> Option<ClawhubSkillSummary> {
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

pub(crate) fn normalize_skillhub_library_response(
    body: &Value,
    cursor: Option<&str>,
    limit: u32,
) -> ClawhubLibraryResponse {
    let all_items = body
        .get("skills")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let offset = parse_skillhub_cursor(cursor);
    let limit = limit as usize;
    let items: Vec<ClawhubLibraryItem> = all_items
        .iter()
        .skip(offset)
        .take(limit)
        .filter_map(normalize_skillhub_library_item)
        .collect();
    let next_cursor = if offset + items.len() < all_items.len() {
        Some((offset + items.len()).to_string())
    } else {
        None
    };

    ClawhubLibraryResponse {
        items,
        next_cursor,
        last_synced_at: None,
    }
}

pub(crate) fn normalize_skillhub_search_candidates(body: &Value) -> Vec<ClawhubSkillSummary> {
    body.get("skills")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(normalize_skillhub_search_skill)
        .collect()
}

pub(crate) fn is_sync_timestamp_stale(raw: &str, ttl_seconds: i64) -> bool {
    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|dt| {
            let age = Utc::now().signed_duration_since(dt.with_timezone(&Utc));
            age.num_seconds() > ttl_seconds
        })
        .unwrap_or(true)
}

pub(crate) fn normalize_skillhub_catalog_index_rows(
    body: &Value,
    synced_at: &str,
) -> Vec<SkillhubCatalogIndexRow> {
    body.get("skills")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|item| {
            let normalized = normalize_skillhub_library_item(item)?;
            let description = skillhub_text_fallback(item, "description_zh", "description");
            let updated_at = item
                .get("updated_at")
                .and_then(|v| v.as_str())
                .or_else(|| item.get("updatedAt").and_then(|v| v.as_str()))
                .or_else(|| item.get("version").and_then(|v| v.as_str()))
                .map(|v| v.to_string());
            Some(SkillhubCatalogIndexRow {
                slug: normalized.slug,
                name: normalized.name,
                summary: normalized.summary,
                description,
                github_url: normalized.github_url,
                source_url: normalized.source_url,
                tags_json: serde_json::to_string(&normalized.tags)
                    .unwrap_or_else(|_| "[]".to_string()),
                stars: normalized.stars,
                downloads: normalized.downloads,
                updated_at,
                synced_at: synced_at.to_string(),
            })
        })
        .collect()
}

pub(crate) fn normalize_skill_detail(
    raw: &Value,
    fallback_slug: &str,
) -> Option<ClawhubSkillDetail> {
    let payload = raw.get("skill").unwrap_or(raw);
    let slug = payload
        .get("slug")
        .and_then(|v| v.as_str())
        .or_else(|| raw.get("slug").and_then(|v| v.as_str()))
        .unwrap_or(fallback_slug)
        .to_string();
    let name = payload
        .get("displayName")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("name").and_then(|v| v.as_str()))
        .or_else(|| raw.get("displayName").and_then(|v| v.as_str()))
        .or_else(|| raw.get("name").and_then(|v| v.as_str()))
        .unwrap_or(&slug)
        .to_string();
    let summary = payload
        .get("summary")
        .and_then(|v| v.as_str())
        .or_else(|| raw.get("summary").and_then(|v| v.as_str()))
        .unwrap_or_default()
        .to_string();
    let description = payload
        .get("description")
        .and_then(|v| v.as_str())
        .or_else(|| raw.get("description").and_then(|v| v.as_str()))
        .unwrap_or(summary.as_str())
        .to_string();
    let tags = payload
        .get("tags")
        .map(|v| {
            if let Some(obj) = v.as_object() {
                obj.keys().cloned().collect::<Vec<String>>()
            } else if let Some(arr) = v.as_array() {
                arr.iter()
                    .filter_map(|it| it.as_str().map(|s| s.to_string()))
                    .collect::<Vec<String>>()
            } else {
                Vec::new()
            }
        })
        .unwrap_or_default();
    let stars = payload
        .get("stats")
        .and_then(|s| s.get("stars"))
        .and_then(|v| v.as_i64())
        .or_else(|| {
            raw.get("stats")
                .and_then(|s| s.get("stars"))
                .and_then(|v| v.as_i64())
        })
        .or_else(|| payload.get("stars").and_then(|v| v.as_i64()))
        .unwrap_or(0);
    let downloads = payload
        .get("stats")
        .and_then(|s| s.get("downloads"))
        .and_then(|v| v.as_i64())
        .or_else(|| {
            raw.get("stats")
                .and_then(|s| s.get("downloads"))
                .and_then(|v| v.as_i64())
        })
        .unwrap_or(0);
    let author = payload
        .get("author")
        .and_then(|v| v.as_str())
        .or_else(|| {
            payload
                .get("owner")
                .and_then(|o| o.get("name"))
                .and_then(|v| v.as_str())
        })
        .or_else(|| raw.get("author").and_then(|v| v.as_str()))
        .map(|s| s.to_string());
    let github_url = payload
        .get("github_url")
        .and_then(|v| v.as_str())
        .or_else(|| raw.get("github_url").and_then(|v| v.as_str()))
        .map(|s| s.to_string());
    let source_url = payload
        .get("source_url")
        .and_then(|v| v.as_str())
        .or_else(|| raw.get("source_url").and_then(|v| v.as_str()))
        .map(|s| s.to_string());
    let updated_at = payload
        .get("updatedAt")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("updated_at").and_then(|v| v.as_str()))
        .or_else(|| raw.get("updatedAt").and_then(|v| v.as_str()))
        .or_else(|| raw.get("updated_at").and_then(|v| v.as_str()))
        .map(|s| s.to_string());
    let readme = payload
        .get("readme")
        .and_then(|v| v.as_str())
        .or_else(|| raw.get("readme").and_then(|v| v.as_str()))
        .map(|s| s.to_string());

    Some(ClawhubSkillDetail {
        slug,
        name,
        summary,
        description,
        author,
        github_url,
        source_url,
        updated_at,
        stars,
        downloads,
        tags,
        readme,
    })
}

pub(crate) fn tokenize_query(input: &str) -> Vec<String> {
    let mut out: Vec<String> = input
        .split(|c: char| !c.is_alphanumeric() && !('\u{4E00}'..='\u{9FFF}').contains(&c))
        .map(|s| s.trim().to_lowercase())
        .filter(|s| s.chars().count() >= 2)
        .collect();
    out.sort();
    out.dedup();
    out
}

pub(crate) fn calc_recommendation_score(query: &str, skill: &ClawhubSkillSummary) -> (f64, usize) {
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

pub(crate) fn build_recommend_reason(hits: usize, stars: i64) -> String {
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
