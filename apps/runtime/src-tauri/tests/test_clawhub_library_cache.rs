mod helpers;

use chrono::{Duration, Utc};
use runtime_lib::commands::clawhub::{
    get_clawhub_skill_detail_with_pool, list_clawhub_library_with_pool,
};
use serde_json::json;

const LIST_CACHE_KEY: &str = "clawhub:library:v1:sort=downloads:limit=20:cursor=__first__";
const DETAIL_CACHE_KEY: &str = "clawhub:detail:v1:slug=video-maker";

async fn seed_cache_row(pool: &sqlx::SqlitePool, key: &str, body: &str, fetched_at: &str) {
    sqlx::query(
        "INSERT OR REPLACE INTO clawhub_http_cache (cache_key, body, fetched_at) VALUES (?, ?, ?)",
    )
    .bind(key)
    .bind(body)
    .bind(fetched_at)
    .execute(pool)
    .await
    .expect("seed cache row");
}

#[tokio::test]
async fn list_uses_fresh_cache_when_network_unavailable() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    std::env::set_var("CLAWHUB_API_BASE", "http://127.0.0.1:1");
    std::env::set_var("SKILLHUB_CATALOG_URL", "http://127.0.0.1:1/skills.json");

    let body = json!({
        "items": [
            {
                "slug": "video-maker",
                "displayName": "Video Maker",
                "summary": "Generate short videos",
                "tags": { "video": true, "creator": true },
                "stats": { "stars": 12, "downloads": 99 }
            }
        ],
        "nextCursor": null
    });
    seed_cache_row(
        &pool,
        LIST_CACHE_KEY,
        &body.to_string(),
        &Utc::now().to_rfc3339(),
    )
    .await;

    let response =
        list_clawhub_library_with_pool(&pool, None, Some(20), Some("downloads".to_string()))
            .await
            .expect("list from cache");

    assert_eq!(response.items.len(), 1);
    assert_eq!(response.items[0].slug, "video-maker");
    assert_eq!(response.items[0].downloads, 99);

    std::env::remove_var("SKILLHUB_CATALOG_URL");
}

#[tokio::test]
async fn detail_falls_back_to_stale_cache_when_network_unavailable() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    std::env::set_var("CLAWHUB_API_BASE", "http://127.0.0.1:1");

    let stale_time = (Utc::now() - Duration::hours(36)).to_rfc3339();
    let detail = json!({
        "skill": {
            "slug": "video-maker",
            "displayName": "Video Maker",
            "summary": "Generate short videos",
            "description": "Create videos from scripts",
            "tags": { "video": true },
            "stats": { "stars": 100, "downloads": 5000 }
        }
    });
    seed_cache_row(&pool, DETAIL_CACHE_KEY, &detail.to_string(), &stale_time).await;

    let response = get_clawhub_skill_detail_with_pool(&pool, "video-maker".to_string())
        .await
        .expect("detail from stale cache");

    assert_eq!(response.slug, "video-maker");
    assert_eq!(response.downloads, 5000);
    assert_eq!(response.description, "Create videos from scripts");
}

#[tokio::test]
async fn list_reads_local_skillhub_index_in_popularity_order() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let synced_at = Utc::now().to_rfc3339();

    for (slug, name, downloads, stars) in [
        ("mid-download", "Mid Download", 120, 20),
        ("top-download", "Top Download", 300, 5),
        ("tie-break", "A Name", 120, 30),
    ] {
        sqlx::query(
            "INSERT INTO skillhub_catalog_index (
                slug, name, summary, description, github_url, source_url, tags_json, stars, downloads, updated_at, synced_at
            ) VALUES (?, ?, '', '', NULL, NULL, '[]', ?, ?, NULL, ?)",
        )
        .bind(slug)
        .bind(name)
        .bind(stars)
        .bind(downloads)
        .bind(&synced_at)
        .execute(&pool)
        .await
        .expect("insert skillhub index row");
    }

    let response =
        list_clawhub_library_with_pool(&pool, None, Some(20), Some("downloads".to_string()))
            .await
            .expect("list from local index");

    let ordered: Vec<&str> = response.items.iter().map(|item| item.slug.as_str()).collect();
    assert_eq!(ordered, vec!["top-download", "tie-break", "mid-download"]);
    assert_eq!(response.last_synced_at.as_deref(), Some(synced_at.as_str()));
}
