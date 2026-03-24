use skillpack_rs::SkillManifest;
use tauri::State;

#[path = "skills/types.rs"]
mod types;

#[path = "skills/helpers.rs"]
mod helpers;

#[path = "skills/local_skill_service.rs"]
mod local_skill_service;

#[path = "skills/industry_bundle_service.rs"]
mod industry_bundle_service;

pub use types::{
    DbState, ImportResult, IndustryBundleUpdateCheck, IndustryInstallResult,
    InstalledSkillSummary, LocalImportBatchResult, LocalImportFailedItem,
    LocalImportInstalledItem, LocalSkillPreview,
};
pub use local_skill_service::{
    ensure_skill_display_name_available, import_local_skill_to_pool, import_local_skills_to_pool,
};
pub use industry_bundle_service::{
    check_industry_bundle_update_from_pool, install_industry_bundle_to_pool,
};

#[tauri::command]
pub async fn render_local_skill_preview(
    name: String,
    description: String,
    when_to_use: String,
    target_dir: Option<String>,
) -> Result<LocalSkillPreview, String> {
    local_skill_service::render_local_skill_preview(name, description, when_to_use, target_dir).await
}

#[tauri::command]
pub async fn create_local_skill(
    name: String,
    description: String,
    when_to_use: String,
    target_dir: Option<String>,
) -> Result<String, String> {
    local_skill_service::create_local_skill(name, description, when_to_use, target_dir).await
}

#[tauri::command]
pub async fn install_skill(
    pack_path: String,
    username: String,
    db: State<'_, DbState>,
) -> Result<SkillManifest, String> {
    local_skill_service::install_skill(pack_path, username, &db.0).await
}

#[tauri::command]
pub async fn import_local_skill(
    dir_path: String,
    db: State<'_, DbState>,
) -> Result<LocalImportBatchResult, String> {
    local_skill_service::import_local_skill(dir_path, &db.0).await
}

#[tauri::command]
pub async fn refresh_local_skill(
    skill_id: String,
    db: State<'_, DbState>,
) -> Result<SkillManifest, String> {
    local_skill_service::refresh_local_skill(skill_id, &db.0).await
}

#[tauri::command]
pub async fn install_industry_bundle(
    bundle_path: String,
    install_root: Option<String>,
    db: State<'_, DbState>,
) -> Result<IndustryInstallResult, String> {
    industry_bundle_service::install_industry_bundle(bundle_path, install_root, &db.0).await
}

#[tauri::command]
pub async fn check_industry_bundle_update(
    bundle_path: String,
    db: State<'_, DbState>,
) -> Result<IndustryBundleUpdateCheck, String> {
    industry_bundle_service::check_industry_bundle_update(bundle_path, &db.0).await
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
