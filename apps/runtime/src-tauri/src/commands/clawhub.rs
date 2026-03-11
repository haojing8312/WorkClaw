use chrono::Utc;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::future::Future;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;
use tauri::{AppHandle, Manager, State};
use tokio::sync::Mutex;
use walkdir::WalkDir;
use zip::ZipArchive;

use crate::agent::types::LLMResponse;
use crate::commands::runtime_preferences::get_runtime_preferences_with_pool;
use crate::commands::skills::{
    ensure_skill_display_name_available, import_local_skill_to_pool, DbState, ImportResult,
};

const DEFAULT_CLAWHUB_BASE: &str = "https://www.clawhub.ai";
const CLAWHUB_LIBRARY_CACHE_TTL_SECONDS: i64 = 10 * 60;
const CLAWHUB_DETAIL_CACHE_TTL_SECONDS: i64 = 24 * 60 * 60;
const CLAWHUB_FIRST_CURSOR: &str = "__first__";

static CLAWHUB_REFRESH_INFLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawhubSkillSummary {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub github_url: Option<String>,
    pub source_url: Option<String>,
    #[serde(default)]
    pub stars: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawhubLibraryItem {
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub stars: i64,
    pub downloads: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawhubLibraryResponse {
    pub items: Vec<ClawhubLibraryItem>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawhubSkillDetail {
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub author: Option<String>,
    pub github_url: Option<String>,
    pub source_url: Option<String>,
    pub updated_at: Option<String>,
    pub stars: i64,
    pub downloads: i64,
    pub tags: Vec<String>,
    pub readme: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawhubSkillRecommendation {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub stars: i64,
    pub score: f64,
    pub reason: String,
    pub github_url: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClawhubUpdateStatus {
    pub skill_id: String,
    pub has_update: bool,
    pub local_hash: String,
    pub remote_hash: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredSkillDir {
    pub name: String,
    pub dir_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubRepoSkippedImport {
    pub dir_path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubRepoInstallResult {
    pub repo_dir: String,
    pub detected_skills: Vec<DiscoveredSkillDir>,
    pub imported_manifests: Vec<skillpack_rs::SkillManifest>,
    pub skipped: Vec<GithubRepoSkippedImport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubRepoDownloadResult {
    pub repo_dir: String,
    pub detected_skills: Vec<DiscoveredSkillDir>,
}

fn clawhub_base_url() -> String {
    std::env::var("CLAWHUB_API_BASE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CLAWHUB_BASE.to_string())
}

fn clawhub_refresh_inflight() -> &'static Mutex<HashSet<String>> {
    CLAWHUB_REFRESH_INFLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

fn normalize_library_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(20).max(1).min(100)
}

fn normalize_library_sort(sort: Option<&str>) -> String {
    let clean = sort.unwrap_or("updated").trim();
    if clean.is_empty() {
        "updated".to_string()
    } else {
        clean.to_string()
    }
}

fn build_library_cache_key(cursor: Option<&str>, limit: u32, sort: &str) -> String {
    let normalized_cursor = cursor
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .unwrap_or(CLAWHUB_FIRST_CURSOR);
    format!(
        "clawhub:library:v1:sort={}:limit={}:cursor={}",
        sort, limit, normalized_cursor
    )
}

fn build_detail_cache_key(slug: &str) -> String {
    format!("clawhub:detail:v1:slug={}", slug.trim())
}

async fn load_cached_http_body(
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

async fn upsert_http_cache_body(
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

fn sanitize_slug_stable(name: &str) -> String {
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

fn default_market_skill_base_dir(app: &AppHandle) -> PathBuf {
    if let Ok(dir) = app.path().app_data_dir() {
        return dir.join("market-skills");
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
        tags,
        stars,
        downloads,
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

fn normalize_skill_detail(raw: &Value, fallback_slug: &str) -> Option<ClawhubSkillDetail> {
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

fn sanitize_zip_entry_path(name: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for comp in Path::new(name).components() {
        match comp {
            Component::Normal(seg) => out.push(seg),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

fn sha256_hex(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn extract_skill_md_from_zip_bytes(bytes: &[u8]) -> Result<String, String> {
    let reader = Cursor::new(bytes);
    let mut archive = ZipArchive::new(reader).map_err(|e| format!("解压失败: {}", e))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let path = Path::new(entry.name());
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if file_name.eq_ignore_ascii_case("SKILL.md") {
            let mut content = String::new();
            std::io::Read::read_to_string(&mut entry, &mut content)
                .map_err(|e| format!("读取远端 SKILL.md 失败: {}", e))?;
            return Ok(content);
        }
    }
    Err("下载包中未找到 SKILL.md".to_string())
}

async fn download_skill_zip_bytes(client: &Client, repo_url: &str) -> Result<Vec<u8>, String> {
    let base = clawhub_base_url();
    let download_url = format!(
        "{}/api/v1/download?url={}",
        base,
        urlencoding::encode(repo_url.trim())
    );
    let resp = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("下载失败: HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err("下载内容为空".to_string());
    }
    Ok(bytes.to_vec())
}

async fn resolve_repo_url(
    client: &Client,
    slug: &str,
    github_url: Option<String>,
) -> Result<String, String> {
    if let Some(url) = github_url.filter(|u| !u.trim().is_empty()) {
        return Ok(url);
    }
    fetch_skill_detail_url(client, slug)
        .await?
        .ok_or_else(|| "无法从 ClawHub 获取技能仓库地址".to_string())
}

fn is_mostly_cjk(text: &str) -> bool {
    let mut total = 0usize;
    let mut cjk = 0usize;
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        total += 1;
        if ('\u{4E00}'..='\u{9FFF}').contains(&ch)
            || ('\u{3400}'..='\u{4DBF}').contains(&ch)
            || ('\u{F900}'..='\u{FAFF}').contains(&ch)
        {
            cjk += 1;
        }
    }
    total > 0 && cjk * 100 / total >= 60
}

fn normalize_target_language(raw: &str) -> String {
    let normalized = raw.trim();
    if normalized.is_empty() {
        "zh-CN".to_string()
    } else {
        normalized.to_string()
    }
}

fn should_skip_translation(text: &str, target_lang: &str) -> bool {
    if target_lang.eq_ignore_ascii_case("zh-CN")
        || target_lang.to_ascii_lowercase().starts_with("zh")
    {
        return is_mostly_cjk(text);
    }
    false
}

fn parse_google_translate_text(body: &Value) -> Option<String> {
    let arr = body.as_array()?;
    let segments = arr.first()?.as_array()?;
    let mut out = String::new();
    for seg in segments {
        if let Some(piece) = seg.get(0).and_then(|v| v.as_str()) {
            out.push_str(piece);
        }
    }
    if out.trim().is_empty() {
        None
    } else {
        Some(out)
    }
}

async fn translate_text_via_google(
    client: &Client,
    text: &str,
    target_lang: &str,
) -> Result<String, String> {
    let resp = client
        .get("https://translate.googleapis.com/translate_a/single")
        .query(&[
            ("client", "gtx"),
            ("sl", "auto"),
            ("tl", target_lang),
            ("dt", "t"),
            ("q", text),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("翻译服务返回 HTTP {}", resp.status()));
    }
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    parse_google_translate_text(&body).ok_or_else(|| "翻译结果解析失败".to_string())
}

#[derive(Debug, Clone)]
struct TranslationModelConfig {
    api_format: String,
    base_url: String,
    model_name: String,
    api_key: String,
}

impl TranslationModelConfig {
    fn cache_key(&self) -> String {
        format!(
            "model:{}:{}",
            self.api_format.trim().to_ascii_lowercase(),
            self.model_name.trim()
        )
    }
}

async fn load_translation_model(
    pool: &SqlitePool,
    preferred_model_id: &str,
) -> Option<TranslationModelConfig> {
    let preferred = if !preferred_model_id.trim().is_empty() {
        sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT api_format, base_url, model_name, api_key
             FROM model_configs
             WHERE id = ? AND TRIM(api_key) != ''
             LIMIT 1",
        )
        .bind(preferred_model_id.trim())
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    let primary = if preferred.is_none() {
        sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT api_format, base_url, model_name, api_key
             FROM model_configs
             WHERE is_default = 1 AND TRIM(api_key) != ''
             ORDER BY id ASC LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    let fallback = if preferred.is_none() && primary.is_none() {
        sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT api_format, base_url, model_name, api_key
             FROM model_configs
             WHERE TRIM(api_key) != ''
             ORDER BY id ASC LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    preferred
        .or(primary)
        .or(fallback)
        .map(
            |(api_format, base_url, model_name, api_key)| TranslationModelConfig {
                api_format,
                base_url,
                model_name,
                api_key,
            },
        )
}

async fn translate_text_via_model(
    model: &TranslationModelConfig,
    text: &str,
    target_lang: &str,
) -> Result<String, String> {
    let user_prompt = format!(
        "Translate the following text into {target_lang}. Return translation only, no explanation.\n\n{text}"
    );
    let messages = vec![serde_json::json!({
        "role": "user",
        "content": user_prompt
    })];
    let response = match model.api_format.trim().to_ascii_lowercase().as_str() {
        "anthropic" => crate::adapters::anthropic::chat_stream_with_tools(
            &model.base_url,
            &model.api_key,
            &model.model_name,
            "You are a professional translation assistant.",
            messages,
            vec![],
            |_| {},
        )
        .await
        .map_err(|e| e.to_string())?,
        "openai" => crate::adapters::openai::chat_stream_with_tools(
            &model.base_url,
            &model.api_key,
            &model.model_name,
            "You are a professional translation assistant.",
            messages,
            vec![],
            |_| {},
        )
        .await
        .map_err(|e| e.to_string())?,
        _ => {
            return Err("当前默认模型协议不支持翻译".to_string());
        }
    };

    let translated = match response {
        LLMResponse::Text(v) => v,
        LLMResponse::TextWithToolCalls(v, _) => v,
        LLMResponse::ToolCalls(_) => String::new(),
    };
    let clean = translated.trim().to_string();
    if clean.is_empty() {
        Err("模型翻译结果为空".to_string())
    } else {
        Ok(clean)
    }
}

fn find_skill_roots(extract_dir: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for entry in WalkDir::new(extract_dir).min_depth(1).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry
            .path()
            .file_name()
            .and_then(|f| f.to_str())
            .map(|n| n.eq_ignore_ascii_case("SKILL.md"))
            .unwrap_or(false)
        {
            if let Some(parent) = entry.path().parent() {
                roots.push(parent.to_path_buf());
            }
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

fn find_skill_root(extract_dir: &Path) -> Option<PathBuf> {
    find_skill_roots(extract_dir).into_iter().next()
}

fn describe_skill_dir(dir_path: &Path) -> DiscoveredSkillDir {
    let name = std::fs::read_to_string(dir_path.join("SKILL.md"))
        .ok()
        .and_then(|content| crate::agent::skill_config::SkillConfig::parse(&content).name)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            dir_path
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "unnamed-skill".to_string());
    DiscoveredSkillDir {
        name,
        dir_path: dir_path.to_string_lossy().to_string(),
    }
}

fn build_github_repo_key(repo_url: &str, repo_slug: &str) -> String {
    sanitize_slug_stable(if repo_slug.trim().is_empty() {
        repo_url
            .rsplit('/')
            .next()
            .unwrap_or("github-skill")
            .trim_end_matches(".git")
    } else {
        repo_slug.trim()
    })
}

fn extract_github_repo_archive(
    bytes: &[u8],
    base_dir: &Path,
    repo_key: &str,
) -> Result<GithubRepoDownloadResult, String> {
    std::fs::create_dir_all(base_dir).map_err(|e| e.to_string())?;
    let extract_dir = base_dir.join(format!("{}-{}", repo_key, Utc::now().timestamp_millis()));
    std::fs::create_dir_all(&extract_dir).map_err(|e| e.to_string())?;
    extract_zip_to_dir(bytes, &extract_dir)?;

    let roots = find_skill_roots(&extract_dir);
    if roots.is_empty() {
        return Err("未在 GitHub 仓库中发现可导入的 SKILL.md".to_string());
    }

    Ok(GithubRepoDownloadResult {
        repo_dir: extract_dir.to_string_lossy().to_string(),
        detected_skills: roots.iter().map(|dir| describe_skill_dir(dir)).collect(),
    })
}

pub async fn download_github_skill_repo_to_workspace(
    app: &AppHandle,
    repo_url: &str,
    repo_slug: &str,
    workspace: Option<&str>,
) -> Result<GithubRepoDownloadResult, String> {
    let clean_repo_url = repo_url.trim().to_string();
    if clean_repo_url.is_empty() {
        return Err("repo_url 不能为空".to_string());
    }

    let repo_key = build_github_repo_key(&clean_repo_url, repo_slug);
    let client = Client::new();
    let bytes = download_skill_zip_bytes(&client, &clean_repo_url).await?;
    let base_dir = default_workspace_import_base_dir(app, workspace);
    extract_github_repo_archive(&bytes, &base_dir, &repo_key)
}

pub async fn download_github_skill_repo_to_dir(
    repo_url: &str,
    repo_slug: &str,
    base_dir: &Path,
) -> Result<GithubRepoDownloadResult, String> {
    let clean_repo_url = repo_url.trim().to_string();
    if clean_repo_url.is_empty() {
        return Err("repo_url 不能为空".to_string());
    }

    let repo_key = build_github_repo_key(&clean_repo_url, repo_slug);
    let client = Client::new();
    let bytes = download_skill_zip_bytes(&client, &clean_repo_url).await?;
    extract_github_repo_archive(&bytes, base_dir, &repo_key)
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
    let direct = body
        .get("github_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let nested = body
        .get("skill")
        .and_then(|s| s.get("github_url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(direct.or(nested))
}

async fn check_missing_mcp(
    pool: &SqlitePool,
    cfg: &crate::agent::skill_config::SkillConfig,
) -> Result<Vec<String>, String> {
    let mut missing = Vec::new();
    for dep in &cfg.mcp_servers {
        let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM mcp_servers WHERE name = ?")
            .bind(&dep.name)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

        if exists.is_none() {
            missing.push(dep.name.clone());
        }
    }
    Ok(missing)
}

#[tauri::command]
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

#[tauri::command]
pub async fn recommend_clawhub_skills(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<ClawhubSkillRecommendation>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    let client = Client::new();
    let body = fetch_library_body(
        &client,
        None,
        limit.unwrap_or(20).max(5).min(50),
        "downloads",
        Some(q),
    )
    .await?;
    let items = body
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut recs: Vec<ClawhubSkillRecommendation> = items
        .iter()
        .filter_map(normalize_search_skill_from_library_item)
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

async fn fetch_library_body(
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

fn normalize_library_response(body: &Value) -> ClawhubLibraryResponse {
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
    }
}

pub async fn list_clawhub_library_with_pool(
    pool: &SqlitePool,
    cursor: Option<String>,
    limit: Option<u32>,
    sort: Option<String>,
) -> Result<ClawhubLibraryResponse, String> {
    let normalized_limit = normalize_library_limit(limit);
    let normalized_sort = normalize_library_sort(sort.as_deref());
    let cursor_ref = cursor.as_deref();
    let cache_key = build_library_cache_key(cursor_ref, normalized_limit, &normalized_sort);

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
                spawn_refresh_if_needed(key_for_refresh.clone(), async move {
                    let client = Client::new();
                    if let Ok(fresh_json) = fetch_library_body(
                        &client,
                        cursor_for_refresh.as_deref(),
                        normalized_limit,
                        &sort_for_refresh,
                        None,
                    )
                    .await
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
            return Ok(cached_response);
        }
    }

    let client = Client::new();
    let body = fetch_library_body(
        &client,
        cursor_ref,
        normalized_limit,
        &normalized_sort,
        None,
    )
    .await?;
    let response = normalize_library_response(&body);
    let _ = upsert_http_cache_body(pool, &cache_key, &body.to_string()).await;
    Ok(response)
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
        load_cached_http_body(pool, &cache_key, CLAWHUB_DETAIL_CACHE_TTL_SECONDS).await?
    {
        if let Ok(cached_json) = serde_json::from_str::<Value>(&cached_body) {
            if let Some(cached_detail) = normalize_skill_detail(&cached_json, clean_slug) {
                if stale {
                    let key_for_refresh = cache_key.clone();
                    let slug_for_refresh = clean_slug.to_string();
                    let pool_for_refresh = pool.clone();
                    spawn_refresh_if_needed(key_for_refresh.clone(), async move {
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
    if texts.is_empty() {
        return Ok(Vec::new());
    }

    let prefs = get_runtime_preferences_with_pool(pool).await?;
    let target_lang = normalize_target_language(&prefs.default_language);
    if !prefs.immersive_translation_enabled {
        let mut passthrough = Vec::with_capacity(texts.len());
        for source in texts {
            passthrough.push(source.trim().to_string());
        }
        return Ok(passthrough);
    }

    let translation_engine = prefs.translation_engine.trim().to_ascii_lowercase();
    let allow_model = translation_engine != "free_only";
    let allow_free = translation_engine != "model_only";

    let client = Client::new();
    let model_cfg = if allow_model {
        load_translation_model(pool, &prefs.translation_model_id).await
    } else {
        None
    };
    let engine_cache_key = if let Some(cfg) = model_cfg.as_ref() {
        cfg.cache_key()
    } else if allow_free {
        "google-gtx".to_string()
    } else {
        "model-missing".to_string()
    };

    let mut out = Vec::with_capacity(texts.len());
    for source in texts {
        let clean = source.trim().to_string();
        if clean.is_empty() {
            out.push(String::new());
            continue;
        }
        if should_skip_translation(&clean, &target_lang) {
            out.push(clean);
            continue;
        }

        let cache_key = format!(
            "{}:{}:{}",
            target_lang,
            engine_cache_key,
            sha256_hex(&clean)
        );
        let cached: Option<(String,)> =
            sqlx::query_as("SELECT translated_text FROM skill_i18n_cache WHERE cache_key = ?")
                .bind(&cache_key)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;
        if let Some((translated,)) = cached {
            out.push(translated);
            continue;
        }

        let translated = if let Some(model) = model_cfg.as_ref() {
            match translate_text_via_model(model, &clean, &target_lang).await {
                Ok(v) if !v.trim().is_empty() => v,
                _ if allow_free => {
                    match translate_text_via_google(&client, &clean, &target_lang).await {
                        Ok(v) if !v.trim().is_empty() => v,
                        _ => clean.clone(),
                    }
                }
                _ => clean.clone(),
            }
        } else if allow_free {
            match translate_text_via_google(&client, &clean, &target_lang).await {
                Ok(v) if !v.trim().is_empty() => v,
                _ => clean.clone(),
            }
        } else {
            clean.clone()
        };
        let now = Utc::now().to_rfc3339();
        let _ = sqlx::query(
            "INSERT OR REPLACE INTO skill_i18n_cache (cache_key, source_text, translated_text, updated_at) VALUES (?, ?, ?, ?)"
        )
        .bind(&cache_key)
        .bind(&clean)
        .bind(&translated)
        .bind(&now)
        .execute(pool)
        .await;
        out.push(translated);
    }

    Ok(out)
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
    let clean_slug = sanitize_slug_stable(slug.trim());
    if clean_slug.is_empty() {
        return Err("slug 不能为空".to_string());
    }

    let client = Client::new();
    let repo_url = resolve_repo_url(&client, &clean_slug, github_url).await?;
    let bytes = download_skill_zip_bytes(&client, &repo_url).await?;

    let base_dir = default_market_skill_base_dir(&app);
    std::fs::create_dir_all(&base_dir).map_err(|e| e.to_string())?;
    let extract_dir = base_dir.join(format!("{}-{}", clean_slug, Utc::now().timestamp_millis()));
    std::fs::create_dir_all(&extract_dir).map_err(|e| e.to_string())?;
    extract_zip_to_dir(&bytes, &extract_dir)?;

    let skill_root =
        find_skill_root(&extract_dir).ok_or_else(|| "未在下载包中找到 SKILL.md".to_string())?;
    let skill_md_path = skill_root.join("SKILL.md");
    let content = std::fs::read_to_string(&skill_md_path)
        .map_err(|e| format!("读取 SKILL.md 失败: {}", e))?;
    let config = crate::agent::skill_config::SkillConfig::parse(&content);

    let name = config
        .name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| clean_slug.clone());
    let manifest = skillpack_rs::SkillManifest {
        id: format!("clawhub-{}", clean_slug),
        name,
        description: config.description.clone().unwrap_or_default(),
        version: "clawhub".to_string(),
        author: "ClawHub".to_string(),
        recommended_model: config.model.clone().unwrap_or_default(),
        tags: vec!["clawhub".to_string()],
        created_at: Utc::now(),
        username_hint: None,
        encrypted_verify: String::new(),
    };
    ensure_skill_display_name_available(&db.0, &manifest.name, &manifest.id).await?;

    let manifest_json = serde_json::to_string(&manifest).map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR REPLACE INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type) VALUES (?, ?, ?, ?, ?, 'local')",
    )
    .bind(&manifest.id)
    .bind(&manifest_json)
    .bind(&now)
    .bind("")
    .bind(skill_root.to_string_lossy().to_string())
    .execute(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    let missing_mcp = check_missing_mcp(&db.0, &config).await?;
    Ok(ImportResult {
        manifest,
        missing_mcp,
    })
}

#[tauri::command]
pub async fn install_github_skill_repo(
    repo_url: String,
    repo_slug: String,
    workspace: Option<String>,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<GithubRepoInstallResult, String> {
    let download =
        download_github_skill_repo_to_workspace(&app, &repo_url, &repo_slug, workspace.as_deref())
            .await?;

    let mut imported_manifests = Vec::new();
    let mut skipped = Vec::new();
    for skill in &download.detected_skills {
        let dir_path = skill.dir_path.clone();
        match import_local_skill_to_pool(dir_path.clone(), &db.0, &[]).await {
            Ok(result) => imported_manifests.push(result.manifest),
            Err(reason) => skipped.push(GithubRepoSkippedImport { dir_path, reason }),
        }
    }

    if imported_manifests.is_empty() {
        let reason = skipped
            .first()
            .map(|item| item.reason.clone())
            .unwrap_or_else(|| "未导入任何技能".to_string());
        return Err(reason);
    }

    Ok(GithubRepoInstallResult {
        repo_dir: download.repo_dir,
        detected_skills: download.detected_skills,
        imported_manifests,
        skipped,
    })
}

#[tauri::command]
pub async fn check_clawhub_skill_update(
    skill_id: String,
    db: State<'_, DbState>,
) -> Result<ClawhubUpdateStatus, String> {
    if !skill_id.starts_with("clawhub-") {
        return Err("仅支持检查 ClawHub 来源技能".to_string());
    }
    let slug = skill_id.trim_start_matches("clawhub-").to_string();
    if slug.trim().is_empty() {
        return Err("无效 skill_id".to_string());
    }

    let (pack_path, source_type): (String, String) = sqlx::query_as(
        "SELECT pack_path, COALESCE(source_type, 'encrypted') FROM installed_skills WHERE id = ?",
    )
    .bind(&skill_id)
    .fetch_one(&db.0)
    .await
    .map_err(|e| format!("Skill 不存在: {}", e))?;

    if source_type != "local" {
        return Err("该技能不是本地可更新类型".to_string());
    }

    let local_skill_path = Path::new(&pack_path).join("SKILL.md");
    let local_content = std::fs::read_to_string(&local_skill_path)
        .map_err(|e| format!("读取本地 SKILL.md 失败: {}", e))?;
    let local_hash = sha256_hex(&local_content);

    let client = Client::new();
    let repo_url = resolve_repo_url(&client, &slug, None).await?;
    let bytes = download_skill_zip_bytes(&client, &repo_url).await?;
    let remote_content = extract_skill_md_from_zip_bytes(&bytes)?;
    let remote_hash = sha256_hex(&remote_content);

    let has_update = local_hash != remote_hash;
    Ok(ClawhubUpdateStatus {
        skill_id,
        has_update,
        local_hash,
        remote_hash,
        message: if has_update {
            "发现远端更新，可执行更新".to_string()
        } else {
            "当前已是最新版本".to_string()
        },
    })
}

#[tauri::command]
pub async fn update_clawhub_skill(
    skill_id: String,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<ImportResult, String> {
    if !skill_id.starts_with("clawhub-") {
        return Err("仅支持更新 ClawHub 来源技能".to_string());
    }
    let slug = skill_id.trim_start_matches("clawhub-").to_string();
    install_clawhub_skill(slug, None, db, app).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use zip::write::FileOptions;

    fn build_skill_repo_zip() -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            let options = FileOptions::default();
            writer
                .add_directory("repo-main/skills/brainstorming/", options)
                .expect("add brainstorming dir");
            writer
                .start_file("repo-main/skills/brainstorming/SKILL.md", options)
                .expect("start brainstorming skill");
            use std::io::Write as _;
            writer
                .write_all(b"---\nname: brainstorming\n---\n")
                .expect("write brainstorming skill");
            writer
                .add_directory("repo-main/skills/debugging/", options)
                .expect("add debugging dir");
            writer
                .start_file("repo-main/skills/debugging/SKILL.md", options)
                .expect("start debugging skill");
            writer
                .write_all(b"---\nname: debugging\n---\n")
                .expect("write debugging skill");
            writer.finish().expect("finish zip");
        }
        cursor.into_inner()
    }

    #[test]
    fn find_skill_roots_returns_all_matching_skill_directories() {
        let tmp = tempdir().expect("tempdir");
        let root = tmp.path();
        std::fs::create_dir_all(root.join("repo-a/skills/brainstorming")).expect("mkdir first");
        std::fs::create_dir_all(root.join("repo-a/skills/debugging")).expect("mkdir second");
        std::fs::write(
            root.join("repo-a/skills/brainstorming/SKILL.md"),
            "---\nname: brainstorming\n---\n",
        )
        .expect("write first skill");
        std::fs::write(
            root.join("repo-a/skills/debugging/SKILL.md"),
            "---\nname: debugging\n---\n",
        )
        .expect("write second skill");

        let roots = find_skill_roots(root);

        assert_eq!(roots.len(), 2);
        assert!(roots.iter().any(|path| path.ends_with("brainstorming")));
        assert!(roots.iter().any(|path| path.ends_with("debugging")));
    }

    #[test]
    fn extract_github_repo_archive_returns_repo_dir_and_detected_skills() {
        let tmp = tempdir().expect("tempdir");
        let zip_bytes = build_skill_repo_zip();

        let result =
            extract_github_repo_archive(&zip_bytes, tmp.path(), "superpowers").expect("extract");

        assert!(result.repo_dir.contains("superpowers-"));
        assert_eq!(result.detected_skills.len(), 2);
        assert!(result
            .detected_skills
            .iter()
            .any(|skill| skill.name.eq_ignore_ascii_case("brainstorming")));
        assert!(result
            .detected_skills
            .iter()
            .any(|skill| skill.name.eq_ignore_ascii_case("debugging")));
    }

    #[test]
    fn build_github_repo_key_prefers_slug_and_strips_git_suffix_from_url() {
        assert_eq!(
            build_github_repo_key(
                "https://github.com/obra/superpowers.git",
                "obra/superpowers"
            ),
            "obra-superpowers"
        );
        assert_eq!(
            build_github_repo_key("https://github.com/obra/superpowers.git", ""),
            "superpowers"
        );
    }
}

fn extract_zip_to_dir(bytes: &[u8], extract_dir: &Path) -> Result<(), String> {
    let reader = Cursor::new(bytes);
    let mut archive = ZipArchive::new(reader).map_err(|e| format!("解压失败: {}", e))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel_path) = sanitize_zip_entry_path(entry.name()) else {
            continue;
        };
        let out_path = extract_dir.join(rel_path);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut outfile = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut outfile).map_err(|e| e.to_string())?;
    }
    Ok(())
}
