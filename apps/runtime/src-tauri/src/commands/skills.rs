use chrono::Utc;
use skillpack_rs::{verify_and_unpack, SkillManifest};
use sqlx::SqlitePool;
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use tauri::State;

pub struct DbState(pub SqlitePool);

/// 本地 Skill 导入结果，包含 manifest 和缺失的 MCP 服务器列表
#[derive(serde::Serialize)]
pub struct ImportResult {
    pub manifest: skillpack_rs::SkillManifest,
    pub missing_mcp: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct LocalSkillPreview {
    pub markdown: String,
    pub save_path: String,
}

#[derive(serde::Serialize, Clone)]
pub struct InstalledSkillSummary {
    pub id: String,
    pub name: String,
}

#[derive(serde::Serialize, Clone)]
pub struct IndustryInstallResult {
    pub pack_id: String,
    pub version: String,
    pub installed_skills: Vec<InstalledSkillSummary>,
    pub missing_mcp: Vec<String>,
}

#[derive(serde::Serialize, Clone)]
pub struct IndustryBundleUpdateCheck {
    pub pack_id: String,
    pub current_version: Option<String>,
    pub candidate_version: String,
    pub has_update: bool,
    pub message: String,
}

fn sanitize_slug(name: &str) -> String {
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
        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        format!("expert-skill-{:x}", hasher.finish())
    } else {
        trimmed
    }
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

fn build_local_skill_id(seed: &str, dir_path: &str) -> String {
    let slug = sanitize_slug_stable(seed);
    if slug != "skill" {
        return format!("local-{}", slug);
    }

    let mut hasher = DefaultHasher::new();
    dir_path.hash(&mut hasher);
    format!("local-skill-{:x}", hasher.finish())
}

fn default_skill_base_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".skillmint").join("skills")
}

fn normalize_skill_description(description: &str, when_to_use: &str) -> String {
    let trimmed = description.trim();
    let fallback = format!("Use when {}", when_to_use.trim());

    if trimmed.is_empty() {
        return fallback;
    }

    if trimmed.to_ascii_lowercase().starts_with("use when") {
        trimmed.to_string()
    } else {
        format!("Use when {}", trimmed)
    }
}

fn render_local_skill_markdown(name: &str, description: &str, when_to_use: &str) -> String {
    let normalized_description = normalize_skill_description(description, when_to_use);
    let overview = if description.trim().is_empty() {
        "该技能用于稳定处理特定任务流程。"
    } else {
        description.trim()
    };

    crate::builtin_skills::local_skill_template_markdown()
        .replace("{{SKILL_NAME}}", name)
        .replace("{{SKILL_DESCRIPTION}}", &normalized_description)
        .replace("{{SKILL_TITLE}}", name)
        .replace("{{SKILL_OVERVIEW}}", overview)
        .replace("{{SKILL_WHEN_TO_USE}}", when_to_use.trim())
}

fn read_skill_markdown_with_fallback(dir_path: &str) -> Result<String, String> {
    let dir = Path::new(dir_path);
    let upper = dir.join("SKILL.md");
    if upper.exists() {
        return std::fs::read_to_string(upper).map_err(|e| format!("无法读取 SKILL.md: {}", e));
    }
    let lower = dir.join("skill.md");
    if lower.exists() {
        return std::fs::read_to_string(lower).map_err(|e| format!("无法读取 skill.md: {}", e));
    }
    Err("无法读取 SKILL.md: 未找到 SKILL.md".to_string())
}

fn merge_tags(base: Vec<String>, extra: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in base.into_iter().chain(extra.iter().cloned()) {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_lowercase();
        if seen.insert(key) {
            out.push(trimmed.to_string());
        }
    }
    out
}

fn parse_semver(version: &str) -> Option<(u64, u64, u64)> {
    let parts: Vec<&str> = version.trim().split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let major = parts[0].parse::<u64>().ok()?;
    let minor = parts[1].parse::<u64>().ok()?;
    let patch = parts[2]
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap_or("0")
        .parse::<u64>()
        .ok()?;
    Some((major, minor, patch))
}

fn compare_semver(left: &str, right: &str) -> Ordering {
    match (parse_semver(left), parse_semver(right)) {
        (Some(a), Some(b)) => a.cmp(&b),
        _ => left.cmp(right),
    }
}

fn extract_tag_value(tags: &[String], prefix: &str) -> Option<String> {
    let p = prefix.to_ascii_lowercase();
    tags.iter().find_map(|tag| {
        let lower = tag.to_ascii_lowercase();
        if lower.starts_with(&p) {
            Some(tag[prefix.len()..].to_string())
        } else {
            None
        }
    })
}

fn normalize_display_name(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_lowercase()
}

pub async fn ensure_skill_display_name_available(
    pool: &SqlitePool,
    incoming_name: &str,
    incoming_id: &str,
) -> Result<(), String> {
    let target = normalize_display_name(incoming_name);
    if target.is_empty() {
        return Ok(());
    }

    let rows = sqlx::query_as::<_, (String,)>("SELECT manifest FROM installed_skills")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    for (manifest_json,) in rows {
        let Ok(manifest) = serde_json::from_str::<SkillManifest>(&manifest_json) else {
            continue;
        };
        if manifest.id == incoming_id {
            continue;
        }
        if normalize_display_name(&manifest.name) == target {
            let conflict_name = manifest.name.trim();
            return Err(format!("DUPLICATE_SKILL_NAME:{conflict_name}"));
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn render_local_skill_preview(
    name: String,
    description: String,
    when_to_use: String,
    target_dir: Option<String>,
) -> Result<LocalSkillPreview, String> {
    let preview_name = if name.trim().is_empty() {
        "expert-skill".to_string()
    } else {
        name.trim().to_string()
    };
    let preview_when = if when_to_use.trim().is_empty() {
        "需要在特定任务场景中提供稳定执行能力".to_string()
    } else {
        when_to_use.trim().to_string()
    };

    let base_dir = match target_dir
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        Some(dir) => PathBuf::from(dir),
        None => default_skill_base_dir(),
    };
    let save_path = base_dir.join(sanitize_slug(&preview_name));

    Ok(LocalSkillPreview {
        markdown: render_local_skill_markdown(&preview_name, description.trim(), &preview_when),
        save_path: save_path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn create_local_skill(
    name: String,
    description: String,
    when_to_use: String,
    target_dir: Option<String>,
) -> Result<String, String> {
    let clean_name = name.trim();
    let clean_when = when_to_use.trim();
    if clean_name.is_empty() {
        return Err("技能名称不能为空".to_string());
    }
    if clean_when.is_empty() {
        return Err("使用场景不能为空".to_string());
    }

    let slug = sanitize_slug(clean_name);
    let base_dir = match target_dir
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        Some(dir) => PathBuf::from(dir),
        None => default_skill_base_dir(),
    };

    let skill_dir = base_dir.join(&slug);
    if skill_dir.exists() {
        return Err(format!("技能目录已存在: {}", skill_dir.to_string_lossy()));
    }
    std::fs::create_dir_all(&skill_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let content = render_local_skill_markdown(clean_name, description.trim(), clean_when);

    let skill_md = skill_dir.join("SKILL.md");
    std::fs::write(&skill_md, content).map_err(|e| format!("写入 SKILL.md 失败: {}", e))?;

    Ok(skill_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn install_skill(
    pack_path: String,
    username: String,
    db: State<'_, DbState>,
) -> Result<SkillManifest, String> {
    let unpacked = verify_and_unpack(&pack_path, &username).map_err(|e| e.to_string())?;
    ensure_skill_display_name_available(&db.0, &unpacked.manifest.name, &unpacked.manifest.id)
        .await?;

    let manifest_json = serde_json::to_string(&unpacked.manifest).map_err(|e| e.to_string())?;

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR REPLACE INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type) VALUES (?, ?, ?, ?, ?, 'encrypted')"
    )
    .bind(&unpacked.manifest.id)
    .bind(&manifest_json)
    .bind(&now)
    .bind(&username)
    .bind(&pack_path)
    .execute(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(unpacked.manifest)
}

pub async fn import_local_skill_to_pool(
    dir_path: String,
    pool: &SqlitePool,
    extra_tags: &[String],
) -> Result<ImportResult, String> {
    let content = read_skill_markdown_with_fallback(&dir_path)?;

    // 解析 frontmatter
    let config = crate::agent::skill_config::SkillConfig::parse(&content);
    let parsed_tags = crate::commands::packaging::parse_skill_tags(&content);
    let merged_tags = merge_tags(parsed_tags, extra_tags);

    // 构造 manifest
    let name = config.name.clone().unwrap_or_else(|| {
        // 使用目录名作为 fallback
        std::path::Path::new(&dir_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed-skill".to_string())
    });
    let id_seed = std::path::Path::new(&dir_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| name.clone());
    let skill_id = build_local_skill_id(&id_seed, &dir_path);

    let manifest = SkillManifest {
        id: skill_id.clone(),
        name: name.clone(),
        description: config.description.unwrap_or_default(),
        version: "local".to_string(),
        author: String::new(),
        recommended_model: config.model.unwrap_or_default(),
        tags: merged_tags,
        created_at: Utc::now(),
        username_hint: None,
        encrypted_verify: String::new(),
    };
    ensure_skill_display_name_available(pool, &manifest.name, &skill_id).await?;

    let manifest_json = serde_json::to_string(&manifest).map_err(|e| e.to_string())?;

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR REPLACE INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type) VALUES (?, ?, ?, ?, ?, 'local')"
    )
    .bind(&skill_id)
    .bind(&manifest_json)
    .bind(&now)
    .bind("")  // 本地 Skill 无需 username
    .bind(&dir_path)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    // 检查 MCP 依赖：哪些声明的 MCP 服务器尚未在数据库中配置
    let mut missing_mcp = Vec::new();
    for dep in &config.mcp_servers {
        let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM mcp_servers WHERE name = ?")
            .bind(&dep.name)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

        if exists.is_none() {
            missing_mcp.push(dep.name.clone());
        }
    }

    Ok(ImportResult {
        manifest,
        missing_mcp,
    })
}

/// 导入本地 Skill 目录（读取 SKILL.md 解析 frontmatter）
#[tauri::command]
pub async fn import_local_skill(
    dir_path: String,
    db: State<'_, DbState>,
) -> Result<ImportResult, String> {
    import_local_skill_to_pool(dir_path, &db.0, &[]).await
}

/// 刷新本地 Skill（重新读取 SKILL.md 更新 manifest）
#[tauri::command]
pub async fn refresh_local_skill(
    skill_id: String,
    db: State<'_, DbState>,
) -> Result<SkillManifest, String> {
    // 从 DB 获取 pack_path（即目录路径）
    let (pack_path, source_type): (String, String) = sqlx::query_as(
        "SELECT pack_path, COALESCE(source_type, 'encrypted') FROM installed_skills WHERE id = ?",
    )
    .bind(&skill_id)
    .fetch_one(&db.0)
    .await
    .map_err(|e| format!("Skill 不存在 (skill_id={}): {}", skill_id, e))?;

    if source_type != "local" {
        return Err(format!("Skill {} 不是本地 Skill，无法刷新", skill_id));
    }

    // 重新读取 SKILL.md
    let content = read_skill_markdown_with_fallback(&pack_path)?;

    let config = crate::agent::skill_config::SkillConfig::parse(&content);
    let parsed_tags = crate::commands::packaging::parse_skill_tags(&content);

    let name = config.name.clone().unwrap_or_else(|| {
        std::path::Path::new(&pack_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed-skill".to_string())
    });

    let manifest = SkillManifest {
        id: skill_id.clone(),
        name,
        description: config.description.unwrap_or_default(),
        version: "local".to_string(),
        author: String::new(),
        recommended_model: config.model.unwrap_or_default(),
        tags: parsed_tags,
        created_at: Utc::now(),
        username_hint: None,
        encrypted_verify: String::new(),
    };

    let manifest_json = serde_json::to_string(&manifest).map_err(|e| e.to_string())?;

    // 更新 DB 中的 manifest
    sqlx::query("UPDATE installed_skills SET manifest = ? WHERE id = ?")
        .bind(&manifest_json)
        .bind(&skill_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    Ok(manifest)
}

pub async fn install_industry_bundle_to_pool(
    bundle_path: String,
    install_root: Option<String>,
    pool: &SqlitePool,
) -> Result<IndustryInstallResult, String> {
    let unpacked =
        crate::commands::packaging::unpack_industry_bundle_to_root(&bundle_path, install_root)?;

    let mut installed_skills = Vec::new();
    let mut missing_mcp_set = HashSet::new();
    for skill_dir in &unpacked.skill_dirs {
        let local_name = Path::new(skill_dir)
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or_default()
            .to_string();
        let slug = local_name
            .split_once("--")
            .map(|(_, right)| right.to_string())
            .unwrap_or(local_name.clone());
        let skill_meta = unpacked
            .manifest
            .skills
            .iter()
            .find(|item| item.slug == slug);

        let mut extra_tags = vec![
            format!("pack:{}", unpacked.manifest.pack_id),
            format!("pack-version:{}", unpacked.manifest.version),
        ];
        if !unpacked.manifest.industry_tag.trim().is_empty() {
            extra_tags.push(format!("industry:{}", unpacked.manifest.industry_tag));
        }
        if let Some(meta) = skill_meta {
            extra_tags.extend(meta.tags.clone());
        }

        let import = import_local_skill_to_pool(skill_dir.clone(), pool, &extra_tags).await?;
        installed_skills.push(InstalledSkillSummary {
            id: import.manifest.id,
            name: import.manifest.name,
        });
        for mcp in import.missing_mcp {
            missing_mcp_set.insert(mcp);
        }
    }

    let mut missing_mcp = missing_mcp_set.into_iter().collect::<Vec<_>>();
    missing_mcp.sort();
    Ok(IndustryInstallResult {
        pack_id: unpacked.manifest.pack_id,
        version: unpacked.manifest.version,
        installed_skills,
        missing_mcp,
    })
}

#[tauri::command]
pub async fn install_industry_bundle(
    bundle_path: String,
    install_root: Option<String>,
    db: State<'_, DbState>,
) -> Result<IndustryInstallResult, String> {
    install_industry_bundle_to_pool(bundle_path, install_root, &db.0).await
}

pub async fn check_industry_bundle_update_from_pool(
    bundle_path: String,
    pool: &SqlitePool,
) -> Result<IndustryBundleUpdateCheck, String> {
    let manifest =
        crate::commands::packaging::read_industry_bundle_manifest_from_path(&bundle_path)?;
    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT manifest FROM installed_skills WHERE COALESCE(source_type, 'local') = 'local'",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut current_version: Option<String> = None;
    for (json,) in rows {
        let Ok(skill_manifest) = serde_json::from_str::<SkillManifest>(&json) else {
            continue;
        };
        let Some(pack_id) = extract_tag_value(&skill_manifest.tags, "pack:") else {
            continue;
        };
        if pack_id != manifest.pack_id {
            continue;
        }
        let Some(version) = extract_tag_value(&skill_manifest.tags, "pack-version:") else {
            continue;
        };
        current_version = match current_version {
            None => Some(version),
            Some(existing) => {
                if compare_semver(&version, &existing) == Ordering::Greater {
                    Some(version)
                } else {
                    Some(existing)
                }
            }
        };
    }

    let has_update = match current_version.as_ref() {
        Some(current) => compare_semver(&manifest.version, current) == Ordering::Greater,
        None => true,
    };
    let message = match current_version.as_ref() {
        Some(current) if has_update => format!("发现新版本：{} -> {}", current, manifest.version),
        Some(current) => format!("已是最新版本（当前 {}）", current),
        None => format!("尚未安装，可导入版本 {}", manifest.version),
    };

    Ok(IndustryBundleUpdateCheck {
        pack_id: manifest.pack_id,
        current_version,
        candidate_version: manifest.version,
        has_update,
        message,
    })
}

#[tauri::command]
pub async fn check_industry_bundle_update(
    bundle_path: String,
    db: State<'_, DbState>,
) -> Result<IndustryBundleUpdateCheck, String> {
    check_industry_bundle_update_from_pool(bundle_path, &db.0).await
}

#[tauri::command]
pub async fn list_skills(db: State<'_, DbState>) -> Result<Vec<SkillManifest>, String> {
    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT manifest FROM installed_skills ORDER BY installed_at DESC",
    )
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    rows.iter()
        .map(|(json,)| serde_json::from_str::<SkillManifest>(json).map_err(|e| e.to_string()))
        .collect()
}

#[tauri::command]
pub async fn delete_skill(skill_id: String, db: State<'_, DbState>) -> Result<(), String> {
    sqlx::query("DELETE FROM installed_skills WHERE id = ?")
        .bind(&skill_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
