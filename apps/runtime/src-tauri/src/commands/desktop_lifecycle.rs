#[path = "desktop_lifecycle/types.rs"]
mod types;

#[path = "desktop_lifecycle/filesystem.rs"]
mod filesystem;

#[path = "desktop_lifecycle/database_snapshot.rs"]
mod database_snapshot;

#[path = "desktop_lifecycle/diagnostics_service.rs"]
mod diagnostics_service;

use crate::agent::runtime::{
    RuntimeObservabilitySnapshot, RuntimeObservabilityState, RuntimeObservedEvent,
};
use crate::commands::skills::DbState;
use crate::diagnostics::ManagedDiagnosticsState;
use crate::runtime_environment::runtime_paths_from_app;
use crate::runtime_root_migration;
use std::collections::HashSet;
use std::path::Path;
use tauri::{AppHandle, Manager, State};

pub(crate) use database_snapshot::{collect_database_counts, collect_database_storage_snapshot};
pub(crate) use diagnostics_service::{build_desktop_environment_summary, build_diagnostics_status};
pub(crate) use filesystem::{
    clear_directory_contents, merge_cleanup_result, open_path_with_system,
    resolve_desktop_lifecycle_paths,
};
pub(crate) use types::{
    DesktopCleanupResult, DesktopDiagnosticsStatus, DesktopLifecyclePaths,
    FrontendDiagnosticPayload,
};

#[tauri::command]
pub async fn get_desktop_lifecycle_paths(app: AppHandle) -> Result<DesktopLifecyclePaths, String> {
    resolve_desktop_lifecycle_paths(&app).await
}

#[tauri::command]
pub async fn open_desktop_path(path: String) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("目录路径不能为空".to_string());
    }
    open_path_with_system(Path::new(trimmed))
}

#[tauri::command]
pub async fn clear_desktop_cache_and_logs(app: AppHandle) -> Result<DesktopCleanupResult, String> {
    let mut result = DesktopCleanupResult::default();
    let mut seen = HashSet::new();
    let runtime_paths = runtime_paths_from_app(&app)?;
    let candidate_dirs = [runtime_paths.cache_dir, runtime_paths.diagnostics.logs_dir];

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
pub fn schedule_desktop_runtime_root_migration(
    app: AppHandle,
    target_root: String,
) -> Result<(), String> {
    let runtime_environment = app.state::<crate::runtime_environment::ManagedRuntimeEnvironment>();
    let trimmed = target_root.trim();
    if trimmed.is_empty() {
        return Err("目录路径不能为空".to_string());
    }

    runtime_root_migration::schedule_runtime_root_migration(
        &runtime_environment.0.bootstrap_location.bootstrap_path,
        Path::new(trimmed),
    )
    .map_err(|error| error.to_string())?;

    app.restart();
}

#[tauri::command]
pub async fn export_desktop_environment_summary(
    app: AppHandle,
    _db: State<'_, DbState>,
    diagnostics_state: State<'_, ManagedDiagnosticsState>,
) -> Result<String, String> {
    let paths = resolve_desktop_lifecycle_paths(&app).await?;
    let status = build_diagnostics_status(diagnostics_state.0.as_ref())?;
    Ok(build_desktop_environment_summary(
        &app.package_info().version.to_string(),
        std::env::consts::OS,
        &paths.runtime_root_dir,
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
pub async fn get_runtime_observability_snapshot(
    observability: State<'_, RuntimeObservabilityState>,
) -> Result<RuntimeObservabilitySnapshot, String> {
    Ok(observability.0.snapshot())
}

#[tauri::command]
pub async fn get_runtime_recent_events(
    observability: State<'_, RuntimeObservabilityState>,
    limit: Option<usize>,
) -> Result<Vec<RuntimeObservedEvent>, String> {
    let mut events = observability.0.recent_events();
    if let Some(limit) = limit {
        if events.len() > limit {
            let start = events.len().saturating_sub(limit);
            events = events.split_off(start);
        }
    }
    Ok(events)
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
    diagnostics_service::export_desktop_diagnostics_bundle(
        &app,
        &db.0,
        diagnostics_state.0.as_ref(),
    )
    .await
}

#[tauri::command]
pub async fn record_frontend_diagnostic_event(
    payload: FrontendDiagnosticPayload,
    diagnostics_state: State<'_, ManagedDiagnosticsState>,
) -> Result<(), String> {
    diagnostics_service::record_frontend_diagnostic_event(&payload, diagnostics_state.0.as_ref())
}

#[cfg(test)]
mod tests {
    use super::diagnostics_service::export_diagnostics_bundle;
    use super::types::{
        CrashSummaryInfo, DesktopDiagnosticsExportPayload, DesktopDiagnosticsStatus,
    };
    use super::{build_desktop_environment_summary, clear_directory_contents};
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
            audit_dir: "C:\\Users\\me\\AppData\\Roaming\\WorkClaw\\diagnostics\\audit".to_string(),
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
            &status,
        );

        assert!(summary.contains("Diagnostics"));
        assert!(summary.contains("Runtime Root: C:\\Users\\me\\AppData\\Roaming\\WorkClaw"));
        assert!(summary.contains("Diagnostics Audit"));
        assert!(summary.contains("Abnormal Previous Run: yes"));
        assert!(summary.contains("Latest Crash: 2026-03-13T10:00:00Z panic occurred"));
    }

    #[test]
    fn exports_diagnostics_bundle_zip() {
        let dir = tempdir().expect("temp dir");
        let export_dir = dir.path().join("exports");
        std::fs::create_dir_all(&export_dir).expect("create export dir");

        let payload = DesktopDiagnosticsExportPayload {
            environment_summary: "# Environment Summary".to_string(),
            route_attempt_logs_json: "[]".to_string(),
            session_runs_json: "[]".to_string(),
            session_run_events_json: "[]".to_string(),
            session_run_traces_json: "[]".to_string(),
            runtime_observability_snapshot_json: "{}".to_string(),
            runtime_recent_events_json: "[]".to_string(),
            latest_crash_json: Some("{\"message\":\"panic occurred\"}".to_string()),
            runtime_log_files: Vec::new(),
            audit_log_files: Vec::new(),
        };

        let zip_path =
            export_diagnostics_bundle(&export_dir, "run-1", &payload).expect("export bundle");

        assert!(zip_path.exists());
        assert_eq!(zip_path.extension().and_then(|v| v.to_str()), Some("zip"));
        let file = std::fs::File::open(&zip_path).expect("open zip");
        let mut archive = zip::ZipArchive::new(file).expect("read zip");
        let _ = archive
            .by_name("session-run-traces.json")
            .expect("session run traces entry in zip");
        let _ = archive
            .by_name("runtime-observability-snapshot.json")
            .expect("observability snapshot entry in zip");
        let _ = archive
            .by_name("runtime-recent-events.json")
            .expect("runtime recent events entry in zip");
    }

    #[test]
    fn exports_audit_logs_in_diagnostics_bundle_zip() {
        let dir = tempdir().expect("temp dir");
        let export_dir = dir.path().join("exports");
        std::fs::create_dir_all(&export_dir).expect("create export dir");
        let audit_log = dir.path().join("audit-2026-03-20.jsonl");
        std::fs::write(&audit_log, "{\"event\":\"create_session\"}\n").expect("write audit");

        let payload = DesktopDiagnosticsExportPayload {
            environment_summary: "# Environment Summary".to_string(),
            route_attempt_logs_json: "[]".to_string(),
            session_runs_json: "[]".to_string(),
            session_run_events_json: "[]".to_string(),
            session_run_traces_json: "[]".to_string(),
            runtime_observability_snapshot_json: "{}".to_string(),
            runtime_recent_events_json: "[]".to_string(),
            latest_crash_json: None,
            runtime_log_files: Vec::new(),
            audit_log_files: vec![audit_log],
        };

        let zip_path =
            export_diagnostics_bundle(&export_dir, "run-2", &payload).expect("export bundle");

        let file = std::fs::File::open(&zip_path).expect("open zip");
        let mut archive = zip::ZipArchive::new(file).expect("read zip");
        let mut audit_entry = archive
            .by_name("audit/audit-2026-03-20.jsonl")
            .expect("audit entry in zip");
        let mut content = String::new();
        use std::io::Read;
        audit_entry
            .read_to_string(&mut content)
            .expect("read audit entry");
        assert!(content.contains("create_session"));
    }
}
