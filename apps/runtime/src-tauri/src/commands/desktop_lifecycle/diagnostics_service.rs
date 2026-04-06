use super::types::{
    CrashSummaryInfo, DesktopDiagnosticsExportPayload, DesktopDiagnosticsStatus,
    FrontendDiagnosticPayload,
};
use crate::agent::runtime::RuntimeObservabilityState;
use crate::commands::session_runs::export_session_run_trace_with_pool;
use crate::diagnostics::{self};
use sqlx::SqlitePool;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use zip::write::FileOptions;

fn read_last_clean_exit_at(paths: &diagnostics::DiagnosticsPaths) -> Option<String> {
    let path = paths.state_dir.join("last-clean-exit.json");
    let content = fs::read_to_string(path).ok()?;
    let parsed = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    parsed
        .get("ended_at")
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

pub(crate) fn build_diagnostics_status(
    state: &diagnostics::DiagnosticsState,
) -> Result<DesktopDiagnosticsStatus, String> {
    let latest_crash =
        diagnostics::read_latest_crash_summary(&state.paths)?.map(|summary| CrashSummaryInfo {
            timestamp: summary.timestamp,
            message: summary.message,
            run_id: summary.run_id,
        });

    Ok(DesktopDiagnosticsStatus {
        diagnostics_dir: state.paths.root.to_string_lossy().to_string(),
        logs_dir: state.paths.logs_dir.to_string_lossy().to_string(),
        audit_dir: state.paths.audit_dir.to_string_lossy().to_string(),
        crashes_dir: state.paths.crashes_dir.to_string_lossy().to_string(),
        exports_dir: state.paths.exports_dir.to_string_lossy().to_string(),
        current_run_id: state.run_id.clone(),
        abnormal_previous_run: state.abnormal_previous_run.was_abnormal_exit,
        last_clean_exit_at: read_last_clean_exit_at(&state.paths),
        latest_crash,
    })
}

pub(crate) fn build_desktop_environment_summary(
    version: &str,
    platform: &str,
    runtime_root_dir: &str,
    diagnostics_status: &DesktopDiagnosticsStatus,
) -> String {
    format!(
        "# WorkClaw Environment Summary\n\n- Version: {version}\n- Platform: {platform}\n- Runtime Root: {runtime_root_dir}\n- Diagnostics: {}\n- Diagnostics Logs: {}\n- Diagnostics Audit: {}\n- Diagnostics Crashes: {}\n- Diagnostics Exports: {}\n- Current Run ID: {}\n- Abnormal Previous Run: {}\n- Last Clean Exit: {}\n- Latest Crash: {}\n",
        diagnostics_status.diagnostics_dir,
        diagnostics_status.logs_dir,
        diagnostics_status.audit_dir,
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

fn list_recent_runtime_log_files(
    paths: &diagnostics::DiagnosticsPaths,
) -> Result<Vec<PathBuf>, String> {
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

fn list_recent_audit_log_files(
    paths: &diagnostics::DiagnosticsPaths,
) -> Result<Vec<PathBuf>, String> {
    if !paths.audit_dir.exists() {
        return Ok(Vec::new());
    }
    let mut files: Vec<PathBuf> = fs::read_dir(&paths.audit_dir)
        .map_err(|e| format!("读取审计日志目录失败 {}: {}", paths.audit_dir.display(), e))?
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .filter(|path| path.extension().and_then(|v| v.to_str()) == Some("jsonl"))
        .collect();
    files.sort();
    files.reverse();
    files.truncate(3);
    Ok(files)
}

pub(crate) fn export_diagnostics_bundle(
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
    add_text("session-run-traces.json", &payload.session_run_traces_json)?;
    add_text(
        "runtime-observability-snapshot.json",
        &payload.runtime_observability_snapshot_json,
    )?;
    add_text(
        "runtime-recent-events.json",
        &payload.runtime_recent_events_json,
    )?;
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
    for audit_path in &payload.audit_log_files {
        if let Ok(content) = fs::read_to_string(audit_path) {
            let file_name = audit_path
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("audit.jsonl");
            add_text(&format!("audit/{file_name}"), &content)?;
        }
    }

    zip.finish()
        .map_err(|e| format!("完成诊断包失败 {}: {}", archive_path.display(), e))?;
    Ok(archive_path)
}

pub(crate) async fn export_desktop_diagnostics_bundle(
    app: &AppHandle,
    pool: &SqlitePool,
    diagnostics_state: &diagnostics::DiagnosticsState,
) -> Result<String, String> {
    let lifecycle_paths = super::filesystem::resolve_desktop_lifecycle_paths(app).await?;
    let diagnostics_status = build_diagnostics_status(diagnostics_state)?;
    let environment_summary = build_desktop_environment_summary(
        &app.package_info().version.to_string(),
        std::env::consts::OS,
        &lifecycle_paths.runtime_root_dir,
        &diagnostics_status,
    );

    let route_attempt_logs = sqlx::query_as::<_, (String, String, String, String, i64, i64, String, bool, String, String)>(
        "SELECT session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, success, error_message, created_at
         FROM route_attempt_logs ORDER BY created_at DESC LIMIT 200"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let session_runs =
        sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
            "SELECT id, session_id, status, error_kind, error_message, created_at, updated_at
         FROM session_runs ORDER BY updated_at DESC LIMIT 100",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
    let session_run_events = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT session_id, event_type, payload_json, created_at
         FROM session_run_events ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(pool)
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
                    "run_id": row.0,
                    "session_id": row.1,
                    "status": row.2,
                    "error_kind": row.3,
                    "error_message": row.4,
                    "created_at": row.5,
                    "updated_at": row.6,
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
    let trace_runs = sqlx::query_as::<_, (String, String)>(
        "SELECT id, session_id
         FROM session_runs
         ORDER BY updated_at DESC
         LIMIT 25",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let mut trace_exports = Vec::new();
    for (run_id, session_id) in trace_runs {
        match export_session_run_trace_with_pool(pool, &session_id, &run_id).await {
            Ok(trace) => {
                trace_exports.push(serde_json::to_value(trace).map_err(|e| e.to_string())?)
            }
            Err(error) => trace_exports.push(serde_json::json!({
                "session_id": session_id,
                "run_id": run_id,
                "error": error,
            })),
        }
    }
    let session_run_traces_json =
        serde_json::to_string_pretty(&trace_exports).map_err(|e| e.to_string())?;
    let (runtime_observability_snapshot_json, runtime_recent_events_json) =
        if let Some(observability) = app.try_state::<RuntimeObservabilityState>() {
            (
                serde_json::to_string_pretty(&observability.0.snapshot())
                    .map_err(|e| e.to_string())?,
                serde_json::to_string_pretty(&observability.0.recent_events())
                    .map_err(|e| e.to_string())?,
            )
        } else {
            ("{}".to_string(), "[]".to_string())
        };
    let latest_crash_json = diagnostics::read_latest_crash_summary(&diagnostics_state.paths)?
        .map(|summary| serde_json::to_string_pretty(&summary))
        .transpose()
        .map_err(|e| e.to_string())?;
    let runtime_log_files = list_recent_runtime_log_files(&diagnostics_state.paths)?;
    let audit_log_files = list_recent_audit_log_files(&diagnostics_state.paths)?;

    let bundle = export_diagnostics_bundle(
        &diagnostics_state.paths.exports_dir,
        &diagnostics_state.run_id,
        &DesktopDiagnosticsExportPayload {
            environment_summary,
            route_attempt_logs_json,
            session_runs_json,
            session_run_events_json,
            session_run_traces_json,
            runtime_observability_snapshot_json,
            runtime_recent_events_json,
            latest_crash_json,
            runtime_log_files,
            audit_log_files,
        },
    )?;

    let _ = diagnostics::write_log_record(
        &diagnostics_state.paths,
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

pub(crate) fn record_frontend_diagnostic_event(
    payload: &FrontendDiagnosticPayload,
    diagnostics_state: &diagnostics::DiagnosticsState,
) -> Result<(), String> {
    diagnostics::write_log_record(
        &diagnostics_state.paths,
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
