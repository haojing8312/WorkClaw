use chrono::Utc;
use reqwest::Client;
use sqlx::SqlitePool;
use std::path::Path;
use tauri::AppHandle;

use crate::commands::skills::{
    ensure_skill_display_name_available, import_local_skill_to_pool, ImportResult,
};

use super::types::{ClawhubUpdateStatus, GithubRepoInstallResult, GithubRepoSkippedImport};

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

fn parse_clawhub_skill_id(skill_id: &str) -> Result<String, String> {
    if !skill_id.starts_with("clawhub-") {
        return Err("仅支持检查 ClawHub 来源技能".to_string());
    }
    let slug = skill_id.trim_start_matches("clawhub-").to_string();
    if slug.trim().is_empty() {
        return Err("无效 skill_id".to_string());
    }
    Ok(slug)
}

pub async fn install_clawhub_skill(
    slug: String,
    github_url: Option<String>,
    pool: &SqlitePool,
    app: &AppHandle,
) -> Result<ImportResult, String> {
    let clean_slug = super::sanitize_slug_stable(slug.trim());
    if clean_slug.is_empty() {
        return Err("slug 不能为空".to_string());
    }

    let client = Client::new();
    let bytes = super::download_skill_bytes_with_fallback(&client, &clean_slug, github_url).await?;

    let base_dir = super::default_market_skill_base_dir(app);
    std::fs::create_dir_all(&base_dir).map_err(|e| e.to_string())?;
    let extract_dir = base_dir.join(format!("{}-{}", clean_slug, Utc::now().timestamp_millis()));
    std::fs::create_dir_all(&extract_dir).map_err(|e| e.to_string())?;
    super::extract_zip_to_dir(&bytes, &extract_dir)?;

    let skill_root = super::find_skill_root(&extract_dir)
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
    ensure_skill_display_name_available(pool, &manifest.name, &manifest.id).await?;

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
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    let missing_mcp = check_missing_mcp(pool, &config).await?;
    Ok(ImportResult {
        manifest,
        missing_mcp,
    })
}

pub async fn install_github_skill_repo(
    repo_url: String,
    repo_slug: String,
    workspace: Option<String>,
    pool: &SqlitePool,
    app: &AppHandle,
) -> Result<GithubRepoInstallResult, String> {
    let download = super::download_github_skill_repo_to_workspace(
        app,
        &repo_url,
        &repo_slug,
        workspace.as_deref(),
    )
    .await?;

    let mut imported_manifests = Vec::new();
    let mut skipped = Vec::new();
    for skill in &download.detected_skills {
        let dir_path = skill.dir_path.clone();
        match import_local_skill_to_pool(dir_path.clone(), pool, &[]).await {
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

pub async fn check_clawhub_skill_update(
    skill_id: String,
    pool: &SqlitePool,
) -> Result<ClawhubUpdateStatus, String> {
    let slug = parse_clawhub_skill_id(&skill_id)?;

    let (pack_path, source_type): (String, String) = sqlx::query_as(
        "SELECT pack_path, COALESCE(source_type, 'encrypted') FROM installed_skills WHERE id = ?",
    )
    .bind(&skill_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Skill 不存在: {}", e))?;

    if source_type != "local" {
        return Err("该技能不是本地可更新类型".to_string());
    }

    let local_skill_path = Path::new(&pack_path).join("SKILL.md");
    let local_content = std::fs::read_to_string(&local_skill_path)
        .map_err(|e| format!("读取本地 SKILL.md 失败: {}", e))?;
    let local_hash = super::sha256_hex(&local_content);

    let client = Client::new();
    let bytes = super::download_skill_bytes_with_fallback(&client, &slug, None).await?;
    let remote_content = super::extract_skill_md_from_zip_bytes(&bytes)?;
    let remote_hash = super::sha256_hex(&remote_content);

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

pub async fn update_clawhub_skill(
    skill_id: String,
    pool: &SqlitePool,
    app: &AppHandle,
) -> Result<ImportResult, String> {
    let slug = parse_clawhub_skill_id(&skill_id)?;
    install_clawhub_skill(slug, None, pool, app).await
}

#[cfg(test)]
mod tests {
    use super::parse_clawhub_skill_id;

    #[test]
    fn parse_clawhub_skill_id_accepts_valid_skill_id() {
        assert_eq!(
            parse_clawhub_skill_id("clawhub-self-improving-agent").expect("valid skill id"),
            "self-improving-agent".to_string()
        );
    }

    #[test]
    fn parse_clawhub_skill_id_rejects_invalid_prefix() {
        let error = parse_clawhub_skill_id("github-self-improving-agent").expect_err("invalid");
        assert_eq!(error, "仅支持检查 ClawHub 来源技能");
    }
}
