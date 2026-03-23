use reqwest::{Client, StatusCode};
use serde_json::Value;
use sqlx::SqlitePool;

use super::repo::{load_cached_http_body, upsert_http_cache_body};
use super::support::{build_detail_cache_key, clawhub_base_url, normalize_skill_detail};
use super::types::ClawhubSkillDetail;

pub async fn get_clawhub_skill_detail_with_pool(
    pool: &SqlitePool,
    slug: String,
) -> Result<ClawhubSkillDetail, String> {
    let clean_slug = slug.trim();
    if clean_slug.is_empty() {
        return Err("slug 不能为空".to_string());
    }

    let cache_key = build_detail_cache_key(clean_slug);
    if let Some((cached_body, stale)) =
        load_cached_http_body(pool, &cache_key, super::CLAWHUB_DETAIL_CACHE_TTL_SECONDS).await?
    {
        if let Ok(cached_json) = serde_json::from_str::<Value>(&cached_body) {
            if let Some(cached_detail) = normalize_skill_detail(&cached_json, clean_slug) {
                if stale {
                    let key_for_refresh = cache_key.clone();
                    let slug_for_refresh = clean_slug.to_string();
                    let pool_for_refresh = pool.clone();
                    super::spawn_refresh_if_needed(key_for_refresh.clone(), async move {
                        let client = Client::new();
                        if let Ok(fresh_json) =
                            fetch_skill_detail_body(&client, &slug_for_refresh).await
                        {
                            let _ = upsert_http_cache_body(
                                &pool_for_refresh,
                                &key_for_refresh,
                                &fresh_json.to_string(),
                            )
                            .await;
                        }
                    });
                }
                return Ok(cached_detail);
            }
        }
    }

    let client = Client::new();
    let body = fetch_skill_detail_body(&client, clean_slug).await?;
    let detail =
        normalize_skill_detail(&body, clean_slug).ok_or_else(|| "解析技能详情失败".to_string())?;
    let _ = upsert_http_cache_body(pool, &cache_key, &body.to_string()).await;
    Ok(detail)
}

pub(crate) async fn resolve_repo_url(
    client: &Client,
    slug: &str,
    github_url: Option<String>,
) -> Result<String, String> {
    if let Some(url) = github_url
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
    {
        if super::build_github_archive_urls(&url).is_some() {
            return Ok(url);
        }
    }
    fetch_skill_detail_url(client, slug)
        .await?
        .ok_or_else(|| "无法从 ClawHub 获取技能仓库地址".to_string())
}

async fn fetch_skill_detail_candidate(client: &Client, url: &str) -> Result<Option<Value>, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("ClawHub 详情加载失败: {}", e))?;
    if resp.status().is_success() {
        return resp
            .json::<Value>()
            .await
            .map(Some)
            .map_err(|e| e.to_string());
    }
    if resp.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    Err(format!("ClawHub 详情加载失败: HTTP {}", resp.status()))
}

async fn fetch_skill_detail_from_search(
    client: &Client,
    slug: &str,
) -> Result<Option<Value>, String> {
    let base = clawhub_base_url();
    let search_url = format!(
        "{}/api/v1/search?query={}&page=1&limit=20",
        base,
        urlencoding::encode(slug.trim())
    );
    let resp = match client.get(&search_url).send().await {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    if !resp.status().is_success() {
        return Ok(None);
    }

    let body: Value = match resp.json().await {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let items = body
        .get("results")
        .and_then(|v| v.as_array())
        .or_else(|| body.get("skills").and_then(|v| v.as_array()))
        .cloned()
        .or_else(|| body.as_array().cloned())
        .unwrap_or_default();
    let target_slug = slug.trim().to_ascii_lowercase();
    let matched = items
        .iter()
        .find(|entry| {
            entry
                .get("slug")
                .and_then(|v| v.as_str())
                .map(|value| value.eq_ignore_ascii_case(slug.trim()))
                .unwrap_or(false)
        })
        .cloned()
        .or_else(|| {
            items
                .iter()
                .find(|entry| {
                    entry
                        .get("name")
                        .or_else(|| entry.get("displayName"))
                        .and_then(|v| v.as_str())
                        .map(|name| name.to_ascii_lowercase().contains(&target_slug))
                        .unwrap_or(false)
                })
                .cloned()
        });

    Ok(matched.map(|item| serde_json::json!({ "skill": item })))
}

async fn fetch_skill_detail_body(client: &Client, slug: &str) -> Result<Value, String> {
    let base = clawhub_base_url();
    let endpoints = [
        format!("{}/api/v1/skill?slug={}", base, urlencoding::encode(slug)),
        format!("{}/api/v1/skill/{}", base, urlencoding::encode(slug)),
        format!("{}/api/v1/skills/{}", base, urlencoding::encode(slug)),
    ];
    let mut found_not_found = false;

    for endpoint in endpoints {
        match fetch_skill_detail_candidate(client, &endpoint).await {
            Ok(Some(body)) => return Ok(body),
            Ok(None) => found_not_found = true,
            Err(error) => return Err(error),
        }
    }

    if let Some(fallback) = fetch_skill_detail_from_search(client, slug).await? {
        return Ok(fallback);
    }

    if found_not_found {
        return Err("ClawHub 详情加载失败: HTTP 404 Not Found".to_string());
    }
    Err("ClawHub 详情加载失败".to_string())
}

async fn fetch_skill_detail_url(client: &Client, slug: &str) -> Result<Option<String>, String> {
    let body = fetch_skill_detail_body(client, slug).await?;
    Ok(extract_repo_url_from_detail_body(&body))
}

pub(crate) fn extract_repo_url_from_detail_body(body: &Value) -> Option<String> {
    let payload = body.get("skill").unwrap_or(body);
    let direct = payload
        .get("github_url")
        .and_then(|v| v.as_str())
        .or_else(|| body.get("github_url").and_then(|v| v.as_str()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if direct.is_some() {
        return direct;
    }

    let slug = payload
        .get("slug")
        .and_then(|v| v.as_str())
        .or_else(|| body.get("slug").and_then(|v| v.as_str()))
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    let owner = body
        .get("owner")
        .and_then(|o| o.get("handle"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            payload
                .get("owner")
                .and_then(|o| o.get("handle"))
                .and_then(|v| v.as_str())
        })
        .map(str::trim)
        .filter(|s| !s.is_empty())?;

    Some(format!("https://github.com/{}/{}", owner, slug))
}
