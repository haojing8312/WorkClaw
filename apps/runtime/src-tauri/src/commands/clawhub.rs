use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};
use tauri::{AppHandle, Manager, State};
use walkdir::WalkDir;
use zip::ZipArchive;

use crate::commands::skills::{DbState, ImportResult};

const DEFAULT_CLAWHUB_BASE: &str = "https://www.clawhub.ai";

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

fn clawhub_base_url() -> String {
    std::env::var("CLAWHUB_API_BASE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CLAWHUB_BASE.to_string())
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
    Path::new(&home).join(".skillmint").join("market-skills")
}

fn normalize_skill(item: &Value) -> Option<ClawhubSkillSummary> {
    let name = item.get("name")?.as_str()?.to_string();
    let slug = item.get("slug")?.as_str()?.to_string();
    let description = item
        .get("description")
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
        .map(|s| s.to_string());
    let stars = item
        .get("stars")
        .or_else(|| item.get("github_stars"))
        .and_then(|v| v.as_i64())
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
        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or_default();
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

async fn resolve_repo_url(client: &Client, slug: &str, github_url: Option<String>) -> Result<String, String> {
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

fn parse_google_translate_text(body: &Value) -> Option<String> {
    let arr = body.as_array()?;
    let segments = arr.first()?.as_array()?;
    let mut out = String::new();
    for seg in segments {
        if let Some(piece) = seg.get(0).and_then(|v| v.as_str()) {
            out.push_str(piece);
        }
    }
    if out.trim().is_empty() { None } else { Some(out) }
}

async fn translate_text_to_zh(client: &Client, text: &str) -> Result<String, String> {
    let resp = client
        .get("https://translate.googleapis.com/translate_a/single")
        .query(&[
            ("client", "gtx"),
            ("sl", "auto"),
            ("tl", "zh-CN"),
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

fn find_skill_root(extract_dir: &Path) -> Option<PathBuf> {
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
            return entry.path().parent().map(|p| p.to_path_buf());
        }
    }
    None
}

async fn fetch_skill_detail_url(client: &Client, slug: &str) -> Result<Option<String>, String> {
    let base = clawhub_base_url();
    let query_url = format!(
        "{}/api/v1/skill?slug={}",
        base,
        urlencoding::encode(slug)
    );
    if let Ok(resp) = client.get(&query_url).send().await {
        if resp.status().is_success() {
            let body: Value = resp.json().await.map_err(|e| e.to_string())?;
            let direct = body
                .get("github_url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let nested = body
                .get("skill")
                .and_then(|s| s.get("github_url"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if direct.is_some() || nested.is_some() {
                return Ok(direct.or(nested));
            }
        }
    }

    let path_url = format!("{}/api/v1/skill/{}", base, urlencoding::encode(slug));
    let resp = client.get(&path_url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
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

async fn check_missing_mcp(pool: &SqlitePool, cfg: &crate::agent::skill_config::SkillConfig) -> Result<Vec<String>, String> {
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

    let base = clawhub_base_url();
    let url = format!(
        "{}/api/v1/search?query={}&page={}&limit={}",
        base,
        urlencoding::encode(q),
        page.unwrap_or(1),
        limit.unwrap_or(20)
    );
    let client = Client::new();
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("ClawHub 搜索失败: HTTP {}", resp.status()));
    }
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;

    let items = body
        .get("results")
        .and_then(|v| v.as_array())
        .or_else(|| body.get("skills").and_then(|v| v.as_array()))
        .cloned()
        .or_else(|| body.as_array().cloned())
        .unwrap_or_default();

    let mut out: Vec<ClawhubSkillSummary> = items
        .iter()
        .filter_map(normalize_skill)
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

    let base = clawhub_base_url();
    let url = format!(
        "{}/api/v1/search?query={}&page=1&limit={}",
        base,
        urlencoding::encode(q),
        limit.unwrap_or(20).max(5).min(50)
    );
    let client = Client::new();
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("推荐检索失败: HTTP {}", resp.status()));
    }
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    let items = body
        .get("results")
        .and_then(|v| v.as_array())
        .or_else(|| body.get("skills").and_then(|v| v.as_array()))
        .cloned()
        .or_else(|| body.as_array().cloned())
        .unwrap_or_default();

    let mut recs: Vec<ClawhubSkillRecommendation> = items
        .iter()
        .filter_map(normalize_skill)
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
        .collect();

    recs.sort_by(|a, b| b
        .score
        .partial_cmp(&a.score)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| b.stars.cmp(&a.stars))
        .then_with(|| a.name.cmp(&b.name)));

    let out_limit = limit.unwrap_or(5).max(1).min(10) as usize;
    recs.truncate(out_limit);
    Ok(recs)
}

#[tauri::command]
pub async fn list_clawhub_library(
    cursor: Option<String>,
    limit: Option<u32>,
    sort: Option<String>,
) -> Result<ClawhubLibraryResponse, String> {
    let base = clawhub_base_url();
    let mut url = format!(
        "{}/api/v1/skills?limit={}&sort={}",
        base,
        limit.unwrap_or(20).max(1).min(100),
        urlencoding::encode(sort.as_deref().unwrap_or("updated"))
    );
    if let Some(c) = cursor.as_deref().filter(|s| !s.trim().is_empty()) {
        url.push_str("&cursor=");
        url.push_str(&urlencoding::encode(c));
    }

    let client = Client::new();
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("ClawHub 列表加载失败: HTTP {}", resp.status()));
    }
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
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
        .or_else(|| body.get("next_cursor").and_then(|v| v.as_str()).map(|s| s.to_string()));

    Ok(ClawhubLibraryResponse {
        items: items.iter().filter_map(normalize_library_item).collect(),
        next_cursor,
    })
}

#[tauri::command]
pub async fn translate_clawhub_texts(
    texts: Vec<String>,
    db: State<'_, DbState>,
) -> Result<Vec<String>, String> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }

    let client = Client::new();
    let mut out = Vec::with_capacity(texts.len());
    for source in texts {
        let clean = source.trim().to_string();
        if clean.is_empty() {
            out.push(String::new());
            continue;
        }
        if is_mostly_cjk(&clean) {
            out.push(clean);
            continue;
        }

        let cache_key = format!("zh-CN:{}", sha256_hex(&clean));
        let cached: Option<(String,)> = sqlx::query_as(
            "SELECT translated_text FROM skill_i18n_cache WHERE cache_key = ?"
        )
        .bind(&cache_key)
        .fetch_optional(&db.0)
        .await
        .map_err(|e| e.to_string())?;
        if let Some((translated,)) = cached {
            out.push(translated);
            continue;
        }

        let translated = match translate_text_to_zh(&client, &clean).await {
            Ok(v) if !v.trim().is_empty() => v,
            _ => clean.clone(),
        };
        let now = Utc::now().to_rfc3339();
        let _ = sqlx::query(
            "INSERT OR REPLACE INTO skill_i18n_cache (cache_key, source_text, translated_text, updated_at) VALUES (?, ?, ?, ?)"
        )
        .bind(&cache_key)
        .bind(&clean)
        .bind(&translated)
        .bind(&now)
        .execute(&db.0)
        .await;
        out.push(translated);
    }

    Ok(out)
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

    let skill_root = find_skill_root(&extract_dir)
        .ok_or_else(|| "未在下载包中找到 SKILL.md".to_string())?;
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
    Ok(ImportResult { manifest, missing_mcp })
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
        "SELECT pack_path, COALESCE(source_type, 'encrypted') FROM installed_skills WHERE id = ?"
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
