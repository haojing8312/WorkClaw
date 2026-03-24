use crate::commands::skills::DbState;
use tauri::{AppHandle, State};

#[path = "runtime_preferences/autostart.rs"]
mod autostart;

#[path = "runtime_preferences/repo.rs"]
mod repo;

#[path = "runtime_preferences/service.rs"]
mod service;

#[path = "runtime_preferences/types.rs"]
mod types;

pub use autostart::sync_launch_at_login;
pub use service::{
    get_runtime_preferences_with_pool, resolve_default_work_dir_with_pool,
    set_runtime_preferences_with_pool,
};
pub use types::{RuntimePreferences, RuntimePreferencesInput};

#[tauri::command]
pub async fn get_runtime_preferences(db: State<'_, DbState>) -> Result<RuntimePreferences, String> {
    get_runtime_preferences_with_pool(&db.0).await
}

#[tauri::command]
pub async fn set_runtime_preferences(
    input: RuntimePreferencesInput,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<RuntimePreferences, String> {
    let prefs = set_runtime_preferences_with_pool(&db.0, input).await?;
    sync_launch_at_login(&app, prefs.launch_at_login)?;
    Ok(prefs)
}

#[tauri::command]
pub async fn resolve_default_work_dir(db: State<'_, DbState>) -> Result<String, String> {
    resolve_default_work_dir_with_pool(&db.0).await
}

#[cfg(test)]
mod tests {
    #[path = "../../../../tests/helpers/mod.rs"]
    mod helpers;

    use super::*;
    use serde_json::json;
    use sqlx::SqlitePool;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_memory_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        sqlx::query(
            "CREATE TABLE app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create app_settings table");

        pool
    }

    #[tokio::test]
    async fn runtime_preferences_defaults_are_stable() {
        let pool = setup_memory_pool().await;

        let prefs = get_runtime_preferences_with_pool(&pool)
            .await
            .expect("load runtime preferences");
        let prefs_json = serde_json::to_value(&prefs).expect("serialize runtime preferences");

        assert_eq!(prefs_json["default_language"], json!("zh-CN"));
        assert_eq!(prefs_json["launch_at_login"], json!(false));
        assert_eq!(prefs_json["launch_minimized"], json!(false));
        assert_eq!(prefs_json["close_to_tray"], json!(true));
        assert_eq!(prefs_json["operation_permission_mode"], json!("standard"));
    }

    #[tokio::test]
    async fn runtime_preferences_round_trip_desktop_settings() {
        let pool = setup_memory_pool().await;
        let input: RuntimePreferencesInput = serde_json::from_value(json!({
            "default_work_dir": "E:\\workspace",
            "launch_at_login": true,
            "launch_minimized": true,
            "close_to_tray": false,
            "operation_permission_mode": "full_access"
        }))
        .expect("deserialize runtime preferences input");

        set_runtime_preferences_with_pool(&pool, input)
            .await
            .expect("save runtime preferences");

        let prefs = get_runtime_preferences_with_pool(&pool)
            .await
            .expect("reload runtime preferences");
        let prefs_json = serde_json::to_value(&prefs).expect("serialize runtime preferences");

        assert_eq!(prefs_json["default_work_dir"], json!("E:\\workspace"));
        assert_eq!(prefs_json["launch_at_login"], json!(true));
        assert_eq!(prefs_json["launch_minimized"], json!(true));
        assert_eq!(prefs_json["close_to_tray"], json!(false));
        assert_eq!(
            prefs_json["operation_permission_mode"],
            json!("full_access")
        );
    }

    #[tokio::test]
    async fn runtime_preferences_partial_update_keeps_existing_translation_settings() {
        let (pool, tmp) = helpers::setup_test_db().await;
        let dir_a = tmp.path().join("a").to_string_lossy().to_string();
        let dir_b = tmp.path().join("b").to_string_lossy().to_string();

        set_runtime_preferences_with_pool(
            &pool,
            RuntimePreferencesInput {
                default_work_dir: Some(dir_a),
                default_language: Some("en-US".to_string()),
                immersive_translation_enabled: Some(false),
                immersive_translation_display: Some("bilingual_inline".to_string()),
                immersive_translation_trigger: Some("manual".to_string()),
                translation_engine: Some("model_only".to_string()),
                translation_model_id: Some("model-a".to_string()),
                launch_at_login: Some(true),
                launch_minimized: Some(false),
                close_to_tray: Some(false),
                operation_permission_mode: Some("full_access".to_string()),
            },
        )
        .await
        .expect("seed runtime preferences");

        let updated = set_runtime_preferences_with_pool(
            &pool,
            RuntimePreferencesInput {
                default_work_dir: Some(dir_b.clone()),
                default_language: None,
                immersive_translation_enabled: None,
                immersive_translation_display: None,
                immersive_translation_trigger: None,
                translation_engine: None,
                translation_model_id: None,
                launch_at_login: None,
                launch_minimized: None,
                close_to_tray: None,
                operation_permission_mode: None,
            },
        )
        .await
        .expect("partial update runtime preferences");

        assert_eq!(updated.default_work_dir, dir_b);
        assert_eq!(updated.default_language, "en-US");
        assert!(!updated.immersive_translation_enabled);
        assert_eq!(updated.immersive_translation_display, "bilingual_inline");
        assert_eq!(updated.immersive_translation_trigger, "manual");
        assert_eq!(updated.translation_engine, "model_only");
        assert_eq!(updated.translation_model_id, "model-a");
        assert_eq!(updated.operation_permission_mode, "full_access");
    }

    #[tokio::test]
    async fn runtime_preferences_invalid_permission_mode_falls_back_to_standard() {
        let (pool, _tmp) = helpers::setup_test_db().await;

        let saved = set_runtime_preferences_with_pool(
            &pool,
            RuntimePreferencesInput {
                default_work_dir: None,
                default_language: None,
                immersive_translation_enabled: None,
                immersive_translation_display: None,
                immersive_translation_trigger: None,
                translation_engine: None,
                translation_model_id: None,
                launch_at_login: None,
                launch_minimized: None,
                close_to_tray: None,
                operation_permission_mode: Some("weird".to_string()),
            },
        )
        .await
        .expect("save runtime preferences");

        assert_eq!(saved.operation_permission_mode, "standard");
    }
}
