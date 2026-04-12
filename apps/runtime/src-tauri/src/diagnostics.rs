use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::backtrace::Backtrace;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::AppHandle;

#[derive(Debug, Clone)]
pub struct DiagnosticsPaths {
    pub root: PathBuf,
    pub logs_dir: PathBuf,
    pub audit_dir: PathBuf,
    pub crashes_dir: PathBuf,
    pub exports_dir: PathBuf,
    pub state_dir: PathBuf,
}

impl DiagnosticsPaths {
    pub fn new(root: PathBuf) -> Self {
        Self {
            logs_dir: root.join("logs"),
            audit_dir: root.join("audit"),
            crashes_dir: root.join("crashes"),
            exports_dir: root.join("exports"),
            state_dir: root.join("state"),
            root,
        }
    }

    pub fn from_app(app: &AppHandle) -> Self {
        if let Ok(runtime_paths) = crate::runtime_environment::runtime_paths_from_app(app) {
            return Self::from_runtime_paths(&runtime_paths);
        }

        let fallback = crate::runtime_paths::RuntimePaths::new(crate::runtime_paths::resolve_runtime_root());
        Self::from_runtime_paths(&fallback)
    }

    pub fn from_runtime_paths(runtime_paths: &crate::runtime_paths::RuntimePaths) -> Self {
        Self::new(runtime_paths.diagnostics.root.clone())
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Serialize)]
struct LogRecord<'a> {
    timestamp: String,
    level: LogLevel,
    source: &'a str,
    event: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<Value>,
}

#[derive(Debug, Serialize)]
struct AuditRecord<'a> {
    timestamp: String,
    source: &'a str,
    event: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrashSummary {
    pub timestamp: String,
    pub thread: String,
    pub message: String,
    pub location: Option<String>,
    pub backtrace: Option<String>,
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunStateMarker {
    pub run_id: String,
    pub started_at: String,
    #[serde(default)]
    pub pid: u32,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AbnormalRunStatus {
    pub was_abnormal_exit: bool,
    pub previous_run_id: Option<String>,
    pub previous_started_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DiagnosticsState {
    pub paths: DiagnosticsPaths,
    pub run_id: String,
    pub version: String,
    pub abnormal_previous_run: AbnormalRunStatus,
}

pub struct ManagedDiagnosticsState(pub Arc<DiagnosticsState>);

pub fn ensure_diagnostics_dirs(paths: &DiagnosticsPaths) -> Result<(), String> {
    for dir in [
        &paths.root,
        &paths.logs_dir,
        &paths.audit_dir,
        &paths.crashes_dir,
        &paths.exports_dir,
        &paths.state_dir,
    ] {
        fs::create_dir_all(dir)
            .map_err(|e| format!("创建诊断目录失败 {}: {}", dir.display(), e))?;
    }
    Ok(())
}

fn append_jsonl_record(path: &Path, json: &str, error_prefix: &str) -> Result<PathBuf, String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("{error_prefix} {}: {}", path.display(), e))?;
    writeln!(file, "{json}").map_err(|e| format!("写入诊断日志失败 {}: {}", path.display(), e))?;
    Ok(path.to_path_buf())
}

pub fn write_log_record(
    paths: &DiagnosticsPaths,
    level: LogLevel,
    source: &str,
    event: &str,
    message: &str,
    context: Option<Value>,
) -> Result<PathBuf, String> {
    ensure_diagnostics_dirs(paths)?;
    let filename = format!("runtime-{}.jsonl", Utc::now().format("%Y-%m-%d"));
    let path = paths.logs_dir.join(filename);
    let record = LogRecord {
        timestamp: Utc::now().to_rfc3339(),
        level,
        source,
        event,
        message,
        context,
    };
    let json = serde_json::to_string(&record).map_err(|e| e.to_string())?;
    append_jsonl_record(&path, &json, "打开诊断日志失败")
}

pub fn write_audit_record(
    paths: &DiagnosticsPaths,
    source: &str,
    event: &str,
    message: &str,
    context: Option<Value>,
) -> Result<PathBuf, String> {
    ensure_diagnostics_dirs(paths)?;
    let filename = format!("audit-{}.jsonl", Utc::now().format("%Y-%m-%d"));
    let path = paths.audit_dir.join(filename);
    let record = AuditRecord {
        timestamp: Utc::now().to_rfc3339(),
        source,
        event,
        message,
        context,
    };
    let json = serde_json::to_string(&record).map_err(|e| e.to_string())?;
    append_jsonl_record(&path, &json, "打开审计日志失败")
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FileSnapshot {
    pub path: String,
    pub exists: bool,
    pub size_bytes: Option<u64>,
    pub modified_at: Option<String>,
}

pub fn collect_file_snapshot(path: &Path) -> FileSnapshot {
    match fs::metadata(path) {
        Ok(metadata) => FileSnapshot {
            path: path.to_string_lossy().to_string(),
            exists: true,
            size_bytes: Some(metadata.len()),
            modified_at: metadata
                .modified()
                .ok()
                .map(|value| chrono::DateTime::<Utc>::from(value).to_rfc3339()),
        },
        Err(_) => FileSnapshot {
            path: path.to_string_lossy().to_string(),
            exists: false,
            size_bytes: None,
            modified_at: None,
        },
    }
}

pub fn collect_sqlite_storage_snapshot(app_data_dir: &Path) -> Value {
    let db_path = app_data_dir.join("workclaw.db");
    let wal_path = app_data_dir.join("workclaw.db-wal");
    let shm_path = app_data_dir.join("workclaw.db-shm");
    serde_json::json!({
        "db": collect_file_snapshot(&db_path),
        "wal": collect_file_snapshot(&wal_path),
        "shm": collect_file_snapshot(&shm_path),
    })
}

fn active_run_marker_path(paths: &DiagnosticsPaths) -> PathBuf {
    paths.state_dir.join("active-run.json")
}

fn clean_exit_marker_path(paths: &DiagnosticsPaths) -> PathBuf {
    paths.state_dir.join("last-clean-exit.json")
}

pub fn write_active_run_marker(
    paths: &DiagnosticsPaths,
    marker: &RunStateMarker,
) -> Result<(), String> {
    ensure_diagnostics_dirs(paths)?;
    let payload = serde_json::to_vec_pretty(marker).map_err(|e| e.to_string())?;
    fs::write(active_run_marker_path(paths), payload)
        .map_err(|e| format!("写入运行状态失败: {}", e))
}

pub fn clear_active_run_marker(paths: &DiagnosticsPaths) -> Result<(), String> {
    let active = active_run_marker_path(paths);
    if active.exists() {
        fs::remove_file(&active)
            .map_err(|e| format!("清理运行状态失败 {}: {}", active.display(), e))?;
    }
    Ok(())
}

pub fn write_clean_exit_marker(paths: &DiagnosticsPaths, run_id: &str) -> Result<(), String> {
    ensure_diagnostics_dirs(paths)?;
    let payload = serde_json::json!({
        "run_id": run_id,
        "ended_at": Utc::now().to_rfc3339(),
    });
    fs::write(
        clean_exit_marker_path(paths),
        serde_json::to_vec_pretty(&payload).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("写入正常退出标记失败: {}", e))
}

pub fn detect_abnormal_previous_run(paths: &DiagnosticsPaths) -> Result<AbnormalRunStatus, String> {
    let active = active_run_marker_path(paths);
    if !active.exists() {
        return Ok(AbnormalRunStatus::default());
    }
    let content = fs::read_to_string(&active)
        .map_err(|e| format!("读取运行状态失败 {}: {}", active.display(), e))?;
    let marker: RunStateMarker =
        serde_json::from_str(&content).map_err(|e| format!("解析运行状态失败: {}", e))?;
    Ok(AbnormalRunStatus {
        was_abnormal_exit: true,
        previous_run_id: Some(marker.run_id),
        previous_started_at: Some(marker.started_at),
    })
}

pub fn record_crash_summary(
    paths: &DiagnosticsPaths,
    summary: &CrashSummary,
) -> Result<PathBuf, String> {
    ensure_diagnostics_dirs(paths)?;
    let filename = format!(
        "crash-{}.json",
        summary
            .timestamp
            .replace(':', "-")
            .replace('T', "_")
            .replace('Z', "Z")
    );
    let path = paths.crashes_dir.join(filename);
    fs::write(
        &path,
        serde_json::to_vec_pretty(summary).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("写入崩溃摘要失败 {}: {}", path.display(), e))?;
    Ok(path)
}

pub fn read_latest_crash_summary(paths: &DiagnosticsPaths) -> Result<Option<CrashSummary>, String> {
    if !paths.crashes_dir.exists() {
        return Ok(None);
    }
    let mut latest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(&paths.crashes_dir)
        .map_err(|e| format!("读取崩溃目录失败 {}: {}", paths.crashes_dir.display(), e))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .map_err(|e| format!("读取崩溃文件时间失败 {}: {}", path.display(), e))?;
        match &latest {
            Some((current, _)) if modified <= *current => {}
            _ => latest = Some((modified, path)),
        }
    }

    let Some((_, path)) = latest else {
        return Ok(None);
    };
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("读取崩溃摘要失败 {}: {}", path.display(), e))?;
    let summary = serde_json::from_str(&content)
        .map_err(|e| format!("解析崩溃摘要失败 {}: {}", path.display(), e))?;
    Ok(Some(summary))
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    "panic occurred".to_string()
}

pub fn install_panic_hook(paths: DiagnosticsPaths, run_id: String) {
    let paths = Arc::new(paths);
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let summary = CrashSummary {
            timestamp: Utc::now().to_rfc3339(),
            thread: std::thread::current()
                .name()
                .unwrap_or("unnamed")
                .to_string(),
            message: panic_message(info.payload()),
            location: info
                .location()
                .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column())),
            backtrace: Some(Backtrace::force_capture().to_string()),
            run_id: Some(run_id.clone()),
        };
        let _ = record_crash_summary(&paths, &summary);
        let _ = write_log_record(
            &paths,
            LogLevel::Error,
            "runtime",
            "panic",
            &summary.message,
            Some(serde_json::json!({
                "thread": summary.thread,
                "location": summary.location,
            })),
        );
        previous_hook(info);
    }));
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn initialize_for_app(app: &AppHandle, version: String) -> Result<DiagnosticsState, String> {
    let paths = DiagnosticsPaths::from_app(app);
    initialize_with_paths(paths, version)
}

pub fn initialize_with_paths(
    paths: DiagnosticsPaths,
    version: String,
) -> Result<DiagnosticsState, String> {
    ensure_diagnostics_dirs(&paths)?;
    let abnormal_previous_run = detect_abnormal_previous_run(&paths)?;
    let run_id = uuid::Uuid::new_v4().to_string();
    write_active_run_marker(
        &paths,
        &RunStateMarker {
            run_id: run_id.clone(),
            started_at: Utc::now().to_rfc3339(),
            pid: std::process::id(),
            version: version.clone(),
        },
    )?;
    install_panic_hook(paths.clone(), run_id.clone());
    let _ = write_log_record(
        &paths,
        LogLevel::Info,
        "runtime",
        "startup",
        "diagnostics initialized",
        Some(serde_json::json!({
            "run_id": run_id,
            "abnormal_previous_run": abnormal_previous_run.was_abnormal_exit,
        })),
    );
    Ok(DiagnosticsState {
        paths,
        run_id,
        version,
        abnormal_previous_run,
    })
}

pub fn mark_clean_exit(state: &DiagnosticsState) -> Result<(), String> {
    write_clean_exit_marker(&state.paths, &state.run_id)?;
    clear_active_run_marker(&state.paths)?;
    let _ = write_log_record(
        &state.paths,
        LogLevel::Info,
        "runtime",
        "shutdown",
        "clean shutdown",
        Some(serde_json::json!({
            "run_id": state.run_id,
        })),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use std::path::Path;
    use tempfile::tempdir;

    use super::{
        clear_active_run_marker, collect_sqlite_storage_snapshot, detect_abnormal_previous_run,
        ensure_diagnostics_dirs, read_latest_crash_summary, record_crash_summary,
        write_audit_record, write_log_record, CrashSummary, DiagnosticsPaths, LogLevel,
    };

    fn diagnostics_root(root: &Path) -> DiagnosticsPaths {
        DiagnosticsPaths::new(root.to_path_buf())
    }

    #[test]
    fn ensures_diagnostics_directory_tree() {
        let dir = tempdir().expect("temp dir");
        let paths = diagnostics_root(dir.path());

        ensure_diagnostics_dirs(&paths).expect("ensure diagnostics dirs");

        assert!(paths.root.exists());
        assert!(paths.logs_dir.exists());
        assert!(paths.audit_dir.exists());
        assert!(paths.crashes_dir.exists());
        assert!(paths.exports_dir.exists());
        assert!(paths.state_dir.exists());
    }

    #[test]
    fn writes_jsonl_runtime_log_record() {
        let dir = tempdir().expect("temp dir");
        let paths = diagnostics_root(dir.path());
        ensure_diagnostics_dirs(&paths).expect("ensure diagnostics dirs");

        let log_path = write_log_record(
            &paths,
            LogLevel::Info,
            "runtime",
            "startup",
            "startup complete",
            Some(json!({"version":"0.2.12"})),
        )
        .expect("write log record");

        let content = std::fs::read_to_string(log_path).expect("read log");
        assert!(content.contains("\"source\":\"runtime\""));
        assert!(content.contains("\"event\":\"startup\""));
        assert!(content.contains("\"message\":\"startup complete\""));
    }

    #[test]
    fn writes_jsonl_audit_record() {
        let dir = tempdir().expect("temp dir");
        let paths = diagnostics_root(dir.path());
        ensure_diagnostics_dirs(&paths).expect("ensure diagnostics dirs");

        let log_path = write_audit_record(
            &paths,
            "session",
            "create_session",
            "session created",
            Some(json!({"session_id":"s1"})),
        )
        .expect("write audit record");

        let content = std::fs::read_to_string(log_path).expect("read audit log");
        assert!(content.contains("\"source\":\"session\""));
        assert!(content.contains("\"event\":\"create_session\""));
        assert!(content.contains("\"message\":\"session created\""));
    }

    #[test]
    fn collects_sqlite_storage_snapshot_for_db_and_wal_files() {
        let dir = tempdir().expect("temp dir");
        std::fs::write(dir.path().join("workclaw.db"), "db").expect("write db");
        std::fs::write(dir.path().join("workclaw.db-wal"), "wal").expect("write wal");

        let snapshot = collect_sqlite_storage_snapshot(dir.path());

        assert_eq!(snapshot["db"]["exists"].as_bool(), Some(true));
        assert_eq!(snapshot["wal"]["exists"].as_bool(), Some(true));
        assert_eq!(snapshot["shm"]["exists"].as_bool(), Some(false));
    }

    #[test]
    fn detects_abnormal_previous_run_from_stale_active_marker() {
        let dir = tempdir().expect("temp dir");
        let paths = diagnostics_root(dir.path());
        ensure_diagnostics_dirs(&paths).expect("ensure diagnostics dirs");
        std::fs::write(
            paths.state_dir.join("active-run.json"),
            r#"{"run_id":"r1","started_at":"2026-03-13T10:00:00Z"}"#,
        )
        .expect("write active marker");

        let status = detect_abnormal_previous_run(&paths).expect("detect abnormal run");
        assert!(status.was_abnormal_exit);
        assert_eq!(status.previous_run_id.as_deref(), Some("r1"));
    }

    #[test]
    fn clears_active_run_marker_after_clean_exit() {
        let dir = tempdir().expect("temp dir");
        let paths = diagnostics_root(dir.path());
        ensure_diagnostics_dirs(&paths).expect("ensure diagnostics dirs");
        std::fs::write(
            paths.state_dir.join("active-run.json"),
            r#"{"run_id":"r1","started_at":"2026-03-13T10:00:00Z"}"#,
        )
        .expect("write active marker");

        clear_active_run_marker(&paths).expect("clear marker");

        assert!(!paths.state_dir.join("active-run.json").exists());
    }

    #[test]
    fn reads_latest_crash_summary_metadata() {
        let dir = tempdir().expect("temp dir");
        let paths = diagnostics_root(dir.path());
        ensure_diagnostics_dirs(&paths).expect("ensure diagnostics dirs");

        let summary = CrashSummary {
            timestamp: "2026-03-13T10:00:00Z".to_string(),
            thread: "main".to_string(),
            message: "panic occurred".to_string(),
            location: Some("src/lib.rs:10".to_string()),
            backtrace: None,
            run_id: Some("run-1".to_string()),
        };
        record_crash_summary(&paths, &summary).expect("record crash");

        let latest = read_latest_crash_summary(&paths)
            .expect("read latest crash")
            .expect("latest crash exists");
        assert_eq!(latest.message, "panic occurred");
        assert_eq!(latest.run_id.as_deref(), Some("run-1"));
    }
}
