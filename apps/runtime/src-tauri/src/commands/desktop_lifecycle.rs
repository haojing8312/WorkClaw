use crate::commands::runtime_preferences::resolve_default_work_dir_with_pool;
use crate::commands::skills::DbState;
use crate::diagnostics::{self, ManagedDiagnosticsState};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager, State};
use zip::write::FileOptions;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DesktopLifecyclePaths {
    pub app_data_dir: String,
    pub cache_dir: String,
    pub log_dir: String,
    pub diagnostics_dir: String,
    pub default_work_dir: String,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct DesktopCleanupResult {
    pub removed_files: usize,
    pub removed_dirs: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CrashSummaryInfo {
    pub timestamp: String,
    pub message: String,
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DesktopDiagnosticsStatus {
    pub diagnostics_dir: String,
    pub logs_dir: String,
    pub crashes_dir: String,
    pub exports_dir: String,
    pub current_run_id: String,
    pub abnormal_previous_run: bool,
    pub last_clean_exit_at: Option<String>,
    pub latest_crash: Option<CrashSummaryInfo>,
}

#[derive(Debug, Clone)]
pub struct DesktopDiagnosticsExportPayload {
    pub environment_summary: String,
    pub route_attempt_logs_json: String,
    pub session_runs_json: String,
    pub session_run_events_json: String,
    pub latest_crash_json: Option<String>,
    pub runtime_log_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct FrontendDiagnosticPayload {
    pub kind: String,
    pub message: String,
    pub stack: Option<String>,
    pub source: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub href: Option<String>,
}

async fn resolve_desktop_lifecycle_paths(
    app: &AppHandle,
    pool: &SqlitePool,
) -> Result<DesktopLifecyclePaths, String> {
    let default_work_dir = resolve_default_work_dir_with_pool(pool)
        .await
        .unwrap_or_default();
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    let log_dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    let diagnostics_dir = diagnostics::DiagnosticsPaths::from_app(app).root;

    Ok(DesktopLifecyclePaths {
        app_data_dir: app_data_dir.to_string_lossy().to_string(),
        cache_dir: cache_dir.to_string_lossy().to_string(),
        log_dir: log_dir.to_string_lossy().to_string(),
        diagnostics_dir: diagnostics_dir.to_string_lossy().to_string(),
        default_work_dir,
    })
}

fn clear_directory_contents(path: &Path) -> Result<DesktopCleanupResult, String> {
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

fn read_last_clean_exit_at(paths: &diagnostics::DiagnosticsPaths) -> Option<String> {
    let path = paths.state_dir.join("last-clean-exit.json");
    let content = fs::read_to_string(path).ok()?;
    let parsed = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    parsed
        .get("ended_at")
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

fn build_diagnostics_status(
    state: &diagnostics::DiagnosticsState,
) -> Result<DesktopDiagnosticsStatus, String> {
    let latest_crash = diagnostics::read_latest_crash_summary(&state.paths)?.map(|summary| {
        CrashSummaryInfo {
            timestamp: summary.timestamp,
            message: summary.message,
            run_id: summary.run_id,
        }
    });

    Ok(DesktopDiagnosticsStatus {
        diagnostics_dir: state.paths.root.to_string_lossy().to_string(),
        logs_dir: state.paths.logs_dir.to_string_lossy().to_string(),
        crashes_dir: state.paths.crashes_dir.to_string_lossy().to_string(),
        exports_dir: state.paths.exports_dir.to_string_lossy().to_string(),
        current_run_id: state.run_id.clone(),
        abnormal_previous_run: state.abnormal_previous_run.was_abnormal_exit,
        last_clean_exit_at: read_last_clean_exit_at(&state.paths),
        latest_crash,
    })
}

fn build_desktop_environment_summary(
    version: &str,
    platform: &str,
    app_data_dir: &str,
    cache_dir: &str,
    log_dir: &str,
    default_work_dir: &str,
    diagnostics_status: &DesktopDiagnosticsStatus,
) -> String {
    format!(
        "# WorkClaw Environment Summary\n\n- Version: {version}\n- Platform: {platform}\n- Application Data: {app_data_dir}\n- Cache: {cache_dir}\n- Logs: {log_dir}\n- Default Workspace: {}\n- Diagnostics: {}\n- Diagnostics Logs: {}\n- Diagnostics Crashes: {}\n- Diagnostics Exports: {}\n- Current Run ID: {}\n- Abnormal Previous Run: {}\n- Last Clean Exit: {}\n- Latest Crash: {}\n",
        if default_work_dir.trim().is_empty() {
            "未设置".to_string()
        } else {
            default_work_dir.to_string()
        },
        diagnostics_status.diagnostics_dir,
        diagnostics_status.logs_dir,
        diagnostics_status.crashes_dir,
        diagnostics_status.exports_dir,
        diagnostics_status.current_run_id,
        if diagnostics_status.abnormal_previous_run {
            "yes"
        } else {
            "no"
        },
        diagnostics_status
            .last_clean_exit_at
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        diagnostics_status
            .latest_crash
            .as_ref()
            .map(|crash| format!("{} {}", crash.timestamp, crash.message))
            .unwrap_or_else(|| "none".to_string())
    )
}

fn list_recent_runtime_log_files(paths: &diagnostics::DiagnosticsPaths) -> Result<Vec<PathBuf>, String> {
    if !paths.logs_dir.exists() {
        return Ok(Vec::new());
    }
    let mut files: Vec<PathBuf> = fs::read_dir(&paths.logs_dir)
        .map_err(|e| format!("读取诊断日志目录失败 {}: {}", paths.logs_dir.display(), e))?
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .filter(|path| path.extension().and_then(|v| v.to_str()) == Some("jsonl"))
        .collect();
    files.sort();
    files.reverse();
    files.truncate(3);
    Ok(files)
}

fn export_diagnostics_bundle(
    export_dir: &Path,
    run_id: &str,
    payload: &DesktopDiagnosticsExportPayload,
) -> Result<PathBuf, String> {
    fs::create_dir_all(export_dir)
        .map_err(|e| format!("创建诊断导出目录失败 {}: {}", export_dir.display(), e))?;
    let archive_path = export_dir.join(format!(
        "diagnostics-{}-{}.zip",
        run_id,
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    ));
    let file = fs::File::create(&archive_path)
        .map_err(|e| format!("创建诊断包失败 {}: {}", archive_path.display(), e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = FileOptions::default();

    let mut add_text = |name: &str, content: &str| -> Result<(), String> {
        zip.start_file(name, options)
            .map_err(|e| format!("写入诊断包文件 {} 失败: {}", name, e))?;
        zip.write_all(content.as_bytes())
            .map_err(|e| format!("写入诊断包内容 {} 失败: {}", name, e))
    };

    add_text("environment-summary.md", &payload.environment_summary)?;
    add_text("route-attempt-logs.json", &payload.route_attempt_logs_json)?;
    add_text("session-runs.json", &payload.session_runs_json)?;
    add_text("session-run-events.json", &payload.session_run_events_json)?;
    if let Some(crash_json) = &payload.latest_crash_json {
        add_text("latest-crash.json", crash_json)?;
    }

    for log_path in &payload.runtime_log_files {
        if let Ok(content) = fs::read_to_string(log_path) {
            let file_name = log_path
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("runtime.jsonl");
            add_text(&format!("logs/{file_name}"), &content)?;
        }
    }

    zip.finish()
        .map_err(|e| format!("完成诊断包失败 {}: {}", archive_path.display(), e))?;
    Ok(archive_path)
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
    diagnostics_state: State<'_, ManagedDiagnosticsState>,
) -> Result<String, String> {
    let paths = resolve_desktop_lifecycle_paths(&app, &db.0).await?;
    let status = build_diagnostics_status(diagnostics_state.0.as_ref())?;
    Ok(build_desktop_environment_summary(
        &app.package_info().version.to_string(),
        std::env::consts::OS,
        &paths.app_data_dir,
        &paths.cache_dir,
        &paths.log_dir,
        &paths.default_work_dir,
        &status,
    ))
}

#[tauri::command]
pub async fn get_desktop_diagnostics_status(
    diagnostics_state: State<'_, ManagedDiagnosticsState>,
) -> Result<DesktopDiagnosticsStatus, String> {
    build_diagnostics_status(diagnostics_state.0.as_ref())
}

#[tauri::command]
pub async fn open_desktop_diagnostics_dir(
    diagnostics_state: State<'_, ManagedDiagnosticsState>,
) -> Result<(), String> {
    open_path_with_system(&diagnostics_state.0.paths.root)
}

#[tauri::command]
pub async fn export_desktop_diagnostics_bundle(
    app: AppHandle,
    db: State<'_, DbState>,
    diagnostics_state: State<'_, ManagedDiagnosticsState>,
) -> Result<String, String> {
    let lifecycle_paths = resolve_desktop_lifecycle_paths(&app, &db.0).await?;
    let diagnostics_status = build_diagnostics_status(diagnostics_state.0.as_ref())?;
    let environment_summary = build_desktop_environment_summary(
        &app.package_info().version.to_string(),
        std::env::consts::OS,
        &lifecycle_paths.app_data_dir,
        &lifecycle_paths.cache_dir,
        &lifecycle_paths.log_dir,
        &lifecycle_paths.default_work_dir,
        &diagnostics_status,
    );

    let route_attempt_logs = sqlx::query_as::<_, (String, String, String, String, i64, i64, String, bool, String, String)>(
        "SELECT session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, success, error_message, created_at
         FROM route_attempt_logs ORDER BY created_at DESC LIMIT 200"
    )
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;
    let session_runs = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT session_id, status, error_kind, error_message, created_at, updated_at
         FROM session_runs ORDER BY updated_at DESC LIMIT 100"
    )
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;
    let session_run_events = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT session_id, event_type, payload_json, created_at
         FROM session_run_events ORDER BY created_at DESC LIMIT 100"
    )
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    let route_attempt_logs_json = serde_json::to_string_pretty(
        &route_attempt_logs
            .into_iter()
            .map(|row| {
                serde_json::json!({
                    "session_id": row.0,
                    "capability": row.1,
                    "api_format": row.2,
                    "model_name": row.3,
                    "attempt_index": row.4,
                    "retry_index": row.5,
                    "error_kind": row.6,
                    "success": row.7,
                    "error_message": row.8,
                    "created_at": row.9,
                })
            })
            .collect::<Vec<_>>(),
    )
    .map_err(|e| e.to_string())?;
    let session_runs_json = serde_json::to_string_pretty(
        &session_runs
            .into_iter()
            .map(|row| {
                serde_json::json!({
                    "session_id": row.0,
                    "status": row.1,
                    "error_kind": row.2,
                    "error_message": row.3,
                    "created_at": row.4,
                    "updated_at": row.5,
                })
            })
            .collect::<Vec<_>>(),
    )
    .map_err(|e| e.to_string())?;
    let session_run_events_json = serde_json::to_string_pretty(
        &session_run_events
            .into_iter()
            .map(|row| {
                let payload_preview: String = row.2.chars().take(400).collect();
                serde_json::json!({
                    "session_id": row.0,
                    "event_type": row.1,
                    "payload_preview": payload_preview,
                    "created_at": row.3,
                })
            })
            .collect::<Vec<_>>(),
    )
    .map_err(|e| e.to_string())?;
    let latest_crash_json = diagnostics::read_latest_crash_summary(&diagnostics_state.0.paths)?
        .map(|summary| serde_json::to_string_pretty(&summary))
        .transpose()
        .map_err(|e| e.to_string())?;
    let runtime_log_files = list_recent_runtime_log_files(&diagnostics_state.0.paths)?;

    let bundle = export_diagnostics_bundle(
        &diagnostics_state.0.paths.exports_dir,
        &diagnostics_state.0.run_id,
        &DesktopDiagnosticsExportPayload {
            environment_summary,
            route_attempt_logs_json,
            session_runs_json,
            session_run_events_json,
            latest_crash_json,
            runtime_log_files,
        },
    )?;

    let _ = diagnostics::write_log_record(
        &diagnostics_state.0.paths,
        diagnostics::LogLevel::Info,
        "diagnostics",
        "export_bundle",
        "desktop diagnostics bundle exported",
        Some(serde_json::json!({
            "path": bundle.to_string_lossy().to_string(),
        })),
    );

    Ok(bundle.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn record_frontend_diagnostic_event(
    payload: FrontendDiagnosticPayload,
    diagnostics_state: State<'_, ManagedDiagnosticsState>,
) -> Result<(), String> {
    diagnostics::write_log_record(
        &diagnostics_state.0.paths,
        diagnostics::LogLevel::Error,
        "frontend",
        &payload.kind,
        &payload.message,
        Some(serde_json::json!({
            "stack": payload.stack,
            "source": payload.source,
            "line": payload.line,
            "column": payload.column,
            "href": payload.href,
        })),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        build_desktop_environment_summary, clear_directory_contents, export_diagnostics_bundle,
        CrashSummaryInfo, DesktopDiagnosticsExportPayload, DesktopDiagnosticsStatus,
    };
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

    #[test]
    fn environment_summary_includes_diagnostics_section() {
        let status = DesktopDiagnosticsStatus {
            diagnostics_dir: "C:\\Users\\me\\AppData\\Roaming\\WorkClaw\\diagnostics".to_string(),
            logs_dir: "C:\\Users\\me\\AppData\\Roaming\\WorkClaw\\diagnostics\\logs".to_string(),
            crashes_dir: "C:\\Users\\me\\AppData\\Roaming\\WorkClaw\\diagnostics\\crashes"
                .to_string(),
            exports_dir: "C:\\Users\\me\\AppData\\Roaming\\WorkClaw\\diagnostics\\exports"
                .to_string(),
            current_run_id: "run-1".to_string(),
            abnormal_previous_run: true,
            last_clean_exit_at: Some("2026-03-13T09:59:00Z".to_string()),
            latest_crash: Some(CrashSummaryInfo {
                timestamp: "2026-03-13T10:00:00Z".to_string(),
                message: "panic occurred".to_string(),
                run_id: Some("run-0".to_string()),
            }),
        };

        let summary = build_desktop_environment_summary(
            "0.2.12",
            "windows",
            "C:\\Users\\me\\AppData\\Roaming\\WorkClaw",
            "C:\\Users\\me\\AppData\\Local\\WorkClaw\\cache",
            "C:\\Users\\me\\AppData\\Local\\WorkClaw\\logs",
            "E:\\workspace",
            &status,
        );

        assert!(summary.contains("Diagnostics"));
        assert!(summary.contains("Abnormal Previous Run: yes"));
        assert!(summary.contains("Latest Crash: 2026-03-13T10:00:00Z panic occurred"));
    }

    #[test]
    fn exports_diagnostics_bundle_zip() {
        let dir = tempdir().expect("temp dir");
        let export_dir = dir.path().join("exports");
        std::fs::create_dir_all(&export_dir).expect("create export dir");

        let payload = DesktopDiagnosticsExportPayload {
            environment_summary: "# WorkClaw Environment Summary".to_string(),
            route_attempt_logs_json: "[]".to_string(),
            session_runs_json: "[]".to_string(),
            session_run_events_json: "[]".to_string(),
            latest_crash_json: Some("{\"message\":\"panic occurred\"}".to_string()),
            runtime_log_files: Vec::new(),
        };

        let zip_path =
            export_diagnostics_bundle(&export_dir, "run-1", &payload).expect("export bundle");

        assert!(zip_path.exists());
        assert_eq!(zip_path.extension().and_then(|v| v.to_str()), Some("zip"));
    }
}
