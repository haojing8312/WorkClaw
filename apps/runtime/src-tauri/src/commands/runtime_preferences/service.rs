use super::repo::{get_app_setting, set_app_setting};
use super::types::{
    RuntimePreferences, RuntimePreferencesInput, DEFAULT_CLOSE_TO_TRAY,
    DEFAULT_IMMERSIVE_TRANSLATION_DISPLAY, DEFAULT_IMMERSIVE_TRANSLATION_ENABLED,
    DEFAULT_IMMERSIVE_TRANSLATION_TRIGGER, DEFAULT_LANGUAGE, DEFAULT_LAUNCH_AT_LOGIN,
    DEFAULT_LAUNCH_MINIMIZED, DEFAULT_OPERATION_PERMISSION_MODE, DEFAULT_TRANSLATION_ENGINE,
    KEY_RUNTIME_CLOSE_TO_TRAY, KEY_RUNTIME_DEFAULT_LANGUAGE, KEY_RUNTIME_DEFAULT_WORK_DIR,
    KEY_RUNTIME_IMMERSIVE_TRANSLATION_DISPLAY, KEY_RUNTIME_IMMERSIVE_TRANSLATION_ENABLED,
    KEY_RUNTIME_IMMERSIVE_TRANSLATION_TRIGGER, KEY_RUNTIME_LAUNCH_AT_LOGIN,
    KEY_RUNTIME_LAUNCH_MINIMIZED, KEY_RUNTIME_OPERATION_PERMISSION_MODE,
    KEY_RUNTIME_TRANSLATION_ENGINE, KEY_RUNTIME_TRANSLATION_MODEL_ID,
};
use sqlx::SqlitePool;
use std::path::PathBuf;

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

fn normalize_operation_permission_mode(raw: &str) -> String {
    match raw.trim() {
        "full_access" => "full_access".to_string(),
        _ => DEFAULT_OPERATION_PERMISSION_MODE.to_string(),
    }
}

pub(crate) fn compute_default_work_dir_with_home() -> String {
    let fallback = PathBuf::from("C:\\Users\\Default");
    let base = home_dir_from_env().unwrap_or(fallback);
    base.join("WorkClaw")
        .join("workspace")
        .to_string_lossy()
        .to_string()
}

pub async fn get_runtime_preferences_with_pool(
    pool: &SqlitePool,
) -> Result<RuntimePreferences, String> {
    let saved_dir = get_app_setting(pool, KEY_RUNTIME_DEFAULT_WORK_DIR).await?;
    let dir = saved_dir
        .map(|v| normalize_path(&v))
        .filter(|v| !v.is_empty())
        .unwrap_or_else(compute_default_work_dir_with_home);
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
    let launch_at_login = parse_bool_setting(
        get_app_setting(pool, KEY_RUNTIME_LAUNCH_AT_LOGIN).await?,
        DEFAULT_LAUNCH_AT_LOGIN,
    );
    let launch_minimized = parse_bool_setting(
        get_app_setting(pool, KEY_RUNTIME_LAUNCH_MINIMIZED).await?,
        DEFAULT_LAUNCH_MINIMIZED,
    );
    let close_to_tray = parse_bool_setting(
        get_app_setting(pool, KEY_RUNTIME_CLOSE_TO_TRAY).await?,
        DEFAULT_CLOSE_TO_TRAY,
    );
    let operation_permission_mode = get_app_setting(pool, KEY_RUNTIME_OPERATION_PERMISSION_MODE)
        .await?
        .map(|v| normalize_operation_permission_mode(&v))
        .unwrap_or_else(|| DEFAULT_OPERATION_PERMISSION_MODE.to_string());
    Ok(RuntimePreferences {
        default_work_dir: dir,
        default_language,
        immersive_translation_enabled,
        immersive_translation_display,
        immersive_translation_trigger,
        translation_engine,
        translation_model_id,
        launch_at_login,
        launch_minimized,
        close_to_tray,
        operation_permission_mode,
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
    let launch_at_login = input.launch_at_login.unwrap_or(current.launch_at_login);
    let launch_minimized = input.launch_minimized.unwrap_or(current.launch_minimized);
    let close_to_tray = input.close_to_tray.unwrap_or(current.close_to_tray);
    let operation_permission_mode = input
        .operation_permission_mode
        .map(|v| normalize_operation_permission_mode(&v))
        .unwrap_or(current.operation_permission_mode);

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
        KEY_RUNTIME_LAUNCH_AT_LOGIN,
        if launch_at_login { "true" } else { "false" },
    )
    .await?;
    set_app_setting(
        pool,
        KEY_RUNTIME_LAUNCH_MINIMIZED,
        if launch_minimized { "true" } else { "false" },
    )
    .await?;
    set_app_setting(
        pool,
        KEY_RUNTIME_CLOSE_TO_TRAY,
        if close_to_tray { "true" } else { "false" },
    )
    .await?;
    set_app_setting(
        pool,
        KEY_RUNTIME_OPERATION_PERMISSION_MODE,
        &operation_permission_mode,
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
        launch_at_login,
        launch_minimized,
        close_to_tray,
        operation_permission_mode,
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
