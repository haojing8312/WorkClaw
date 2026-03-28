use super::types::{
    CrashSummaryInfo, DesktopDiagnosticsExportPayload, DesktopDiagnosticsStatus,
    FrontendDiagnosticPayload, RuntimeDiagnosticEventPreview,
    RuntimeDiagnosticsAdmissionsSummary, RuntimeDiagnosticsCountEntry,
    RuntimeDiagnosticsSummary, RuntimeDiagnosticsTotalSummary, RuntimeDiagnosticsTurnsSummary,
};
use crate::agent::runtime::{
    observability::RuntimeObservabilitySnapshot, RuntimeObservedEvent, RuntimeObservedRunEvent,
    RuntimeObservabilityState,
};
use crate::commands::session_runs::export_session_run_trace_with_pool;
use crate::diagnostics::{self};
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use zip::write::FileOptions;

const SUMMARY_TOP_KIND_LIMIT: usize = 5;
const SUMMARY_RECENT_EVENT_PREVIEW_LIMIT: usize = 10;

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
    app_data_dir: &str,
    cache_dir: &str,
    log_dir: &str,
    default_work_dir: &str,
    diagnostics_status: &DesktopDiagnosticsStatus,
) -> String {
    format!(
        "# WorkClaw Environment Summary\n\n- Version: {version}\n- Platform: {platform}\n- Application Data: {app_data_dir}\n- Cache: {cache_dir}\n- Logs: {log_dir}\n- Default Workspace: {}\n- Diagnostics: {}\n- Diagnostics Logs: {}\n- Diagnostics Audit: {}\n- Diagnostics Crashes: {}\n- Diagnostics Exports: {}\n- Current Run ID: {}\n- Abnormal Previous Run: {}\n- Last Clean Exit: {}\n- Latest Crash: {}\n",
        if default_work_dir.trim().is_empty() {
            "未设置".to_string()
        } else {
            default_work_dir.to_string()
        },
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

fn top_count_entries(map: &BTreeMap<String, u64>) -> Vec<RuntimeDiagnosticsCountEntry> {
    let mut entries: Vec<(String, u64)> = map.iter().map(|(kind, count)| (kind.clone(), *count)).collect();
    entries.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    entries.truncate(SUMMARY_TOP_KIND_LIMIT);
    entries
        .into_iter()
        .map(|(kind, count)| RuntimeDiagnosticsCountEntry { kind, count })
        .collect()
}

fn is_loop_like_warning(kind: &str) -> bool {
    let lowered = kind.trim().to_ascii_lowercase();
    lowered.contains("loop") || lowered.contains("repeat") || lowered.contains("ping_pong")
}

fn has_buffered_loop_guard_signal(recent_events: &[RuntimeObservedEvent]) -> bool {
    recent_events.iter().any(|event| match event {
        RuntimeObservedEvent::SessionRun(run_event) => run_event
            .warning_kind
            .as_deref()
            .is_some_and(is_loop_like_warning),
        RuntimeObservedEvent::AdmissionConflict(_) => false,
    })
}

fn summarize_run_detail(event: &RuntimeObservedRunEvent) -> Option<String> {
    let mut details = Vec::new();
    if let Some(status) = event.status.as_deref().filter(|value| !value.trim().is_empty()) {
        details.push(format!("status={status}"));
    }
    if let Some(tool_name) = event.tool_name.as_deref().filter(|value| !value.trim().is_empty()) {
        details.push(format!("tool={tool_name}"));
    }
    if let Some(approval_id) = event
        .approval_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        details.push(format!("approval={approval_id}"));
    }
    if let Some(warning_kind) = event
        .warning_kind
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        details.push(format!("warning={warning_kind}"));
    }
    if let Some(error_kind) = event
        .error_kind
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        details.push(format!("error={error_kind}"));
    }
    if let Some(child_session_id) = event
        .child_session_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        details.push(format!("child_session={child_session_id}"));
    }
    if let Some(message) = event.message.as_deref().filter(|value| !value.trim().is_empty()) {
        details.push(format!("message={message}"));
    }

    if details.is_empty() {
        None
    } else {
        Some(details.join(", "))
    }
}

fn summarize_runtime_observed_event(event: &RuntimeObservedEvent) -> RuntimeDiagnosticEventPreview {
    match event {
        RuntimeObservedEvent::SessionRun(run_event) => RuntimeDiagnosticEventPreview {
            kind: "session_run".to_string(),
            event_type: Some(run_event.event_type.clone()),
            session_id: Some(run_event.session_id.clone()),
            run_id: Some(run_event.run_id.clone()),
            created_at: run_event.created_at.clone(),
            detail: summarize_run_detail(run_event),
        },
        RuntimeObservedEvent::AdmissionConflict(conflict) => RuntimeDiagnosticEventPreview {
            kind: "admission_conflict".to_string(),
            event_type: None,
            session_id: Some(conflict.session_id.clone()),
            run_id: None,
            created_at: conflict.created_at.clone(),
            detail: Some(format!("{}: {}", conflict.code, conflict.message)),
        },
    }
}

pub(crate) fn build_runtime_diagnostics_summary(
    snapshot: &RuntimeObservabilitySnapshot,
    recent_events: &[RuntimeObservedEvent],
) -> RuntimeDiagnosticsSummary {
    let mut recent_event_preview: Vec<RuntimeDiagnosticEventPreview> = recent_events
        .iter()
        .rev()
        .take(SUMMARY_RECENT_EVENT_PREVIEW_LIMIT)
        .map(summarize_runtime_observed_event)
        .collect();
    recent_event_preview.reverse();

    let mut summary = RuntimeDiagnosticsSummary {
        turns: RuntimeDiagnosticsTurnsSummary {
            active: snapshot.turns.active,
            completed: snapshot.turns.completed,
            failed: snapshot.turns.failed,
            cancelled: snapshot.turns.cancelled,
            average_latency_ms: snapshot.turns.average_latency_ms,
            max_latency_ms: snapshot.turns.max_latency_ms,
        },
        admissions: RuntimeDiagnosticsAdmissionsSummary {
            conflicts: snapshot.admissions.conflicts,
        },
        approvals: RuntimeDiagnosticsTotalSummary {
            total: snapshot.approvals.requested_total,
        },
        child_sessions: RuntimeDiagnosticsTotalSummary {
            total: snapshot.child_sessions.linked_total,
        },
        compaction: RuntimeDiagnosticsTotalSummary {
            total: snapshot.compaction.runs,
        },
        guard_top_warning_kinds: top_count_entries(&snapshot.guard.warnings_by_kind),
        failover_top_error_kinds: top_count_entries(&snapshot.failover.errors_by_kind),
        recent_event_preview,
        hints: Vec::new(),
    };

    if summary.admissions.conflicts > 0 {
        summary.hints.push(format!(
            "Admission conflicts: {}. Session serialization is blocking overlapping runs.",
            summary.admissions.conflicts
        ));
    }
    if has_buffered_loop_guard_signal(recent_events) {
        summary
            .hints
            .push("loop guard activity observed in buffered runtime events.".to_string());
    }
    if let Some(top_failover_kind) = summary.failover_top_error_kinds.first() {
        summary.hints.push(format!(
            "Top tracked failover error kind: {}.",
            top_failover_kind.kind
        ));
    }

    summary
}

fn render_runtime_diagnostics_summary_markdown(summary: &RuntimeDiagnosticsSummary) -> String {
    let mut lines = vec![
        "# Runtime Diagnostics Summary".to_string(),
        String::new(),
        "## Turns".to_string(),
        format!("- Active: {}", summary.turns.active),
        format!("- Completed: {}", summary.turns.completed),
        format!("- Failed: {}", summary.turns.failed),
        format!("- Cancelled: {}", summary.turns.cancelled),
        format!("- Average latency (ms): {}", summary.turns.average_latency_ms),
        format!("- Max latency (ms): {}", summary.turns.max_latency_ms),
        String::new(),
        "## Admissions".to_string(),
        format!("- Admission conflicts: {}", summary.admissions.conflicts),
        String::new(),
        "## Approvals".to_string(),
        format!("- Requested total: {}", summary.approvals.total),
        String::new(),
        "## Child Sessions".to_string(),
        format!("- Linked total: {}", summary.child_sessions.total),
        String::new(),
        "## Compaction".to_string(),
        format!("- Runs: {}", summary.compaction.total),
    ];

    if !summary.guard_top_warning_kinds.is_empty() {
        lines.push(String::new());
        lines.push("## Top Guard Warnings".to_string());
        for entry in &summary.guard_top_warning_kinds {
            lines.push(format!("- {}: {}", entry.kind, entry.count));
        }
    }

    if !summary.failover_top_error_kinds.is_empty() {
        lines.push(String::new());
        lines.push("## Top Failover Errors".to_string());
        for entry in &summary.failover_top_error_kinds {
            lines.push(format!("- {}: {}", entry.kind, entry.count));
        }
    }

    if !summary.recent_event_preview.is_empty() {
        lines.push(String::new());
        lines.push("## Recent Event Preview".to_string());
        for event in &summary.recent_event_preview {
            let mut detail = vec![format!("{} {}", event.created_at, event.kind)];
            if let Some(event_type) = event.event_type.as_deref() {
                detail.push(format!("type={event_type}"));
            }
            if let Some(session_id) = event.session_id.as_deref() {
                detail.push(format!("session={session_id}"));
            }
            if let Some(run_id) = event.run_id.as_deref() {
                detail.push(format!("run={run_id}"));
            }
            if let Some(extra) = event.detail.as_deref() {
                detail.push(extra.to_string());
            }
            lines.push(format!("- {}", detail.join(" | ")));
        }
    }

    if !summary.hints.is_empty() {
        lines.push(String::new());
        lines.push("## Hints".to_string());
        for hint in &summary.hints {
            lines.push(format!("- {hint}"));
        }
    }

    lines.join("\n")
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
    add_text("runtime-recent-events.json", &payload.runtime_recent_events_json)?;
    add_text(
        "runtime-diagnostics-summary.json",
        &payload.runtime_diagnostics_summary_json,
    )?;
    add_text(
        "runtime-diagnostics-summary.md",
        &payload.runtime_diagnostics_summary_md,
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
    let lifecycle_paths = super::filesystem::resolve_desktop_lifecycle_paths(app, pool).await?;
    let diagnostics_status = build_diagnostics_status(diagnostics_state)?;
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
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let session_runs = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
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
            Ok(trace) => trace_exports.push(serde_json::to_value(trace).map_err(|e| e.to_string())?),
            Err(error) => trace_exports.push(serde_json::json!({
                "session_id": session_id,
                "run_id": run_id,
                "error": error,
            })),
        }
    }
    let session_run_traces_json =
        serde_json::to_string_pretty(&trace_exports).map_err(|e| e.to_string())?;
    let (
        runtime_observability_snapshot_json,
        runtime_recent_events_json,
        runtime_diagnostics_summary_json,
        runtime_diagnostics_summary_md,
    ) = if let Some(observability) = app.try_state::<RuntimeObservabilityState>() {
        let snapshot = observability.0.snapshot();
        let recent_events = observability.0.recent_events();
        let summary = build_runtime_diagnostics_summary(&snapshot, &recent_events);
        (
            serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?,
            serde_json::to_string_pretty(&recent_events).map_err(|e| e.to_string())?,
            serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?,
            render_runtime_diagnostics_summary_markdown(&summary),
        )
    } else {
        let summary = RuntimeDiagnosticsSummary::default();
        (
            "{}".to_string(),
            "[]".to_string(),
            serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?,
            render_runtime_diagnostics_summary_markdown(&summary),
        )
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
            runtime_diagnostics_summary_json,
            runtime_diagnostics_summary_md,
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
