use super::helpers::{
    build_local_skill_id, merge_tags, normalize_display_name, read_skill_markdown_with_fallback,
    render_local_skill_markdown, sanitize_slug,
};
use super::types::{
    ImportResult, LocalImportBatchResult, LocalImportFailedItem, LocalImportInstalledItem,
    LocalSkillPreview,
};
use crate::runtime_environment::runtime_paths_from_app;
use chrono::Utc;
use skillpack_rs::{verify_and_unpack, SkillManifest};
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

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

pub async fn render_local_skill_preview(
    name: String,
    description: String,
    when_to_use: String,
    target_dir: Option<String>,
    app: &tauri::AppHandle,
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
        Some(dir) => std::path::PathBuf::from(dir),
        None => runtime_paths_from_app(app)?.skills_dir,
    };
    let save_path = base_dir.join(sanitize_slug(&preview_name));

    Ok(LocalSkillPreview {
        markdown: render_local_skill_markdown(&preview_name, description.trim(), &preview_when),
        save_path: save_path.to_string_lossy().to_string(),
    })
}

pub async fn create_local_skill(
    name: String,
    description: String,
    when_to_use: String,
    target_dir: Option<String>,
    app: &tauri::AppHandle,
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
        Some(dir) => std::path::PathBuf::from(dir),
        None => runtime_paths_from_app(app)?.skills_dir,
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

pub async fn install_skill(
    pack_path: String,
    username: String,
    pool: &SqlitePool,
) -> Result<SkillManifest, String> {
    let unpacked = verify_and_unpack(&pack_path, &username).map_err(|e| e.to_string())?;
    ensure_skill_display_name_available(pool, &unpacked.manifest.name, &unpacked.manifest.id)
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
    .execute(pool)
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

    let config = crate::agent::skill_config::SkillConfig::parse(&content);
    let parsed_tags = crate::commands::packaging::parse_skill_tags(&content);
    let merged_tags = merge_tags(parsed_tags, extra_tags);

    let name = config.name.clone().unwrap_or_else(|| {
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
    .bind("")
    .bind(&dir_path)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

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

fn discover_local_skill_dirs(dir_path: &str) -> Result<Vec<PathBuf>, String> {
    let root = Path::new(dir_path);
    if !root.exists() {
        return Err(format!("目录不存在: {dir_path}"));
    }
    if !root.is_dir() {
        return Err(format!("所选路径不是目录: {dir_path}"));
    }

    let has_skill_md = |dir: &Path| dir.join("SKILL.md").exists() || dir.join("skill.md").exists();

    if has_skill_md(root) {
        return Ok(vec![root.to_path_buf()]);
    }

    let mut discovered = Vec::new();
    let mut seen = HashSet::new();

    let mut push_if_skill_dir = |dir: PathBuf| {
        let normalized = dir.to_string_lossy().to_string();
        if has_skill_md(&dir) && seen.insert(normalized) {
            discovered.push(dir);
        }
    };

    let mut first_level = std::fs::read_dir(root)
        .map_err(|e| format!("读取目录失败: {}", e))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    first_level.sort();

    for child in first_level {
        if has_skill_md(&child) {
            push_if_skill_dir(child);
            continue;
        }

        let mut second_level = match std::fs::read_dir(&child) {
            Ok(iter) => iter
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect::<Vec<_>>(),
            Err(_) => continue,
        };
        second_level.sort();

        for grandchild in second_level {
            push_if_skill_dir(grandchild);
        }
    }

    Ok(discovered)
}

pub async fn import_local_skills_to_pool(
    dir_path: String,
    pool: &SqlitePool,
    extra_tags: &[String],
) -> Result<LocalImportBatchResult, String> {
    let discovered = discover_local_skill_dirs(&dir_path)?;
    if discovered.is_empty() {
        return Err("未找到可导入的 SKILL.md 目录".to_string());
    }

    let mut installed = Vec::new();
    let mut failed = Vec::new();
    let mut missing_mcp_set = HashSet::new();

    for skill_dir in discovered {
        let skill_dir_path = skill_dir.to_string_lossy().to_string();
        match import_local_skill_to_pool(skill_dir_path.clone(), pool, extra_tags).await {
            Ok(result) => {
                for mcp in result.missing_mcp {
                    missing_mcp_set.insert(mcp);
                }
                installed.push(LocalImportInstalledItem {
                    dir_path: skill_dir_path,
                    manifest: result.manifest,
                });
            }
            Err(error) => {
                let name_hint = skill_dir
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown-skill".to_string());
                failed.push(LocalImportFailedItem {
                    dir_path: skill_dir_path,
                    name_hint,
                    error,
                });
            }
        }
    }

    if installed.is_empty() {
        let first_error = failed
            .first()
            .map(|item| item.error.clone())
            .unwrap_or_else(|| "未导入任何技能".to_string());
        return Err(first_error);
    }

    let mut missing_mcp = missing_mcp_set.into_iter().collect::<Vec<_>>();
    missing_mcp.sort();

    Ok(LocalImportBatchResult {
        installed,
        failed,
        missing_mcp,
    })
}

pub async fn import_local_skill(
    dir_path: String,
    pool: &SqlitePool,
) -> Result<LocalImportBatchResult, String> {
    import_local_skills_to_pool(dir_path, pool, &[]).await
}

pub async fn refresh_local_skill(
    skill_id: String,
    pool: &SqlitePool,
) -> Result<SkillManifest, String> {
    let (manifest_json, pack_path, source_type): (String, String, String) = sqlx::query_as(
        "SELECT manifest, pack_path, COALESCE(source_type, 'encrypted') FROM installed_skills WHERE id = ?",
    )
    .bind(&skill_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Skill 不存在 (skill_id={}): {}", skill_id, e))?;

    if !matches!(source_type.as_str(), "local" | "vendored") {
        return Err(format!("Skill {} 不是目录型 Skill，无法刷新", skill_id));
    }

    let content = read_skill_markdown_with_fallback(&pack_path)?;

    let config = crate::agent::skill_config::SkillConfig::parse(&content);
    let parsed_tags = crate::commands::packaging::parse_skill_tags(&content);

    let name = config.name.clone().unwrap_or_else(|| {
        std::path::Path::new(&pack_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed-skill".to_string())
    });

    let existing_manifest =
        serde_json::from_str::<SkillManifest>(&manifest_json).map_err(|e| e.to_string())?;

    let manifest = SkillManifest {
        id: skill_id.clone(),
        name,
        description: config.description.unwrap_or_default(),
        version: existing_manifest.version,
        author: existing_manifest.author,
        recommended_model: config.model.unwrap_or_default(),
        tags: parsed_tags,
        created_at: existing_manifest.created_at,
        username_hint: existing_manifest.username_hint,
        encrypted_verify: existing_manifest.encrypted_verify,
    };

    let manifest_json = serde_json::to_string(&manifest).map_err(|e| e.to_string())?;

    sqlx::query("UPDATE installed_skills SET manifest = ? WHERE id = ?")
        .bind(&manifest_json)
        .bind(&skill_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(manifest)
}
