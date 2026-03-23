use reqwest::Client;
use serde_json::Value;

use super::repo::fetch_skillhub_catalog_body;
use super::support::{
    build_recommend_reason, calc_recommendation_score, clawhub_base_url, normalize_library_item,
    normalize_search_skill_from_library_item, normalize_skillhub_search_candidates,
};
use super::types::{ClawhubLibraryResponse, ClawhubSkillRecommendation, ClawhubSkillSummary};

pub async fn search_clawhub_skills(
    query: String,
    page: Option<u32>,
    limit: Option<u32>,
) -> Result<Vec<ClawhubSkillSummary>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    let client = Client::new();
    let mut out: Vec<ClawhubSkillSummary> = match fetch_skillhub_catalog_body(&client).await {
        Ok(body) => normalize_skillhub_search_candidates(&body)
            .into_iter()
            .filter(|item| {
                let haystack = format!(
                    "{} {} {}",
                    item.slug.to_ascii_lowercase(),
                    item.name.to_ascii_lowercase(),
                    item.description.to_ascii_lowercase()
                );
                haystack.contains(&q.to_ascii_lowercase())
            })
            .collect(),
        Err(_) => {
            let body = fetch_library_body(
                &client,
                None,
                limit.unwrap_or(20),
                if page.unwrap_or(1) > 1 {
                    "updated"
                } else {
                    "downloads"
                },
                Some(q),
            )
            .await?;
            body.get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(normalize_search_skill_from_library_item)
                .collect()
        }
    };
    out.sort_by(|a, b| b.stars.cmp(&a.stars).then_with(|| a.name.cmp(&b.name)));
    out.truncate(limit.unwrap_or(20) as usize);
    Ok(out)
}

pub async fn recommend_clawhub_skills(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<ClawhubSkillRecommendation>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    let client = Client::new();
    let candidates: Vec<ClawhubSkillSummary> = match fetch_skillhub_catalog_body(&client).await {
        Ok(body) => normalize_skillhub_search_candidates(&body),
        Err(_) => {
            let body = fetch_library_body(
                &client,
                None,
                limit.unwrap_or(20).max(5).min(50),
                "downloads",
                Some(q),
            )
            .await?;
            body.get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(normalize_search_skill_from_library_item)
                .collect()
        }
    };

    let mut recs: Vec<ClawhubSkillRecommendation> = candidates
        .into_iter()
        .map(|skill| {
            let (score, hits) = calc_recommendation_score(q, &skill);
            ClawhubSkillRecommendation {
                slug: skill.slug,
                name: skill.name,
                description: skill.description,
                stars: skill.stars,
                score,
                reason: build_recommend_reason(hits, skill.stars),
                github_url: skill.github_url,
                source_url: skill.source_url,
            }
        })
        .filter(|rec| rec.score >= 8.0)
        .collect();

    recs.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.stars.cmp(&a.stars))
            .then_with(|| a.name.cmp(&b.name))
    });

    let out_limit = limit.unwrap_or(5).max(1).min(10) as usize;
    recs.truncate(out_limit);
    Ok(recs)
}

pub(crate) async fn fetch_library_body(
    client: &Client,
    cursor: Option<&str>,
    limit: u32,
    sort: &str,
    query: Option<&str>,
) -> Result<Value, String> {
    let base = clawhub_base_url();
    let mut url = format!(
        "{}/api/v1/skills?limit={}&sort={}&nonSuspicious=true",
        base,
        limit,
        urlencoding::encode(sort)
    );
    if let Some(q) = query.map(str::trim).filter(|value| !value.is_empty()) {
        url.push_str("&q=");
        url.push_str(&urlencoding::encode(q));
    }
    if let Some(c) = cursor.filter(|s| !s.trim().is_empty()) {
        url.push_str("&cursor=");
        url.push_str(&urlencoding::encode(c));
    }

    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("ClawHub 列表加载失败: HTTP {}", resp.status()));
    }
    resp.json().await.map_err(|e| e.to_string())
}

pub(crate) fn normalize_library_response(body: &Value) -> ClawhubLibraryResponse {
    let items = body
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .or_else(|| body.as_array().cloned())
        .unwrap_or_default();

    let next_cursor = body
        .get("nextCursor")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            body.get("next_cursor")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

    ClawhubLibraryResponse {
        items: items.iter().filter_map(normalize_library_item).collect(),
        next_cursor,
        last_synced_at: None,
    }
}
