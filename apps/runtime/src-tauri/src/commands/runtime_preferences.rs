use crate::commands::skills::DbState;
use sqlx::SqlitePool;
use std::env;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::fmt::Write as FmtWrite;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::fs::{self, File};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::Command;
use tauri::{AppHandle, State};

const KEY_RUNTIME_DEFAULT_WORK_DIR: &str = "runtime_default_work_dir";
const KEY_RUNTIME_DEFAULT_LANGUAGE: &str = "runtime_default_language";
const KEY_RUNTIME_IMMERSIVE_TRANSLATION_ENABLED: &str = "runtime_immersive_translation_enabled";
const KEY_RUNTIME_IMMERSIVE_TRANSLATION_DISPLAY: &str = "runtime_immersive_translation_display";
const KEY_RUNTIME_IMMERSIVE_TRANSLATION_TRIGGER: &str = "runtime_immersive_translation_trigger";
const KEY_RUNTIME_TRANSLATION_ENGINE: &str = "runtime_translation_engine";
const KEY_RUNTIME_TRANSLATION_MODEL_ID: &str = "runtime_translation_model_id";
const KEY_RUNTIME_LAUNCH_AT_LOGIN: &str = "runtime_launch_at_login";
const KEY_RUNTIME_LAUNCH_MINIMIZED: &str = "runtime_launch_minimized";
const KEY_RUNTIME_CLOSE_TO_TRAY: &str = "runtime_close_to_tray";
const KEY_RUNTIME_OPERATION_PERMISSION_MODE: &str = "runtime_operation_permission_mode";

const DEFAULT_LANGUAGE: &str = "zh-CN";
const DEFAULT_IMMERSIVE_TRANSLATION_ENABLED: bool = true;
const DEFAULT_IMMERSIVE_TRANSLATION_DISPLAY: &str = "translated_only";
const DEFAULT_IMMERSIVE_TRANSLATION_TRIGGER: &str = "auto";
const DEFAULT_TRANSLATION_ENGINE: &str = "model_then_free";
const DEFAULT_LAUNCH_AT_LOGIN: bool = false;
const DEFAULT_LAUNCH_MINIMIZED: bool = false;
const DEFAULT_CLOSE_TO_TRAY: bool = true;
const DEFAULT_OPERATION_PERMISSION_MODE: &str = "standard";
const AUTOSTART_NAME: &str = "dev.workclaw.runtime";

#[cfg(target_os = "windows")]
fn format_windows_command_failure(action: &str, output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let details = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("退出码 {:?}", output.status.code())
    };
    format!("{action}失败: {details}")
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RuntimePreferences {
    pub default_work_dir: String,
    pub default_language: String,
    pub immersive_translation_enabled: bool,
    pub immersive_translation_display: String,
    pub immersive_translation_trigger: String,
    pub translation_engine: String,
    pub translation_model_id: String,
    pub launch_at_login: bool,
    pub launch_minimized: bool,
    pub close_to_tray: bool,
    pub operation_permission_mode: String,
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn resolve_home_dir() -> Result<PathBuf, String> {
    if let Some(home) = env::var_os("HOME") {
        return Ok(PathBuf::from(home));
    }

    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .ok_or_else(|| "无法解析用户主目录".to_string())
}

pub fn sync_launch_at_login(_app: &AppHandle, enabled: bool) -> Result<(), String> {
    let exe_path = env::current_exe().map_err(|e| format!("读取当前可执行文件路径失败: {e}"))?;

    if exe_path.to_string_lossy().is_empty() {
        return Err("可执行文件路径为空，无法设置开机启动".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        let quoted = format!("\"{}\"", exe_path.to_string_lossy());
        if enabled {
            let output = Command::new("reg")
                .args([
                    "add",
                    "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    "/v",
                    AUTOSTART_NAME,
                    "/t",
                    "REG_SZ",
                    "/d",
                    quoted.as_str(),
                    "/f",
                ])
                .output()
                .map_err(|e| format!("设置 Windows 开机启动失败: {e}"))?;
            if !output.status.success() {
                return Err(format_windows_command_failure(
                    "设置 Windows 开机启动",
                    &output,
                ));
            }
        } else {
            let query_output = Command::new("reg")
                .args([
                    "query",
                    "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    "/v",
                    AUTOSTART_NAME,
                ])
                .output()
                .map_err(|e| format!("查询 Windows 开机启动状态失败: {e}"))?;

            if query_output.status.success() {
                let delete_output = Command::new("reg")
                    .args([
                        "delete",
                        "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                        "/v",
                        AUTOSTART_NAME,
                        "/f",
                    ])
                    .output()
                    .map_err(|e| format!("移除 Windows 开机启动失败: {e}"))?;
                if !delete_output.status.success() {
                    return Err(format_windows_command_failure(
                        "移除 Windows 开机启动",
                        &delete_output,
                    ));
                }
            }
        }
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let home = resolve_home_dir()?;
        let launch_dir = home.join("Library").join("LaunchAgents");
        fs::create_dir_all(&launch_dir).map_err(|e| format!("创建 LaunchAgents 目录失败: {e}"))?;

        let plist_path = launch_dir.join(format!("{AUTOSTART_NAME}.plist"));
        if !enabled {
            match fs::remove_file(&plist_path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
                Err(e) => Err(format!("移除 LaunchAgent 文件失败: {e}")),
            }?;
            return Ok(());
        }

        let exe_path_s = exe_path.to_string_lossy();
        let mut plist = String::new();
        plist.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        plist.push_str("<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n");
        plist.push_str("<plist version=\"1.0\">\n<dict>\n");
        plist.push_str("  <key>Label</key>\n");
        let _ = writeln!(plist, "  <string>{}</string>", AUTOSTART_NAME);
        plist.push_str("  <key>ProgramArguments</key>\n  <array>\n");
        let _ = writeln!(plist, "    <string>{}</string>", exe_path_s);
        plist.push_str("  </array>\n");
        plist.push_str("  <key>RunAtLoad</key>\n  <true/>\n");
        plist.push_str("  <key>KeepAlive</key>\n  <false/>\n");
        plist.push_str("</dict>\n</plist>\n");

        let mut file =
            File::create(&plist_path).map_err(|e| format!("写入 LaunchAgent 文件失败: {e}"))?;
        use std::io::Write;
        file.write_all(plist.as_bytes())
            .map_err(|e| format!("写入 LaunchAgent 文件失败: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let home = resolve_home_dir()?;
        let base = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let autostart_dir = base.join("autostart");
        fs::create_dir_all(&autostart_dir).map_err(|e| format!("创建 autostart 目录失败: {e}"))?;

        let desktop_path = autostart_dir.join(format!("{AUTOSTART_NAME}.desktop"));
        if !enabled {
            match fs::remove_file(&desktop_path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
                Err(e) => Err(format!("移除自启动配置失败: {e}")),
            }?;
            return Ok(());
        }

        let exe_path_s = exe_path.to_string_lossy();
        let mut desktop = String::new();
        desktop.push_str("[Desktop Entry]\n");
        desktop.push_str("Type=Application\n");
        desktop.push_str("Name=WorkClaw\n");
        let _ = writeln!(desktop, "Exec={}", exe_path_s);
        desktop.push_str("X-GNOME-Autostart-enabled=true\n");
        fs::write(&desktop_path, desktop).map_err(|e| format!("写入 Desktop 文件失败: {e}"))?;
        return Ok(());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    Ok(())
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
    pub launch_at_login: Option<bool>,
    pub launch_minimized: Option<bool>,
    pub close_to_tray: Option<bool>,
    pub operation_permission_mode: Option<String>,
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

fn normalize_operation_permission_mode(raw: &str) -> String {
    match raw.trim() {
        "full_access" => "full_access".to_string(),
        _ => DEFAULT_OPERATION_PERMISSION_MODE.to_string(),
    }
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

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_autostart_failure_message_prefers_stderr() {
        use std::os::windows::process::ExitStatusExt;

        let output = std::process::Output {
            status: std::process::ExitStatus::from_raw(1),
            stdout: Vec::new(),
            stderr: b"The system was unable to find the specified registry key or value.".to_vec(),
        };

        let message = format_windows_command_failure("移除 Windows 开机启动", &output);
        assert_eq!(
            message,
            "移除 Windows 开机启动失败: The system was unable to find the specified registry key or value."
        );
    }
}
