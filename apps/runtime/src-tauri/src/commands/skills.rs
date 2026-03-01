use sqlx::SqlitePool;
use tauri::State;
use skillpack_rs::{verify_and_unpack, SkillManifest};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

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

    let base_dir = match target_dir.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => default_skill_base_dir(),
    };
    let save_path = base_dir.join(sanitize_slug(&preview_name));

    Ok(LocalSkillPreview {
        markdown: render_local_skill_markdown(
            &preview_name,
            description.trim(),
            &preview_when,
        ),
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
    let base_dir = match target_dir.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => default_skill_base_dir(),
    };

    let skill_dir = base_dir.join(&slug);
    if skill_dir.exists() {
        return Err(format!("技能目录已存在: {}", skill_dir.to_string_lossy()));
    }
    std::fs::create_dir_all(&skill_dir)
        .map_err(|e| format!("创建目录失败: {}", e))?;

    let content = render_local_skill_markdown(clean_name, description.trim(), clean_when);

    let skill_md = skill_dir.join("SKILL.md");
    std::fs::write(&skill_md, content)
        .map_err(|e| format!("写入 SKILL.md 失败: {}", e))?;

    Ok(skill_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn install_skill(
    pack_path: String,
    username: String,
    db: State<'_, DbState>,
) -> Result<SkillManifest, String> {
    let unpacked = verify_and_unpack(&pack_path, &username)
        .map_err(|e| e.to_string())?;

    let manifest_json = serde_json::to_string(&unpacked.manifest)
        .map_err(|e| e.to_string())?;

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

/// 导入本地 Skill 目录（读取 SKILL.md 解析 frontmatter）
#[tauri::command]
pub async fn import_local_skill(
    dir_path: String,
    db: State<'_, DbState>,
) -> Result<ImportResult, String> {
    // 读取 SKILL.md
    let skill_md_path = std::path::Path::new(&dir_path).join("SKILL.md");
    let content = std::fs::read_to_string(&skill_md_path)
        .map_err(|e| format!("无法读取 SKILL.md: {}", e))?;

    // 解析 frontmatter
    let config = crate::agent::skill_config::SkillConfig::parse(&content);

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
        tags: Vec::new(),
        created_at: Utc::now(),
        username_hint: None,
        encrypted_verify: String::new(),
    };

    let manifest_json = serde_json::to_string(&manifest)
        .map_err(|e| e.to_string())?;

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR REPLACE INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type) VALUES (?, ?, ?, ?, ?, 'local')"
    )
    .bind(&skill_id)
    .bind(&manifest_json)
    .bind(&now)
    .bind("")  // 本地 Skill 无需 username
    .bind(&dir_path)
    .execute(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    // 检查 MCP 依赖：哪些声明的 MCP 服务器尚未在数据库中配置
    let mut missing_mcp = Vec::new();
    for dep in &config.mcp_servers {
        let exists: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM mcp_servers WHERE name = ?"
        )
        .bind(&dep.name)
        .fetch_optional(&db.0)
        .await
        .map_err(|e| e.to_string())?;

        if exists.is_none() {
            missing_mcp.push(dep.name.clone());
        }
    }

    Ok(ImportResult { manifest, missing_mcp })
}

/// 刷新本地 Skill（重新读取 SKILL.md 更新 manifest）
#[tauri::command]
pub async fn refresh_local_skill(
    skill_id: String,
    db: State<'_, DbState>,
) -> Result<SkillManifest, String> {
    // 从 DB 获取 pack_path（即目录路径）
    let (pack_path, source_type): (String, String) = sqlx::query_as(
        "SELECT pack_path, COALESCE(source_type, 'encrypted') FROM installed_skills WHERE id = ?"
    )
    .bind(&skill_id)
    .fetch_one(&db.0)
    .await
    .map_err(|e| format!("Skill 不存在 (skill_id={}): {}", skill_id, e))?;

    if source_type != "local" {
        return Err(format!("Skill {} 不是本地 Skill，无法刷新", skill_id));
    }

    // 重新读取 SKILL.md
    let skill_md_path = std::path::Path::new(&pack_path).join("SKILL.md");
    let content = std::fs::read_to_string(&skill_md_path)
        .map_err(|e| format!("无法读取 SKILL.md: {}", e))?;

    let config = crate::agent::skill_config::SkillConfig::parse(&content);

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
        tags: Vec::new(),
        created_at: Utc::now(),
        username_hint: None,
        encrypted_verify: String::new(),
    };

    let manifest_json = serde_json::to_string(&manifest)
        .map_err(|e| e.to_string())?;

    // 更新 DB 中的 manifest
    sqlx::query("UPDATE installed_skills SET manifest = ? WHERE id = ?")
        .bind(&manifest_json)
        .bind(&skill_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    Ok(manifest)
}

#[tauri::command]
pub async fn list_skills(db: State<'_, DbState>) -> Result<Vec<SkillManifest>, String> {
    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT manifest FROM installed_skills ORDER BY installed_at DESC"
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
