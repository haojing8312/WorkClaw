use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

use crate::commands::skills::{DbState, ImportResult};
use crate::runtime_environment::runtime_paths_from_app;

mod detail_service;
mod download_service;
mod install_service;
mod repo;
mod search_service;
mod support;
mod translation_service;
mod types;

pub(crate) use detail_service::resolve_repo_url;
pub(crate) use download_service::{
    build_github_archive_urls, download_skill_bytes_with_fallback, extract_skill_md_from_zip_bytes,
    extract_zip_to_dir, find_skill_root,
};
pub use download_service::{
    download_github_skill_repo_to_dir, download_github_skill_repo_to_workspace,
};
pub use repo::{list_clawhub_library_with_pool, sync_skillhub_catalog_with_pool};
pub(crate) use search_service::{fetch_library_body, normalize_library_response};
pub(crate) use support::*;
pub use types::{
    ClawhubLibraryItem, ClawhubLibraryResponse, ClawhubSkillDetail, ClawhubSkillRecommendation,
    ClawhubSkillSummary, ClawhubUpdateStatus, DiscoveredSkillDir, GithubRepoDownloadResult,
    GithubRepoInstallResult, GithubRepoSkippedImport, SkillhubCatalogSyncStatus,
};

const SKILLHUB_INDEX_SYNC_TTL_SECONDS: i64 = 6 * 60 * 60;
const CLAWHUB_LIBRARY_CACHE_TTL_SECONDS: i64 = 10 * 60;
const CLAWHUB_DETAIL_CACHE_TTL_SECONDS: i64 = 24 * 60 * 60;

static CLAWHUB_REFRESH_INFLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
fn clawhub_refresh_inflight() -> &'static Mutex<HashSet<String>> {
    CLAWHUB_REFRESH_INFLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

fn spawn_refresh_if_needed<F>(cache_key: String, task: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    tauri::async_runtime::spawn(async move {
        let guard = clawhub_refresh_inflight();
        {
            let mut inflight = guard.lock().await;
            if !inflight.insert(cache_key.clone()) {
                return;
            }
        }

        task.await;

        let mut inflight = guard.lock().await;
        inflight.remove(&cache_key);
    });
}

fn default_market_skill_base_dir(app: &AppHandle) -> PathBuf {
    if let Ok(runtime_paths) = runtime_paths_from_app(app) {
        return runtime_paths.market_skills_dir;
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".workclaw").join("market-skills")
}

fn default_workspace_import_base_dir(app: &AppHandle, workspace: Option<&str>) -> PathBuf {
    if let Some(dir) = workspace
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        return dir.join(".workclaw-imports");
    }
    default_market_skill_base_dir(app)
}

pub fn workspace_import_base_dir(workspace: &str) -> PathBuf {
    PathBuf::from(workspace.trim()).join(".workclaw-imports")
}

fn sha256_hex(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[tauri::command]
pub async fn search_clawhub_skills(
    query: String,
    page: Option<u32>,
    limit: Option<u32>,
) -> Result<Vec<ClawhubSkillSummary>, String> {
    search_service::search_clawhub_skills(query, page, limit).await
}

#[tauri::command]
pub async fn recommend_clawhub_skills(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<ClawhubSkillRecommendation>, String> {
    search_service::recommend_clawhub_skills(query, limit).await
}

#[tauri::command]
pub async fn list_clawhub_library(
    cursor: Option<String>,
    limit: Option<u32>,
    sort: Option<String>,
    db: State<'_, DbState>,
) -> Result<ClawhubLibraryResponse, String> {
    list_clawhub_library_with_pool(&db.0, cursor, limit, sort).await
}

#[tauri::command]
pub async fn sync_skillhub_catalog(
    force: Option<bool>,
    db: State<'_, DbState>,
) -> Result<SkillhubCatalogSyncStatus, String> {
    sync_skillhub_catalog_with_pool(&db.0, force.unwrap_or(false)).await
}

pub async fn get_clawhub_skill_detail_with_pool(
    pool: &SqlitePool,
    slug: String,
) -> Result<ClawhubSkillDetail, String> {
    detail_service::get_clawhub_skill_detail_with_pool(pool, slug).await
}

#[tauri::command]
pub async fn get_clawhub_skill_detail(
    slug: String,
    db: State<'_, DbState>,
) -> Result<ClawhubSkillDetail, String> {
    get_clawhub_skill_detail_with_pool(&db.0, slug).await
}

pub async fn translate_texts_with_preferences_with_pool(
    pool: &SqlitePool,
    texts: Vec<String>,
) -> Result<Vec<String>, String> {
    translation_service::translate_texts_with_preferences_with_pool(pool, texts).await
}

#[tauri::command]
pub async fn translate_texts_with_preferences(
    texts: Vec<String>,
    scene: Option<String>,
    db: State<'_, DbState>,
) -> Result<Vec<String>, String> {
    let _ = scene;
    translate_texts_with_preferences_with_pool(&db.0, texts).await
}

#[tauri::command]
pub async fn translate_clawhub_texts(
    texts: Vec<String>,
    db: State<'_, DbState>,
) -> Result<Vec<String>, String> {
    translate_texts_with_preferences_with_pool(&db.0, texts).await
}

#[tauri::command]
pub async fn install_clawhub_skill(
    slug: String,
    github_url: Option<String>,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<ImportResult, String> {
    install_service::install_clawhub_skill(slug, github_url, &db.0, &app).await
}

#[tauri::command]
pub async fn install_github_skill_repo(
    repo_url: String,
    repo_slug: String,
    workspace: Option<String>,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<GithubRepoInstallResult, String> {
    install_service::install_github_skill_repo(repo_url, repo_slug, workspace, &db.0, &app).await
}

#[tauri::command]
pub async fn check_clawhub_skill_update(
    skill_id: String,
    db: State<'_, DbState>,
) -> Result<ClawhubUpdateStatus, String> {
    install_service::check_clawhub_skill_update(skill_id, &db.0).await
}

#[tauri::command]
pub async fn update_clawhub_skill(
    skill_id: String,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<ImportResult, String> {
    install_service::update_clawhub_skill(skill_id, &db.0, &app).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_repo_url_from_detail_body_prefers_direct_and_falls_back_to_owner_handle() {
        let direct = serde_json::json!({
            "skill": {
                "slug": "video-maker",
                "github_url": "https://github.com/acme/video-maker"
            },
            "owner": { "handle": "ignored" }
        });
        assert_eq!(
            detail_service::extract_repo_url_from_detail_body(&direct),
            Some("https://github.com/acme/video-maker".to_string())
        );

        let inferred = serde_json::json!({
            "skill": { "slug": "self-improving-agent" },
            "owner": { "handle": "pskoett" }
        });
        assert_eq!(
            detail_service::extract_repo_url_from_detail_body(&inferred),
            Some("https://github.com/pskoett/self-improving-agent".to_string())
        );
    }

    #[test]
    fn normalize_skillhub_library_item_maps_skillhub_fields() {
        let item = serde_json::json!({
            "slug": "self-improving-agent",
            "name": "self-improving-agent",
            "description": "Captures learnings",
            "description_zh": "记录学习",
            "version": "3.0.1",
            "homepage": "https://clawhub.ai/skills/self-improving-agent",
            "downloads": 206819,
            "stars": 1975,
            "owner": "pskoett",
            "tags": ["automation", "latest"]
        });

        let normalized =
            normalize_skillhub_library_item(&item).expect("skillhub item should normalize");

        assert_eq!(normalized.slug, "self-improving-agent");
        assert_eq!(normalized.name, "self-improving-agent");
        assert_eq!(normalized.summary, "记录学习");
        assert_eq!(
            normalized.source_url.as_deref(),
            Some("https://clawhub.ai/skills/self-improving-agent")
        );
        assert_eq!(normalized.github_url, None);
        assert_eq!(normalized.downloads, 206819);
        assert_eq!(normalized.stars, 1975);
        assert_eq!(
            normalized.tags,
            vec!["automation".to_string(), "latest".to_string()]
        );
    }

    #[test]
    fn normalize_skillhub_search_summary_uses_owner_and_chinese_description() {
        let item = serde_json::json!({
            "slug": "self-improving-agent",
            "name": "self-improving-agent",
            "description": "Captures learnings",
            "description_zh": "记录学习",
            "homepage": "https://clawhub.ai/skills/self-improving-agent",
            "downloads": 206819,
            "stars": 1975,
            "owner": "pskoett",
            "tags": ["automation"]
        });

        let normalized =
            normalize_skillhub_search_skill(&item).expect("skillhub search item should normalize");

        assert_eq!(normalized.slug, "self-improving-agent");
        assert_eq!(normalized.name, "self-improving-agent");
        assert_eq!(normalized.description, "记录学习");
        assert_eq!(
            normalized.source_url.as_deref(),
            Some("https://clawhub.ai/skills/self-improving-agent")
        );
        assert_eq!(normalized.github_url, None);
        assert_eq!(normalized.stars, 1975);
    }
}
