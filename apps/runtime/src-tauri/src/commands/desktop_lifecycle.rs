use crate::commands::runtime_preferences::resolve_default_work_dir_with_pool;
use crate::commands::skills::DbState;
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DesktopLifecyclePaths {
    pub app_data_dir: String,
    pub cache_dir: String,
    pub log_dir: String,
    pub default_work_dir: String,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct DesktopCleanupResult {
    pub removed_files: usize,
    pub removed_dirs: usize,
}

async fn resolve_desktop_lifecycle_paths(
    app: &AppHandle,
    pool: &SqlitePool,
) -> Result<DesktopLifecyclePaths, String> {
    let default_work_dir = resolve_default_work_dir_with_pool(pool).await.unwrap_or_default();
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    let log_dir = app.path().app_log_dir().map_err(|e| e.to_string())?;

    Ok(DesktopLifecyclePaths {
        app_data_dir: app_data_dir.to_string_lossy().to_string(),
        cache_dir: cache_dir.to_string_lossy().to_string(),
        log_dir: log_dir.to_string_lossy().to_string(),
        default_work_dir,
    })
}

fn clear_directory_contents(path: &Path) -> Result<DesktopCleanupResult, String> {
    if !path.exists() {
        return Ok(DesktopCleanupResult::default());
    }

    let mut result = DesktopCleanupResult::default();
    let entries = fs::read_dir(path).map_err(|e| format!("读取目录失败 {}: {}", path.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败 {}: {}", path.display(), e))?;
        let target = entry.path();
        if target.is_dir() {
            fs::remove_dir_all(&target)
                .map_err(|e| format!("删除目录失败 {}: {}", target.display(), e))?;
            result.removed_dirs += 1;
        } else {
            fs::remove_file(&target)
                .map_err(|e| format!("删除文件失败 {}: {}", target.display(), e))?;
            result.removed_files += 1;
        }
    }
    Ok(result)
}

fn merge_cleanup_result(acc: &mut DesktopCleanupResult, next: DesktopCleanupResult) {
    acc.removed_files += next.removed_files;
    acc.removed_dirs += next.removed_dirs;
}

fn open_path_with_system(target: &Path) -> Result<(), String> {
    if !target.exists() {
        return Err(format!("目录不存在: {}", target.display()));
    }

    #[cfg(target_os = "windows")]
    let status = Command::new("explorer").arg(target).status();

    #[cfg(target_os = "macos")]
    let status = Command::new("open").arg(target).status();

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let status = Command::new("xdg-open").arg(target).status();

    let status = status.map_err(|e| format!("打开目录失败 {}: {}", target.display(), e))?;
    if !status.success() {
        return Err(format!("打开目录失败: {}", target.display()));
    }
    Ok(())
}

#[tauri::command]
pub async fn get_desktop_lifecycle_paths(
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<DesktopLifecyclePaths, String> {
    resolve_desktop_lifecycle_paths(&app, &db.0).await
}

#[tauri::command]
pub async fn open_desktop_path(path: String) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("目录路径不能为空".to_string());
    }
    open_path_with_system(&PathBuf::from(trimmed))
}

#[tauri::command]
pub async fn clear_desktop_cache_and_logs(app: AppHandle) -> Result<DesktopCleanupResult, String> {
    let mut result = DesktopCleanupResult::default();
    let mut seen = HashSet::new();
    let candidate_dirs = [
        app.path().app_cache_dir().map_err(|e| e.to_string())?,
        app.path().app_log_dir().map_err(|e| e.to_string())?,
    ];

    for dir in candidate_dirs {
        let key = dir.to_string_lossy().to_string();
        if !seen.insert(key) {
            continue;
        }
        merge_cleanup_result(&mut result, clear_directory_contents(&dir)?);
    }

    Ok(result)
}

#[tauri::command]
pub async fn export_desktop_environment_summary(
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let paths = resolve_desktop_lifecycle_paths(&app, &db.0).await?;
    let version = app.package_info().version.to_string();
    let summary = format!(
        "# WorkClaw Environment Summary\n\n- Version: {version}\n- Platform: {}\n- Application Data: {}\n- Cache: {}\n- Logs: {}\n- Default Workspace: {}\n",
        std::env::consts::OS,
        paths.app_data_dir,
        paths.cache_dir,
        paths.log_dir,
        if paths.default_work_dir.trim().is_empty() {
            "未设置".to_string()
        } else {
            paths.default_work_dir
        }
    );
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::clear_directory_contents;
    use tempfile::tempdir;

    #[test]
    fn clear_directory_contents_removes_top_level_files_and_dirs() {
        let dir = tempdir().expect("temp dir");
        std::fs::write(dir.path().join("cache.log"), "log").expect("write file");
        std::fs::create_dir_all(dir.path().join("nested")).expect("create nested dir");
        std::fs::write(dir.path().join("nested").join("trace.txt"), "trace")
            .expect("write nested file");

        let result = clear_directory_contents(dir.path()).expect("clear contents");

        assert_eq!(result.removed_files, 1);
        assert_eq!(result.removed_dirs, 1);
        assert!(dir.path().read_dir().expect("read dir").next().is_none());
    }
}
