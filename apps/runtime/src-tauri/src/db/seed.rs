use anyhow::Result;
use sqlx::SqlitePool;
use std::path::Path;

pub(super) fn build_builtin_manifest_json(skill_id: &str, skill_markdown: &str) -> String {
    let builtin_config = crate::agent::skill_config::SkillConfig::parse(skill_markdown);
    let builtin_name = builtin_config.name.unwrap_or_else(|| skill_id.to_string());
    let builtin_description = builtin_config.description.unwrap_or_default();

    serde_json::json!({
        "id": skill_id,
        "name": builtin_name,
        "description": builtin_description,
        "version": "1.0.0",
        "author": "WorkClaw",
        "recommended_model": "",
        "tags": [],
        "created_at": "2026-01-01T00:00:00Z",
        "username_hint": null,
        "encrypted_verify": ""
    })
    .to_string()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) async fn sync_builtin_skills(pool: &SqlitePool) -> Result<()> {
    let vendor_root = std::env::temp_dir().join("workclaw-missing-vendor-root");
    sync_builtin_skills_with_root(pool, &vendor_root).await
}

pub(super) async fn sync_builtin_skills_with_root(
    pool: &SqlitePool,
    vendor_root: &Path,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    std::fs::create_dir_all(vendor_root)?;
    for entry in crate::builtin_skills::builtin_skill_entries() {
        let skill_dir = vendor_root.join(entry.id);
        sync_builtin_skill_directory(entry.id, &skill_dir)?;
        let builtin_json = build_builtin_manifest_json(entry.id, entry.markdown);
        sqlx::query(
            "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type)
             VALUES (?, ?, ?, '', ?, 'vendored')
             ON CONFLICT(id) DO UPDATE SET
               manifest = excluded.manifest,
               username = '',
               pack_path = excluded.pack_path,
               source_type = 'vendored'",
        )
        .bind(entry.id)
        .bind(&builtin_json)
        .bind(&now)
        .bind(skill_dir.to_string_lossy().to_string())
        .execute(pool)
        .await?;
    }

    Ok(())
}

fn sync_builtin_skill_directory(skill_id: &str, skill_dir: &Path) -> Result<()> {
    let files = crate::builtin_skills::builtin_skill_files(skill_id)
        .ok_or_else(|| anyhow::anyhow!("missing embedded builtin skill assets for {}", skill_id))?;
    if skill_dir.exists() {
        std::fs::remove_dir_all(skill_dir)?;
    }
    std::fs::create_dir_all(skill_dir)?;
    for (relative_path, bytes) in files {
        let target = skill_dir.join(relative_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(target, bytes)?;
    }
    Ok(())
}

pub(super) async fn seed_runtime_defaults(pool: &SqlitePool, app_dir: &Path) -> Result<()> {
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO app_settings (key, value) VALUES ('runtime_launch_at_login', 'false')",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO app_settings (key, value) VALUES ('runtime_launch_minimized', 'false')",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO app_settings (key, value) VALUES ('runtime_close_to_tray', 'true')",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO app_settings (key, value) VALUES ('route_max_call_depth', '4')",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO app_settings (key, value) VALUES ('route_node_timeout_seconds', '60')",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO app_settings (key, value) VALUES ('route_retry_count', '0')",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO app_settings (key, value) VALUES ('runtime_default_work_dir', '')",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO app_settings (key, value) VALUES ('runtime_default_language', 'zh-CN')",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO app_settings (key, value) VALUES ('runtime_immersive_translation_enabled', 'true')",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO app_settings (key, value) VALUES ('runtime_immersive_translation_display', 'translated_only')",
    )
    .execute(pool)
    .await;

    let vendor_root = app_dir.join("skills").join("vendor");
    let _ = sync_builtin_skills_with_root(pool, &vendor_root).await;
    crate::team_templates::seed_builtin_team_templates_with_root(pool, app_dir).await?;

    Ok(())
}
