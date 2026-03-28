use serde::Serialize;
use std::path::PathBuf;

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
    pub audit_dir: String,
    pub crashes_dir: String,
    pub exports_dir: String,
    pub current_run_id: String,
    pub abnormal_previous_run: bool,
    pub last_clean_exit_at: Option<String>,
    pub latest_crash: Option<CrashSummaryInfo>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct RuntimeDiagnosticsTurnsSummary {
    pub active: usize,
    pub completed: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub average_latency_ms: u64,
    pub max_latency_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct RuntimeDiagnosticsAdmissionsSummary {
    pub conflicts: u64,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct RuntimeDiagnosticsTotalSummary {
    pub total: u64,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct RuntimeDiagnosticsCountEntry {
    pub kind: String,
    pub count: u64,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct RuntimeDiagnosticEventPreview {
    pub kind: String,
    pub event_type: Option<String>,
    pub session_id: Option<String>,
    pub run_id: Option<String>,
    pub created_at: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct RuntimeDiagnosticsSummary {
    pub turns: RuntimeDiagnosticsTurnsSummary,
    pub admissions: RuntimeDiagnosticsAdmissionsSummary,
    pub approvals: RuntimeDiagnosticsTotalSummary,
    pub child_sessions: RuntimeDiagnosticsTotalSummary,
    pub compaction: RuntimeDiagnosticsTotalSummary,
    pub guard_top_warning_kinds: Vec<RuntimeDiagnosticsCountEntry>,
    pub failover_top_error_kinds: Vec<RuntimeDiagnosticsCountEntry>,
    pub recent_event_preview: Vec<RuntimeDiagnosticEventPreview>,
    pub hints: Vec<String>,
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
    pub runtime_diagnostics_summary_json: String,
    pub runtime_diagnostics_summary_md: String,
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
