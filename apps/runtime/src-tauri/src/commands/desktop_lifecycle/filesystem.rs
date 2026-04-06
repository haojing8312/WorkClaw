use super::types::{DesktopCleanupResult, DesktopLifecyclePaths};
use crate::runtime_bootstrap::BootstrapMigrationStatus;
use crate::runtime_environment::runtime_environment_from_app;
use std::fs;
use std::path::Path;
use std::process::Command;
use tauri::AppHandle;

fn bootstrap_migration_status_label(status: BootstrapMigrationStatus) -> String {
    match status {
        BootstrapMigrationStatus::Pending => "pending",
        BootstrapMigrationStatus::InProgress => "in_progress",
        BootstrapMigrationStatus::Failed => "failed",
        BootstrapMigrationStatus::Completed => "completed",
        BootstrapMigrationStatus::RolledBack => "rolled_back",
    }
    .to_string()
}

pub(crate) async fn resolve_desktop_lifecycle_paths(
    app: &AppHandle,
) -> Result<DesktopLifecyclePaths, String> {
    let environment = runtime_environment_from_app(app)?;
    let pending_runtime_root_dir = environment
        .bootstrap
        .pending_migration
        .as_ref()
        .map(|migration| migration.to_root.clone());
    let last_runtime_migration_status = environment
        .bootstrap
        .last_migration_result
        .as_ref()
        .map(|result| bootstrap_migration_status_label(result.status));
    let last_runtime_migration_message = environment
        .bootstrap
        .last_migration_result
        .as_ref()
        .and_then(|result| result.message.clone());

    Ok(DesktopLifecyclePaths {
        runtime_root_dir: environment.paths.root.to_string_lossy().to_string(),
        pending_runtime_root_dir,
        last_runtime_migration_status,
        last_runtime_migration_message,
    })
}

pub(crate) fn clear_directory_contents(path: &Path) -> Result<DesktopCleanupResult, String> {
    if !path.exists() {
        return Ok(DesktopCleanupResult::default());
    }

    let mut result = DesktopCleanupResult::default();
    let entries =
        fs::read_dir(path).map_err(|e| format!("读取目录失败 {}: {}", path.display(), e))?;
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

pub(crate) fn merge_cleanup_result(acc: &mut DesktopCleanupResult, next: DesktopCleanupResult) {
    acc.removed_files += next.removed_files;
    acc.removed_dirs += next.removed_dirs;
}

pub(crate) fn open_path_with_system(target: &Path) -> Result<(), String> {
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
