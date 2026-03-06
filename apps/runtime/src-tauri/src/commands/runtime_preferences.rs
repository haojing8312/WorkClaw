use crate::commands::skills::DbState;
use sqlx::SqlitePool;
use std::path::PathBuf;
use tauri::State;

const KEY_RUNTIME_DEFAULT_WORK_DIR: &str = "runtime_default_work_dir";
const KEY_RUNTIME_DEFAULT_LANGUAGE: &str = "runtime_default_language";
const KEY_RUNTIME_IMMERSIVE_TRANSLATION_ENABLED: &str = "runtime_immersive_translation_enabled";
const KEY_RUNTIME_IMMERSIVE_TRANSLATION_DISPLAY: &str = "runtime_immersive_translation_display";
const KEY_RUNTIME_IMMERSIVE_TRANSLATION_TRIGGER: &str = "runtime_immersive_translation_trigger";
const KEY_RUNTIME_TRANSLATION_ENGINE: &str = "runtime_translation_engine";
const KEY_RUNTIME_TRANSLATION_MODEL_ID: &str = "runtime_translation_model_id";
const KEY_RUNTIME_AUTO_UPDATE_ENABLED: &str = "runtime_auto_update_enabled";
const KEY_RUNTIME_UPDATE_CHANNEL: &str = "runtime_update_channel";
const KEY_RUNTIME_DISMISSED_UPDATE_VERSION: &str = "runtime_dismissed_update_version";
const KEY_RUNTIME_LAST_UPDATE_CHECK_AT: &str = "runtime_last_update_check_at";

const DEFAULT_LANGUAGE: &str = "zh-CN";
const DEFAULT_IMMERSIVE_TRANSLATION_ENABLED: bool = true;
const DEFAULT_IMMERSIVE_TRANSLATION_DISPLAY: &str = "translated_only";
const DEFAULT_IMMERSIVE_TRANSLATION_TRIGGER: &str = "auto";
const DEFAULT_TRANSLATION_ENGINE: &str = "model_then_free";
const DEFAULT_AUTO_UPDATE_ENABLED: bool = true;
const DEFAULT_UPDATE_CHANNEL: &str = "stable";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RuntimePreferences {
    pub default_work_dir: String,
    pub default_language: String,
    pub immersive_translation_enabled: bool,
    pub immersive_translation_display: String,
    pub immersive_translation_trigger: String,
    pub translation_engine: String,
    pub translation_model_id: String,
    pub auto_update_enabled: bool,
    pub update_channel: String,
    pub dismissed_update_version: String,
    pub last_update_check_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RuntimePreferencesInput {
    pub default_work_dir: Option<String>,
    pub default_language: Option<String>,
    pub immersive_translation_enabled: Option<bool>,
    pub immersive_translation_display: Option<String>,
    pub immersive_translation_trigger: Option<String>,
    pub translation_engine: Option<String>,
    pub translation_model_id: Option<String>,
    pub auto_update_enabled: Option<bool>,
    pub update_channel: Option<String>,
    pub dismissed_update_version: Option<String>,
    pub last_update_check_at: Option<String>,
}

fn home_dir_from_env() -> Option<PathBuf> {
    std::env::var("USERPROFILE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .map(PathBuf::from)
        })
}

fn compute_default_work_dir() -> String {
    let fallback = PathBuf::from("C:\\Users\\Default");
    let base = home_dir_from_env().unwrap_or(fallback);
    base.join("WorkClaw")
        .join("workspace")
        .to_string_lossy()
        .to_string()
}

fn normalize_path(raw: &str) -> String {
    raw.trim().to_string()
}

fn normalize_language(raw: &str) -> String {
    let normalized = raw.trim();
    if normalized.is_empty() {
        DEFAULT_LANGUAGE.to_string()
    } else {
        normalized.to_string()
    }
}

fn parse_bool_setting(raw: Option<String>, default: bool) -> bool {
    match raw.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => default,
    }
}

fn normalize_immersive_display(raw: &str) -> String {
    match raw.trim() {
        "bilingual_inline" => "bilingual_inline".to_string(),
        _ => DEFAULT_IMMERSIVE_TRANSLATION_DISPLAY.to_string(),
    }
}

fn normalize_immersive_trigger(raw: &str) -> String {
    match raw.trim() {
        "manual" => "manual".to_string(),
        _ => DEFAULT_IMMERSIVE_TRANSLATION_TRIGGER.to_string(),
    }
}

fn normalize_translation_engine(raw: &str) -> String {
    match raw.trim() {
        "model_only" => "model_only".to_string(),
        "free_only" => "free_only".to_string(),
        _ => DEFAULT_TRANSLATION_ENGINE.to_string(),
    }
}

fn normalize_translation_model_id(raw: &str) -> String {
    raw.trim().to_string()
}

fn normalize_update_channel(raw: &str) -> String {
    match raw.trim() {
        "stable" => "stable".to_string(),
        _ => DEFAULT_UPDATE_CHANNEL.to_string(),
    }
}

fn normalize_optional_text(raw: &str) -> String {
    raw.trim().to_string()
}

async fn get_app_setting(pool: &SqlitePool, key: &str) -> Result<Option<String>, String> {
    let row =
        sqlx::query_as::<_, (String,)>("SELECT value FROM app_settings WHERE key = ? LIMIT 1")
            .bind(key)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(row.map(|(v,)| v))
}

async fn set_app_setting(pool: &SqlitePool, key: &str, value: &str) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_runtime_preferences_with_pool(
    pool: &SqlitePool,
) -> Result<RuntimePreferences, String> {
    let saved_dir = get_app_setting(pool, KEY_RUNTIME_DEFAULT_WORK_DIR).await?;
    let dir = saved_dir
        .map(|v| normalize_path(&v))
        .filter(|v| !v.is_empty())
        .unwrap_or_else(compute_default_work_dir);
    let default_language = get_app_setting(pool, KEY_RUNTIME_DEFAULT_LANGUAGE)
        .await?
        .map(|v| normalize_language(&v))
        .unwrap_or_else(|| DEFAULT_LANGUAGE.to_string());
    let immersive_translation_enabled = parse_bool_setting(
        get_app_setting(pool, KEY_RUNTIME_IMMERSIVE_TRANSLATION_ENABLED).await?,
        DEFAULT_IMMERSIVE_TRANSLATION_ENABLED,
    );
    let immersive_translation_display =
        get_app_setting(pool, KEY_RUNTIME_IMMERSIVE_TRANSLATION_DISPLAY)
            .await?
            .map(|v| normalize_immersive_display(&v))
            .unwrap_or_else(|| DEFAULT_IMMERSIVE_TRANSLATION_DISPLAY.to_string());
    let immersive_translation_trigger =
        get_app_setting(pool, KEY_RUNTIME_IMMERSIVE_TRANSLATION_TRIGGER)
            .await?
            .map(|v| normalize_immersive_trigger(&v))
            .unwrap_or_else(|| DEFAULT_IMMERSIVE_TRANSLATION_TRIGGER.to_string());
    let translation_engine = get_app_setting(pool, KEY_RUNTIME_TRANSLATION_ENGINE)
        .await?
        .map(|v| normalize_translation_engine(&v))
        .unwrap_or_else(|| DEFAULT_TRANSLATION_ENGINE.to_string());
    let translation_model_id = get_app_setting(pool, KEY_RUNTIME_TRANSLATION_MODEL_ID)
        .await?
        .map(|v| normalize_translation_model_id(&v))
        .unwrap_or_default();
    let auto_update_enabled = parse_bool_setting(
        get_app_setting(pool, KEY_RUNTIME_AUTO_UPDATE_ENABLED).await?,
        DEFAULT_AUTO_UPDATE_ENABLED,
    );
    let update_channel = get_app_setting(pool, KEY_RUNTIME_UPDATE_CHANNEL)
        .await?
        .map(|v| normalize_update_channel(&v))
        .unwrap_or_else(|| DEFAULT_UPDATE_CHANNEL.to_string());
    let dismissed_update_version = get_app_setting(pool, KEY_RUNTIME_DISMISSED_UPDATE_VERSION)
        .await?
        .map(|v| normalize_optional_text(&v))
        .unwrap_or_default();
    let last_update_check_at = get_app_setting(pool, KEY_RUNTIME_LAST_UPDATE_CHECK_AT)
        .await?
        .map(|v| normalize_optional_text(&v))
        .unwrap_or_default();
    Ok(RuntimePreferences {
        default_work_dir: dir,
        default_language,
        immersive_translation_enabled,
        immersive_translation_display,
        immersive_translation_trigger,
        translation_engine,
        translation_model_id,
        auto_update_enabled,
        update_channel,
        dismissed_update_version,
        last_update_check_at,
    })
}

pub async fn set_runtime_preferences_with_pool(
    pool: &SqlitePool,
    input: RuntimePreferencesInput,
) -> Result<RuntimePreferences, String> {
    let current = get_runtime_preferences_with_pool(pool).await?;

    let default_work_dir = if let Some(raw) = input.default_work_dir {
        let normalized = normalize_path(&raw);
        if normalized.is_empty() {
            return Err("default_work_dir cannot be empty".to_string());
        }
        normalized
    } else {
        current.default_work_dir
    };
    let default_language = input
        .default_language
        .map(|v| normalize_language(&v))
        .unwrap_or(current.default_language);
    let immersive_translation_enabled = input
        .immersive_translation_enabled
        .unwrap_or(current.immersive_translation_enabled);
    let immersive_translation_display = input
        .immersive_translation_display
        .map(|v| normalize_immersive_display(&v))
        .unwrap_or(current.immersive_translation_display);
    let immersive_translation_trigger = input
        .immersive_translation_trigger
        .map(|v| normalize_immersive_trigger(&v))
        .unwrap_or(current.immersive_translation_trigger);
    let translation_engine = input
        .translation_engine
        .map(|v| normalize_translation_engine(&v))
        .unwrap_or(current.translation_engine);
    let translation_model_id = input
        .translation_model_id
        .map(|v| normalize_translation_model_id(&v))
        .unwrap_or(current.translation_model_id);
    let auto_update_enabled = input
        .auto_update_enabled
        .unwrap_or(current.auto_update_enabled);
    let update_channel = input
        .update_channel
        .map(|v| normalize_update_channel(&v))
        .unwrap_or(current.update_channel);
    let dismissed_update_version = input
        .dismissed_update_version
        .map(|v| normalize_optional_text(&v))
        .unwrap_or(current.dismissed_update_version);
    let last_update_check_at = input
        .last_update_check_at
        .map(|v| normalize_optional_text(&v))
        .unwrap_or(current.last_update_check_at);

    set_app_setting(pool, KEY_RUNTIME_DEFAULT_WORK_DIR, &default_work_dir).await?;
    set_app_setting(pool, KEY_RUNTIME_DEFAULT_LANGUAGE, &default_language).await?;
    set_app_setting(
        pool,
        KEY_RUNTIME_IMMERSIVE_TRANSLATION_ENABLED,
        if immersive_translation_enabled {
            "true"
        } else {
            "false"
        },
    )
    .await?;
    set_app_setting(
        pool,
        KEY_RUNTIME_IMMERSIVE_TRANSLATION_DISPLAY,
        &immersive_translation_display,
    )
    .await?;
    set_app_setting(
        pool,
        KEY_RUNTIME_IMMERSIVE_TRANSLATION_TRIGGER,
        &immersive_translation_trigger,
    )
    .await?;
    set_app_setting(pool, KEY_RUNTIME_TRANSLATION_ENGINE, &translation_engine).await?;
    set_app_setting(
        pool,
        KEY_RUNTIME_TRANSLATION_MODEL_ID,
        &translation_model_id,
    )
    .await?;
    set_app_setting(
        pool,
        KEY_RUNTIME_AUTO_UPDATE_ENABLED,
        if auto_update_enabled { "true" } else { "false" },
    )
    .await?;
    set_app_setting(pool, KEY_RUNTIME_UPDATE_CHANNEL, &update_channel).await?;
    set_app_setting(
        pool,
        KEY_RUNTIME_DISMISSED_UPDATE_VERSION,
        &dismissed_update_version,
    )
    .await?;
    set_app_setting(
        pool,
        KEY_RUNTIME_LAST_UPDATE_CHECK_AT,
        &last_update_check_at,
    )
    .await?;
    Ok(RuntimePreferences {
        default_work_dir,
        default_language,
        immersive_translation_enabled,
        immersive_translation_display,
        immersive_translation_trigger,
        translation_engine,
        translation_model_id,
        auto_update_enabled,
        update_channel,
        dismissed_update_version,
        last_update_check_at,
    })
}

pub async fn resolve_default_work_dir_with_pool(pool: &SqlitePool) -> Result<String, String> {
    let prefs = get_runtime_preferences_with_pool(pool).await?;
    let dir = normalize_path(&prefs.default_work_dir);
    if dir.is_empty() {
        return Err("default work dir is empty".to_string());
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create default work dir: {e}"))?;
    Ok(dir)
}

#[tauri::command]
pub async fn get_runtime_preferences(db: State<'_, DbState>) -> Result<RuntimePreferences, String> {
    get_runtime_preferences_with_pool(&db.0).await
}

#[tauri::command]
pub async fn set_runtime_preferences(
    input: RuntimePreferencesInput,
    db: State<'_, DbState>,
) -> Result<RuntimePreferences, String> {
    set_runtime_preferences_with_pool(&db.0, input).await
}

#[tauri::command]
pub async fn resolve_default_work_dir(db: State<'_, DbState>) -> Result<String, String> {
    resolve_default_work_dir_with_pool(&db.0).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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
    async fn runtime_preferences_defaults_include_updater_settings() {
        let pool = setup_memory_pool().await;

        let prefs = get_runtime_preferences_with_pool(&pool)
            .await
            .expect("load runtime preferences");
        let prefs_json = serde_json::to_value(&prefs).expect("serialize runtime preferences");

        assert_eq!(prefs_json["default_language"], json!("zh-CN"));
        assert_eq!(prefs_json["auto_update_enabled"], json!(true));
        assert_eq!(prefs_json["update_channel"], json!("stable"));
        assert_eq!(prefs_json["dismissed_update_version"], json!(""));
        assert_eq!(prefs_json["last_update_check_at"], json!(""));
    }

    #[tokio::test]
    async fn runtime_preferences_round_trip_updater_settings() {
        let pool = setup_memory_pool().await;
        let input: RuntimePreferencesInput = serde_json::from_value(json!({
            "default_work_dir": "E:\\workspace",
            "auto_update_enabled": false,
            "update_channel": "stable",
            "dismissed_update_version": "0.2.4",
            "last_update_check_at": "2026-03-06T10:00:00Z"
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
        assert_eq!(prefs_json["auto_update_enabled"], json!(false));
        assert_eq!(prefs_json["update_channel"], json!("stable"));
        assert_eq!(prefs_json["dismissed_update_version"], json!("0.2.4"));
        assert_eq!(
            prefs_json["last_update_check_at"],
            json!("2026-03-06T10:00:00Z")
        );
    }
}
