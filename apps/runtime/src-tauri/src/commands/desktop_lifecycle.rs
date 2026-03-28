#[path = "desktop_lifecycle/types.rs"]
mod types;

#[path = "desktop_lifecycle/filesystem.rs"]
mod filesystem;

#[path = "desktop_lifecycle/database_snapshot.rs"]
mod database_snapshot;

#[path = "desktop_lifecycle/diagnostics_service.rs"]
mod diagnostics_service;

use crate::commands::skills::DbState;
use crate::agent::runtime::{RuntimeObservability, RuntimeObservabilityState};
use crate::diagnostics::ManagedDiagnosticsState;
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
    FrontendDiagnosticPayload, RuntimeDiagnosticsSummary,
};

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
    open_path_with_system(Path::new(trimmed))
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
pub async fn get_runtime_diagnostics_summary(
    app: AppHandle,
) -> Result<RuntimeDiagnosticsSummary, String> {
    let (snapshot, recent_events) = if let Some(observability) =
        app.try_state::<RuntimeObservabilityState>()
    {
        (observability.0.snapshot(), observability.0.recent_events())
    } else {
        let observability = RuntimeObservability::default();
        (observability.snapshot(), observability.recent_events())
    };
    Ok(diagnostics_service::build_runtime_diagnostics_summary(
        &snapshot,
        &recent_events,
    ))
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
    diagnostics_service::export_desktop_diagnostics_bundle(&app, &db.0, diagnostics_state.0.as_ref())
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
    use super::diagnostics_service::{
        build_runtime_diagnostics_summary, export_diagnostics_bundle,
    };
    use super::types::{
        CrashSummaryInfo, DesktopDiagnosticsExportPayload, DesktopDiagnosticsStatus,
        RuntimeDiagnosticEventPreview, RuntimeDiagnosticsCountEntry,
        RuntimeDiagnosticsSummary,
    };
    use super::{build_desktop_environment_summary, clear_directory_contents};
    use crate::agent::runtime::{
        RuntimeObservedEvent, RuntimeObservedRunEvent, RuntimeObservability,
    };
    use tempfile::tempdir;

    fn observed_run_event(
        session_id: &str,
        run_id: &str,
        event_type: &str,
        created_at: &str,
    ) -> RuntimeObservedRunEvent {
        RuntimeObservedRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.to_string(),
            event_type: event_type.to_string(),
            created_at: created_at.to_string(),
            status: None,
            tool_name: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            child_session_id: None,
            message: None,
        }
    }

    fn build_summary_fixture() -> (RuntimeDiagnosticsSummary, Vec<RuntimeObservedEvent>) {
        let subject = RuntimeObservability::new(32);
        subject.record_admission_conflict("session-1");
        subject.record_admission_conflict("session-2");
        subject.record_compaction_run();
        subject.record_failover_error_kind("network");
        subject.record_failover_error_kind("network");
        subject.record_failover_error_kind("network");
        subject.record_failover_error_kind("network");
        subject.record_failover_error_kind("timeout");

        subject.record_recent_event(RuntimeObservedEvent::SessionRun(observed_run_event(
            "session-1",
            "run-1",
            "run_started",
            "2026-03-28T10:00:00Z",
        )));
        let mut warning = observed_run_event(
            "session-1",
            "run-1",
            "run_guard_warning",
            "2026-03-28T10:00:01Z",
        );
        warning.warning_kind = Some("loop_detected".to_string());
        warning.message = Some("loop detected".to_string());
        subject.record_recent_event(RuntimeObservedEvent::SessionRun(warning));

        let mut warning = observed_run_event(
            "session-1",
            "run-1",
            "run_guard_warning",
            "2026-03-28T10:00:02Z",
        );
        warning.warning_kind = Some("tool_timeout".to_string());
        warning.message = Some("tool timed out".to_string());
        subject.record_recent_event(RuntimeObservedEvent::SessionRun(warning));

        let mut approval = observed_run_event(
            "session-1",
            "run-1",
            "approval_requested",
            "2026-03-28T10:00:03Z",
        );
        approval.approval_id = Some("approval-1".to_string());
        approval.child_session_id = Some("child-1".to_string());
        subject.record_recent_event(RuntimeObservedEvent::SessionRun(approval));

        let mut child_tool = observed_run_event(
            "session-1",
            "run-1",
            "tool_started",
            "2026-03-28T10:00:04Z",
        );
        child_tool.child_session_id = Some("child-1".to_string());
        child_tool.tool_name = Some("search".to_string());
        subject.record_recent_event(RuntimeObservedEvent::SessionRun(child_tool));

        subject.record_recent_event(RuntimeObservedEvent::SessionRun(observed_run_event(
            "session-1",
            "run-1",
            "run_completed",
            "2026-03-28T10:00:05Z",
        )));
        subject.record_recent_event(RuntimeObservedEvent::SessionRun(observed_run_event(
            "session-2",
            "run-2",
            "run_started",
            "2026-03-28T10:01:00Z",
        )));
        let mut failed = observed_run_event(
            "session-2",
            "run-2",
            "run_failed",
            "2026-03-28T10:01:04Z",
        );
        failed.error_kind = Some("network".to_string());
        failed.message = Some("provider unavailable".to_string());
        subject.record_recent_event(RuntimeObservedEvent::SessionRun(failed));
        subject.record_recent_event(RuntimeObservedEvent::SessionRun(observed_run_event(
            "session-3",
            "run-3",
            "run_started",
            "2026-03-28T10:02:00Z",
        )));
        subject.record_recent_event(RuntimeObservedEvent::SessionRun(observed_run_event(
            "session-3",
            "run-3",
            "run_completed",
            "2026-03-28T10:02:03Z",
        )));
        subject.record_recent_event(RuntimeObservedEvent::SessionRun(observed_run_event(
            "session-3",
            "run-3",
            "tool_started",
            "2026-03-28T10:02:04Z",
        )));

        let snapshot = subject.snapshot();
        let recent_events = subject.recent_events();
        let summary = build_runtime_diagnostics_summary(&snapshot, &recent_events);
        (summary, recent_events)
    }

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
            "C:\\Users\\me\\AppData\\Local\\WorkClaw\\cache",
            "C:\\Users\\me\\AppData\\Local\\WorkClaw\\logs",
            "E:\\workspace",
            &status,
        );

        assert!(summary.contains("Diagnostics"));
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
            environment_summary: "# WorkClaw Environment Summary".to_string(),
            route_attempt_logs_json: "[]".to_string(),
            session_runs_json: "[]".to_string(),
            session_run_events_json: "[]".to_string(),
            session_run_traces_json: "[]".to_string(),
            runtime_observability_snapshot_json: "{}".to_string(),
            runtime_recent_events_json: "[]".to_string(),
            runtime_diagnostics_summary_json: "{\"turns\":{\"active\":0,\"completed\":0,\"failed\":0,\"cancelled\":0,\"average_latency_ms\":0,\"max_latency_ms\":0},\"admissions\":{\"conflicts\":0},\"guard_top_warning_kinds\":[],\"failover_top_error_kinds\":[],\"recent_event_preview\":[],\"hints\":[]}".to_string(),
            runtime_diagnostics_summary_md: "# Runtime Diagnostics Summary".to_string(),
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
        let _ = archive
            .by_name("runtime-diagnostics-summary.json")
            .expect("runtime diagnostics summary json entry in zip");
        let _ = archive
            .by_name("runtime-diagnostics-summary.md")
            .expect("runtime diagnostics summary md entry in zip");
    }

    #[test]
    fn exports_audit_logs_in_diagnostics_bundle_zip() {
        let dir = tempdir().expect("temp dir");
        let export_dir = dir.path().join("exports");
        std::fs::create_dir_all(&export_dir).expect("create export dir");
        let audit_log = dir.path().join("audit-2026-03-20.jsonl");
        std::fs::write(&audit_log, "{\"event\":\"create_session\"}\n").expect("write audit");

        let payload = DesktopDiagnosticsExportPayload {
            environment_summary: "# WorkClaw Environment Summary".to_string(),
            route_attempt_logs_json: "[]".to_string(),
            session_runs_json: "[]".to_string(),
            session_run_events_json: "[]".to_string(),
            session_run_traces_json: "[]".to_string(),
            runtime_observability_snapshot_json: "{}".to_string(),
            runtime_recent_events_json: "[]".to_string(),
            runtime_diagnostics_summary_json: "{\"turns\":{\"active\":0,\"completed\":0,\"failed\":0,\"cancelled\":0,\"average_latency_ms\":0,\"max_latency_ms\":0},\"admissions\":{\"conflicts\":0},\"guard_top_warning_kinds\":[],\"failover_top_error_kinds\":[],\"recent_event_preview\":[],\"hints\":[]}".to_string(),
            runtime_diagnostics_summary_md: "# Runtime Diagnostics Summary".to_string(),
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

    #[test]
    fn summary_builder_emits_compact_counts_and_hints() {
        let (summary, recent_events) = build_summary_fixture();

        assert_eq!(summary.turns.active, 0);
        assert_eq!(summary.turns.completed, 2);
        assert_eq!(summary.turns.failed, 1);
        assert_eq!(summary.admissions.conflicts, 2);
        assert_eq!(
            summary.guard_top_warning_kinds,
            vec![
                RuntimeDiagnosticsCountEntry {
                    kind: "loop_detected".to_string(),
                    count: 1,
                },
                RuntimeDiagnosticsCountEntry {
                    kind: "tool_timeout".to_string(),
                    count: 1,
                },
            ]
        );
        assert_eq!(
            summary.failover_top_error_kinds,
            vec![
                RuntimeDiagnosticsCountEntry {
                    kind: "network".to_string(),
                    count: 4,
                },
                RuntimeDiagnosticsCountEntry {
                    kind: "timeout".to_string(),
                    count: 1,
                },
            ]
        );
        assert_eq!(summary.recent_event_preview.len(), 10);
        assert!(matches!(
            summary.recent_event_preview.first(),
            Some(RuntimeDiagnosticEventPreview {
                run_id: Some(run_id),
                ..
            }) if run_id == "run-1"
        ));
        assert!(summary
            .hints
            .iter()
            .any(|hint| hint.contains("Admission conflicts: 2") || hint.contains("conflicts")));
        assert!(summary
            .hints
            .iter()
            .any(|hint| hint.contains("network")));
        assert!(summary
            .hints
            .iter()
            .any(|hint| hint.contains("loop")));
        assert_eq!(recent_events.len(), 13);
    }

    #[test]
    fn export_bundle_includes_runtime_diagnostics_summary_files() {
        let dir = tempdir().expect("temp dir");
        let export_dir = dir.path().join("exports");
        std::fs::create_dir_all(&export_dir).expect("create export dir");

        let payload = DesktopDiagnosticsExportPayload {
            environment_summary: "# WorkClaw Environment Summary".to_string(),
            route_attempt_logs_json: "[]".to_string(),
            session_runs_json: "[]".to_string(),
            session_run_events_json: "[]".to_string(),
            session_run_traces_json: "[]".to_string(),
            runtime_observability_snapshot_json: "{\"admissions\":{\"conflicts\":2}}".to_string(),
            runtime_recent_events_json: "[]".to_string(),
            runtime_diagnostics_summary_json: "{\"admissions\":{\"conflicts\":2},\"hints\":[\"Admission conflicts: 2\"]}".to_string(),
            runtime_diagnostics_summary_md: "# Runtime Diagnostics Summary\n\n- Admission conflicts: 2".to_string(),
            latest_crash_json: None,
            runtime_log_files: Vec::new(),
            audit_log_files: Vec::new(),
        };

        let zip_path =
            export_diagnostics_bundle(&export_dir, "run-3", &payload).expect("export bundle");

        let file = std::fs::File::open(&zip_path).expect("open zip");
        let mut archive = zip::ZipArchive::new(file).expect("read zip");
        use std::io::Read;
        let summary_json_content = {
            let mut summary_json = archive
                .by_name("runtime-diagnostics-summary.json")
                .expect("summary json entry in zip");
            let mut content = String::new();
            summary_json
                .read_to_string(&mut content)
                .expect("read summary json");
            content
        };
        assert!(
            summary_json_content.contains("\"conflicts\": 2")
                || summary_json_content.contains("\"conflicts\":2")
        );

        let summary_md_content = {
            let mut summary_md = archive
                .by_name("runtime-diagnostics-summary.md")
                .expect("summary md entry in zip");
            let mut content = String::new();
            summary_md
                .read_to_string(&mut content)
                .expect("read summary md");
            content
        };
        assert!(summary_md_content.contains("Admission conflicts: 2"));
    }
}
