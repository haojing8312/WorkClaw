use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DesktopLifecyclePaths {
    pub runtime_root_dir: String,
    pub pending_runtime_root_dir: Option<String>,
    pub last_runtime_migration_status: Option<String>,
    pub last_runtime_migration_message: Option<String>,
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
    pub audit_dir: String,
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
    pub session_run_traces_json: String,
    pub runtime_observability_snapshot_json: String,
    pub runtime_recent_events_json: String,
    pub latest_crash_json: Option<String>,
    pub runtime_log_files: Vec<PathBuf>,
    pub audit_log_files: Vec<PathBuf>,
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
