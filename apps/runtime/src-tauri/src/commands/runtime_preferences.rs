use crate::commands::skills::DbState;
use sqlx::SqlitePool;
use std::path::PathBuf;
use tauri::State;

const KEY_RUNTIME_DEFAULT_WORK_DIR: &str = "runtime_default_work_dir";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RuntimePreferences {
    pub default_work_dir: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RuntimePreferencesInput {
    pub default_work_dir: String,
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

async fn get_app_setting(pool: &SqlitePool, key: &str) -> Result<Option<String>, String> {
    let row = sqlx::query_as::<_, (String,)>("SELECT value FROM app_settings WHERE key = ? LIMIT 1")
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

pub async fn get_runtime_preferences_with_pool(pool: &SqlitePool) -> Result<RuntimePreferences, String> {
    let saved = get_app_setting(pool, KEY_RUNTIME_DEFAULT_WORK_DIR).await?;
    let dir = saved
        .map(|v| normalize_path(&v))
        .filter(|v| !v.is_empty())
        .unwrap_or_else(compute_default_work_dir);
    Ok(RuntimePreferences {
        default_work_dir: dir,
    })
}

pub async fn set_runtime_preferences_with_pool(
    pool: &SqlitePool,
    input: RuntimePreferencesInput,
) -> Result<RuntimePreferences, String> {
    let normalized = normalize_path(&input.default_work_dir);
    if normalized.is_empty() {
        return Err("default_work_dir cannot be empty".to_string());
    }
    set_app_setting(pool, KEY_RUNTIME_DEFAULT_WORK_DIR, &normalized).await?;
    Ok(RuntimePreferences {
        default_work_dir: normalized,
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
