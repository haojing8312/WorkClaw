use chrono::Utc;
use serde_json::Value;
use sqlx::SqlitePool;

use super::support::{
    is_sync_timestamp_stale, normalize_skillhub_catalog_index_rows,
    normalize_skillhub_library_response, skillhub_catalog_url,
};
use super::types::{ClawhubLibraryItem, ClawhubLibraryResponse, SkillhubCatalogSyncStatus};
use super::{
    fetch_library_body, normalize_library_response, CLAWHUB_LIBRARY_CACHE_TTL_SECONDS,
    SKILLHUB_INDEX_SYNC_TTL_SECONDS,
};

pub(crate) async fn load_cached_http_body(
    pool: &SqlitePool,
    cache_key: &str,
    ttl_seconds: i64,
) -> Result<Option<(String, bool)>, String> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT body, fetched_at FROM clawhub_http_cache WHERE cache_key = ?")
            .bind(cache_key)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

    let Some((body, fetched_at)) = row else {
        return Ok(None);
    };

    let stale = chrono::DateTime::parse_from_rfc3339(&fetched_at)
        .map(|dt| {
            let age = Utc::now().signed_duration_since(dt.with_timezone(&Utc));
            age.num_seconds() > ttl_seconds
        })
        .unwrap_or(true);

    Ok(Some((body, stale)))
}

pub(crate) async fn upsert_http_cache_body(
    pool: &SqlitePool,
    cache_key: &str,
    body: &str,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR REPLACE INTO clawhub_http_cache (cache_key, body, fetched_at) VALUES (?, ?, ?)",
    )
    .bind(cache_key)
    .bind(body)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) async fn fetch_skillhub_catalog_body(client: &reqwest::Client) -> Result<Value, String> {
    let resp = client
        .get(skillhub_catalog_url())
        .send()
        .await
        .map_err(|e| format!("SkillHub 列表加载失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("SkillHub 列表加载失败: HTTP {}", resp.status()));
    }
    resp.json().await.map_err(|e| e.to_string())
}

async fn read_skillhub_index_last_synced_at(pool: &SqlitePool) -> Result<Option<String>, String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT synced_at FROM skillhub_catalog_index ORDER BY synced_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(row.map(|(synced_at,)| synced_at))
}

async fn count_skillhub_index_rows(pool: &SqlitePool) -> Result<i64, String> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM skillhub_catalog_index")
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(count)
}

async fn read_local_skillhub_library_page(
    pool: &SqlitePool,
    cursor: Option<&str>,
    limit: u32,
) -> Result<Option<ClawhubLibraryResponse>, String> {
    let total = count_skillhub_index_rows(pool).await?;
    if total <= 0 {
        return Ok(None);
    }

    let offset = super::support::parse_skillhub_cursor(cursor) as i64;
    let rows: Vec<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        i64,
        i64,
    )> = sqlx::query_as(
        "SELECT slug, name, summary, github_url, source_url, tags_json, stars, downloads
         FROM skillhub_catalog_index
         ORDER BY downloads DESC, stars DESC, name ASC
         LIMIT ? OFFSET ?",
    )
    .bind(limit as i64)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let items = rows
        .into_iter()
        .map(
            |(slug, name, summary, github_url, source_url, tags_json, stars, downloads)| {
                ClawhubLibraryItem {
                    slug,
                    name,
                    summary,
                    github_url,
                    source_url,
                    tags: serde_json::from_str::<Vec<String>>(&tags_json).unwrap_or_default(),
                    stars,
                    downloads,
                }
            },
        )
        .collect::<Vec<_>>();

    let item_count = items.len() as i64;
    let next_cursor = if offset + item_count < total {
        Some((offset + item_count).to_string())
    } else {
        None
    };

    Ok(Some(ClawhubLibraryResponse {
        items,
        next_cursor,
        last_synced_at: read_skillhub_index_last_synced_at(pool).await?,
    }))
}

fn spawn_skillhub_sync_if_needed(pool: SqlitePool, force: bool) {
    super::spawn_refresh_if_needed("skillhub:index:sync".to_string(), async move {
        let _ = sync_skillhub_catalog_with_pool(&pool, force).await;
    });
}

pub async fn sync_skillhub_catalog_with_pool(
    pool: &SqlitePool,
    force: bool,
) -> Result<SkillhubCatalogSyncStatus, String> {
    let existing_total = count_skillhub_index_rows(pool).await?;
    let last_synced_at = read_skillhub_index_last_synced_at(pool).await?;
    let stale = last_synced_at
        .as_deref()
        .map(|raw| is_sync_timestamp_stale(raw, SKILLHUB_INDEX_SYNC_TTL_SECONDS))
        .unwrap_or(true);
    if !force && existing_total > 0 && !stale {
        return Ok(SkillhubCatalogSyncStatus {
            total_skills: existing_total as usize,
            last_synced_at,
            refreshed: false,
        });
    }

    let client = reqwest::Client::new();
    let body = fetch_skillhub_catalog_body(&client).await?;
    let synced_at = Utc::now().to_rfc3339();
    let rows = normalize_skillhub_catalog_index_rows(&body, &synced_at);

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM skillhub_catalog_index")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    for row in &rows {
        sqlx::query(
            "INSERT INTO skillhub_catalog_index (
                slug, name, summary, description, github_url, source_url, tags_json, stars, downloads, updated_at, synced_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&row.slug)
        .bind(&row.name)
        .bind(&row.summary)
        .bind(&row.description)
        .bind(&row.github_url)
        .bind(&row.source_url)
        .bind(&row.tags_json)
        .bind(row.stars)
        .bind(row.downloads)
        .bind(&row.updated_at)
        .bind(&row.synced_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(SkillhubCatalogSyncStatus {
        total_skills: rows.len(),
        last_synced_at: Some(synced_at),
        refreshed: true,
    })
}

pub async fn list_clawhub_library_with_pool(
    pool: &SqlitePool,
    cursor: Option<String>,
    limit: Option<u32>,
    sort: Option<String>,
) -> Result<ClawhubLibraryResponse, String> {
    let normalized_limit = super::support::normalize_library_limit(limit);
    let normalized_sort = super::support::normalize_library_sort(sort.as_deref());
    let cursor_ref = cursor.as_deref();
    let local_last_synced_at = read_skillhub_index_last_synced_at(pool).await?;
    if let Some(response) =
        read_local_skillhub_library_page(pool, cursor_ref, normalized_limit).await?
    {
        if local_last_synced_at
            .as_deref()
            .map(|raw| is_sync_timestamp_stale(raw, SKILLHUB_INDEX_SYNC_TTL_SECONDS))
            .unwrap_or(true)
        {
            spawn_skillhub_sync_if_needed(pool.clone(), false);
        }
        return Ok(response);
    }

    if sync_skillhub_catalog_with_pool(pool, false).await.is_ok() {
        if let Some(response) =
            read_local_skillhub_library_page(pool, cursor_ref, normalized_limit).await?
        {
            return Ok(response);
        }
    }

    let cache_key =
        super::support::build_library_cache_key(cursor_ref, normalized_limit, &normalized_sort);

    if let Some((cached_body, stale)) =
        load_cached_http_body(pool, &cache_key, CLAWHUB_LIBRARY_CACHE_TTL_SECONDS).await?
    {
        if let Ok(cached_json) = serde_json::from_str::<Value>(&cached_body) {
            let cached_response = normalize_library_response(&cached_json);
            if stale {
                let key_for_refresh = cache_key.clone();
                let pool_for_refresh = pool.clone();
                let sort_for_refresh = normalized_sort.clone();
                let cursor_for_refresh = cursor.clone();
                super::spawn_refresh_if_needed(key_for_refresh.clone(), async move {
                    let client = reqwest::Client::new();
                    let fresh_json = match fetch_skillhub_catalog_body(&client).await {
                        Ok(body) => {
                            let synced_at = Utc::now().to_rfc3339();
                            serde_json::to_value(normalize_skillhub_library_response(
                                &body,
                                cursor_for_refresh.as_deref(),
                                normalized_limit,
                            ))
                            .ok()
                            .map(|mut value| {
                                value["last_synced_at"] = Value::String(synced_at);
                                value
                            })
                        }
                        Err(_) => fetch_library_body(
                            &client,
                            cursor_for_refresh.as_deref(),
                            normalized_limit,
                            &sort_for_refresh,
                            None,
                        )
                        .await
                        .ok(),
                    };
                    if let Some(fresh_json) = fresh_json {
                        let _ = upsert_http_cache_body(
                            &pool_for_refresh,
                            &key_for_refresh,
                            &fresh_json.to_string(),
                        )
                        .await;
                    }
                });
            }
            return Ok(cached_response);
        }
    }

    let client = reqwest::Client::new();
    let response = match fetch_skillhub_catalog_body(&client).await {
        Ok(body) => normalize_skillhub_library_response(&body, cursor_ref, normalized_limit),
        Err(_) => {
            let body = fetch_library_body(
                &client,
                cursor_ref,
                normalized_limit,
                &normalized_sort,
                None,
            )
            .await?;
            normalize_library_response(&body)
        }
    };
    let _ = upsert_http_cache_body(
        pool,
        &cache_key,
        &serde_json::to_string(&response).map_err(|e| e.to_string())?,
    )
    .await;
    Ok(response)
}
