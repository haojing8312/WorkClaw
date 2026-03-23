use crate::commands::feishu_gateway::{
    dispatch_feishu_inbound_to_workclaw_with_pool_and_app, get_app_setting,
    list_enabled_employee_feishu_connections_with_pool, list_feishu_pairing_allow_from_with_pool,
    list_feishu_pairing_requests_with_pool, set_app_setting, upsert_feishu_pairing_request_with_pool,
};
use crate::commands::skills::DbState;
use crate::im::types::{ImEvent, ImEventType};
use crate::windows_process::hide_console_window;
use reqwest::Client;
use sqlx::SqlitePool;
use std::fs;
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct OpenClawPluginInstallInput {
    pub plugin_id: String,
    pub npm_spec: String,
    pub version: String,
    pub install_path: String,
    #[serde(default = "default_source_type")]
    pub source_type: String,
    #[serde(default)]
    pub manifest_json: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct OpenClawPluginInstallRecord {
    pub plugin_id: String,
    pub npm_spec: String,
    pub version: String,
    pub install_path: String,
    pub source_type: String,
    pub manifest_json: String,
    pub installed_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct OpenClawPluginInspectionMeta {
    pub id: Option<String>,
    pub label: Option<String>,
    #[serde(rename = "selectionLabel")]
    pub selection_label: Option<String>,
    #[serde(rename = "docsPath")]
    pub docs_path: Option<String>,
    #[serde(rename = "docsLabel")]
    pub docs_label: Option<String>,
    pub blurb: Option<String>,
    pub aliases: Option<Vec<String>>,
    pub order: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct OpenClawPluginChannelInspection {
    pub id: Option<String>,
    pub meta: Option<OpenClawPluginInspectionMeta>,
    pub capabilities: Option<serde_json::Value>,
    #[serde(rename = "reloadConfigPrefixes")]
    pub reload_config_prefixes: Vec<String>,
    #[serde(rename = "hasPairing")]
    pub has_pairing: bool,
    #[serde(rename = "hasSetup")]
    pub has_setup: bool,
    #[serde(rename = "hasOnboarding")]
    pub has_onboarding: bool,
    #[serde(rename = "hasDirectory")]
    pub has_directory: bool,
    #[serde(rename = "hasOutbound")]
    pub has_outbound: bool,
    #[serde(rename = "hasThreading")]
    pub has_threading: bool,
    #[serde(rename = "hasActions")]
    pub has_actions: bool,
    #[serde(rename = "hasStatus")]
    pub has_status: bool,
    #[serde(rename = "targetHint")]
    pub target_hint: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct OpenClawPluginToolInspection {
    pub id: Option<String>,
    pub name: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct OpenClawPluginInspectionSummary {
    pub channels: Vec<OpenClawPluginChannelInspection>,
    pub tools: Vec<OpenClawPluginToolInspection>,
    #[serde(rename = "commandNames")]
    pub command_names: Vec<String>,
    #[serde(rename = "cliCommandNames")]
    pub cli_command_names: Vec<String>,
    #[serde(rename = "gatewayMethods")]
    pub gateway_methods: Vec<String>,
    #[serde(rename = "hookCounts")]
    pub hook_counts: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct OpenClawPluginInspectionResult {
    #[serde(rename = "pluginRoot")]
    pub plugin_root: String,
    #[serde(rename = "preparedRoot")]
    pub prepared_root: String,
    pub manifest: serde_json::Value,
    #[serde(rename = "entryPath")]
    pub entry_path: String,
    pub summary: OpenClawPluginInspectionSummary,
    #[serde(rename = "logRecordCount")]
    pub log_record_count: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct OpenClawPluginChannelHost {
    pub plugin_id: String,
    pub npm_spec: String,
    pub version: String,
    pub channel: String,
    pub display_name: String,
    pub capabilities: Vec<String>,
    pub reload_config_prefixes: Vec<String>,
    pub target_hint: Option<String>,
    pub docs_path: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct OpenClawPluginChannelAccountSnapshot {
    #[serde(rename = "accountId")]
    pub account_id: String,
    pub account: serde_json::Value,
    #[serde(rename = "describedAccount")]
    pub described_account: serde_json::Value,
    #[serde(rename = "allowFrom")]
    pub allow_from: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct OpenClawPluginChannelSnapshot {
    #[serde(rename = "channelId")]
    pub channel_id: String,
    #[serde(rename = "defaultAccountId")]
    pub default_account_id: Option<String>,
    #[serde(rename = "accountIds")]
    pub account_ids: Vec<String>,
    pub accounts: Vec<OpenClawPluginChannelAccountSnapshot>,
    #[serde(rename = "reloadConfigPrefixes")]
    pub reload_config_prefixes: Vec<String>,
    #[serde(rename = "targetHint")]
    pub target_hint: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct OpenClawPluginChannelSnapshotResult {
    #[serde(rename = "pluginRoot")]
    pub plugin_root: String,
    #[serde(rename = "preparedRoot")]
    pub prepared_root: String,
    pub manifest: serde_json::Value,
    #[serde(rename = "entryPath")]
    pub entry_path: String,
    pub snapshot: OpenClawPluginChannelSnapshot,
    #[serde(rename = "logRecordCount")]
    pub log_record_count: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct OpenClawPluginFeishuRuntimeStatus {
    pub plugin_id: String,
    pub account_id: String,
    pub running: bool,
    pub started_at: Option<String>,
    pub last_stop_at: Option<String>,
    pub last_event_at: Option<String>,
    pub last_error: Option<String>,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub recent_logs: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuPluginEnvironmentStatus {
    pub node_available: bool,
    pub npm_available: bool,
    pub node_version: Option<String>,
    pub npm_version: Option<String>,
    pub can_install_plugin: bool,
    pub can_start_runtime: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuSetupProgress {
    pub environment: FeishuPluginEnvironmentStatus,
    pub credentials_configured: bool,
    pub plugin_installed: bool,
    pub plugin_version: Option<String>,
    pub runtime_running: bool,
    pub runtime_last_error: Option<String>,
    pub auth_status: String,
    pub pending_pairings: usize,
    pub default_routing_employee_name: Option<String>,
    pub scoped_routing_count: usize,
    pub summary_state: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawPluginFeishuOutboundSendRequest {
    pub request_id: String,
    pub account_id: String,
    pub target: String,
    pub thread_id: Option<String>,
    pub text: String,
    pub mode: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawPluginFeishuOutboundDeliveryResult {
    pub delivered: bool,
    pub channel: String,
    pub account_id: String,
    pub target: String,
    pub thread_id: Option<String>,
    pub text: String,
    pub mode: String,
    pub message_id: String,
    pub chat_id: String,
    pub sequence: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawPluginFeishuOutboundSendResult {
    pub request_id: String,
    pub request: OpenClawPluginFeishuOutboundSendRequest,
    pub result: OpenClawPluginFeishuOutboundDeliveryResult,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawPluginFeishuOutboundCommandErrorEvent {
    pub request_id: Option<String>,
    pub command: Option<String>,
    pub error: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct OpenClawPluginFeishuOutboundSendCommandPayload {
    request_id: String,
    command: String,
    account_id: String,
    target: String,
    thread_id: Option<String>,
    text: String,
    mode: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct OpenClawPluginFeishuAdvancedSettings {
    pub groups_json: String,
    pub dms_json: String,
    pub footer_json: String,
    pub account_overrides_json: String,
    pub render_mode: String,
    pub streaming: String,
    pub text_chunk_limit: String,
    pub chunk_mode: String,
    pub reply_in_thread: String,
    pub group_session_scope: String,
    pub topic_session_mode: String,
    pub markdown_mode: String,
    pub markdown_table_mode: String,
    pub heartbeat_visibility: String,
    pub heartbeat_interval_ms: String,
    pub media_max_mb: String,
    pub http_timeout_ms: String,
    pub config_writes: String,
    pub webhook_host: String,
    pub webhook_port: String,
    pub dynamic_agent_creation_enabled: String,
    pub dynamic_agent_creation_workspace_template: String,
    pub dynamic_agent_creation_agent_dir_template: String,
    pub dynamic_agent_creation_max_agents: String,
}

impl Default for OpenClawPluginFeishuRuntimeStatus {
    fn default() -> Self {
        Self {
            plugin_id: "openclaw-lark".to_string(),
            account_id: "default".to_string(),
            running: false,
            started_at: None,
            last_stop_at: None,
            last_event_at: None,
            last_error: None,
            pid: None,
            port: None,
            recent_logs: Vec::new(),
        }
    }
}

impl Default for FeishuPluginEnvironmentStatus {
    fn default() -> Self {
        Self {
            node_available: false,
            npm_available: false,
            node_version: None,
            npm_version: None,
            can_install_plugin: false,
            can_start_runtime: false,
            error: None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct OpenClawPluginFeishuCredentialProbeResult {
    pub ok: bool,
    pub app_id: String,
    pub bot_name: Option<String>,
    pub bot_open_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpenClawLarkInstallerMode {
    Create,
    Link,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct OpenClawLarkInstallerSessionStatus {
    pub running: bool,
    pub mode: Option<OpenClawLarkInstallerMode>,
    pub started_at: Option<String>,
    pub last_output_at: Option<String>,
    pub last_error: Option<String>,
    pub prompt_hint: Option<String>,
    pub recent_output: Vec<String>,
}

impl Default for OpenClawLarkInstallerSessionStatus {
    fn default() -> Self {
        Self {
            running: false,
            mode: None,
            started_at: None,
            last_output_at: None,
            last_error: None,
            prompt_hint: None,
            recent_output: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct OpenClawLarkInstallerAutoInputState {
    selection_sent: bool,
    app_id_sent: bool,
    app_secret_sent: bool,
}

impl Default for OpenClawLarkInstallerAutoInputState {
    fn default() -> Self {
        Self {
            selection_sent: false,
            app_id_sent: false,
            app_secret_sent: false,
        }
    }
}

const OPENCLAW_SHIM_VERSION: &str = "2026.3.8";

#[derive(Clone, Default)]
pub struct OpenClawPluginFeishuRuntimeState(pub Arc<Mutex<OpenClawPluginFeishuRuntimeStore>>);

#[derive(Default)]
pub struct OpenClawPluginFeishuRuntimeStore {
    process: Option<Arc<Mutex<Option<Child>>>>,
    stdin: Option<Arc<Mutex<ChildStdin>>>,
    pending_outbound_send_results: HashMap<
        String,
        std::sync::mpsc::SyncSender<Result<OpenClawPluginFeishuOutboundSendResult, String>>,
    >,
    status: OpenClawPluginFeishuRuntimeStatus,
}

#[derive(Clone, Default)]
pub struct OpenClawLarkInstallerSessionState(pub Arc<Mutex<OpenClawLarkInstallerSessionStore>>);

#[derive(Default)]
pub struct OpenClawLarkInstallerSessionStore {
    process: Option<Arc<Mutex<Option<Child>>>>,
    stdin: Option<Arc<Mutex<ChildStdin>>>,
    auto: OpenClawLarkInstallerAutoInputState,
    app_id: Option<String>,
    app_secret: Option<String>,
    status: OpenClawLarkInstallerSessionStatus,
}

fn default_source_type() -> String {
    "npm".to_string()
}

fn normalize_required(value: &str, field: &str) -> Result<String, String> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Err(format!("{field} is required"));
    }
    Ok(normalized)
}

fn app_setting_string_or_default(value: Option<String>, default: &str) -> String {
    value
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn normalize_manifest_json(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok("{}".to_string());
    }
    serde_json::from_str::<serde_json::Value>(trimmed)
        .map_err(|e| format!("manifest_json must be valid json: {e}"))?;
    Ok(trimmed.to_string())
}

pub async fn upsert_openclaw_plugin_install_with_pool(
    pool: &SqlitePool,
    input: OpenClawPluginInstallInput,
) -> Result<OpenClawPluginInstallRecord, String> {
    let plugin_id = normalize_required(&input.plugin_id, "plugin_id")?;
    let npm_spec = normalize_required(&input.npm_spec, "npm_spec")?;
    let version = normalize_required(&input.version, "version")?;
    let install_path = normalize_required(&input.install_path, "install_path")?;
    let source_type = normalize_required(&input.source_type, "source_type")?;
    let manifest_json = normalize_manifest_json(&input.manifest_json)?;
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO installed_openclaw_plugins (
            plugin_id,
            npm_spec,
            version,
            install_path,
            source_type,
            manifest_json,
            installed_at,
            updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(plugin_id) DO UPDATE SET
            npm_spec = excluded.npm_spec,
            version = excluded.version,
            install_path = excluded.install_path,
            source_type = excluded.source_type,
            manifest_json = excluded.manifest_json,
            updated_at = excluded.updated_at",
    )
    .bind(&plugin_id)
    .bind(&npm_spec)
    .bind(&version)
    .bind(&install_path)
    .bind(&source_type)
    .bind(&manifest_json)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    let row = sqlx::query_as::<_, (String, String, String, String, String, String, String, String)>(
        "SELECT plugin_id, npm_spec, version, install_path, source_type, manifest_json, installed_at, updated_at
         FROM installed_openclaw_plugins
         WHERE plugin_id = ?",
    )
    .bind(&plugin_id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(OpenClawPluginInstallRecord {
        plugin_id: row.0,
        npm_spec: row.1,
        version: row.2,
        install_path: row.3,
        source_type: row.4,
        manifest_json: row.5,
        installed_at: row.6,
        updated_at: row.7,
    })
}

pub async fn list_openclaw_plugin_installs_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<OpenClawPluginInstallRecord>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String)>(
        "SELECT plugin_id, npm_spec, version, install_path, source_type, manifest_json, installed_at, updated_at
         FROM installed_openclaw_plugins
         ORDER BY installed_at DESC, plugin_id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|row| OpenClawPluginInstallRecord {
            plugin_id: row.0,
            npm_spec: row.1,
            version: row.2,
            install_path: row.3,
            source_type: row.4,
            manifest_json: row.5,
            installed_at: row.6,
            updated_at: row.7,
        })
        .collect())
}

pub async fn delete_openclaw_plugin_install_with_pool(
    pool: &SqlitePool,
    plugin_id: &str,
) -> Result<(), String> {
    let normalized = normalize_required(plugin_id, "plugin_id")?;
    sqlx::query("DELETE FROM installed_openclaw_plugins WHERE plugin_id = ?")
        .bind(normalized)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn resolve_plugin_host_dir() -> PathBuf {
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .to_path_buf();

    fn packaged_plugin_host_candidates(base_dir: &Path) -> Vec<PathBuf> {
        vec![
            base_dir.join("resources").join("plugin-host"),
            base_dir.join("_up_").join("plugin-host"),
            base_dir.join("plugin-host"),
        ]
    }

    let dev_dir = manifest_root.join("plugin-host");
    if dev_dir.exists() {
        return dev_dir;
    }

    let mut candidates = Vec::new();
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.extend(packaged_plugin_host_candidates(exe_dir));
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.extend(packaged_plugin_host_candidates(&cwd));
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .unwrap_or_else(|| manifest_root.join("plugin-host"))
}

fn resolve_plugin_host_inspect_script() -> PathBuf {
    resolve_plugin_host_dir()
        .join("scripts")
        .join("inspect-plugin.mjs")
}

fn resolve_plugin_host_run_feishu_script() -> PathBuf {
    resolve_plugin_host_dir()
        .join("scripts")
        .join("run-feishu-host.mjs")
}

fn normalize_command_version_output(output: &[u8]) -> Option<String> {
    String::from_utf8_lossy(output)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

#[cfg(target_os = "windows")]
fn expand_windows_env_tokens(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            result.push(ch);
            continue;
        }
        let mut name = String::new();
        while let Some(next) = chars.peek().copied() {
            chars.next();
            if next == '%' {
                break;
            }
            name.push(next);
        }
        if name.is_empty() {
            result.push('%');
            continue;
        }
        if let Some(expanded) = std::env::var_os(&name) {
            result.push_str(&expanded.to_string_lossy());
        } else {
            result.push('%');
            result.push_str(&name);
            result.push('%');
        }
    }
    result
}

#[cfg(target_os = "windows")]
fn parse_windows_path_entries(raw: &str) -> Vec<PathBuf> {
    raw.split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(expand_windows_env_tokens)
        .filter(|entry| !entry.trim().is_empty())
        .map(PathBuf::from)
        .collect()
}

#[cfg(target_os = "windows")]
fn parse_windows_registry_path_output(output: &str) -> Vec<PathBuf> {
    for line in output.lines() {
        let trimmed = line.trim();
        if !trimmed.to_ascii_lowercase().starts_with("path") {
            continue;
        }
        let Some(type_start) = trimmed.find("REG_") else {
            continue;
        };
        let after_name = trimmed[type_start..].trim();
        let mut parts = after_name.splitn(2, char::is_whitespace);
        let _value_type = parts.next();
        let value = parts.next().unwrap_or("").trim();
        if value.is_empty() {
            return Vec::new();
        }
        return parse_windows_path_entries(value);
    }
    Vec::new()
}

#[cfg(target_os = "windows")]
fn read_windows_registry_path_entries(scope: &str) -> Vec<PathBuf> {
    let mut command = Command::new("reg");
    command.args(["query", scope, "/v", "Path"]);
    hide_console_window(&mut command);
    match command.output() {
        Ok(output) if output.status.success() => {
            parse_windows_registry_path_output(&String::from_utf8_lossy(&output.stdout))
        }
        _ => Vec::new(),
    }
}

fn dedupe_path_entries(entries: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries {
        let key = if cfg!(target_os = "windows") {
            entry.to_string_lossy().to_lowercase()
        } else {
            entry.to_string_lossy().to_string()
        };
        if seen.insert(key) {
            deduped.push(entry);
        }
    }
    deduped
}

fn build_effective_path_entries(
    current_path: Option<&OsStr>,
    prepend: &[PathBuf],
    extra_entries: &[PathBuf],
) -> Vec<PathBuf> {
    let current_entries = current_path
        .map(std::env::split_paths)
        .into_iter()
        .flatten()
        .filter(|entry| !entry.as_os_str().is_empty());
    dedupe_path_entries(
        prepend
            .iter()
            .cloned()
            .chain(current_entries)
            .chain(extra_entries.iter().cloned()),
    )
}

#[cfg(target_os = "windows")]
fn collect_windows_registry_path_entries() -> Vec<PathBuf> {
    dedupe_path_entries(
        read_windows_registry_path_entries(r"HKCU\Environment")
            .into_iter()
            .chain(read_windows_registry_path_entries(
                r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
            )),
    )
}

#[cfg(not(target_os = "windows"))]
fn collect_windows_registry_path_entries() -> Vec<PathBuf> {
    Vec::new()
}

fn effective_command_path_entries(prepend: &[PathBuf]) -> Vec<PathBuf> {
    build_effective_path_entries(
        std::env::var_os("PATH").as_deref(),
        prepend,
        &collect_windows_registry_path_entries(),
    )
}

fn apply_command_search_path(command: &mut Command, prepend: &[PathBuf]) {
    let entries = effective_command_path_entries(prepend);
    if entries.is_empty() {
        return;
    }
    if let Ok(joined) = std::env::join_paths(entries) {
        command.env("PATH", joined);
    }
}

fn probe_command_version_with_program(
    command: &Path,
    args: &[&str],
) -> Result<Option<String>, String> {
    let mut process = Command::new(command);
    process.args(args);
    apply_command_search_path(&mut process, &[]);
    hide_console_window(&mut process);
    match process.output() {
        Ok(output) => {
            if output.status.success() {
                Ok(normalize_command_version_output(&output.stdout)
                    .or_else(|| normalize_command_version_output(&output.stderr)))
            } else {
                let detail = normalize_command_version_output(&output.stderr)
                    .or_else(|| normalize_command_version_output(&output.stdout))
                    .unwrap_or_else(|| {
                        format!("{} exited with status {}", command.display(), output.status)
                    });
                Err(detail)
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn probe_command_version(command: &str, args: &[&str]) -> Result<Option<String>, String> {
    probe_command_version_with_program(Path::new(command), args)
}

#[cfg(target_os = "windows")]
fn collect_windows_node_command_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(PathBuf::from("node"));
    candidates.push(PathBuf::from("node.exe"));

    for key in ["NVM_SYMLINK", "NVM_HOME"] {
        if let Some(value) = std::env::var_os(key) {
            let base = PathBuf::from(value);
            if !base.as_os_str().is_empty() {
                candidates.push(base.join("node.exe"));
            }
        }
    }

    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        candidates.push(PathBuf::from(program_files).join("nodejs").join("node.exe"));
    }
    if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
        candidates.push(PathBuf::from(program_files_x86).join("nodejs").join("node.exe"));
    }
    if let Some(local_app_data) = std::env::var_os("LocalAppData") {
        candidates.push(
            PathBuf::from(local_app_data)
                .join("Programs")
                .join("nodejs")
                .join("node.exe"),
        );
    }

    for entry in effective_command_path_entries(&[]) {
        if entry.as_os_str().is_empty() {
            continue;
        }
        candidates.push(entry.join("node.exe"));
        candidates.push(entry.join("node"));
    }

    dedupe_path_entries(candidates)
}

#[cfg(target_os = "windows")]
fn probe_windows_node_version(args: &[&str]) -> Result<Option<String>, String> {
    let mut last_error = None;
    for candidate in collect_windows_node_command_candidates() {
        match probe_command_version_with_program(&candidate, args) {
            Ok(Some(version)) => return Ok(Some(version)),
            Ok(None) => continue,
            Err(error) => {
                last_error = Some(format!("{}: {error}", candidate.display()));
            }
        }
    }
    if let Some(error) = last_error {
        Err(error)
    } else {
        Ok(None)
    }
}

#[cfg(not(target_os = "windows"))]
fn probe_windows_node_version(args: &[&str]) -> Result<Option<String>, String> {
    probe_command_version("node", args)
}

fn derive_feishu_plugin_environment_status(
    node_probe: Result<Option<String>, String>,
    npm_probe: Result<Option<String>, String>,
    runtime_script_exists: bool,
) -> FeishuPluginEnvironmentStatus {
    let mut status = FeishuPluginEnvironmentStatus::default();
    let mut errors = Vec::new();

    match node_probe {
        Ok(version) => {
            status.node_available = version.is_some();
            status.node_version = version;
            if !status.node_available {
                errors.push("未检测到 Node.js".to_string());
            }
        }
        Err(error) => {
            errors.push(format!("检测 Node.js 失败: {error}"));
        }
    }

    match npm_probe {
        Ok(version) => {
            status.npm_available = version.is_some();
            status.npm_version = version;
            if !status.npm_available {
                errors.push("未检测到 npm".to_string());
            }
        }
        Err(error) => {
            errors.push(format!("检测 npm 失败: {error}"));
        }
    }

    if !runtime_script_exists {
        errors.push("飞书插件运行脚本缺失".to_string());
    }

    status.can_install_plugin = status.node_available && status.npm_available;
    status.can_start_runtime = status.node_available && runtime_script_exists;
    status.error = if errors.is_empty() {
        None
    } else {
        Some(errors.join("；"))
    };
    status
}

fn get_feishu_plugin_environment_status_internal() -> FeishuPluginEnvironmentStatus {
    derive_feishu_plugin_environment_status(
        probe_windows_node_version(&["--version"]),
        probe_command_version(resolve_npm_command(), &["--version"]),
        resolve_plugin_host_run_feishu_script().exists(),
    )
}

fn resolve_employee_agent_identity(
    employee_id: &str,
    role_id: &str,
    openclaw_agent_id: &str,
) -> String {
    let openclaw_agent_id = openclaw_agent_id.trim();
    if !openclaw_agent_id.is_empty() {
        return openclaw_agent_id.to_string();
    }

    let employee_id = employee_id.trim();
    if !employee_id.is_empty() {
        return employee_id.to_string();
    }

    role_id.trim().to_string()
}

async fn default_feishu_routing_employee_name_with_pool(
    pool: &SqlitePool,
) -> Result<Option<String>, String> {
    let binding = sqlx::query_as::<_, (String,)>(
        "SELECT agent_id
         FROM im_routing_bindings
         WHERE channel = 'feishu'
           AND enabled = 1
           AND trim(peer_id) = ''
         ORDER BY priority ASC, updated_at DESC
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some((binding_agent_id,)) = binding else {
        return Ok(None);
    };

    let employees = sqlx::query_as::<_, (String, String, String, String, i64)>(
        "SELECT employee_id, role_id, COALESCE(openclaw_agent_id, ''), name, enabled
         FROM agent_employees",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(employees.into_iter().find_map(
        |(employee_id, role_id, openclaw_agent_id, name, enabled)| {
            if enabled == 0 {
                return None;
            }
            let resolved = resolve_employee_agent_identity(&employee_id, &role_id, &openclaw_agent_id);
            if resolved.eq_ignore_ascii_case(binding_agent_id.trim()) {
                Some(name)
            } else {
                None
            }
        },
    ))
}

async fn count_scoped_feishu_routing_bindings_with_pool(pool: &SqlitePool) -> Result<usize, String> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM im_routing_bindings
         WHERE channel = 'feishu'
           AND enabled = 1
           AND trim(peer_id) != ''",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(count.max(0) as usize)
}

fn derive_feishu_setup_summary_state(
    environment: &FeishuPluginEnvironmentStatus,
    credentials_configured: bool,
    plugin_installed: bool,
    runtime_running: bool,
    runtime_last_error: Option<&str>,
    auth_status: &str,
    pending_pairings: usize,
    default_routing_employee_name: Option<&str>,
    scoped_routing_count: usize,
) -> String {
    if !environment.can_install_plugin || !environment.can_start_runtime {
        return "env_missing".to_string();
    }
    if runtime_last_error
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
    {
        return "runtime_error".to_string();
    }
    if !plugin_installed {
        return "plugin_not_installed".to_string();
    }
    if !credentials_configured {
        return "ready_to_bind".to_string();
    }
    if !runtime_running {
        return "plugin_starting".to_string();
    }
    if pending_pairings > 0 {
        return "awaiting_pairing_approval".to_string();
    }
    if auth_status != "approved" {
        return "awaiting_auth".to_string();
    }
    if default_routing_employee_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
        && scoped_routing_count == 0
    {
        return "ready_for_routing".to_string();
    }
    "ready".to_string()
}

async fn get_feishu_setup_progress_with_pool(
    pool: &SqlitePool,
    runtime_state: &OpenClawPluginFeishuRuntimeState,
) -> Result<FeishuSetupProgress, String> {
    let environment = get_feishu_plugin_environment_status_internal();
    let app_id = get_app_setting(pool, "feishu_app_id")
        .await?
        .unwrap_or_default();
    let app_secret = get_app_setting(pool, "feishu_app_secret")
        .await?
        .unwrap_or_default();
    let credentials_configured = !app_id.trim().is_empty() && !app_secret.trim().is_empty();

    let install = get_openclaw_plugin_install_by_id_with_pool(pool, "openclaw-lark").await.ok();
    let runtime_status = current_feishu_runtime_status(runtime_state);
    let pairing_requests = list_feishu_pairing_requests_with_pool(pool, None).await?;
    let pending_pairings = pairing_requests
        .iter()
        .filter(|record| record.status == "pending")
        .count();
    let auth_status = if pairing_requests.iter().any(|record| record.status == "approved") {
        "approved".to_string()
    } else if credentials_configured && runtime_status.running {
        "pending".to_string()
    } else {
        "unknown".to_string()
    };
    let default_routing_employee_name = default_feishu_routing_employee_name_with_pool(pool).await?;
    let scoped_routing_count = count_scoped_feishu_routing_bindings_with_pool(pool).await?;
    let summary_state = derive_feishu_setup_summary_state(
        &environment,
        credentials_configured,
        install.is_some(),
        runtime_status.running,
        runtime_status.last_error.as_deref(),
        &auth_status,
        pending_pairings,
        default_routing_employee_name.as_deref(),
        scoped_routing_count,
    );

    Ok(FeishuSetupProgress {
        environment,
        credentials_configured,
        plugin_installed: install.is_some(),
        plugin_version: install.as_ref().map(|record| record.version.clone()),
        runtime_running: runtime_status.running,
        runtime_last_error: runtime_status.last_error,
        auth_status,
        pending_pairings,
        default_routing_employee_name,
        scoped_routing_count,
        summary_state,
    })
}

fn should_auto_restore_feishu_runtime(progress: &FeishuSetupProgress) -> bool {
    progress.plugin_installed
        && progress.credentials_configured
        && !progress.runtime_running
        && progress.auth_status == "approved"
}

#[cfg(target_os = "windows")]
fn resolve_npm_command() -> &'static str {
    "npm.cmd"
}

#[cfg(not(target_os = "windows"))]
fn resolve_npm_command() -> &'static str {
    "npm"
}

fn resolve_openclaw_plugin_workspace_root(
    app: &AppHandle,
    plugin_id: &str,
) -> Result<PathBuf, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let normalized = normalize_required(plugin_id, "plugin_id")?;
    Ok(app_data_dir.join("openclaw-plugins").join(normalized))
}

fn resolve_openclaw_shim_root(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app_data_dir: {e}"))?;
    Ok(app_data_dir.join("openclaw-cli-shim"))
}

fn build_openclaw_shim_state_file_path(shim_root: &Path) -> PathBuf {
    shim_root.join("state.json")
}

fn resolve_controlled_openclaw_state_root(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app_data_dir: {e}"))?;
    Ok(app_data_dir.join("openclaw-state"))
}

fn build_plugin_host_fixture_root_from_app_data_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("plugin-host-fixtures")
}

fn resolve_plugin_host_fixture_root(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app_data_dir: {e}"))?;
    Ok(build_plugin_host_fixture_root_from_app_data_dir(&app_data_dir))
}

fn ensure_controlled_openclaw_state_projection(
    state_root: &Path,
    plugin_install_path: &Path,
) -> Result<(), String> {
    let plugin_projection_dir = state_root.join("extensions").join("openclaw-lark");
    let projected_node_modules = plugin_projection_dir.join("node_modules");
    let source_node_modules = plugin_install_path
        .parent()
        .and_then(|parent| parent.parent())
        .map(|parent| parent.join("node_modules"));

    fs::create_dir_all(&projected_node_modules).map_err(|error| {
        format!(
            "failed to prepare controlled OpenClaw plugin projection {}: {error}",
            projected_node_modules.display()
        )
    })?;

    if let Some(source_node_modules) = source_node_modules {
        let marker_path = projected_node_modules.join(".workclaw-origin.txt");
        let marker = format!(
            "source={}\nplugin_install={}\n",
            source_node_modules.display(),
            plugin_install_path.display()
        );
        fs::write(&marker_path, marker).map_err(|error| {
            format!(
                "failed to write controlled OpenClaw projection marker {}: {error}",
                marker_path.display()
            )
        })?;
    }

    Ok(())
}

#[derive(Debug, Default, serde::Deserialize)]
struct OpenClawShimRecordedCommand {
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct OpenClawShimStateSnapshot {
    #[serde(default)]
    config: serde_json::Value,
    #[serde(default)]
    commands: Vec<OpenClawShimRecordedCommand>,
}

fn read_openclaw_shim_state_snapshot(shim_root: &Path) -> Result<OpenClawShimStateSnapshot, String> {
    let state_path = build_openclaw_shim_state_file_path(shim_root);
    if !state_path.exists() {
        return Ok(OpenClawShimStateSnapshot::default());
    }

    let raw = fs::read_to_string(&state_path)
        .map_err(|error| format!("failed to read openclaw shim state: {error}"))?;
    if raw.trim().is_empty() {
        return Ok(OpenClawShimStateSnapshot::default());
    }

    serde_json::from_str::<OpenClawShimStateSnapshot>(&raw)
        .map_err(|error| format!("failed to parse openclaw shim state: {error}"))
}

fn get_json_path_string(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }

    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn derive_feishu_credentials_from_shim_snapshot(
    snapshot: &OpenClawShimStateSnapshot,
) -> Option<(String, String)> {
    let credential_paths: [(&[&str], &[&str]); 3] = [
        (
            &["channels", "feishu", "appId"],
            &["channels", "feishu", "appSecret"],
        ),
        (
            &["channels", "feishu", "accounts", "default", "appId"],
            &["channels", "feishu", "accounts", "default", "appSecret"],
        ),
        (&["feishu", "appId"], &["feishu", "appSecret"]),
    ];

    for (app_id_path, app_secret_path) in credential_paths {
        let app_id = get_json_path_string(&snapshot.config, &app_id_path);
        let app_secret = get_json_path_string(&snapshot.config, &app_secret_path);
        if let (Some(app_id), Some(app_secret)) = (app_id, app_secret) {
            return Some((app_id, app_secret));
        }
    }

    let mut app_id = None;
    let mut app_secret = None;
    for command in &snapshot.commands {
        let args = &command.args;
        if args.len() < 4 || args[0] != "config" || args[1] != "set" {
            continue;
        }

        let key = args[2].trim();
        let value = args[3].trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }

        let normalized_key = key.to_ascii_lowercase();
        if !normalized_key.contains("feishu") {
            continue;
        }
        if normalized_key.ends_with(".appid") {
            app_id = Some(value.to_string());
        } else if normalized_key.ends_with(".appsecret") {
            app_secret = Some(value.to_string());
        }
    }

    match (app_id, app_secret) {
        (Some(app_id), Some(app_secret)) => Some((app_id, app_secret)),
        _ => None,
    }
}

async fn sync_feishu_gateway_credentials_from_shim_with_pool(
    pool: &SqlitePool,
    shim_root: &Path,
) -> Result<bool, String> {
    let snapshot = read_openclaw_shim_state_snapshot(shim_root)?;
    let Some((app_id, app_secret)) = derive_feishu_credentials_from_shim_snapshot(&snapshot) else {
        return Ok(false);
    };

    let current_app_id = get_app_setting(pool, "feishu_app_id")
        .await?
        .unwrap_or_default();
    let current_app_secret = get_app_setting(pool, "feishu_app_secret")
        .await?
        .unwrap_or_default();

    if current_app_id.trim() == app_id.trim() && current_app_secret.trim() == app_secret.trim() {
        return Ok(false);
    }

    set_app_setting(pool, "feishu_app_id", app_id.trim()).await?;
    set_app_setting(pool, "feishu_app_secret", app_secret.trim()).await?;
    Ok(true)
}

fn get_json_path<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn resolve_openclaw_state_secret_ref(
    config: &serde_json::Value,
    state_root: &Path,
    secret_ref: &serde_json::Value,
) -> Option<String> {
    let source = secret_ref.get("source")?.as_str()?.trim();
    let id = secret_ref.get("id")?.as_str()?.trim();
    if source.eq_ignore_ascii_case("env") {
        if let Some(value) = std::env::var_os(id).and_then(|value| value.into_string().ok()) {
            let trimmed = value.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }

        let env_path = state_root.join(".env");
        let raw = fs::read_to_string(env_path).ok()?;
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some((key, value)) = trimmed.split_once('=') else {
                continue;
            };
            if key.trim() == id {
                let secret = value.trim().to_string();
                if !secret.is_empty() {
                    return Some(secret);
                }
            }
        }
        return None;
    }

    if !source.eq_ignore_ascii_case("file") {
        return None;
    }

    let provider_name = secret_ref.get("provider")?.as_str()?.trim();
    let provider = get_json_path(config, &["secrets", "providers", provider_name])?;
    let provider_path = provider.get("path")?.as_str()?.trim();
    if provider_path.is_empty() {
        return None;
    }
    let home_dir = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let file_path = if provider_path.starts_with("~/") {
        home_dir?.join(provider_path.trim_start_matches("~/"))
    } else {
        PathBuf::from(provider_path)
    };
    let raw = fs::read_to_string(file_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;

    if provider
        .get("mode")
        .and_then(|entry| entry.as_str())
        .map(|mode| mode == "singleValue")
        .unwrap_or(false)
    {
        return json
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
    }

    let mut current = &json;
    for segment in id.trim_start_matches('/').split('/') {
        if segment.is_empty() {
            continue;
        }
        let unescaped = segment.replace("~1", "/").replace("~0", "~");
        current = current.get(unescaped)?;
    }
    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn derive_feishu_credentials_from_openclaw_state_config(
    config: &serde_json::Value,
    state_root: &Path,
) -> Option<(String, String)> {
    let feishu = get_json_path(config, &["channels", "feishu"])?;
    let app_id = feishu
        .get("appId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)?;

    let app_secret_value = feishu.get("appSecret")?;
    let app_secret = match app_secret_value {
        serde_json::Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return None;
            }
            trimmed.to_string()
        }
        serde_json::Value::Object(_) => resolve_openclaw_state_secret_ref(config, state_root, app_secret_value)?,
        _ => return None,
    };

    Some((app_id, app_secret))
}

async fn sync_feishu_gateway_credentials_from_openclaw_state_with_pool(
    pool: &SqlitePool,
    state_root: &Path,
) -> Result<bool, String> {
    let config_path = state_root.join("openclaw.json");
    if !config_path.exists() {
        return Ok(false);
    }

    let raw = fs::read_to_string(&config_path).map_err(|error| {
        format!(
            "failed to read controlled OpenClaw config {}: {error}",
            config_path.display()
        )
    })?;
    if raw.trim().is_empty() {
        return Ok(false);
    }
    let config: serde_json::Value = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "failed to parse controlled OpenClaw config {}: {error}",
            config_path.display()
        )
    })?;

    let Some((app_id, app_secret)) =
        derive_feishu_credentials_from_openclaw_state_config(&config, state_root)
    else {
        return Ok(false);
    };

    let current_app_id = get_app_setting(pool, "feishu_app_id")
        .await?
        .unwrap_or_default();
    let current_app_secret = get_app_setting(pool, "feishu_app_secret")
        .await?
        .unwrap_or_default();

    if current_app_id.trim() == app_id.trim() && current_app_secret.trim() == app_secret.trim() {
        return Ok(false);
    }

    set_app_setting(pool, "feishu_app_id", app_id.trim()).await?;
    set_app_setting(pool, "feishu_app_secret", app_secret.trim()).await?;
    Ok(true)
}

fn build_openclaw_shim_script(state_file: &Path) -> String {
    let state_file_str = state_file.to_string_lossy().replace('\\', "\\\\");
    format!(
        r#"#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const stateFile = process.env.WORKCLAW_OPENCLAW_SHIM_STATE_FILE || "{state_file_str}";
const version = process.env.WORKCLAW_OPENCLAW_SHIM_VERSION || "{OPENCLAW_SHIM_VERSION}";

function loadState() {{
  if (!fs.existsSync(stateFile)) {{
    return {{ config: {{}}, commands: [] }};
  }}

  try {{
    const raw = fs.readFileSync(stateFile, "utf8").trim();
    if (!raw) {{
      return {{ config: {{}}, commands: [] }};
    }}
    const parsed = JSON.parse(raw);
    return {{
      config: parsed && typeof parsed.config === "object" && parsed.config ? parsed.config : {{}},
      commands: Array.isArray(parsed?.commands) ? parsed.commands : [],
    }};
  }} catch (_error) {{
    return {{ config: {{}}, commands: [] }};
  }}
}}

function saveState(state) {{
  fs.mkdirSync(path.dirname(stateFile), {{ recursive: true }});
  fs.writeFileSync(stateFile, JSON.stringify(state, null, 2));
}}

function getPathValue(root, pathParts) {{
  let current = root;
  for (const part of pathParts) {{
    if (!current || typeof current !== "object" || !(part in current)) {{
      return undefined;
    }}
    current = current[part];
  }}
  return current;
}}

function setPathValue(root, pathParts, value) {{
  let current = root;
  for (let index = 0; index < pathParts.length - 1; index += 1) {{
    const part = pathParts[index];
    if (!current[part] || typeof current[part] !== "object") {{
      current[part] = {{}};
    }}
    current = current[part];
  }}
  current[pathParts[pathParts.length - 1]] = value;
}}

function parseValue(raw, useJson) {{
  if (!useJson) {{
    return raw;
  }}
  return JSON.parse(raw);
}}

function recordCommand(state, args) {{
  state.commands.push({{ at: new Date().toISOString(), args }});
  if (state.commands.length > 50) {{
    state.commands = state.commands.slice(-50);
  }}
}}

const args = process.argv.slice(2);
const state = loadState();

if (args.length === 0 || args[0] === "-v" || args[0] === "--version" || args[0] === "version") {{
  console.log(version);
  process.exit(0);
}}

if (args[0] === "config" && args[1] === "get" && typeof args[2] === "string") {{
  const value = getPathValue(state.config, args[2].split("."));
  if (value === undefined) {{
    process.exit(0);
  }}
  if (typeof value === "string") {{
    console.log(value);
  }} else {{
    console.log(JSON.stringify(value));
  }}
  process.exit(0);
}}

if (args[0] === "config" && args[1] === "set" && typeof args[2] === "string" && typeof args[3] === "string") {{
  const useJson = args.includes("--json");
  try {{
    setPathValue(state.config, args[2].split("."), parseValue(args[3], useJson));
    recordCommand(state, args);
    saveState(state);
    console.log(`updated ${{args[2]}}`);
    process.exit(0);
  }} catch (error) {{
    console.error(`[workclaw-openclaw-shim] failed to parse value: ${{error instanceof Error ? error.message : String(error)}}`);
    process.exit(1);
  }}
}}

if (args[0] === "gateway" && (args[1] === "restart" || args[1] === "start" || args[1] === "stop")) {{
  recordCommand(state, args);
  saveState(state);
  console.log(`gateway ${{args[1]}} requested via WorkClaw shim`);
  process.exit(0);
}}

if ((args[0] === "plugins" || args[0] === "plugin") && (args[1] === "install" || args[1] === "uninstall") && typeof args[2] === "string") {{
  recordCommand(state, args);
  saveState(state);
  console.log(`plugin ${{args[1]}} satisfied via WorkClaw shim: ${{args[2]}}`);
  process.exit(0);
}}

if (args[0] === "pairing" && args[1] === "approve" && typeof args[2] === "string" && typeof args[3] === "string") {{
  recordCommand(state, args);
  saveState(state);
  console.log(`pairing approved for ${{args[2]}} ${{args[3]}}`);
  process.exit(0);
}}

console.error(`[workclaw-openclaw-shim] unsupported command: ${{args.join(" ")}}`);
process.exit(2);
"#
    )
}

fn ensure_openclaw_cli_shim(shim_root: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(shim_root)
        .map_err(|e| format!("failed to create openclaw shim dir: {e}"))?;

    let state_file = build_openclaw_shim_state_file_path(shim_root);
    if !state_file.exists() {
        fs::write(&state_file, "{\n  \"config\": {},\n  \"commands\": []\n}")
            .map_err(|e| format!("failed to initialize openclaw shim state: {e}"))?;
    }

    let script_path = shim_root.join("openclaw-shim.mjs");
    fs::write(&script_path, build_openclaw_shim_script(&state_file))
        .map_err(|e| format!("failed to write openclaw shim script: {e}"))?;

    #[cfg(windows)]
    {
        let cmd_path = shim_root.join("openclaw.cmd");
        let cmd_contents = format!("@echo off\r\n\"{}\" \"{}\" %*\r\n", "node", script_path.display());
        fs::write(&cmd_path, cmd_contents)
            .map_err(|e| format!("failed to write openclaw shim cmd wrapper: {e}"))?;
    }

    #[cfg(not(windows))]
    {
        let shell_path = shim_root.join("openclaw");
        let shell_contents = format!("#!/usr/bin/env sh\nnode \"{}\" \"$@\"\n", script_path.display());
        fs::write(&shell_path, shell_contents)
            .map_err(|e| format!("failed to write openclaw shim wrapper: {e}"))?;
        let mut permissions = fs::metadata(&shell_path)
            .map_err(|e| format!("failed to read openclaw shim wrapper metadata: {e}"))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&shell_path, permissions)
            .map_err(|e| format!("failed to mark openclaw shim wrapper executable: {e}"))?;
    }

    Ok(shim_root.to_path_buf())
}

fn prepend_env_path(command: &mut Command, shim_dir: &Path) {
    apply_command_search_path(command, &[shim_dir.to_path_buf()]);
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn push_installer_output(status: &mut OpenClawLarkInstallerSessionStatus, line: &str) {
    status.recent_output.push(line.to_string());
    if status.recent_output.len() > 200 {
        let overflow = status.recent_output.len() - 200;
        status.recent_output.drain(0..overflow);
    }
    status.last_output_at = Some(now_rfc3339());
}

fn infer_installer_prompt_hint(line: &str) -> Option<String> {
    let normalized = line.to_lowercase();
    if normalized.contains("what would you like to do") || line.contains("请选择操作") {
        return Some("请选择“新建机器人”或“关联已有机器人”".to_string());
    }
    if normalized.contains("enter your app id") || line.contains("请输入 App ID") {
        return Some("请输入机器人 App ID".to_string());
    }
    if normalized.contains("enter your app secret") || line.contains("请输入 App Secret") {
        return Some("请输入机器人 App Secret".to_string());
    }
    if normalized.contains("scan with feishu to create your bot") || line.contains("扫码") {
        return Some("请使用飞书扫码完成机器人创建".to_string());
    }
    if normalized.contains("fetching configuration results") || line.contains("正在获取你的机器人配置结果") {
        return Some("正在等待飞书官方接口返回机器人 App ID / App Secret，请稍候。".to_string());
    }
    if normalized.contains("[debug] poll result:") && normalized.contains("authorization_pending") {
        return Some("飞书官方接口仍在等待这次扫码配置完成回传结果（authorization_pending）。".to_string());
    }
    if normalized.contains("[debug] poll result:") && normalized.contains("slow_down") {
        return Some("飞书官方接口要求放慢轮询频率，仍在继续等待配置结果。".to_string());
    }
    if normalized.contains("[debug] poll result:") && normalized.contains("expired_token") {
        return Some("这次扫码会话已过期，请重新启动新建机器人向导。".to_string());
    }
    if normalized.contains("[debug] poll result:") && normalized.contains("access_denied") {
        return Some("飞书端已拒绝本次授权，请重新发起新建机器人向导。".to_string());
    }
    None
}

fn derive_installer_auto_input(
    mode: &OpenClawLarkInstallerMode,
    app_id: Option<&str>,
    app_secret: Option<&str>,
    line: &str,
    auto: &mut OpenClawLarkInstallerAutoInputState,
) -> Option<String> {
    let normalized = line.to_lowercase();
    let has_choice_prompt =
        normalized.contains("what would you like to do") || line.contains("请选择操作");
    if has_choice_prompt && !auto.selection_sent {
        auto.selection_sent = true;
        return Some(match mode {
            OpenClawLarkInstallerMode::Create => "\r".to_string(),
            OpenClawLarkInstallerMode::Link => "\u{1b}[B\r".to_string(),
        });
    }

    let has_app_id_prompt =
        normalized.contains("enter your app id") || line.contains("请输入 App ID");
    if has_app_id_prompt {
        if let Some(value) = app_id.filter(|value| !value.trim().is_empty()) {
            auto.app_id_sent = true;
            return Some(format!("{}\r", value.trim()));
        }
    }

    let has_app_secret_prompt =
        normalized.contains("enter your app secret") || line.contains("请输入 App Secret");
    if has_app_secret_prompt {
        if let Some(value) = app_secret.filter(|value| !value.trim().is_empty()) {
            auto.app_secret_sent = true;
            return Some(format!("{}\r", value.trim()));
        }
    }

    None
}

fn merge_feishu_runtime_status_event(
    status: &mut OpenClawPluginFeishuRuntimeStatus,
    value: &serde_json::Value,
) {
    let Some(event) = value.get("event").and_then(|entry| entry.as_str()) else {
        return;
    };

    match event {
        "status" => {
            status.last_event_at = Some(now_rfc3339());
            if let Some(patch) = value.get("patch").and_then(|entry| entry.as_object()) {
                if let Some(account_id) = patch.get("accountId").and_then(|entry| entry.as_str()) {
                    status.account_id = account_id.to_string();
                }
                if let Some(port) = patch.get("port").and_then(|entry| entry.as_u64()) {
                    status.port = Some(port as u16);
                }
                if let Some(last_error) = patch.get("lastError").and_then(|entry| entry.as_str()) {
                    let normalized = last_error.trim();
                    status.last_error = if normalized.is_empty() {
                        None
                    } else {
                        Some(normalized.to_string())
                    };
                }
            }
        }
        "log" => {
            status.last_event_at = Some(now_rfc3339());
            let level = value
                .get("level")
                .and_then(|entry| entry.as_str())
                .unwrap_or("info")
                .trim()
                .to_string();
            let scope = value
                .get("scope")
                .and_then(|entry| entry.as_str())
                .unwrap_or("runtime")
                .trim()
                .to_string();
            let message = value
                .get("message")
                .and_then(|entry| entry.as_str())
                .unwrap_or("")
                .trim()
                .to_string();

            if !message.is_empty() {
                let entry = format!("[{level}] {scope}: {message}");
                status.recent_logs.push(entry.clone());
                if status.recent_logs.len() > 40 {
                    let overflow = status.recent_logs.len() - 40;
                    status.recent_logs.drain(0..overflow);
                }
                if level == "error" {
                    status.last_error = Some(entry);
                }
            }
        }
        "fatal" => {
            status.last_event_at = Some(now_rfc3339());
            if let Some(error) = value.get("error").and_then(|entry| entry.as_str()) {
                let normalized = error.trim();
                if !normalized.is_empty() {
                    status.last_error = Some(normalized.to_string());
                    status
                        .recent_logs
                        .push(format!("[fatal] runtime: {normalized}"));
                    if status.recent_logs.len() > 40 {
                        let overflow = status.recent_logs.len() - 40;
                        status.recent_logs.drain(0..overflow);
                    }
                }
            }
        }
        _ => {}
    }
}

fn trim_recent_runtime_logs(status: &mut OpenClawPluginFeishuRuntimeStatus) {
    if status.recent_logs.len() > 40 {
        let overflow = status.recent_logs.len() - 40;
        status.recent_logs.drain(0..overflow);
    }
}

fn build_feishu_runtime_outbound_send_command_payload(
    request: &OpenClawPluginFeishuOutboundSendRequest,
) -> Result<OpenClawPluginFeishuOutboundSendCommandPayload, String> {
    let request_id = normalize_required(&request.request_id, "request_id")?;
    let account_id = app_setting_string_or_default(Some(request.account_id.clone()), "default");
    let target = normalize_required(&request.target, "target")?;
    let text = request.text.trim().to_string();
    let mode = app_setting_string_or_default(Some(request.mode.clone()), "text");

    Ok(OpenClawPluginFeishuOutboundSendCommandPayload {
        request_id,
        command: "send_message".to_string(),
        account_id,
        target,
        thread_id: request
            .thread_id
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        text,
        mode,
    })
}

fn register_pending_feishu_runtime_outbound_send_waiter(
    state: &OpenClawPluginFeishuRuntimeState,
    request_id: &str,
) -> Result<std::sync::mpsc::Receiver<Result<OpenClawPluginFeishuOutboundSendResult, String>>, String>
{
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let mut guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock feishu runtime state".to_string())?;
    if guard.pending_outbound_send_results.contains_key(request_id) {
        return Err(format!("duplicate outbound requestId: {request_id}"));
    }
    guard
        .pending_outbound_send_results
        .insert(request_id.to_string(), sender);
    guard.status.last_event_at = Some(now_rfc3339());
    guard
        .status
        .recent_logs
        .push(format!("[outbound] queued send_message requestId={request_id}"));
    trim_recent_runtime_logs(&mut guard.status);
    Ok(receiver)
}

fn deliver_pending_feishu_runtime_outbound_send_result(
    state: &OpenClawPluginFeishuRuntimeState,
    result: OpenClawPluginFeishuOutboundSendResult,
) -> bool {
    let request_id = result.request_id.clone();
    let sender = {
        let mut guard = match state.0.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        guard.status.last_event_at = Some(now_rfc3339());
        guard
            .status
            .recent_logs
            .push(format!("[outbound] send_result requestId={request_id}"));
        trim_recent_runtime_logs(&mut guard.status);
        guard.pending_outbound_send_results.remove(&request_id)
    };

    match sender {
        Some(sender) => sender.send(Ok(result)).is_ok(),
        None => {
            if let Ok(mut guard) = state.0.lock() {
                guard.status.recent_logs.push(format!(
                    "[warn] runtime: unhandled outbound send_result requestId={request_id}"
                ));
                trim_recent_runtime_logs(&mut guard.status);
            }
            false
        }
    }
}

fn deliver_pending_feishu_runtime_outbound_command_error(
    state: &OpenClawPluginFeishuRuntimeState,
    error_event: OpenClawPluginFeishuOutboundCommandErrorEvent,
) -> bool {
    let error_message = error_event.error.trim().to_string();
    let Some(request_id) = error_event
        .request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        let failed = fail_pending_feishu_runtime_outbound_send_waiters(
            state,
            if error_message.is_empty() {
                "official feishu runtime reported an outbound command error".to_string()
            } else {
                format!("official feishu runtime command error: {error_message}")
            },
        );
        return failed > 0;
    };

    let sender = {
        let mut guard = match state.0.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        guard.status.last_event_at = Some(now_rfc3339());
        guard.status.last_error = Some(error_message.clone());
        guard.status.recent_logs.push(format!(
            "[outbound] command_error requestId={request_id}: {error_message}"
        ));
        trim_recent_runtime_logs(&mut guard.status);
        guard.pending_outbound_send_results.remove(request_id)
    };

    match sender {
        Some(sender) => sender
            .send(Err(format!(
                "official feishu runtime command error: {error_message}"
            )))
            .is_ok(),
        None => false,
    }
}

fn fail_pending_feishu_runtime_outbound_send_waiters(
    state: &OpenClawPluginFeishuRuntimeState,
    error: String,
) -> usize {
    let senders = {
        let mut guard = match state.0.lock() {
            Ok(guard) => guard,
            Err(_) => return 0,
        };
        guard.status.last_event_at = Some(now_rfc3339());
        guard.status.recent_logs.push(format!("[outbound] {error}"));
        trim_recent_runtime_logs(&mut guard.status);
        std::mem::take(&mut guard.pending_outbound_send_results)
    };

    let mut count = 0;
    for (_request_id, sender) in senders {
        let _ = sender.send(Err(error.clone()));
        count += 1;
    }
    count
}

fn parse_openclaw_plugin_feishu_runtime_send_result_event(
    value: &serde_json::Value,
) -> Result<OpenClawPluginFeishuOutboundSendResult, String> {
    let event = value
        .get("event")
        .and_then(|entry| entry.as_str())
        .unwrap_or_default();
    if event != "send_result" {
        return Err(format!("unexpected outbound event: {event}"));
    }
    serde_json::from_value::<OpenClawPluginFeishuOutboundSendResult>(value.clone())
        .map_err(|error| format!("invalid send_result event: {error}"))
}

fn parse_openclaw_plugin_feishu_runtime_command_error_event(
    value: &serde_json::Value,
) -> Result<OpenClawPluginFeishuOutboundCommandErrorEvent, String> {
    let event = value
        .get("event")
        .and_then(|entry| entry.as_str())
        .unwrap_or_default();
    if event != "command_error" {
        return Err(format!("unexpected outbound event: {event}"));
    }
    serde_json::from_value::<OpenClawPluginFeishuOutboundCommandErrorEvent>(value.clone())
        .map_err(|error| format!("invalid command_error event: {error}"))
}

pub fn handle_openclaw_plugin_feishu_runtime_send_result_event(
    state: &OpenClawPluginFeishuRuntimeState,
    value: &serde_json::Value,
) -> bool {
    match parse_openclaw_plugin_feishu_runtime_send_result_event(value) {
        Ok(result) => deliver_pending_feishu_runtime_outbound_send_result(state, result),
        Err(error) => {
            if let Ok(mut guard) = state.0.lock() {
                guard.status.last_error = Some(error.clone());
                guard
                    .status
                    .recent_logs
                    .push(format!("[error] runtime: {error}"));
                trim_recent_runtime_logs(&mut guard.status);
                guard.status.last_event_at = Some(now_rfc3339());
            }
            false
        }
    }
}

pub fn handle_openclaw_plugin_feishu_runtime_command_error_event(
    state: &OpenClawPluginFeishuRuntimeState,
    value: &serde_json::Value,
) -> bool {
    match parse_openclaw_plugin_feishu_runtime_command_error_event(value) {
        Ok(error_event) => {
            deliver_pending_feishu_runtime_outbound_command_error(state, error_event)
        }
        Err(error) => {
            if let Ok(mut guard) = state.0.lock() {
                guard.status.last_error = Some(error.clone());
                guard
                    .status
                    .recent_logs
                    .push(format!("[error] runtime: {error}"));
                trim_recent_runtime_logs(&mut guard.status);
                guard.status.last_event_at = Some(now_rfc3339());
            }
            false
        }
    }
}

pub fn send_openclaw_plugin_feishu_runtime_outbound_message_in_state(
    state: &OpenClawPluginFeishuRuntimeState,
    request: OpenClawPluginFeishuOutboundSendRequest,
) -> Result<OpenClawPluginFeishuOutboundSendResult, String> {
    let payload = build_feishu_runtime_outbound_send_command_payload(&request)?;
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| format!("failed to serialize outbound send command: {error}"))?;

    let stdin = {
        let guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock feishu runtime state".to_string())?;
        if !guard.status.running {
            return Err("official feishu runtime is not running".to_string());
        }
        guard
            .stdin
            .clone()
            .ok_or_else(|| "official feishu runtime is not accepting outbound commands".to_string())?
    };

    let receiver = register_pending_feishu_runtime_outbound_send_waiter(state, &payload.request_id)?;

    if let Err(error) = {
        let mut stdin_guard = stdin
            .lock()
            .map_err(|_| "failed to lock feishu runtime stdin".to_string())?;
        stdin_guard
            .write_all(payload_json.as_bytes())
            .and_then(|_| stdin_guard.write_all(b"\n"))
            .and_then(|_| stdin_guard.flush())
    } {
        let _ = fail_pending_feishu_runtime_outbound_send_waiters(
            state,
            format!("failed to write outbound command: {error}"),
        );
        return Err(format!("failed to write outbound command: {error}"));
    }

    match receiver.recv_timeout(Duration::from_secs(30)) {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(error)) => Err(error),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            let _ = {
                if let Ok(mut guard) = state.0.lock() {
                    guard
                        .pending_outbound_send_results
                        .remove(&payload.request_id);
                    guard.status.recent_logs.push(format!(
                        "[warn] runtime: outbound send timed out requestId={}",
                        payload.request_id
                    ));
                    trim_recent_runtime_logs(&mut guard.status);
                    guard.status.last_event_at = Some(now_rfc3339());
                }
                Ok::<(), ()>(())
            };
            Err(format!(
                "timed out waiting for outbound send_result for requestId {}",
                payload.request_id
            ))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(format!(
            "official feishu runtime disconnected before outbound send_result for requestId {}",
            payload.request_id
        )),
    }
}

fn merge_pairing_allow_from(base: Option<&str>, extra_entries: Vec<String>) -> serde_json::Value {
    let mut entries = Vec::<String>::new();
    if let Some(value) = base {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            entries.push(trimmed.to_string());
        }
    }
    for value in extra_entries {
        let trimmed = value.trim();
        if !trimmed.is_empty() && !entries.iter().any(|entry| entry == trimmed) {
            entries.push(trimmed.to_string());
        }
    }
    serde_json::Value::Array(
        entries
            .into_iter()
            .map(serde_json::Value::String)
            .collect::<Vec<_>>(),
    )
}

fn current_feishu_runtime_status(
    state: &OpenClawPluginFeishuRuntimeState,
) -> OpenClawPluginFeishuRuntimeStatus {
    state
        .0
        .lock()
        .expect("lock feishu runtime state")
        .status
        .clone()
}

fn resolve_installed_package_dir(workspace: &Path, npm_spec: &str) -> Result<PathBuf, String> {
    let normalized = normalize_required(npm_spec, "npm_spec")?;
    let package_name = normalized
        .split('@')
        .next_back()
        .ok_or_else(|| format!("invalid npm spec: {normalized}"))?;
    let package_path = if normalized.starts_with('@') {
        let parts: Vec<&str> = normalized.split('/').collect();
        if parts.len() < 2 {
            return Err(format!("invalid scoped npm spec: {normalized}"));
        }
        workspace.join("node_modules").join(parts[0]).join(parts[1])
    } else {
        workspace.join("node_modules").join(package_name)
    };
    Ok(package_path)
}

fn load_plugin_manifest_json(package_dir: &Path, package_json: &serde_json::Value) -> String {
    let manifest_path = package_dir.join("openclaw.plugin.json");
    if let Ok(contents) = fs::read_to_string(&manifest_path) {
        if serde_json::from_str::<serde_json::Value>(&contents).is_ok() {
            return contents;
        }
    }

    package_json
        .get("openclaw")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}))
        .to_string()
}

async fn build_feishu_openclaw_config_with_pool(
    pool: &SqlitePool,
) -> Result<serde_json::Value, String> {
    let app_id = get_app_setting(pool, "feishu_app_id")
        .await?
        .unwrap_or_default();
    let app_secret = get_app_setting(pool, "feishu_app_secret")
        .await?
        .unwrap_or_default();
    let ingress_token = get_app_setting(pool, "feishu_ingress_token")
        .await?
        .unwrap_or_default();
    let encrypt_key = get_app_setting(pool, "feishu_encrypt_key")
        .await?
        .unwrap_or_default();
    let employee_connections = list_enabled_employee_feishu_connections_with_pool(pool).await?;
    let default_pairing_allow_from = list_feishu_pairing_allow_from_with_pool(pool, "default")
        .await
        .unwrap_or_default();
    let enabled = !app_id.trim().is_empty() || !employee_connections.is_empty();
    let default_account = if enabled {
        Some("default".to_string())
    } else {
        None
    };
    let default_domain = "feishu";
    let default_connection_mode = "websocket";
    let default_webhook_path = "/feishu/events";
    let default_dm_policy = "pairing";
    let default_group_policy = "allowlist";
    let default_reaction_notifications = "own";
    let default_require_mention = true;
    let default_typing_indicator = true;
    let default_resolve_sender_names = true;
    let default_render_mode = get_app_setting(pool, "feishu_render_mode")
        .await?
        .unwrap_or_else(|| "auto".to_string());
    let default_text_chunk_limit =
        parse_app_setting_i64(get_app_setting(pool, "feishu_text_chunk_limit").await?)
            .unwrap_or(4000);
    let default_chunk_mode = get_app_setting(pool, "feishu_chunk_mode")
        .await?
        .unwrap_or_else(|| "length".to_string());
    let default_markdown_mode = get_app_setting(pool, "feishu_markdown_mode")
        .await?
        .unwrap_or_default();
    let default_markdown_table_mode = get_app_setting(pool, "feishu_markdown_table_mode")
        .await?
        .unwrap_or_default();
    let default_dms = parse_app_setting_json_object(get_app_setting(pool, "feishu_dms").await?);
    let default_footer =
        parse_app_setting_json_object(get_app_setting(pool, "feishu_footer").await?);
    let default_history_limit =
        parse_app_setting_i64(get_app_setting(pool, "feishu_history_limit").await?);
    let default_dm_history_limit =
        parse_app_setting_i64(get_app_setting(pool, "feishu_dm_history_limit").await?);
    let default_group_allow_from =
        parse_app_setting_string_list(get_app_setting(pool, "feishu_group_allow_from").await?);
    let default_group_sender_allow_from = parse_app_setting_string_list(
        get_app_setting(pool, "feishu_group_sender_allow_from").await?,
    );
    let default_group_default_allow_from = parse_app_setting_string_list(
        get_app_setting(pool, "feishu_group_default_allow_from").await?,
    );
    let default_group_default_skills = parse_app_setting_string_list(
        get_app_setting(pool, "feishu_group_default_skills").await?,
    );
    let default_group_default_system_prompt =
        get_app_setting(pool, "feishu_group_default_system_prompt")
            .await?
            .unwrap_or_default();
    let default_group_default_tools = parse_app_setting_json_object(
        get_app_setting(pool, "feishu_group_default_tools").await?,
    );
    let default_group_overrides =
        parse_app_setting_json_object(get_app_setting(pool, "feishu_groups").await?);
    let account_overrides = parse_app_setting_json_object(
        get_app_setting(pool, "feishu_account_overrides").await?,
    );
    let default_streaming =
        parse_app_setting_bool(get_app_setting(pool, "feishu_streaming").await?, false);
    let default_reply_in_thread = get_app_setting(pool, "feishu_reply_in_thread")
        .await?
        .unwrap_or_else(|| "disabled".to_string());
    let default_group_session_scope = get_app_setting(pool, "feishu_group_session_scope")
        .await?
        .unwrap_or_else(|| "group".to_string());
    let default_topic_session_mode = get_app_setting(pool, "feishu_topic_session_mode")
        .await?
        .unwrap_or_else(|| "disabled".to_string());
    let default_webhook_host = get_app_setting(pool, "feishu_webhook_host")
        .await?
        .unwrap_or_default();
    let default_webhook_port =
        parse_app_setting_i64(get_app_setting(pool, "feishu_webhook_port").await?);
    let default_media_max_mb =
        parse_app_setting_i64(get_app_setting(pool, "feishu_media_max_mb").await?);
    let default_http_timeout_ms =
        parse_app_setting_i64(get_app_setting(pool, "feishu_http_timeout_ms").await?);
    let default_config_writes =
        parse_app_setting_bool(get_app_setting(pool, "feishu_config_writes").await?, false);
    let default_actions_reactions = parse_app_setting_bool(
        get_app_setting(pool, "feishu_actions_reactions").await?,
        false,
    );
    let default_block_streaming_coalesce_enabled = parse_app_setting_bool(
        get_app_setting(pool, "feishu_block_streaming_coalesce_enabled").await?,
        false,
    );
    let default_block_streaming_coalesce_min_delay_ms = parse_app_setting_i64(
        get_app_setting(pool, "feishu_block_streaming_coalesce_min_delay_ms").await?,
    );
    let default_block_streaming_coalesce_max_delay_ms = parse_app_setting_i64(
        get_app_setting(pool, "feishu_block_streaming_coalesce_max_delay_ms").await?,
    );
    let default_heartbeat_visibility = get_app_setting(pool, "feishu_heartbeat_visibility")
        .await?
        .unwrap_or_default();
    let default_heartbeat_interval_ms =
        parse_app_setting_i64(get_app_setting(pool, "feishu_heartbeat_interval_ms").await?);
    let default_dynamic_agent_creation_enabled = parse_app_setting_bool(
        get_app_setting(pool, "feishu_dynamic_agent_creation_enabled").await?,
        false,
    );
    let default_dynamic_agent_creation_workspace_template = get_app_setting(
        pool,
        "feishu_dynamic_agent_creation_workspace_template",
    )
    .await?
    .unwrap_or_default();
    let default_dynamic_agent_creation_agent_dir_template = get_app_setting(
        pool,
        "feishu_dynamic_agent_creation_agent_dir_template",
    )
    .await?
    .unwrap_or_default();
    let default_dynamic_agent_creation_max_agents = parse_app_setting_i64(
        get_app_setting(pool, "feishu_dynamic_agent_creation_max_agents").await?,
    );
    let default_tools = serde_json::json!({
        "doc": true,
        "chat": true,
        "wiki": true,
        "drive": true,
        "perm": false,
        "scopes": true
    });
    let default_markdown =
        build_feishu_markdown_projection(&default_markdown_mode, &default_markdown_table_mode);
    let default_heartbeat = build_feishu_heartbeat_projection(
        &default_heartbeat_visibility,
        default_heartbeat_interval_ms,
    );
    let default_block_streaming_coalesce = build_feishu_block_streaming_coalesce_projection(
        default_block_streaming_coalesce_enabled,
        default_block_streaming_coalesce_min_delay_ms,
        default_block_streaming_coalesce_max_delay_ms,
    );
    let default_dynamic_agent_creation = build_feishu_dynamic_agent_creation_projection(
        default_dynamic_agent_creation_enabled,
        &default_dynamic_agent_creation_workspace_template,
        &default_dynamic_agent_creation_agent_dir_template,
        default_dynamic_agent_creation_max_agents,
    );
    let default_groups = build_feishu_default_groups_projection(
        default_require_mention,
        &default_group_session_scope,
        &default_topic_session_mode,
        &default_reply_in_thread,
        default_group_default_allow_from,
        default_group_default_skills,
        &default_group_default_system_prompt,
        default_group_default_tools,
        default_group_overrides,
    );

    let mut accounts = serde_json::Map::new();
    for connection in employee_connections {
        let account_pairing_allow_from =
            list_feishu_pairing_allow_from_with_pool(pool, &connection.employee_id)
                .await
                .unwrap_or_default();
        let account_id = connection.employee_id.clone();
        let mut account_config = serde_json::json!({
                "name": connection.name,
                "appId": connection.app_id,
                "appSecret": connection.app_secret,
                "enabled": true,
                "domain": default_domain,
                "connectionMode": default_connection_mode,
                "webhookPath": default_webhook_path,
                "verificationToken": ingress_token,
                "encryptKey": encrypt_key,
                "webhookHost": default_webhook_host,
                "webhookPort": default_webhook_port,
                "configWrites": default_config_writes,
                "dmPolicy": default_dm_policy,
                "groupPolicy": default_group_policy,
                "groupAllowFrom": default_group_allow_from,
                "groupSenderAllowFrom": default_group_sender_allow_from,
                "requireMention": default_require_mention,
                "groups": default_groups,
                "dms": default_dms,
                "footer": default_footer,
                "markdown": default_markdown,
                "renderMode": default_render_mode,
                "reactionNotifications": default_reaction_notifications,
                "typingIndicator": default_typing_indicator,
                "resolveSenderNames": default_resolve_sender_names,
                "streaming": default_streaming,
                "replyInThread": default_reply_in_thread,
                "historyLimit": default_history_limit,
                "dmHistoryLimit": default_dm_history_limit,
                "groupSessionScope": default_group_session_scope,
                "topicSessionMode": default_topic_session_mode,
                "textChunkLimit": default_text_chunk_limit,
                "chunkMode": default_chunk_mode,
                "blockStreamingCoalesce": default_block_streaming_coalesce,
                "mediaMaxMb": default_media_max_mb,
                "httpTimeoutMs": default_http_timeout_ms,
                "heartbeat": default_heartbeat,
                "dynamicAgentCreation": default_dynamic_agent_creation,
                "tools": default_tools,
                "actions": {
                    "reactions": default_actions_reactions
                },
                "allowFrom": merge_pairing_allow_from(None, account_pairing_allow_from),
            });

        if let Some(overrides) = account_overrides
            .as_object()
            .and_then(|entries| entries.get(&account_id))
        {
            merge_json_value(&mut account_config, overrides.clone());
        }

        accounts.insert(account_id, account_config);
    }

    let mut feishu_channel = serde_json::Map::new();
    feishu_channel.insert("enabled".to_string(), serde_json::json!(enabled));
    feishu_channel.insert(
        "defaultAccount".to_string(),
        serde_json::json!(default_account),
    );
    feishu_channel.insert("appId".to_string(), serde_json::json!(app_id));
    feishu_channel.insert("appSecret".to_string(), serde_json::json!(app_secret));
    feishu_channel.insert(
        "verificationToken".to_string(),
        serde_json::json!(ingress_token),
    );
    feishu_channel.insert("encryptKey".to_string(), serde_json::json!(encrypt_key));
    feishu_channel.insert(
        "webhookHost".to_string(),
        serde_json::json!(default_webhook_host),
    );
    feishu_channel.insert(
        "webhookPort".to_string(),
        serde_json::json!(default_webhook_port),
    );
    feishu_channel.insert(
        "configWrites".to_string(),
        serde_json::json!(default_config_writes),
    );
    feishu_channel.insert("domain".to_string(), serde_json::json!(default_domain));
    feishu_channel.insert(
        "connectionMode".to_string(),
        serde_json::json!(default_connection_mode),
    );
    feishu_channel.insert(
        "webhookPath".to_string(),
        serde_json::json!(default_webhook_path),
    );
    feishu_channel.insert("dmPolicy".to_string(), serde_json::json!(default_dm_policy));
    feishu_channel.insert(
        "groupPolicy".to_string(),
        serde_json::json!(default_group_policy),
    );
    feishu_channel.insert("groupAllowFrom".to_string(), default_group_allow_from);
    feishu_channel.insert(
        "groupSenderAllowFrom".to_string(),
        default_group_sender_allow_from,
    );
    feishu_channel.insert(
        "requireMention".to_string(),
        serde_json::json!(default_require_mention),
    );
    feishu_channel.insert("groups".to_string(), default_groups);
    feishu_channel.insert("dms".to_string(), default_dms);
    feishu_channel.insert("footer".to_string(), default_footer);
    feishu_channel.insert("markdown".to_string(), default_markdown);
    feishu_channel.insert(
        "renderMode".to_string(),
        serde_json::json!(default_render_mode),
    );
    feishu_channel.insert(
        "reactionNotifications".to_string(),
        serde_json::json!(default_reaction_notifications),
    );
    feishu_channel.insert(
        "typingIndicator".to_string(),
        serde_json::json!(default_typing_indicator),
    );
    feishu_channel.insert(
        "resolveSenderNames".to_string(),
        serde_json::json!(default_resolve_sender_names),
    );
    feishu_channel.insert("streaming".to_string(), serde_json::json!(default_streaming));
    feishu_channel.insert(
        "replyInThread".to_string(),
        serde_json::json!(default_reply_in_thread),
    );
    feishu_channel.insert("historyLimit".to_string(), serde_json::json!(default_history_limit));
    feishu_channel.insert(
        "dmHistoryLimit".to_string(),
        serde_json::json!(default_dm_history_limit),
    );
    feishu_channel.insert(
        "groupSessionScope".to_string(),
        serde_json::json!(default_group_session_scope),
    );
    feishu_channel.insert(
        "topicSessionMode".to_string(),
        serde_json::json!(default_topic_session_mode),
    );
    feishu_channel.insert(
        "textChunkLimit".to_string(),
        serde_json::json!(default_text_chunk_limit),
    );
    feishu_channel.insert("chunkMode".to_string(), serde_json::json!(default_chunk_mode));
    feishu_channel.insert(
        "blockStreamingCoalesce".to_string(),
        default_block_streaming_coalesce,
    );
    feishu_channel.insert("mediaMaxMb".to_string(), serde_json::json!(default_media_max_mb));
    feishu_channel.insert(
        "httpTimeoutMs".to_string(),
        serde_json::json!(default_http_timeout_ms),
    );
    feishu_channel.insert("heartbeat".to_string(), default_heartbeat);
    feishu_channel.insert(
        "dynamicAgentCreation".to_string(),
        default_dynamic_agent_creation,
    );
    feishu_channel.insert("tools".to_string(), default_tools);
    feishu_channel.insert(
        "actions".to_string(),
        serde_json::json!({ "reactions": default_actions_reactions }),
    );
    feishu_channel.insert(
        "allowFrom".to_string(),
        merge_pairing_allow_from(None, default_pairing_allow_from),
    );
    feishu_channel.insert(
        "accounts".to_string(),
        serde_json::Value::Object(accounts),
    );

    Ok(serde_json::json!({
        "channels": {
            "feishu": serde_json::Value::Object(feishu_channel)
        },
        "plugins": {
            "entries": {}
        },
        "tools": {
            "profile": "default"
        }
    }))
}

fn parse_app_setting_bool(value: Option<String>, default: bool) -> bool {
    match value
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.to_ascii_lowercase())
        .as_deref()
    {
        Some("1") | Some("true") | Some("yes") | Some("on") => true,
        Some("0") | Some("false") | Some("no") | Some("off") => false,
        _ => default,
    }
}

fn parse_app_setting_i64(value: Option<String>) -> Option<i64> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .and_then(|entry| entry.parse::<i64>().ok())
}

fn parse_app_setting_string_list(value: Option<String>) -> serde_json::Value {
    let Some(raw) = value
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    else {
        return serde_json::json!([]);
    };

    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(raw) {
        if let Some(entries) = parsed.as_array() {
            let normalized = entries
                .iter()
                .filter_map(|entry| match entry {
                    serde_json::Value::String(value) => Some(value.trim().to_string()),
                    serde_json::Value::Number(value) => Some(value.to_string()),
                    _ => None,
                })
                .filter(|entry| !entry.is_empty())
                .map(serde_json::Value::String)
                .collect::<Vec<_>>();
            return serde_json::Value::Array(normalized);
        }
    }

    serde_json::Value::Array(
        raw.split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(|entry| serde_json::Value::String(entry.to_string()))
            .collect::<Vec<_>>(),
    )
}

fn parse_app_setting_json_object(value: Option<String>) -> serde_json::Value {
    let Some(raw) = value.as_deref().map(str::trim).filter(|entry| !entry.is_empty()) else {
        return serde_json::json!({});
    };

    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| serde_json::json!({}))
}

fn build_feishu_markdown_projection(mode: &str, table_mode: &str) -> serde_json::Value {
    let mode = mode.trim();
    let table_mode = table_mode.trim();
    let mut markdown = serde_json::Map::new();
    if !mode.is_empty() {
        markdown.insert(
            "mode".to_string(),
            serde_json::Value::String(mode.to_string()),
        );
    }
    if !table_mode.is_empty() {
        markdown.insert(
            "tableMode".to_string(),
            serde_json::Value::String(table_mode.to_string()),
        );
    }
    serde_json::Value::Object(markdown)
}

fn build_feishu_heartbeat_projection(
    visibility: &str,
    interval_ms: Option<i64>,
) -> serde_json::Value {
    let mut heartbeat = serde_json::Map::new();
    if !visibility.trim().is_empty() {
        heartbeat.insert(
            "visibility".to_string(),
            serde_json::Value::String(visibility.trim().to_string()),
        );
    }
    if let Some(interval_ms) = interval_ms {
        heartbeat.insert("intervalMs".to_string(), serde_json::json!(interval_ms));
    }
    serde_json::Value::Object(heartbeat)
}

fn build_feishu_block_streaming_coalesce_projection(
    enabled: bool,
    min_delay_ms: Option<i64>,
    max_delay_ms: Option<i64>,
) -> serde_json::Value {
    let mut config = serde_json::Map::new();
    if enabled {
        config.insert("enabled".to_string(), serde_json::json!(true));
    }
    if let Some(min_delay_ms) = min_delay_ms {
        config.insert("minDelayMs".to_string(), serde_json::json!(min_delay_ms));
    }
    if let Some(max_delay_ms) = max_delay_ms {
        config.insert("maxDelayMs".to_string(), serde_json::json!(max_delay_ms));
    }
    serde_json::Value::Object(config)
}

fn build_feishu_dynamic_agent_creation_projection(
    enabled: bool,
    workspace_template: &str,
    agent_dir_template: &str,
    max_agents: Option<i64>,
) -> serde_json::Value {
    let mut config = serde_json::Map::new();
    if enabled {
        config.insert("enabled".to_string(), serde_json::json!(true));
    }
    if !workspace_template.trim().is_empty() {
        config.insert(
            "workspaceTemplate".to_string(),
            serde_json::Value::String(workspace_template.trim().to_string()),
        );
    }
    if !agent_dir_template.trim().is_empty() {
        config.insert(
            "agentDirTemplate".to_string(),
            serde_json::Value::String(agent_dir_template.trim().to_string()),
        );
    }
    if let Some(max_agents) = max_agents {
        config.insert("maxAgents".to_string(), serde_json::json!(max_agents));
    }
    serde_json::Value::Object(config)
}

fn build_feishu_default_groups_projection(
    require_mention: bool,
    group_session_scope: &str,
    topic_session_mode: &str,
    reply_in_thread: &str,
    allow_from: serde_json::Value,
    skills: serde_json::Value,
    system_prompt: &str,
    tools: serde_json::Value,
    overrides: serde_json::Value,
) -> serde_json::Value {
    let mut group = serde_json::Map::new();
    group.insert("enabled".to_string(), serde_json::json!(true));
    group.insert(
        "requireMention".to_string(),
        serde_json::json!(require_mention),
    );
    group.insert(
        "groupSessionScope".to_string(),
        serde_json::json!(group_session_scope),
    );
    group.insert(
        "topicSessionMode".to_string(),
        serde_json::json!(topic_session_mode),
    );
    group.insert(
        "replyInThread".to_string(),
        serde_json::json!(reply_in_thread),
    );
    if allow_from.as_array().is_some_and(|items| !items.is_empty()) {
        group.insert("allowFrom".to_string(), allow_from);
    }
    if skills.as_array().is_some_and(|items| !items.is_empty()) {
        group.insert("skills".to_string(), skills);
    }
    if !system_prompt.trim().is_empty() {
        group.insert(
            "systemPrompt".to_string(),
            serde_json::Value::String(system_prompt.trim().to_string()),
        );
    }
    if tools.as_object().is_some_and(|items| !items.is_empty()) {
        group.insert("tools".to_string(), tools);
    }

    let mut groups = serde_json::Map::new();
    groups.insert("*".to_string(), serde_json::Value::Object(group));

    if let Some(entries) = overrides.as_object() {
        for (group_id, value) in entries {
            if group_id.trim().is_empty() {
                continue;
            }
            if value.is_object() {
                groups.insert(group_id.to_string(), value.clone());
            }
        }
    }

    serde_json::Value::Object(groups)
}

fn merge_json_value(target: &mut serde_json::Value, override_value: serde_json::Value) {
    match (target, override_value) {
        (serde_json::Value::Object(target_map), serde_json::Value::Object(override_map)) => {
            for (key, value) in override_map {
                match target_map.get_mut(&key) {
                    Some(existing) => merge_json_value(existing, value),
                    None => {
                        target_map.insert(key, value);
                    }
                }
            }
        }
        (target_slot, override_value) => {
            *target_slot = override_value;
        }
    }
}

fn handle_feishu_runtime_pairing_request_event(
    pool: &SqlitePool,
    status: &mut OpenClawPluginFeishuRuntimeStatus,
    value: &serde_json::Value,
) {
    let sender_id = value
        .get("senderId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty());
    let account_id = value
        .get("accountId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .unwrap_or("default");
    let code = value
        .get("code")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .unwrap_or("PAIRING");

    let Some(sender_id) = sender_id else {
        return;
    };

    match tauri::async_runtime::block_on(upsert_feishu_pairing_request_with_pool(
        pool,
        account_id,
        sender_id,
        "",
        Some(code),
    )) {
        Ok((record, created)) => {
            status.last_event_at = Some(now_rfc3339());
            let entry = format!(
                "[pairing] feishu: {} request {} for {} code={}",
                if created { "created" } else { "reused" },
                record.id,
                record.sender_id,
                record.code
            );
            status.recent_logs.push(entry);
            if status.recent_logs.len() > 40 {
                let overflow = status.recent_logs.len() - 40;
                status.recent_logs.drain(0..overflow);
            }
            if record.code.trim().is_empty() || record.code == "PAIRING" {
                status.last_error = Some(format!(
                    "official runtime emitted placeholder pairing code for {} (raw={code})",
                    record.sender_id
                ));
            }
        }
        Err(error) => {
            status.last_event_at = Some(now_rfc3339());
            status.last_error = Some(format!("failed to persist feishu pairing request: {error}"));
            status.recent_logs.push(format!(
                "[error] runtime: failed to persist feishu pairing request: {error}"
            ));
            if status.recent_logs.len() > 40 {
                let overflow = status.recent_logs.len() - 40;
                status.recent_logs.drain(0..overflow);
            }
        }
    }
}

async fn resolve_feishu_runtime_dispatch_thread_id_with_pool(
    pool: &SqlitePool,
    thread_id: &str,
    account_id: Option<&str>,
    sender_id: Option<&str>,
    chat_type: Option<&str>,
) -> Result<String, String> {
    let normalized_thread_id = thread_id.trim();
    if normalized_thread_id.is_empty() {
        return Err("dispatch_request missing threadId".to_string());
    }

    let is_direct = matches!(chat_type.map(str::trim), Some("direct") | None);
    let looks_like_sender_open_id = normalized_thread_id.starts_with("ou_");
    if !is_direct || !looks_like_sender_open_id {
        return Ok(normalized_thread_id.to_string());
    }

    let normalized_account_id = account_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("default");
    let normalized_sender_id = sender_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(normalized_thread_id);

    let row = sqlx::query_as::<_, (String,)>(
        "SELECT chat_id
         FROM feishu_pairing_requests
         WHERE channel = 'feishu'
           AND account_id = ?
           AND sender_id = ?
           AND chat_id <> ''
         ORDER BY
           CASE status WHEN 'approved' THEN 0 WHEN 'pending' THEN 1 ELSE 2 END,
           updated_at DESC,
           created_at DESC
         LIMIT 1",
    )
    .bind(normalized_account_id)
    .bind(normalized_sender_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("failed to resolve feishu chat_id from pairing requests: {e}"))?;

    Ok(row
        .map(|(chat_id,)| chat_id)
        .filter(|chat_id| !chat_id.trim().is_empty())
        .unwrap_or_else(|| normalized_thread_id.to_string()))
}

async fn parse_feishu_runtime_dispatch_event_with_pool(
    pool: &SqlitePool,
    value: &serde_json::Value,
) -> Result<ImEvent, String> {
    let raw_thread_id = value
        .get("threadId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| "dispatch_request missing threadId".to_string())?;
    let text = value
        .get("text")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let account_id = value
        .get("accountId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let sender_id = value
        .get("senderId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let explicit_chat_id = value
        .get("chatId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let chat_type = value
        .get("chatType")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let message_id = value
        .get("messageId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let role_id = value
        .get("roleId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let thread_id = if matches!(chat_type.as_deref(), Some("direct") | None) {
        if let Some(chat_id) = explicit_chat_id.clone() {
            chat_id
        } else {
            resolve_feishu_runtime_dispatch_thread_id_with_pool(
                pool,
                raw_thread_id,
                account_id.as_deref(),
                sender_id.as_deref(),
                chat_type.as_deref(),
            )
            .await?
        }
    } else {
        resolve_feishu_runtime_dispatch_thread_id_with_pool(
            pool,
            raw_thread_id,
            account_id.as_deref(),
            sender_id.as_deref(),
            chat_type.as_deref(),
        )
        .await?
    };

    Ok(ImEvent {
        channel: "feishu".to_string(),
        event_type: if role_id.is_some() {
            ImEventType::MentionRole
        } else {
            ImEventType::MessageCreated
        },
        thread_id,
        event_id: message_id.clone(),
        message_id,
        text,
        role_id,
        account_id: account_id.clone(),
        tenant_id: account_id,
        sender_id,
        chat_type,
    })
}

async fn get_openclaw_plugin_install_by_id_with_pool(
    pool: &SqlitePool,
    plugin_id: &str,
) -> Result<OpenClawPluginInstallRecord, String> {
    let normalized = normalize_required(plugin_id, "plugin_id")?;
    let row = sqlx::query_as::<_, (String, String, String, String, String, String, String, String)>(
        "SELECT plugin_id, npm_spec, version, install_path, source_type, manifest_json, installed_at, updated_at
         FROM installed_openclaw_plugins
         WHERE plugin_id = ?
         LIMIT 1",
    )
    .bind(&normalized)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("openclaw plugin install not found: {normalized}"))?;

    Ok(OpenClawPluginInstallRecord {
        plugin_id: row.0,
        npm_spec: row.1,
        version: row.2,
        install_path: row.3,
        source_type: row.4,
        manifest_json: row.5,
        installed_at: row.6,
        updated_at: row.7,
    })
}

async fn inspect_openclaw_plugin_with_pool_and_app(
    pool: &SqlitePool,
    plugin_id: &str,
    app: Option<&AppHandle>,
) -> Result<OpenClawPluginInspectionResult, String> {
    let install = get_openclaw_plugin_install_by_id_with_pool(pool, plugin_id).await?;
    let script_path = resolve_plugin_host_inspect_script();
    if !script_path.exists() {
        return Err(format!(
            "plugin host inspect script is missing: {}",
            script_path.display()
        ));
    }

    let plugin_host_dir = resolve_plugin_host_dir();
    let mut command = Command::new("node");
    command
        .current_dir(&plugin_host_dir)
        .arg(script_path)
        .arg("--plugin-root")
        .arg(&install.install_path)
        .arg("--fixture-name")
        .arg(&install.plugin_id);
    if let Some(app) = app {
        command
            .arg("--fixture-root")
            .arg(resolve_plugin_host_fixture_root(app)?);
    }
    apply_command_search_path(&mut command, &[]);
    hide_console_window(&mut command);
    let output = command
        .output()
        .map_err(|e| format!("failed to launch plugin host inspect script: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(format!("plugin host inspect failed: {detail}"));
    }

    serde_json::from_slice::<OpenClawPluginInspectionResult>(&output.stdout)
        .map_err(|e| format!("failed to parse plugin host inspect json: {e}"))
}

pub async fn inspect_openclaw_plugin_with_pool(
    pool: &SqlitePool,
    plugin_id: &str,
) -> Result<OpenClawPluginInspectionResult, String> {
    inspect_openclaw_plugin_with_pool_and_app(pool, plugin_id, None).await
}

async fn get_openclaw_plugin_feishu_channel_snapshot_with_pool_and_app(
    pool: &SqlitePool,
    plugin_id: &str,
    app: Option<&AppHandle>,
) -> Result<OpenClawPluginChannelSnapshotResult, String> {
    let install = get_openclaw_plugin_install_by_id_with_pool(pool, plugin_id).await?;
    let config_json = build_feishu_openclaw_config_with_pool(pool).await?;
    let script_path = resolve_plugin_host_inspect_script();
    if !script_path.exists() {
        return Err(format!(
            "plugin host inspect script is missing: {}",
            script_path.display()
        ));
    }

    let plugin_host_dir = resolve_plugin_host_dir();
    let mut command = Command::new("node");
    command
        .current_dir(&plugin_host_dir)
        .arg(script_path)
        .arg("--plugin-root")
        .arg(&install.install_path)
        .arg("--fixture-name")
        .arg(format!("{}-feishu-snapshot", install.plugin_id))
        .arg("--channel-snapshot")
        .arg("feishu")
        .arg("--config-json")
        .arg(config_json.to_string());
    if let Some(app) = app {
        command
            .arg("--fixture-root")
            .arg(resolve_plugin_host_fixture_root(app)?);
    }
    apply_command_search_path(&mut command, &[]);
    hide_console_window(&mut command);
    let output = command
        .output()
        .map_err(|e| format!("failed to launch plugin host snapshot script: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(format!("plugin host channel snapshot failed: {detail}"));
    }

    serde_json::from_slice::<OpenClawPluginChannelSnapshotResult>(&output.stdout)
        .map_err(|e| format!("failed to parse plugin host channel snapshot json: {e}"))
}

pub async fn get_openclaw_plugin_feishu_channel_snapshot_with_pool(
    pool: &SqlitePool,
    plugin_id: &str,
) -> Result<OpenClawPluginChannelSnapshotResult, String> {
    get_openclaw_plugin_feishu_channel_snapshot_with_pool_and_app(pool, plugin_id, None).await
}

fn derive_channel_capabilities(channel: &OpenClawPluginChannelInspection) -> Vec<String> {
    let mut capabilities = Vec::new();
    let record = channel
        .capabilities
        .as_ref()
        .and_then(|value| value.as_object());

    if let Some(chat_types) = record
        .and_then(|capabilities| capabilities.get("chatTypes"))
        .and_then(|value| value.as_array())
    {
        for chat_type in chat_types.iter().filter_map(|value| value.as_str()) {
            capabilities.push(format!("chat_type:{chat_type}"));
        }
    }

    for (key, tag) in [
        ("media", "media"),
        ("reactions", "reactions"),
        ("threads", "threads"),
        ("polls", "polls"),
        ("nativeCommands", "native_commands"),
        ("blockStreaming", "block_streaming"),
    ] {
        if record
            .and_then(|capabilities| capabilities.get(key))
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            capabilities.push(tag.to_string());
        }
    }

    if channel.has_pairing {
        capabilities.push("pairing".to_string());
    }
    if channel.has_setup {
        capabilities.push("setup".to_string());
    }
    if channel.has_onboarding {
        capabilities.push("onboarding".to_string());
    }
    if channel.has_directory {
        capabilities.push("directory".to_string());
    }
    if channel.has_outbound {
        capabilities.push("outbound".to_string());
    }
    if channel.has_threading {
        capabilities.push("threading".to_string());
    }
    if channel.has_actions {
        capabilities.push("actions".to_string());
    }
    if channel.has_status {
        capabilities.push("status".to_string());
    }

    capabilities.sort();
    capabilities.dedup();
    capabilities
}

fn inspection_to_channel_hosts(
    install: &OpenClawPluginInstallRecord,
    inspection: &OpenClawPluginInspectionResult,
) -> Vec<OpenClawPluginChannelHost> {
    inspection
        .summary
        .channels
        .iter()
        .map(|channel| OpenClawPluginChannelHost {
            plugin_id: install.plugin_id.clone(),
            npm_spec: install.npm_spec.clone(),
            version: install.version.clone(),
            channel: channel
                .id
                .clone()
                .or_else(|| channel.meta.as_ref().and_then(|meta| meta.id.clone()))
                .unwrap_or_else(|| "unknown".to_string()),
            display_name: channel
                .meta
                .as_ref()
                .and_then(|meta| meta.label.clone())
                .or_else(|| channel.id.clone())
                .unwrap_or_else(|| install.plugin_id.clone()),
            capabilities: derive_channel_capabilities(channel),
            reload_config_prefixes: channel.reload_config_prefixes.clone(),
            target_hint: channel.target_hint.clone(),
            docs_path: channel
                .meta
                .as_ref()
                .and_then(|meta| meta.docs_path.clone()),
            status: "ready".to_string(),
            error: None,
        })
        .collect()
}

async fn list_openclaw_plugin_channel_hosts_with_pool_and_app(
    pool: &SqlitePool,
    app: Option<&AppHandle>,
) -> Result<Vec<OpenClawPluginChannelHost>, String> {
    let installs = list_openclaw_plugin_installs_with_pool(pool).await?;
    let mut hosts = Vec::new();

    for install in installs {
        match inspect_openclaw_plugin_with_pool_and_app(pool, &install.plugin_id, app).await {
            Ok(inspection) => {
                hosts.extend(inspection_to_channel_hosts(&install, &inspection));
            }
            Err(error) => {
                hosts.push(OpenClawPluginChannelHost {
                    plugin_id: install.plugin_id.clone(),
                    npm_spec: install.npm_spec.clone(),
                    version: install.version.clone(),
                    channel: install.plugin_id.clone(),
                    display_name: install.plugin_id.clone(),
                    capabilities: Vec::new(),
                    reload_config_prefixes: Vec::new(),
                    target_hint: None,
                    docs_path: None,
                    status: "error".to_string(),
                    error: Some(error),
                });
            }
        }
    }

    Ok(hosts)
}

pub async fn list_openclaw_plugin_channel_hosts_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<OpenClawPluginChannelHost>, String> {
    list_openclaw_plugin_channel_hosts_with_pool_and_app(pool, None).await
}

pub async fn start_openclaw_plugin_feishu_runtime_with_pool(
    pool: &SqlitePool,
    state: &OpenClawPluginFeishuRuntimeState,
    plugin_id: &str,
    account_id: Option<&str>,
    app: Option<tauri::AppHandle>,
) -> Result<OpenClawPluginFeishuRuntimeStatus, String> {
    let normalized_plugin_id = normalize_required(plugin_id, "plugin_id")?;
    let normalized_account_id = normalize_required(account_id.unwrap_or("default"), "account_id")
        .unwrap_or_else(|_| "default".to_string());
    let current_pid = {
        let guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock feishu runtime state".to_string())?;
        guard.status.pid
    };
    let should_stop_existing = {
        let guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock feishu runtime state".to_string())?;
        guard.status.running
            && (guard.status.plugin_id != normalized_plugin_id
                || guard.status.account_id != normalized_account_id)
    };

    if should_stop_existing {
        let _ = stop_openclaw_plugin_feishu_runtime_in_state(state);
    }

    {
        let guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock feishu runtime state".to_string())?;
        if guard.status.running
            && guard.status.plugin_id == normalized_plugin_id
            && guard.status.account_id == normalized_account_id
        {
            return Ok(guard.status.clone());
        }
    }

    let install = get_openclaw_plugin_install_by_id_with_pool(pool, &normalized_plugin_id).await?;
    let stale_pids = cleanup_stale_feishu_runtime_processes(
        &install.install_path,
        &normalized_account_id,
        current_pid,
    )?;
    if !stale_pids.is_empty() {
        if let Ok(mut guard) = state.0.lock() {
            guard.status.last_event_at = Some(now_rfc3339());
            guard.status.recent_logs.push(format!(
                "[runtime] cleaned up stale feishu host pids: {}",
                stale_pids
                    .iter()
                    .map(|pid| pid.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            if guard.status.recent_logs.len() > 40 {
                let overflow = guard.status.recent_logs.len() - 40;
                guard.status.recent_logs.drain(0..overflow);
            }
        }
    }
    let config_json = build_feishu_openclaw_config_with_pool(pool).await?;
    let script_path = resolve_plugin_host_run_feishu_script();
    if !script_path.exists() {
        return Err(format!(
            "plugin host feishu runtime script is missing: {}",
            script_path.display()
        ));
    }

    let plugin_host_dir = resolve_plugin_host_dir();
    let mut command = Command::new("node");
    command
        .current_dir(&plugin_host_dir)
        .arg(script_path)
        .arg("--plugin-root")
        .arg(&install.install_path)
        .arg("--fixture-name")
        .arg(format!("{}-runtime", install.plugin_id))
        .arg("--account-id")
        .arg(&normalized_account_id)
        .arg("--config-json")
        .arg(config_json.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(app) = app.as_ref() {
        command
            .arg("--fixture-root")
            .arg(resolve_plugin_host_fixture_root(app)?);
    }
    apply_command_search_path(&mut command, &[]);
    hide_console_window(&mut command);

    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to launch official feishu runtime: {e}"))?;
    let pid = child.id();
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "failed to capture official feishu runtime stdin".to_string())?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let child_slot = Arc::new(Mutex::new(Some(child)));
    let stdin_slot = Arc::new(Mutex::new(stdin));

    {
        let mut guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock feishu runtime state".to_string())?;
        guard.process = Some(child_slot.clone());
        guard.stdin = Some(stdin_slot.clone());
        guard.status = OpenClawPluginFeishuRuntimeStatus {
            plugin_id: normalized_plugin_id.clone(),
            account_id: normalized_account_id.clone(),
            running: true,
            started_at: Some(now_rfc3339()),
            last_stop_at: None,
            last_event_at: None,
            last_error: None,
            pid: Some(pid),
            port: None,
            recent_logs: Vec::new(),
        };
    }

    if let Some(stdout) = stdout {
        let state_clone = state.clone();
        let pool_clone = pool.clone();
        let app_clone = app.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
                    if let Ok(mut guard) = state_clone.0.lock() {
                        guard
                            .status
                            .recent_logs
                            .push(format!("[stdout] runtime: {}", trimmed));
                        if guard.status.recent_logs.len() > 40 {
                            let overflow = guard.status.recent_logs.len() - 40;
                            guard.status.recent_logs.drain(0..overflow);
                        }
                        guard.status.last_event_at = Some(now_rfc3339());
                    }
                    continue;
                };
                if let Ok(mut guard) = state_clone.0.lock() {
                    let event = value
                        .get("event")
                        .and_then(|entry| entry.as_str())
                        .unwrap_or_default();
                    if event == "send_request" {
                        guard.status.recent_logs.push(
                            value
                                .get("requestId")
                                .and_then(|entry| entry.as_str())
                                .map(|request_id| {
                                    format!("[outbound] send_request requestId={request_id}")
                                })
                                .unwrap_or_else(|| {
                                    "[outbound] send_request received".to_string()
                                }),
                        );
                        trim_recent_runtime_logs(&mut guard.status);
                        guard.status.last_event_at = Some(now_rfc3339());
                        continue;
                    }
                    if event == "send_result" {
                        drop(guard);
                        let handled =
                            handle_openclaw_plugin_feishu_runtime_send_result_event(
                                &state_clone,
                                &value,
                            );
                        if !handled {
                            if let Ok(mut guard) = state_clone.0.lock() {
                                guard.status.recent_logs.push(
                                    "[warn] runtime: unhandled outbound send_result event"
                                        .to_string(),
                                );
                                trim_recent_runtime_logs(&mut guard.status);
                                guard.status.last_event_at = Some(now_rfc3339());
                            }
                        }
                        continue;
                    }
                    if event == "command_error" {
                        drop(guard);
                        let handled =
                            handle_openclaw_plugin_feishu_runtime_command_error_event(
                                &state_clone,
                                &value,
                            );
                        if !handled {
                            if let Ok(mut guard) = state_clone.0.lock() {
                                guard.status.recent_logs.push(
                                    "[warn] runtime: unhandled outbound command_error event"
                                        .to_string(),
                                );
                                trim_recent_runtime_logs(&mut guard.status);
                                guard.status.last_event_at = Some(now_rfc3339());
                            }
                        }
                        continue;
                    }
                    if event == "pairing_request" {
                        handle_feishu_runtime_pairing_request_event(
                            &pool_clone,
                            &mut guard.status,
                            &value,
                        );
                    } else if event == "dispatch_request" {
                        guard.status.last_event_at = Some(now_rfc3339());
                        match tauri::async_runtime::block_on(
                            parse_feishu_runtime_dispatch_event_with_pool(&pool_clone, &value),
                        ) {
                            Ok(inbound) => {
                                if let Some(app_handle) = app_clone.as_ref() {
                                    match tauri::async_runtime::block_on(
                                        dispatch_feishu_inbound_to_workclaw_with_pool_and_app(
                                            &pool_clone,
                                            app_handle,
                                            &inbound,
                                            None,
                                        ),
                                    ) {
                                        Ok(result) => {
                                            guard.status.recent_logs.push(format!(
                                                "[dispatch] feishu: accepted={} deduped={} thread={}",
                                                result.accepted, result.deduped, inbound.thread_id
                                            ));
                                        }
                                        Err(error) => {
                                            guard.status.last_error = Some(format!(
                                                "failed to bridge official feishu dispatch: {error}"
                                            ));
                                            guard.status.recent_logs.push(format!(
                                                "[error] runtime: failed to bridge official feishu dispatch: {error}"
                                            ));
                                        }
                                    }
                                } else {
                                    guard.status.recent_logs.push(
                                        "[warn] runtime: dispatch_request ignored because no app handle was available"
                                            .to_string(),
                                    );
                                }
                            }
                            Err(error) => {
                                guard.status.last_error = Some(format!(
                                    "invalid official feishu dispatch event: {error}"
                                ));
                                guard.status.recent_logs.push(format!(
                                    "[error] runtime: invalid official feishu dispatch event: {error}"
                                ));
                            }
                        }
                        if guard.status.recent_logs.len() > 40 {
                            let overflow = guard.status.recent_logs.len() - 40;
                            guard.status.recent_logs.drain(0..overflow);
                        }
                    } else {
                        merge_feishu_runtime_status_event(&mut guard.status, &value);
                    }
                }
            }
        });
    }

    if let Some(stderr) = stderr {
        let state_clone = state.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                eprintln!("[openclaw-feishu-runtime] {}", trimmed);
                if let Ok(mut guard) = state_clone.0.lock() {
                    guard.status.last_error = Some(trimmed.to_string());
                    guard
                        .status
                        .recent_logs
                        .push(format!("[stderr] runtime: {}", trimmed));
                    if guard.status.recent_logs.len() > 40 {
                        let overflow = guard.status.recent_logs.len() - 40;
                        guard.status.recent_logs.drain(0..overflow);
                    }
                    guard.status.last_event_at = Some(now_rfc3339());
                }
            }
        });
    }

    {
        let state_clone = state.clone();
        let child_slot_clone = child_slot.clone();
        thread::spawn(move || loop {
            let exit_status = {
                let mut child_guard = match child_slot_clone.lock() {
                    Ok(guard) => guard,
                    Err(_) => break,
                };
                if let Some(child) = child_guard.as_mut() {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            let success = status.success();
                            let code = status.code();
                            *child_guard = None;
                            Some((success, code, None::<String>))
                        }
                        Ok(None) => None,
                        Err(error) => {
                            *child_guard = None;
                            Some((false, Some(-1), Some(error.to_string())))
                        }
                    }
                } else {
                    break;
                }
            };

            match exit_status {
                Some((success, code, command_error)) => {
                    let failure_message = if let Some(error) = command_error.as_ref() {
                        format!("official feishu runtime wait failed: {error}")
                    } else {
                        match code {
                            Some(value) if value >= 0 => {
                                format!("official feishu runtime exited with code {value}")
                            }
                            _ => "official feishu runtime exited unexpectedly".to_string(),
                        }
                    };
                    let should_fail_waiters = if let Ok(mut guard) = state_clone.0.lock() {
                        guard.process = None;
                        guard.stdin = None;
                        let should_fail_waiters = !guard.pending_outbound_send_results.is_empty();
                        guard.status.running = false;
                        guard.status.pid = None;
                        guard.status.last_stop_at = Some(now_rfc3339());
                        if !success
                            && guard
                                .status
                                .last_error
                                .as_deref()
                                .unwrap_or("")
                                .trim()
                                .is_empty()
                        {
                            guard.status.last_error = Some(failure_message.clone());
                        }
                        should_fail_waiters
                    } else {
                        false
                    };
                    if should_fail_waiters {
                        let _ = fail_pending_feishu_runtime_outbound_send_waiters(
                            &state_clone,
                            "official feishu runtime exited before outbound result was delivered"
                                .to_string(),
                        );
                    }
                    break;
                }
                None => {
                    thread::sleep(Duration::from_millis(250));
                }
            }
        });
    }

    Ok(current_feishu_runtime_status(state))
}

pub async fn maybe_restore_openclaw_plugin_feishu_runtime_with_pool(
    pool: &SqlitePool,
    state: &OpenClawPluginFeishuRuntimeState,
    app: tauri::AppHandle,
) -> Result<bool, String> {
    let progress = get_feishu_setup_progress_with_pool(pool, state).await?;
    if !should_auto_restore_feishu_runtime(&progress) {
        return Ok(false);
    }

    start_openclaw_plugin_feishu_runtime_with_pool(
        pool,
        state,
        "openclaw-lark",
        None,
        Some(app),
    )
    .await
    .map(|_| true)
}

pub fn stop_openclaw_plugin_feishu_runtime_in_state(
    state: &OpenClawPluginFeishuRuntimeState,
) -> Result<OpenClawPluginFeishuRuntimeStatus, String> {
    let (process, _stdin, should_fail_waiters) = {
        let mut guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock feishu runtime state".to_string())?;
        (
            guard.process.take(),
            guard.stdin.take(),
            !guard.pending_outbound_send_results.is_empty(),
        )
    };

    if let Some(slot) = process {
        if let Ok(mut child_guard) = slot.lock() {
            if let Some(mut child) = child_guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    let mut guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock feishu runtime state".to_string())?;
    guard.status.running = false;
    guard.status.pid = None;
    guard.status.last_stop_at = Some(now_rfc3339());
    drop(guard);

    if should_fail_waiters {
        let _ = fail_pending_feishu_runtime_outbound_send_waiters(
            state,
            "official feishu runtime stopped before outbound result was delivered".to_string(),
        );
    }

    let guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock feishu runtime state".to_string())?;
    Ok(guard.status.clone())
}

fn quote_powershell_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn matches_feishu_runtime_command_line(
    command_line: &str,
    plugin_root: &str,
    account_id: &str,
) -> bool {
    command_line.contains("run-feishu-host.mjs")
        && command_line.contains(&format!("--plugin-root {}", plugin_root))
        && command_line.contains(&format!("--account-id {}", account_id))
}

#[cfg(target_os = "windows")]
fn list_matching_feishu_runtime_pids(
    plugin_root: &str,
    account_id: &str,
) -> Result<Vec<u32>, String> {
    let script = format!(
        "$pluginRoot = {plugin_root}; \
         $accountId = {account_id}; \
         Get-CimInstance Win32_Process | \
         Where-Object {{ \
           $_.Name -eq 'node.exe' -and \
           $_.CommandLine -ne $null -and \
           $_.CommandLine.Contains('run-feishu-host.mjs') -and \
           $_.CommandLine.Contains('--plugin-root') -and \
           $_.CommandLine.Contains($pluginRoot) -and \
           $_.CommandLine.Contains('--account-id') -and \
           $_.CommandLine.Contains($accountId) \
         }} | \
         Select-Object -ExpandProperty ProcessId",
        plugin_root = quote_powershell_literal(plugin_root),
        account_id = quote_powershell_literal(account_id),
    );

    let mut command = Command::new("powershell");
    command.args(["-NoProfile", "-Command", &script]);
    hide_console_window(&mut command);
    let output = command
        .output()
        .map_err(|error| format!("failed to inspect feishu runtime processes: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "failed to inspect feishu runtime processes: {}",
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pids = stdout
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect::<Vec<_>>();
    Ok(pids)
}

#[cfg(not(target_os = "windows"))]
fn list_matching_feishu_runtime_pids(
    _plugin_root: &str,
    _account_id: &str,
) -> Result<Vec<u32>, String> {
    Ok(Vec::new())
}

fn kill_process_tree_by_pid(pid: u32) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("taskkill");
        command.args(["/T", "/F", "/PID", &pid.to_string()]);
        hide_console_window(&mut command);
        command
            .output()
            .map_err(|error| format!("failed to terminate runtime pid {pid}: {error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output()
            .map_err(|error| format!("failed to terminate runtime pid {pid}: {error}"))?;
        Ok(())
    }
}

fn cleanup_stale_feishu_runtime_processes(
    plugin_root: &str,
    account_id: &str,
    keep_pid: Option<u32>,
) -> Result<Vec<u32>, String> {
    let matching = list_matching_feishu_runtime_pids(plugin_root, account_id)?;
    let stale = matching
        .into_iter()
        .filter(|pid| Some(*pid) != keep_pid)
        .collect::<Vec<_>>();

    for pid in &stale {
        let _ = kill_process_tree_by_pid(*pid);
    }

    Ok(stale)
}

fn current_openclaw_lark_installer_session_status(
    state: &OpenClawLarkInstallerSessionState,
) -> OpenClawLarkInstallerSessionStatus {
    state
        .0
        .lock()
        .map(|guard| guard.status.clone())
        .unwrap_or_default()
}

fn feishu_open_api_base_url() -> String {
    std::env::var("WORKCLAW_FEISHU_OPEN_API_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "https://open.feishu.cn".to_string())
}

fn parse_feishu_app_access_token_response(value: serde_json::Value) -> Result<String, String> {
    let code = value
        .get("code")
        .and_then(|entry| entry.as_i64())
        .unwrap_or(-1);
    if code != 0 {
        let msg = value
            .get("msg")
            .and_then(|entry| entry.as_str())
            .unwrap_or("unknown error");
        return Err(format!("API error: {msg}"));
    }

    value
        .get("app_access_token")
        .and_then(|entry| entry.as_str())
        .filter(|entry| !entry.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| "missing app_access_token".to_string())
}

fn parse_feishu_bot_info_response(
    app_id: &str,
    value: serde_json::Value,
) -> OpenClawPluginFeishuCredentialProbeResult {
    let code = value
        .get("code")
        .and_then(|entry| entry.as_i64())
        .unwrap_or(-1);
    if code != 0 {
        let msg = value
            .get("msg")
            .and_then(|entry| entry.as_str())
            .unwrap_or("unknown error");
        return OpenClawPluginFeishuCredentialProbeResult {
            ok: false,
            app_id: app_id.to_string(),
            bot_name: None,
            bot_open_id: None,
            error: Some(format!("API error: {msg}")),
        };
    }

    let bot = value
        .get("bot")
        .or_else(|| value.get("data").and_then(|entry| entry.get("bot")));

    OpenClawPluginFeishuCredentialProbeResult {
        ok: true,
        app_id: app_id.to_string(),
        bot_name: bot
            .and_then(|entry| entry.get("bot_name"))
            .and_then(|entry| entry.as_str())
            .map(str::to_string),
        bot_open_id: bot
            .and_then(|entry| entry.get("open_id"))
            .and_then(|entry| entry.as_str())
            .map(str::to_string),
        error: None,
    }
}

async fn probe_openclaw_plugin_feishu_credentials_with_client(
    client: &Client,
    app_id: &str,
    app_secret: &str,
) -> Result<OpenClawPluginFeishuCredentialProbeResult, String> {
    if app_id.trim().is_empty() || app_secret.trim().is_empty() {
        return Ok(OpenClawPluginFeishuCredentialProbeResult {
            ok: false,
            app_id: app_id.trim().to_string(),
            bot_name: None,
            bot_open_id: None,
            error: Some("missing credentials (appId, appSecret)".to_string()),
        });
    }

    let base_url = feishu_open_api_base_url().trim_end_matches('/').to_string();
    let token_response = client
        .post(format!(
            "{base_url}/open-apis/auth/v3/app_access_token/internal"
        ))
        .json(&serde_json::json!({
            "app_id": app_id,
            "app_secret": app_secret,
        }))
        .send()
        .await
        .map_err(|error| format!("failed to request app_access_token: {error}"))?;
    let token_json: serde_json::Value = token_response
        .json()
        .await
        .map_err(|error| format!("failed to decode app_access_token response: {error}"))?;
    let access_token = match parse_feishu_app_access_token_response(token_json) {
        Ok(token) => token,
        Err(error) => {
            return Ok(OpenClawPluginFeishuCredentialProbeResult {
                ok: false,
                app_id: app_id.to_string(),
                bot_name: None,
                bot_open_id: None,
                error: Some(error),
            })
        }
    };

    let bot_response = client
        .get(format!("{base_url}/open-apis/bot/v3/info"))
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|error| format!("failed to request bot info: {error}"))?;
    let bot_json: serde_json::Value = bot_response
        .json()
        .await
        .map_err(|error| format!("failed to decode bot info response: {error}"))?;

    Ok(parse_feishu_bot_info_response(app_id, bot_json))
}

pub async fn probe_openclaw_plugin_feishu_credentials_with_app_secret(
    app_id: &str,
    app_secret: &str,
) -> Result<OpenClawPluginFeishuCredentialProbeResult, String> {
    let client = Client::builder()
        .build()
        .map_err(|error| format!("failed to build feishu probe client: {error}"))?;
    probe_openclaw_plugin_feishu_credentials_with_client(&client, app_id, app_secret).await
}

pub fn stop_openclaw_lark_installer_session_in_state(
    state: &OpenClawLarkInstallerSessionState,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    let (process, stdin) = {
        let mut guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock installer session state".to_string())?;
        (guard.process.take(), guard.stdin.take())
    };

    drop(stdin);

    if let Some(slot) = process {
        if let Ok(mut child_guard) = slot.lock() {
            if let Some(mut child) = child_guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    let mut guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock installer session state".to_string())?;
    guard.status.running = false;
    if guard.status.started_at.is_some() {
        guard.status.last_output_at = Some(now_rfc3339());
    }
    guard.status.prompt_hint = None;
    Ok(guard.status.clone())
}

pub async fn start_openclaw_lark_installer_session_with_pool(
    pool: &SqlitePool,
    state: &OpenClawLarkInstallerSessionState,
    mode: OpenClawLarkInstallerMode,
    app_id: Option<&str>,
    app_secret: Option<&str>,
    app: &AppHandle,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    let _ = stop_openclaw_lark_installer_session_in_state(state);

    let install = get_openclaw_plugin_install_by_id_with_pool(pool, "openclaw-lark").await?;
    let plugin_install_path = Path::new(&install.install_path);
    let bin_path = Path::new(&install.install_path)
        .join("bin")
        .join("openclaw-lark.js");
    if !bin_path.exists() {
        return Err(format!(
            "official installer binary is missing: {}",
            bin_path.display()
        ));
    }

    let shim_root = resolve_openclaw_shim_root(app)?;
    let shim_dir = ensure_openclaw_cli_shim(&shim_root)?;
    let controlled_openclaw_state_root = resolve_controlled_openclaw_state_root(app)?;
    ensure_controlled_openclaw_state_projection(&controlled_openclaw_state_root, plugin_install_path)?;

    let mut command = Command::new("node");
    command
        .current_dir(plugin_install_path)
        .arg(&bin_path)
        .arg("install")
        .arg("--debug")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console_window(&mut command);
    prepend_env_path(&mut command, &shim_dir);
    command
        .env(
            "WORKCLAW_OPENCLAW_SHIM_STATE_FILE",
            build_openclaw_shim_state_file_path(&shim_dir),
        )
        .env("WORKCLAW_OPENCLAW_SHIM_VERSION", OPENCLAW_SHIM_VERSION)
        .env("OPENCLAW_STATE_DIR", &controlled_openclaw_state_root);

    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to launch official installer: {e}"))?;
    let pid = child.id();
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "failed to capture official installer stdin".to_string())?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let child_slot = Arc::new(Mutex::new(Some(child)));
    let stdin_slot = Arc::new(Mutex::new(stdin));

    {
        let mut guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock installer session state".to_string())?;
        guard.process = Some(child_slot.clone());
        guard.stdin = Some(stdin_slot.clone());
        guard.auto = OpenClawLarkInstallerAutoInputState::default();
        guard.app_id = app_id.map(str::to_string);
        guard.app_secret = app_secret.map(str::to_string);
        guard.status = OpenClawLarkInstallerSessionStatus {
            running: true,
            mode: Some(mode.clone()),
            started_at: Some(now_rfc3339()),
            last_output_at: None,
            last_error: None,
            prompt_hint: Some("正在启动飞书官方安装向导".to_string()),
            recent_output: vec![format!(
                "[system] official installer started (pid={pid}, mode={})",
                match mode {
                    OpenClawLarkInstallerMode::Create => "create",
                    OpenClawLarkInstallerMode::Link => "link",
                }
            )],
        };
    }

    if let Some(stdout) = stdout {
        let state_clone = state.clone();
        let stdin_clone = stdin_slot.clone();
        let mode_clone = mode.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim_end();
                if trimmed.trim().is_empty() {
                    continue;
                }
                let auto_input = {
                    let mut maybe_auto_input = None;
                    if let Ok(mut guard) = state_clone.0.lock() {
                        push_installer_output(&mut guard.status, trimmed);
                        guard.status.prompt_hint = infer_installer_prompt_hint(trimmed);
                        let app_id = guard.app_id.clone();
                        let app_secret = guard.app_secret.clone();
                        maybe_auto_input = derive_installer_auto_input(
                            &mode_clone,
                            app_id.as_deref(),
                            app_secret.as_deref(),
                            trimmed,
                            &mut guard.auto,
                        );
                        if let Some(ref payload) = maybe_auto_input {
                            let display = payload
                                .replace('\r', "\\r")
                                .replace('\n', "\\n")
                                .replace('\u{1b}', "\\u001b");
                            push_installer_output(
                                &mut guard.status,
                                &format!("[auto-input] {display}"),
                            );
                        }
                    }
                    maybe_auto_input
                };

                if let Some(payload) = auto_input {
                    if let Ok(mut stdin_guard) = stdin_clone.lock() {
                        let _ = stdin_guard.write_all(payload.as_bytes());
                        let _ = stdin_guard.flush();
                    }
                }
            }
        });
    }

    if let Some(stderr) = stderr {
        let state_clone = state.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim_end();
                if trimmed.trim().is_empty() {
                    continue;
                }
                eprintln!("[openclaw-lark-installer] {}", trimmed);
                if let Ok(mut guard) = state_clone.0.lock() {
                    guard.status.last_error = Some(trimmed.to_string());
                    push_installer_output(&mut guard.status, &format!("[stderr] {trimmed}"));
                }
            }
        });
    }

    {
        let state_clone = state.clone();
        let child_slot_clone = child_slot.clone();
        thread::spawn(move || loop {
            let exit_status = {
                let mut child_guard = match child_slot_clone.lock() {
                    Ok(guard) => guard,
                    Err(_) => break,
                };
                if let Some(child) = child_guard.as_mut() {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            let success = status.success();
                            let code = status.code();
                            *child_guard = None;
                            Some((success, code, None::<String>))
                        }
                        Ok(None) => None,
                        Err(error) => {
                            *child_guard = None;
                            Some((false, Some(-1), Some(error.to_string())))
                        }
                    }
                } else {
                    break;
                }
            };

            match exit_status {
                Some((success, code, command_error)) => {
                    if let Ok(mut guard) = state_clone.0.lock() {
                        guard.process = None;
                        guard.stdin = None;
                        guard.status.running = false;
                        guard.status.prompt_hint = None;
                        let final_line = if success {
                            "[system] official installer finished".to_string()
                        } else if let Some(error) = command_error {
                            guard.status.last_error =
                                Some(format!("official installer wait failed: {error}"));
                            format!("[system] official installer failed: {error}")
                        } else {
                            let message = match code {
                                Some(value) if value >= 0 => {
                                    format!("official installer exited with code {value}")
                                }
                                _ => "official installer exited unexpectedly".to_string(),
                            };
                            if guard
                                .status
                                .last_error
                                .as_deref()
                                .unwrap_or("")
                                .trim()
                                .is_empty()
                            {
                                guard.status.last_error = Some(message.clone());
                            }
                            format!("[system] {message}")
                        };
                        push_installer_output(&mut guard.status, &final_line);
                    }
                    break;
                }
                None => thread::sleep(Duration::from_millis(250)),
            }
        });
    }

    Ok(current_openclaw_lark_installer_session_status(state))
}

pub fn send_openclaw_lark_installer_input_in_state(
    state: &OpenClawLarkInstallerSessionState,
    input: &str,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    let payload = if input.ends_with('\n') || input.ends_with('\r') {
        input.to_string()
    } else {
        format!("{input}\r")
    };

    let stdin = {
        let guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock installer session state".to_string())?;
        guard
            .stdin
            .clone()
            .ok_or_else(|| "official installer is not accepting input".to_string())?
    };

    {
        let mut stdin_guard = stdin
            .lock()
            .map_err(|_| "failed to lock installer stdin".to_string())?;
        stdin_guard
            .write_all(payload.as_bytes())
            .map_err(|e| format!("failed to send installer input: {e}"))?;
        stdin_guard
            .flush()
            .map_err(|e| format!("failed to flush installer input: {e}"))?;
    }

    let mut guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock installer session state".to_string())?;
    push_installer_output(
        &mut guard.status,
        &format!("[manual-input] {}", input.trim_end()),
    );
    Ok(guard.status.clone())
}

#[tauri::command]
pub async fn start_openclaw_plugin_feishu_runtime(
    plugin_id: String,
    account_id: Option<String>,
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    runtime: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<OpenClawPluginFeishuRuntimeStatus, String> {
    start_openclaw_plugin_feishu_runtime_with_pool(
        &db.0,
        runtime.inner(),
        &plugin_id,
        account_id.as_deref(),
        Some(app),
    )
    .await
}

#[tauri::command]
pub async fn stop_openclaw_plugin_feishu_runtime(
    runtime: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<OpenClawPluginFeishuRuntimeStatus, String> {
    stop_openclaw_plugin_feishu_runtime_in_state(runtime.inner())
}

#[tauri::command]
pub async fn get_openclaw_plugin_feishu_runtime_status(
    runtime: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<OpenClawPluginFeishuRuntimeStatus, String> {
    Ok(current_feishu_runtime_status(runtime.inner()))
}

#[tauri::command]
pub async fn get_feishu_plugin_environment_status() -> Result<FeishuPluginEnvironmentStatus, String> {
    Ok(get_feishu_plugin_environment_status_internal())
}

#[tauri::command]
pub async fn get_feishu_setup_progress(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    runtime: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<FeishuSetupProgress, String> {
    if let Ok(shim_root) = resolve_openclaw_shim_root(&app) {
        let _ = sync_feishu_gateway_credentials_from_shim_with_pool(&db.0, &shim_root).await;
    }
    if let Ok(state_root) = resolve_controlled_openclaw_state_root(&app) {
        let _ = sync_feishu_gateway_credentials_from_openclaw_state_with_pool(&db.0, &state_root).await;
    }
    get_feishu_setup_progress_with_pool(&db.0, runtime.inner()).await
}

pub async fn get_openclaw_plugin_feishu_advanced_settings_with_pool(
    pool: &SqlitePool,
) -> Result<OpenClawPluginFeishuAdvancedSettings, String> {
    Ok(OpenClawPluginFeishuAdvancedSettings {
        groups_json: get_app_setting(pool, "feishu_groups").await?.unwrap_or_default(),
        dms_json: get_app_setting(pool, "feishu_dms").await?.unwrap_or_default(),
        footer_json: get_app_setting(pool, "feishu_footer").await?.unwrap_or_default(),
        account_overrides_json: get_app_setting(pool, "feishu_account_overrides")
            .await?
            .unwrap_or_default(),
        render_mode: app_setting_string_or_default(
            get_app_setting(pool, "feishu_render_mode").await?,
            "auto",
        ),
        streaming: app_setting_string_or_default(
            get_app_setting(pool, "feishu_streaming").await?,
            "false",
        ),
        text_chunk_limit: app_setting_string_or_default(
            get_app_setting(pool, "feishu_text_chunk_limit").await?,
            "4000",
        ),
        chunk_mode: app_setting_string_or_default(
            get_app_setting(pool, "feishu_chunk_mode").await?,
            "length",
        ),
        reply_in_thread: app_setting_string_or_default(
            get_app_setting(pool, "feishu_reply_in_thread").await?,
            "disabled",
        ),
        group_session_scope: app_setting_string_or_default(
            get_app_setting(pool, "feishu_group_session_scope").await?,
            "group",
        ),
        topic_session_mode: app_setting_string_or_default(
            get_app_setting(pool, "feishu_topic_session_mode").await?,
            "disabled",
        ),
        markdown_mode: app_setting_string_or_default(
            get_app_setting(pool, "feishu_markdown_mode").await?,
            "native",
        ),
        markdown_table_mode: app_setting_string_or_default(
            get_app_setting(pool, "feishu_markdown_table_mode").await?,
            "native",
        ),
        heartbeat_visibility: app_setting_string_or_default(
            get_app_setting(pool, "feishu_heartbeat_visibility").await?,
            "visible",
        ),
        heartbeat_interval_ms: app_setting_string_or_default(
            get_app_setting(pool, "feishu_heartbeat_interval_ms").await?,
            "30000",
        ),
        media_max_mb: app_setting_string_or_default(
            get_app_setting(pool, "feishu_media_max_mb").await?,
            "20",
        ),
        http_timeout_ms: app_setting_string_or_default(
            get_app_setting(pool, "feishu_http_timeout_ms").await?,
            "60000",
        ),
        config_writes: app_setting_string_or_default(
            get_app_setting(pool, "feishu_config_writes").await?,
            "false",
        ),
        webhook_host: get_app_setting(pool, "feishu_webhook_host")
            .await?
            .unwrap_or_default(),
        webhook_port: get_app_setting(pool, "feishu_webhook_port")
            .await?
            .unwrap_or_default(),
        dynamic_agent_creation_enabled: get_app_setting(
            pool,
            "feishu_dynamic_agent_creation_enabled",
        )
        .await?
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .unwrap_or_else(|| "false".to_string()),
        dynamic_agent_creation_workspace_template: get_app_setting(
            pool,
            "feishu_dynamic_agent_creation_workspace_template",
        )
        .await?
        .unwrap_or_default(),
        dynamic_agent_creation_agent_dir_template: get_app_setting(
            pool,
            "feishu_dynamic_agent_creation_agent_dir_template",
        )
        .await?
        .unwrap_or_default(),
        dynamic_agent_creation_max_agents: get_app_setting(
            pool,
            "feishu_dynamic_agent_creation_max_agents",
        )
        .await?
        .unwrap_or_default(),
    })
}

pub async fn set_openclaw_plugin_feishu_advanced_settings_with_pool(
    pool: &SqlitePool,
    settings: &OpenClawPluginFeishuAdvancedSettings,
) -> Result<OpenClawPluginFeishuAdvancedSettings, String> {
    set_app_setting(pool, "feishu_groups", settings.groups_json.trim()).await?;
    set_app_setting(pool, "feishu_dms", settings.dms_json.trim()).await?;
    set_app_setting(pool, "feishu_footer", settings.footer_json.trim()).await?;
    set_app_setting(
        pool,
        "feishu_account_overrides",
        settings.account_overrides_json.trim(),
    )
    .await?;
    set_app_setting(pool, "feishu_render_mode", settings.render_mode.trim()).await?;
    set_app_setting(pool, "feishu_streaming", settings.streaming.trim()).await?;
    set_app_setting(
        pool,
        "feishu_text_chunk_limit",
        settings.text_chunk_limit.trim(),
    )
    .await?;
    set_app_setting(pool, "feishu_chunk_mode", settings.chunk_mode.trim()).await?;
    set_app_setting(
        pool,
        "feishu_reply_in_thread",
        settings.reply_in_thread.trim(),
    )
    .await?;
    set_app_setting(
        pool,
        "feishu_group_session_scope",
        settings.group_session_scope.trim(),
    )
    .await?;
    set_app_setting(
        pool,
        "feishu_topic_session_mode",
        settings.topic_session_mode.trim(),
    )
    .await?;
    set_app_setting(pool, "feishu_markdown_mode", settings.markdown_mode.trim()).await?;
    set_app_setting(
        pool,
        "feishu_markdown_table_mode",
        settings.markdown_table_mode.trim(),
    )
    .await?;
    set_app_setting(
        pool,
        "feishu_heartbeat_visibility",
        settings.heartbeat_visibility.trim(),
    )
    .await?;
    set_app_setting(
        pool,
        "feishu_heartbeat_interval_ms",
        settings.heartbeat_interval_ms.trim(),
    )
    .await?;
    set_app_setting(pool, "feishu_media_max_mb", settings.media_max_mb.trim()).await?;
    set_app_setting(pool, "feishu_http_timeout_ms", settings.http_timeout_ms.trim()).await?;
    set_app_setting(pool, "feishu_config_writes", settings.config_writes.trim()).await?;
    set_app_setting(pool, "feishu_webhook_host", settings.webhook_host.trim()).await?;
    set_app_setting(pool, "feishu_webhook_port", settings.webhook_port.trim()).await?;
    set_app_setting(
        pool,
        "feishu_dynamic_agent_creation_enabled",
        settings.dynamic_agent_creation_enabled.trim(),
    )
    .await?;
    set_app_setting(
        pool,
        "feishu_dynamic_agent_creation_workspace_template",
        settings.dynamic_agent_creation_workspace_template.trim(),
    )
    .await?;
    set_app_setting(
        pool,
        "feishu_dynamic_agent_creation_agent_dir_template",
        settings.dynamic_agent_creation_agent_dir_template.trim(),
    )
    .await?;
    set_app_setting(
        pool,
        "feishu_dynamic_agent_creation_max_agents",
        settings.dynamic_agent_creation_max_agents.trim(),
    )
    .await?;
    get_openclaw_plugin_feishu_advanced_settings_with_pool(pool).await
}

#[tauri::command]
pub async fn get_openclaw_plugin_feishu_advanced_settings(
    db: State<'_, DbState>,
) -> Result<OpenClawPluginFeishuAdvancedSettings, String> {
    get_openclaw_plugin_feishu_advanced_settings_with_pool(&db.0).await
}

#[tauri::command]
pub async fn set_openclaw_plugin_feishu_advanced_settings(
    settings: OpenClawPluginFeishuAdvancedSettings,
    db: State<'_, DbState>,
) -> Result<OpenClawPluginFeishuAdvancedSettings, String> {
    set_openclaw_plugin_feishu_advanced_settings_with_pool(&db.0, &settings).await
}

#[tauri::command]
pub async fn start_openclaw_lark_installer_session(
    mode: OpenClawLarkInstallerMode,
    app_id: Option<String>,
    app_secret: Option<String>,
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    installer: State<'_, OpenClawLarkInstallerSessionState>,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    start_openclaw_lark_installer_session_with_pool(
        &db.0,
        installer.inner(),
        mode,
        app_id.as_deref(),
        app_secret.as_deref(),
        &app,
    )
    .await
}

#[tauri::command]
pub async fn get_openclaw_lark_installer_session_status(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    installer: State<'_, OpenClawLarkInstallerSessionState>,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    if let Ok(shim_root) = resolve_openclaw_shim_root(&app) {
        let _ = sync_feishu_gateway_credentials_from_shim_with_pool(&db.0, &shim_root).await;
    }
    if let Ok(state_root) = resolve_controlled_openclaw_state_root(&app) {
        let _ = sync_feishu_gateway_credentials_from_openclaw_state_with_pool(&db.0, &state_root).await;
    }
    Ok(current_openclaw_lark_installer_session_status(
        installer.inner(),
    ))
}

#[tauri::command]
pub async fn send_openclaw_lark_installer_input(
    input: String,
    installer: State<'_, OpenClawLarkInstallerSessionState>,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    send_openclaw_lark_installer_input_in_state(installer.inner(), &input)
}

#[tauri::command]
pub async fn stop_openclaw_lark_installer_session(
    installer: State<'_, OpenClawLarkInstallerSessionState>,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    stop_openclaw_lark_installer_session_in_state(installer.inner())
}

#[tauri::command]
pub async fn probe_openclaw_plugin_feishu_credentials(
    app_id: String,
    app_secret: String,
) -> Result<OpenClawPluginFeishuCredentialProbeResult, String> {
    probe_openclaw_plugin_feishu_credentials_with_app_secret(&app_id, &app_secret).await
}

#[tauri::command]
pub async fn upsert_openclaw_plugin_install(
    input: OpenClawPluginInstallInput,
    db: State<'_, DbState>,
) -> Result<OpenClawPluginInstallRecord, String> {
    upsert_openclaw_plugin_install_with_pool(&db.0, input).await
}

#[tauri::command]
pub async fn list_openclaw_plugin_installs(
    db: State<'_, DbState>,
) -> Result<Vec<OpenClawPluginInstallRecord>, String> {
    list_openclaw_plugin_installs_with_pool(&db.0).await
}

#[tauri::command]
pub async fn delete_openclaw_plugin_install(
    plugin_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    delete_openclaw_plugin_install_with_pool(&db.0, &plugin_id).await
}

#[tauri::command]
pub async fn inspect_openclaw_plugin(
    plugin_id: String,
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<OpenClawPluginInspectionResult, String> {
    inspect_openclaw_plugin_with_pool_and_app(&db.0, &plugin_id, Some(&app)).await
}

#[tauri::command]
pub async fn list_openclaw_plugin_channel_hosts(
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<Vec<OpenClawPluginChannelHost>, String> {
    list_openclaw_plugin_channel_hosts_with_pool_and_app(&db.0, Some(&app)).await
}

#[tauri::command]
pub async fn get_openclaw_plugin_feishu_channel_snapshot(
    plugin_id: String,
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<OpenClawPluginChannelSnapshotResult, String> {
    get_openclaw_plugin_feishu_channel_snapshot_with_pool_and_app(&db.0, &plugin_id, Some(&app))
        .await
}

#[tauri::command]
pub async fn install_openclaw_plugin_from_npm(
    plugin_id: String,
    npm_spec: String,
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<OpenClawPluginInstallRecord, String> {
    let normalized_plugin_id = normalize_required(&plugin_id, "plugin_id")?;
    let normalized_npm_spec = normalize_required(&npm_spec, "npm_spec")?;
    let plugin_root = resolve_openclaw_plugin_workspace_root(&app, &normalized_plugin_id)?;
    let workspace_dir = plugin_root.join("workspace");

    if workspace_dir.exists() {
        fs::remove_dir_all(&workspace_dir).map_err(|e| {
            format!(
                "failed to clean previous plugin workspace {}: {e}",
                workspace_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&workspace_dir).map_err(|e| {
        format!(
            "failed to create plugin workspace {}: {e}",
            workspace_dir.display()
        )
    })?;

    let workspace_package_json = serde_json::json!({
        "name": format!("workclaw-openclaw-plugin-{}", normalized_plugin_id),
        "private": true,
    })
    .to_string();
    fs::write(workspace_dir.join("package.json"), workspace_package_json)
        .map_err(|e| format!("failed to write plugin workspace package.json: {e}"))?;

    let mut command = Command::new(resolve_npm_command());
    command
        .current_dir(&workspace_dir)
        .arg("install")
        .arg("--no-save")
        .arg("--no-package-lock")
        .arg(&normalized_npm_spec);
    apply_command_search_path(&mut command, &[]);
    hide_console_window(&mut command);
    let output = command
        .output()
        .map_err(|e| format!("failed to launch npm install for official plugin: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(format!("安装飞书官方插件失败: {detail}"));
    }

    let package_dir = resolve_installed_package_dir(&workspace_dir, &normalized_npm_spec)?;
    let package_json_path = package_dir.join("package.json");
    let package_json_text = fs::read_to_string(&package_json_path)
        .map_err(|e| format!("failed to read installed plugin package.json: {e}"))?;
    let package_json: serde_json::Value = serde_json::from_str(&package_json_text)
        .map_err(|e| format!("failed to parse installed plugin package.json: {e}"))?;
    let version = package_json
        .get("version")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "installed plugin package.json is missing version".to_string())?
        .to_string();
    let manifest_json = load_plugin_manifest_json(&package_dir, &package_json);

    upsert_openclaw_plugin_install_with_pool(
        &db.0,
        OpenClawPluginInstallInput {
            plugin_id: normalized_plugin_id,
            npm_spec: normalized_npm_spec,
            version,
            install_path: package_dir.to_string_lossy().to_string(),
            source_type: "npm".to_string(),
            manifest_json,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::feishu_gateway::set_app_setting;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_memory_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        sqlx::query(
            "CREATE TABLE installed_openclaw_plugins (
                plugin_id TEXT PRIMARY KEY,
                npm_spec TEXT NOT NULL,
                version TEXT NOT NULL,
                install_path TEXT NOT NULL,
                source_type TEXT NOT NULL DEFAULT 'npm',
                manifest_json TEXT NOT NULL DEFAULT '{}',
                installed_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create installed_openclaw_plugins table");

        sqlx::query(
            "CREATE TABLE installed_skills (
                id TEXT PRIMARY KEY,
                manifest TEXT NOT NULL,
                installed_at TEXT NOT NULL,
                username TEXT NOT NULL,
                pack_path TEXT NOT NULL DEFAULT '',
                source_type TEXT NOT NULL DEFAULT 'encrypted'
            )",
        )
        .execute(&pool)
        .await
        .expect("create installed_skills table");

        sqlx::query(
            "CREATE TABLE app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create app_settings table");

        sqlx::query(
            "CREATE TABLE agent_employees (
                id TEXT PRIMARY KEY,
                employee_id TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL DEFAULT '',
                role_id TEXT NOT NULL DEFAULT '',
                persona TEXT NOT NULL DEFAULT '',
                feishu_open_id TEXT NOT NULL DEFAULT '',
                feishu_app_id TEXT NOT NULL DEFAULT '',
                feishu_app_secret TEXT NOT NULL DEFAULT '',
                primary_skill_id TEXT NOT NULL DEFAULT '',
                default_work_dir TEXT NOT NULL DEFAULT '',
                openclaw_agent_id TEXT NOT NULL DEFAULT '',
                routing_priority INTEGER NOT NULL DEFAULT 100,
                enabled_scopes_json TEXT NOT NULL DEFAULT '[\"app\"]',
                enabled INTEGER NOT NULL DEFAULT 1,
                is_default INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .expect("create agent_employees table");

        sqlx::query(
            "CREATE TABLE agent_employee_skills (
                employee_id TEXT NOT NULL,
                skill_id TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&pool)
        .await
        .expect("create agent_employee_skills table");

        sqlx::query(
            "CREATE TABLE feishu_pairing_requests (
                id TEXT PRIMARY KEY,
                channel TEXT NOT NULL,
                account_id TEXT NOT NULL DEFAULT 'default',
                sender_id TEXT NOT NULL,
                chat_id TEXT NOT NULL DEFAULT '',
                code TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                resolved_at TEXT,
                resolved_by_user TEXT
            )",
        )
        .execute(&pool)
        .await
        .expect("create feishu_pairing_requests table");

        sqlx::query(
            "CREATE TABLE feishu_pairing_allow_from (
                channel TEXT NOT NULL DEFAULT 'feishu',
                account_id TEXT NOT NULL DEFAULT 'default',
                sender_id TEXT NOT NULL,
                source_request_id TEXT NOT NULL DEFAULT '',
                approved_at TEXT NOT NULL,
                approved_by_user TEXT NOT NULL DEFAULT '',
                PRIMARY KEY(channel, account_id, sender_id)
            )",
        )
        .execute(&pool)
        .await
        .expect("create feishu_pairing_allow_from");

        sqlx::query(
            "CREATE TABLE im_routing_bindings (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                account_id TEXT NOT NULL DEFAULT '',
                peer_kind TEXT NOT NULL DEFAULT '',
                peer_id TEXT NOT NULL DEFAULT '',
                guild_id TEXT NOT NULL DEFAULT '',
                team_id TEXT NOT NULL DEFAULT '',
                role_ids_json TEXT NOT NULL DEFAULT '[]',
                connector_meta_json TEXT NOT NULL DEFAULT '{}',
                priority INTEGER NOT NULL DEFAULT 100,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .expect("create im_routing_bindings");

        pool
    }

    #[tokio::test]
    async fn feishu_advanced_settings_round_trip_through_app_settings() {
        let pool = setup_memory_pool().await;
        let saved = set_openclaw_plugin_feishu_advanced_settings_with_pool(
            &pool,
            &OpenClawPluginFeishuAdvancedSettings {
                groups_json: "{\"oc_demo\":{\"enabled\":true}}".to_string(),
                dms_json: "{\"ou_demo\":{\"enabled\":true}}".to_string(),
                footer_json: "{\"status\":true}".to_string(),
                account_overrides_json: "{\"default\":{\"renderMode\":\"card\"}}".to_string(),
                render_mode: "card".to_string(),
                streaming: "true".to_string(),
                text_chunk_limit: "2400".to_string(),
                chunk_mode: "newline".to_string(),
                reply_in_thread: "enabled".to_string(),
                group_session_scope: "group_sender".to_string(),
                topic_session_mode: "enabled".to_string(),
                markdown_mode: "native".to_string(),
                markdown_table_mode: "native".to_string(),
                heartbeat_visibility: "visible".to_string(),
                heartbeat_interval_ms: "30000".to_string(),
                media_max_mb: "20".to_string(),
                http_timeout_ms: "60000".to_string(),
                config_writes: "true".to_string(),
                webhook_host: "127.0.0.1".to_string(),
                webhook_port: "8787".to_string(),
                dynamic_agent_creation_enabled: "true".to_string(),
                dynamic_agent_creation_workspace_template: "workspace/{sender_id}".to_string(),
                dynamic_agent_creation_agent_dir_template: "agents/{sender_id}".to_string(),
                dynamic_agent_creation_max_agents: "48".to_string(),
            },
        )
        .await
        .expect("save advanced settings");

        assert_eq!(
            saved,
            OpenClawPluginFeishuAdvancedSettings {
                groups_json: "{\"oc_demo\":{\"enabled\":true}}".to_string(),
                dms_json: "{\"ou_demo\":{\"enabled\":true}}".to_string(),
                footer_json: "{\"status\":true}".to_string(),
                account_overrides_json: "{\"default\":{\"renderMode\":\"card\"}}".to_string(),
                render_mode: "card".to_string(),
                streaming: "true".to_string(),
                text_chunk_limit: "2400".to_string(),
                chunk_mode: "newline".to_string(),
                reply_in_thread: "enabled".to_string(),
                group_session_scope: "group_sender".to_string(),
                topic_session_mode: "enabled".to_string(),
                markdown_mode: "native".to_string(),
                markdown_table_mode: "native".to_string(),
                heartbeat_visibility: "visible".to_string(),
                heartbeat_interval_ms: "30000".to_string(),
                media_max_mb: "20".to_string(),
                http_timeout_ms: "60000".to_string(),
                config_writes: "true".to_string(),
                webhook_host: "127.0.0.1".to_string(),
                webhook_port: "8787".to_string(),
                dynamic_agent_creation_enabled: "true".to_string(),
                dynamic_agent_creation_workspace_template: "workspace/{sender_id}".to_string(),
                dynamic_agent_creation_agent_dir_template: "agents/{sender_id}".to_string(),
                dynamic_agent_creation_max_agents: "48".to_string(),
            }
        );

        let loaded = get_openclaw_plugin_feishu_advanced_settings_with_pool(&pool)
            .await
            .expect("load advanced settings");
        assert_eq!(loaded, saved);
    }

    #[tokio::test]
    async fn feishu_advanced_settings_returns_projection_defaults_when_unset() {
        let pool = setup_memory_pool().await;

        let loaded = get_openclaw_plugin_feishu_advanced_settings_with_pool(&pool)
            .await
            .expect("load defaults");

        assert_eq!(loaded.render_mode, "auto");
        assert_eq!(loaded.streaming, "false");
        assert_eq!(loaded.text_chunk_limit, "4000");
        assert_eq!(loaded.chunk_mode, "length");
        assert_eq!(loaded.reply_in_thread, "disabled");
        assert_eq!(loaded.group_session_scope, "group");
        assert_eq!(loaded.topic_session_mode, "disabled");
        assert_eq!(loaded.markdown_mode, "native");
        assert_eq!(loaded.markdown_table_mode, "native");
        assert_eq!(loaded.heartbeat_visibility, "visible");
        assert_eq!(loaded.heartbeat_interval_ms, "30000");
        assert_eq!(loaded.media_max_mb, "20");
        assert_eq!(loaded.http_timeout_ms, "60000");
        assert_eq!(loaded.config_writes, "false");
        assert_eq!(loaded.dynamic_agent_creation_enabled, "false");
    }

    #[tokio::test]
    async fn feishu_advanced_settings_treats_blank_rows_as_unset_defaults() {
        let pool = setup_memory_pool().await;
        for key in [
            "feishu_markdown_mode",
            "feishu_markdown_table_mode",
            "feishu_heartbeat_visibility",
            "feishu_heartbeat_interval_ms",
            "feishu_media_max_mb",
            "feishu_http_timeout_ms",
            "feishu_config_writes",
            "feishu_dynamic_agent_creation_enabled",
        ] {
            set_app_setting(&pool, key, "").await.expect("set blank app setting");
        }

        let loaded = get_openclaw_plugin_feishu_advanced_settings_with_pool(&pool)
            .await
            .expect("load defaults from blank rows");

        assert_eq!(loaded.markdown_mode, "native");
        assert_eq!(loaded.markdown_table_mode, "native");
        assert_eq!(loaded.heartbeat_visibility, "visible");
        assert_eq!(loaded.heartbeat_interval_ms, "30000");
        assert_eq!(loaded.media_max_mb, "20");
        assert_eq!(loaded.http_timeout_ms, "60000");
        assert_eq!(loaded.config_writes, "false");
        assert_eq!(loaded.dynamic_agent_creation_enabled, "false");
    }

    #[tokio::test]
    async fn build_feishu_openclaw_config_projects_official_defaults() {
        let pool = setup_memory_pool().await;
        set_app_setting(&pool, "feishu_app_id", "cli_root")
            .await
            .expect("set app id");
        set_app_setting(&pool, "feishu_app_secret", "secret_root")
            .await
            .expect("set app secret");
        set_app_setting(&pool, "feishu_history_limit", "36")
            .await
            .expect("set history limit");
        set_app_setting(&pool, "feishu_dm_history_limit", "10")
            .await
            .expect("set dm history limit");
        set_app_setting(&pool, "feishu_media_max_mb", "20")
            .await
            .expect("set media max mb");
        set_app_setting(&pool, "feishu_http_timeout_ms", "60000")
            .await
            .expect("set http timeout");
        set_app_setting(&pool, "feishu_block_streaming_coalesce_enabled", "true")
            .await
            .expect("set block coalesce enabled");
        set_app_setting(&pool, "feishu_block_streaming_coalesce_min_delay_ms", "100")
            .await
            .expect("set block coalesce min delay");
        set_app_setting(&pool, "feishu_block_streaming_coalesce_max_delay_ms", "300")
            .await
            .expect("set block coalesce max delay");
        set_app_setting(&pool, "feishu_heartbeat_visibility", "visible")
            .await
            .expect("set heartbeat visibility");
        set_app_setting(&pool, "feishu_heartbeat_interval_ms", "30000")
            .await
            .expect("set heartbeat interval");
        set_app_setting(&pool, "feishu_dynamic_agent_creation_enabled", "true")
            .await
            .expect("set dynamic agent creation enabled");
        set_app_setting(
            &pool,
            "feishu_dynamic_agent_creation_workspace_template",
            "workspace/{sender_id}",
        )
        .await
        .expect("set dynamic workspace template");
        set_app_setting(
            &pool,
            "feishu_dynamic_agent_creation_agent_dir_template",
            "agents/{sender_id}",
        )
        .await
        .expect("set dynamic agent dir template");
        set_app_setting(&pool, "feishu_dynamic_agent_creation_max_agents", "48")
            .await
            .expect("set dynamic max agents");
        set_app_setting(
            &pool,
            "feishu_dms",
            "{\"user:carla\":{\"enabled\":true,\"systemPrompt\":\"优先处理私聊任务\"}}",
        )
        .await
        .expect("set dms");
        set_app_setting(
            &pool,
            "feishu_footer",
            "{\"status\":true,\"elapsed\":true}",
        )
        .await
        .expect("set footer");
        set_app_setting(
            &pool,
            "feishu_groups",
            "{\"oc_demo\":{\"enabled\":true,\"requireMention\":false,\"systemPrompt\":\"只处理 demo 群\",\"tools\":{\"allow\":[\"search_web\"]}}}",
        )
        .await
        .expect("set specific groups");

        let config = build_feishu_openclaw_config_with_pool(&pool)
            .await
            .expect("build feishu openclaw config");
        let feishu = &config["channels"]["feishu"];

        assert_eq!(feishu["enabled"], serde_json::json!(true));
        assert_eq!(feishu["defaultAccount"], serde_json::json!("default"));
        assert_eq!(feishu["appId"], serde_json::json!("cli_root"));
        assert_eq!(feishu["appSecret"], serde_json::json!("secret_root"));
        assert_eq!(feishu["domain"], serde_json::json!("feishu"));
        assert_eq!(feishu["connectionMode"], serde_json::json!("websocket"));
        assert_eq!(feishu["webhookPath"], serde_json::json!("/feishu/events"));
        assert_eq!(feishu["dmPolicy"], serde_json::json!("pairing"));
        assert_eq!(feishu["groupPolicy"], serde_json::json!("allowlist"));
        assert_eq!(feishu["requireMention"], serde_json::json!(true));
        assert_eq!(feishu["reactionNotifications"], serde_json::json!("own"));
        assert_eq!(feishu["typingIndicator"], serde_json::json!(true));
        assert_eq!(feishu["resolveSenderNames"], serde_json::json!(true));
        assert_eq!(feishu["streaming"], serde_json::json!(false));
        assert_eq!(feishu["replyInThread"], serde_json::json!("disabled"));
        assert_eq!(feishu["groupSessionScope"], serde_json::json!("group"));
        assert_eq!(feishu["topicSessionMode"], serde_json::json!("disabled"));
        assert_eq!(feishu["groupAllowFrom"], serde_json::json!([]));
        assert_eq!(feishu["groupSenderAllowFrom"], serde_json::json!([]));
        assert_eq!(
            feishu["groups"]["*"],
            serde_json::json!({
                "enabled": true,
                "requireMention": true,
                "groupSessionScope": "group",
                "topicSessionMode": "disabled",
                "replyInThread": "disabled"
            })
        );
        assert_eq!(
            feishu["groups"]["oc_demo"],
            serde_json::json!({
                "enabled": true,
                "requireMention": false,
                "systemPrompt": "只处理 demo 群",
                "tools": {
                    "allow": ["search_web"]
                }
            })
        );
        assert_eq!(feishu["configWrites"], serde_json::json!(false));
        assert_eq!(feishu["webhookHost"], serde_json::json!(""));
        assert_eq!(feishu["webhookPort"], serde_json::Value::Null);
        assert_eq!(feishu["markdown"], serde_json::json!({}));
        assert_eq!(feishu["renderMode"], serde_json::json!("auto"));
        assert_eq!(feishu["textChunkLimit"], serde_json::json!(4000));
        assert_eq!(feishu["chunkMode"], serde_json::json!("length"));
        assert_eq!(
            feishu["blockStreamingCoalesce"],
            serde_json::json!({
                "enabled": true,
                "minDelayMs": 100,
                "maxDelayMs": 300
            })
        );
        assert_eq!(feishu["historyLimit"], serde_json::json!(36));
        assert_eq!(feishu["dmHistoryLimit"], serde_json::json!(10));
        assert_eq!(feishu["mediaMaxMb"], serde_json::json!(20));
        assert_eq!(feishu["httpTimeoutMs"], serde_json::json!(60000));
        assert_eq!(
            feishu["heartbeat"],
            serde_json::json!({
                "visibility": "visible",
                "intervalMs": 30000
            })
        );
        assert_eq!(
            feishu["dynamicAgentCreation"],
            serde_json::json!({
                "enabled": true,
                "workspaceTemplate": "workspace/{sender_id}",
                "agentDirTemplate": "agents/{sender_id}",
                "maxAgents": 48
            })
        );
        assert_eq!(
            feishu["dms"],
            serde_json::json!({
                "user:carla": {
                    "enabled": true,
                    "systemPrompt": "优先处理私聊任务"
                }
            })
        );
        assert_eq!(
            feishu["footer"],
            serde_json::json!({
                "status": true,
                "elapsed": true
            })
        );
        assert_eq!(feishu["actions"], serde_json::json!({ "reactions": false }));
        assert_eq!(
            feishu["tools"],
            serde_json::json!({
                "doc": true,
                "chat": true,
                "wiki": true,
                "drive": true,
                "perm": false,
                "scopes": true
            })
        );
        assert_eq!(feishu["allowFrom"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn build_feishu_openclaw_config_projects_employee_accounts_with_inherited_defaults() {
        let pool = setup_memory_pool().await;
        set_app_setting(&pool, "feishu_app_id", "cli_root")
            .await
            .expect("set app id");
        set_app_setting(&pool, "feishu_app_secret", "secret_root")
            .await
            .expect("set app secret");
        set_app_setting(&pool, "feishu_ingress_token", "verify_root")
            .await
            .expect("set verification token");
        set_app_setting(&pool, "feishu_encrypt_key", "encrypt_root")
            .await
            .expect("set encrypt key");
        set_app_setting(&pool, "feishu_streaming", "true")
            .await
            .expect("set streaming");
        set_app_setting(&pool, "feishu_reply_in_thread", "enabled")
            .await
            .expect("set reply in thread");
        set_app_setting(&pool, "feishu_group_session_scope", "group_sender")
            .await
            .expect("set group session scope");
        set_app_setting(&pool, "feishu_topic_session_mode", "enabled")
            .await
            .expect("set topic session mode");
        set_app_setting(&pool, "feishu_group_allow_from", "[\"ou_group_owner\"]")
            .await
            .expect("set group allow from");
        set_app_setting(
            &pool,
            "feishu_group_sender_allow_from",
            "ou_sender_a,ou_sender_b",
        )
        .await
        .expect("set group sender allow from");
        set_app_setting(&pool, "feishu_webhook_host", "127.0.0.1")
            .await
            .expect("set webhook host");
        set_app_setting(&pool, "feishu_webhook_port", "8787")
            .await
            .expect("set webhook port");
        set_app_setting(&pool, "feishu_config_writes", "true")
            .await
            .expect("set config writes");
        set_app_setting(&pool, "feishu_actions_reactions", "true")
            .await
            .expect("set actions reactions");
        set_app_setting(&pool, "feishu_render_mode", "card")
            .await
            .expect("set render mode");
        set_app_setting(&pool, "feishu_text_chunk_limit", "3200")
            .await
            .expect("set text chunk limit");
        set_app_setting(&pool, "feishu_chunk_mode", "newline")
            .await
            .expect("set chunk mode");
        set_app_setting(&pool, "feishu_markdown_mode", "native")
            .await
            .expect("set markdown mode");
        set_app_setting(&pool, "feishu_markdown_table_mode", "native")
            .await
            .expect("set markdown table mode");
        set_app_setting(&pool, "feishu_history_limit", "40")
            .await
            .expect("set history limit");
        set_app_setting(&pool, "feishu_dm_history_limit", "12")
            .await
            .expect("set dm history limit");
        set_app_setting(&pool, "feishu_media_max_mb", "25")
            .await
            .expect("set media max mb");
        set_app_setting(&pool, "feishu_http_timeout_ms", "45000")
            .await
            .expect("set http timeout ms");
        set_app_setting(&pool, "feishu_block_streaming_coalesce_enabled", "true")
            .await
            .expect("set block coalesce enabled");
        set_app_setting(&pool, "feishu_block_streaming_coalesce_min_delay_ms", "80")
            .await
            .expect("set block coalesce min delay");
        set_app_setting(&pool, "feishu_block_streaming_coalesce_max_delay_ms", "240")
            .await
            .expect("set block coalesce max delay");
        set_app_setting(&pool, "feishu_heartbeat_visibility", "hidden")
            .await
            .expect("set heartbeat visibility");
        set_app_setting(&pool, "feishu_heartbeat_interval_ms", "15000")
            .await
            .expect("set heartbeat interval ms");
        set_app_setting(&pool, "feishu_dynamic_agent_creation_enabled", "true")
            .await
            .expect("set dynamic agent creation enabled");
        set_app_setting(
            &pool,
            "feishu_dynamic_agent_creation_workspace_template",
            "employees/{sender_id}",
        )
        .await
        .expect("set dynamic workspace template");
        set_app_setting(
            &pool,
            "feishu_dynamic_agent_creation_agent_dir_template",
            "agents/{sender_id}",
        )
        .await
        .expect("set dynamic agent dir template");
        set_app_setting(&pool, "feishu_dynamic_agent_creation_max_agents", "24")
            .await
            .expect("set dynamic max agents");
        set_app_setting(
            &pool,
            "feishu_dms",
            "{\"ou_dm_vip\":{\"enabled\":true,\"systemPrompt\":\"仅处理 VIP 私聊\"}}",
        )
        .await
        .expect("set dms");
        set_app_setting(
            &pool,
            "feishu_footer",
            "{\"status\":true,\"elapsed\":false}",
        )
        .await
        .expect("set footer");
        set_app_setting(
            &pool,
            "feishu_groups",
            "{\"oc_ops\":{\"enabled\":true,\"requireMention\":true,\"skills\":[\"ops\"],\"replyInThread\":\"enabled\"}}",
        )
        .await
        .expect("set specific groups");
        set_app_setting(
            &pool,
            "feishu_account_overrides",
            "{\"taizi\":{\"enabled\":false,\"verificationToken\":\"verify_override\",\"renderMode\":\"raw\",\"footer\":{\"status\":false,\"elapsed\":true},\"groups\":{\"oc_ops\":{\"requireMention\":false}}}}",
        )
        .await
        .expect("set account overrides");
        set_app_setting(
            &pool,
            "feishu_group_default_allow_from",
            "[\"ou_group_only\"]",
        )
        .await
        .expect("set group default allowFrom");
        set_app_setting(
            &pool,
            "feishu_group_default_skills",
            "[\"briefing\", \"planner\"]",
        )
        .await
        .expect("set group default skills");
        set_app_setting(
            &pool,
            "feishu_group_default_system_prompt",
            "只处理群内任务分发",
        )
        .await
        .expect("set group default system prompt");
        set_app_setting(
            &pool,
            "feishu_group_default_tools",
            "{\"allow\":[\"read_file\",\"search_web\"]}",
        )
        .await
        .expect("set group default tools");

        sqlx::query(
            "INSERT INTO agent_employees (
                id, employee_id, name, role_id, feishu_app_id, feishu_app_secret, enabled, is_default, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, 1, 1, ?)",
        )
        .bind("emp_1")
        .bind("taizi")
        .bind("太子")
        .bind("taizi")
        .bind("cli_taizi")
        .bind("secret_taizi")
        .bind("2026-03-20T00:00:00Z")
        .execute(&pool)
        .await
        .expect("insert employee account");

        sqlx::query(
            "INSERT INTO feishu_pairing_allow_from (
                channel, account_id, sender_id, source_request_id, approved_at, approved_by_user
             ) VALUES ('feishu', ?, ?, ?, ?, ?)",
        )
        .bind("taizi")
        .bind("ou_allowed")
        .bind("req_1")
        .bind("2026-03-20T00:00:00Z")
        .bind("tester")
        .execute(&pool)
        .await
        .expect("insert approved sender");

        let config = build_feishu_openclaw_config_with_pool(&pool)
            .await
            .expect("build feishu openclaw config");
        let default_account = &config["channels"]["feishu"];
        let account = &config["channels"]["feishu"]["accounts"]["taizi"];

        assert_eq!(account["enabled"], serde_json::json!(false));
        assert_eq!(account["name"], serde_json::json!("太子"));
        assert_eq!(account["appId"], serde_json::json!("cli_taizi"));
        assert_eq!(account["appSecret"], serde_json::json!("secret_taizi"));
        assert_eq!(account["domain"], serde_json::json!("feishu"));
        assert_eq!(account["connectionMode"], serde_json::json!("websocket"));
        assert_eq!(account["webhookPath"], serde_json::json!("/feishu/events"));
        assert_eq!(
            account["verificationToken"],
            serde_json::json!("verify_override")
        );
        assert_eq!(account["encryptKey"], serde_json::json!("encrypt_root"));
        assert_eq!(account["encryptKey"], default_account["encryptKey"]);
        assert_eq!(account["dmPolicy"], default_account["dmPolicy"]);
        assert_eq!(account["groupPolicy"], default_account["groupPolicy"]);
        assert_eq!(account["dmPolicy"], serde_json::json!("pairing"));
        assert_eq!(account["groupPolicy"], serde_json::json!("allowlist"));
        assert_eq!(account["requireMention"], serde_json::json!(true));
        assert_eq!(account["reactionNotifications"], serde_json::json!("own"));
        assert_eq!(account["typingIndicator"], serde_json::json!(true));
        assert_eq!(account["resolveSenderNames"], serde_json::json!(true));
        assert_eq!(account["streaming"], serde_json::json!(true));
        assert_eq!(account["replyInThread"], serde_json::json!("enabled"));
        assert_eq!(
            account["groupSessionScope"],
            serde_json::json!("group_sender")
        );
        assert_eq!(account["topicSessionMode"], serde_json::json!("enabled"));
        assert_eq!(
            account["groupAllowFrom"],
            serde_json::json!(["ou_group_owner"])
        );
        assert_eq!(
            account["groupSenderAllowFrom"],
            serde_json::json!(["ou_sender_a", "ou_sender_b"])
        );
        assert_eq!(
            account["groups"]["*"],
            serde_json::json!({
                "enabled": true,
                "allowFrom": ["ou_group_only"],
                "requireMention": true,
                "skills": ["briefing", "planner"],
                "systemPrompt": "只处理群内任务分发",
                "tools": {
                    "allow": ["read_file", "search_web"]
                },
                "groupSessionScope": "group_sender",
                "topicSessionMode": "enabled",
                "replyInThread": "enabled"
            })
        );
        assert_eq!(
            account["groups"]["oc_ops"],
            serde_json::json!({
                "enabled": true,
                "requireMention": false,
                "skills": ["ops"],
                "replyInThread": "enabled"
            })
        );
        assert_eq!(account["configWrites"], serde_json::json!(true));
        assert_eq!(account["webhookHost"], serde_json::json!("127.0.0.1"));
        assert_eq!(account["webhookPort"], serde_json::json!(8787));
        assert_eq!(
            account["markdown"],
            serde_json::json!({
                "mode": "native",
                "tableMode": "native"
            })
        );
        assert_eq!(account["renderMode"], serde_json::json!("raw"));
        assert_eq!(account["textChunkLimit"], serde_json::json!(3200));
        assert_eq!(account["chunkMode"], serde_json::json!("newline"));
        assert_eq!(
            account["blockStreamingCoalesce"],
            serde_json::json!({
                "enabled": true,
                "minDelayMs": 80,
                "maxDelayMs": 240
            })
        );
        assert_eq!(account["historyLimit"], serde_json::json!(40));
        assert_eq!(account["dmHistoryLimit"], serde_json::json!(12));
        assert_eq!(account["mediaMaxMb"], serde_json::json!(25));
        assert_eq!(account["httpTimeoutMs"], serde_json::json!(45000));
        assert_eq!(
            account["heartbeat"],
            serde_json::json!({
                "visibility": "hidden",
                "intervalMs": 15000
            })
        );
        assert_eq!(
            account["dynamicAgentCreation"],
            serde_json::json!({
                "enabled": true,
                "workspaceTemplate": "employees/{sender_id}",
                "agentDirTemplate": "agents/{sender_id}",
                "maxAgents": 24
            })
        );
        assert_eq!(account["dms"], default_account["dms"]);
        assert_ne!(account["footer"], default_account["footer"]);
        assert_eq!(
            account["dms"],
            serde_json::json!({
                "ou_dm_vip": {
                    "enabled": true,
                    "systemPrompt": "仅处理 VIP 私聊"
                }
            })
        );
        assert_eq!(
            account["footer"],
            serde_json::json!({
                "status": false,
                "elapsed": true
            })
        );
        assert_eq!(
            account["groups"]["oc_ops"],
            serde_json::json!({
                "enabled": true,
                "requireMention": false,
                "skills": ["ops"],
                "replyInThread": "enabled"
            })
        );
        assert_eq!(account["actions"], serde_json::json!({ "reactions": true }));
        assert_eq!(
            account["tools"],
            serde_json::json!({
                "doc": true,
                "chat": true,
                "wiki": true,
                "drive": true,
                "perm": false,
                "scopes": true
            })
        );
        assert_eq!(account["allowFrom"], serde_json::json!(["ou_allowed"]));
    }

    #[test]
    fn installer_auto_input_selects_create_mode_by_default() {
        let mut auto = OpenClawLarkInstallerAutoInputState::default();
        let payload = derive_installer_auto_input(
            &OpenClawLarkInstallerMode::Create,
            None,
            None,
            "What would you like to do (请选择操作):",
            &mut auto,
        );
        assert_eq!(payload.as_deref(), Some("\r"));
        assert!(auto.selection_sent);
    }

    #[test]
    fn installer_auto_input_selects_link_mode_and_sends_credentials() {
        let mut auto = OpenClawLarkInstallerAutoInputState::default();
        let select_payload = derive_installer_auto_input(
            &OpenClawLarkInstallerMode::Link,
            Some("cli_app"),
            Some("secret"),
            "What would you like to do (请选择操作):",
            &mut auto,
        );
        assert_eq!(select_payload.as_deref(), Some("\u{1b}[B\r"));

        let app_id_payload = derive_installer_auto_input(
            &OpenClawLarkInstallerMode::Link,
            Some("cli_app"),
            Some("secret"),
            "Enter your App ID (请输入 App ID):",
            &mut auto,
        );
        assert_eq!(app_id_payload.as_deref(), Some("cli_app\r"));

        let app_secret_payload = derive_installer_auto_input(
            &OpenClawLarkInstallerMode::Link,
            Some("cli_app"),
            Some("secret"),
            "Enter your App Secret [press Enter to confirm] (请输入 App Secret [按回车确认]):",
            &mut auto,
        );
        assert_eq!(app_secret_payload.as_deref(), Some("secret\r"));
    }

    #[test]
    fn installer_prompt_hint_explains_poll_waiting_states() {
        assert_eq!(
            infer_installer_prompt_hint(
                "Fetching configuration results (正在获取你的机器人配置结果)..."
            )
            .as_deref(),
            Some("正在等待飞书官方接口返回机器人 App ID / App Secret，请稍候。")
        );
        assert_eq!(
            infer_installer_prompt_hint(
                "[DEBUG] Poll result: {\"error\":\"authorization_pending\"}"
            )
            .as_deref(),
            Some("飞书官方接口仍在等待这次扫码配置完成回传结果（authorization_pending）。")
        );
    }

    #[test]
    fn derives_environment_status_when_node_and_npm_are_available() {
        let status = derive_feishu_plugin_environment_status(
            Ok(Some("v22.0.0".to_string())),
            Ok(Some("10.8.0".to_string())),
            true,
        );

        assert!(status.node_available);
        assert!(status.npm_available);
        assert_eq!(status.node_version.as_deref(), Some("v22.0.0"));
        assert_eq!(status.npm_version.as_deref(), Some("10.8.0"));
        assert!(status.can_install_plugin);
        assert!(status.can_start_runtime);
        assert_eq!(status.error, None);
    }

    #[test]
    fn derives_environment_status_when_node_is_missing() {
        let status = derive_feishu_plugin_environment_status(
            Ok(None),
            Ok(Some("10.8.0".to_string())),
            true,
        );

        assert!(!status.node_available);
        assert!(status.npm_available);
        assert!(!status.can_install_plugin);
        assert!(!status.can_start_runtime);
        assert_eq!(status.error.as_deref(), Some("未检测到 Node.js"));
    }

    #[test]
    fn derives_environment_status_when_npm_is_missing() {
        let status = derive_feishu_plugin_environment_status(
            Ok(Some("v22.0.0".to_string())),
            Ok(None),
            true,
        );

        assert!(status.node_available);
        assert!(!status.npm_available);
        assert!(status.can_start_runtime);
        assert!(!status.can_install_plugin);
        assert_eq!(status.error.as_deref(), Some("未检测到 npm"));
    }

    #[test]
    fn derives_environment_status_when_runtime_script_is_missing() {
        let status = derive_feishu_plugin_environment_status(
            Ok(Some("v22.0.0".to_string())),
            Ok(Some("10.8.0".to_string())),
            false,
        );

        assert!(status.node_available);
        assert!(status.npm_available);
        assert!(status.can_install_plugin);
        assert!(!status.can_start_runtime);
        assert_eq!(status.error.as_deref(), Some("飞书插件运行脚本缺失"));
    }

    #[test]
    fn derives_setup_summary_state_for_missing_environment() {
        let summary = derive_feishu_setup_summary_state(
            &FeishuPluginEnvironmentStatus::default(),
            false,
            false,
            false,
            None,
            "unknown",
            0,
            None,
            0,
        );
        assert_eq!(summary, "env_missing");
    }

    #[test]
    fn derives_setup_summary_state_for_missing_plugin_install_before_credentials() {
        let summary = derive_feishu_setup_summary_state(
            &FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            false,
            false,
            false,
            None,
            "unknown",
            0,
            None,
            0,
        );
        assert_eq!(summary, "plugin_not_installed");
    }

    #[test]
    fn derives_setup_summary_state_for_missing_credentials_after_plugin_install() {
        let summary = derive_feishu_setup_summary_state(
            &FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            false,
            true,
            false,
            None,
            "unknown",
            0,
            None,
            0,
        );
        assert_eq!(summary, "ready_to_bind");
    }

    #[test]
    fn derives_setup_summary_state_for_pending_auth() {
        let summary = derive_feishu_setup_summary_state(
            &FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            true,
            true,
            true,
            None,
            "pending",
            0,
            None,
            0,
        );
        assert_eq!(summary, "awaiting_auth");
    }

    #[test]
    fn derives_setup_summary_state_for_pending_pairing_approval() {
        let summary = derive_feishu_setup_summary_state(
            &FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            true,
            true,
            true,
            None,
            "pending",
            1,
            None,
            0,
        );
        assert_eq!(summary, "awaiting_pairing_approval");
    }

    #[test]
    fn derives_setup_summary_state_for_ready_for_routing() {
        let summary = derive_feishu_setup_summary_state(
            &FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            true,
            true,
            true,
            None,
            "approved",
            0,
            None,
            0,
        );
        assert_eq!(summary, "ready_for_routing");
    }

    #[test]
    fn derives_setup_summary_state_for_fully_ready_flow() {
        let summary = derive_feishu_setup_summary_state(
            &FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            true,
            true,
            true,
            None,
            "approved",
            0,
            Some("财务刚"),
            1,
        );
        assert_eq!(summary, "ready");
    }

    #[test]
    fn auto_restore_feishu_runtime_when_previous_connection_was_fully_approved() {
        let progress = FeishuSetupProgress {
            environment: FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            credentials_configured: true,
            plugin_installed: true,
            plugin_version: Some("1.0.0".to_string()),
            runtime_running: false,
            runtime_last_error: None,
            auth_status: "approved".to_string(),
            pending_pairings: 0,
            default_routing_employee_name: Some("太子".to_string()),
            scoped_routing_count: 0,
            summary_state: "plugin_starting".to_string(),
        };

        assert!(should_auto_restore_feishu_runtime(&progress));
    }

    #[test]
    fn does_not_auto_restore_feishu_runtime_before_authorization_is_complete() {
        let progress = FeishuSetupProgress {
            environment: FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            credentials_configured: true,
            plugin_installed: true,
            plugin_version: Some("1.0.0".to_string()),
            runtime_running: false,
            runtime_last_error: None,
            auth_status: "pending".to_string(),
            pending_pairings: 0,
            default_routing_employee_name: None,
            scoped_routing_count: 0,
            summary_state: "awaiting_auth".to_string(),
        };

        assert!(!should_auto_restore_feishu_runtime(&progress));
    }

    #[test]
    fn openclaw_shim_script_supports_minimal_installer_commands() {
        let script = build_openclaw_shim_script(Path::new("C:\\temp\\state.json"));
        assert!(script.contains("args[0] === \"config\" && args[1] === \"get\""));
        assert!(script.contains("args[0] === \"config\" && args[1] === \"set\""));
        assert!(
            script.contains(
                "args[0] === \"gateway\" && (args[1] === \"restart\" || args[1] === \"start\" || args[1] === \"stop\")"
            )
        );
        assert!(
            script.contains(
                "(args[0] === \"plugins\" || args[0] === \"plugin\") && (args[1] === \"install\" || args[1] === \"uninstall\")"
            )
        );
        assert!(script.contains("plugin ${args[1]} satisfied via WorkClaw shim"));
        assert!(script.contains("args[0] === \"pairing\" && args[1] === \"approve\""));
        assert!(script.contains(OPENCLAW_SHIM_VERSION));
    }

    #[test]
    fn ensure_openclaw_cli_shim_creates_files() {
        let temp = tempfile::tempdir().expect("temp dir");
        let shim_dir = ensure_openclaw_cli_shim(temp.path()).expect("create shim");
        assert!(shim_dir.join("openclaw-shim.mjs").exists());
        assert!(shim_dir.join("state.json").exists());
        #[cfg(windows)]
        assert!(shim_dir.join("openclaw.cmd").exists());
        #[cfg(not(windows))]
        assert!(shim_dir.join("openclaw").exists());
    }

    #[test]
    fn derive_feishu_credentials_from_shim_snapshot_reads_config_projection() {
        let snapshot = OpenClawShimStateSnapshot {
            config: serde_json::json!({
                "channels": {
                    "feishu": {
                        "appId": "cli_created",
                        "appSecret": "secret_created"
                    }
                }
            }),
            commands: vec![],
        };

        let credentials =
            derive_feishu_credentials_from_shim_snapshot(&snapshot).expect("credentials from config");

        assert_eq!(credentials.0, "cli_created");
        assert_eq!(credentials.1, "secret_created");
    }

    #[test]
    fn derive_feishu_credentials_from_shim_snapshot_falls_back_to_recorded_commands() {
        let snapshot = OpenClawShimStateSnapshot {
            config: serde_json::json!({}),
            commands: vec![
                OpenClawShimRecordedCommand {
                    args: vec![
                        "config".to_string(),
                        "set".to_string(),
                        "channels.feishu.appId".to_string(),
                        "cli_from_command".to_string(),
                    ],
                },
                OpenClawShimRecordedCommand {
                    args: vec![
                        "config".to_string(),
                        "set".to_string(),
                        "channels.feishu.appSecret".to_string(),
                        "secret_from_command".to_string(),
                    ],
                },
            ],
        };

        let credentials =
            derive_feishu_credentials_from_shim_snapshot(&snapshot).expect("credentials from commands");

        assert_eq!(credentials.0, "cli_from_command");
        assert_eq!(credentials.1, "secret_from_command");
    }

    #[tokio::test]
    async fn sync_feishu_gateway_credentials_from_shim_updates_app_settings() {
        let pool = setup_memory_pool().await;
        let temp = tempfile::tempdir().expect("tempdir");
        let shim_root = temp.path().join("openclaw-cli-shim");
        std::fs::create_dir_all(&shim_root).expect("create shim root");
        std::fs::write(
            build_openclaw_shim_state_file_path(&shim_root),
            serde_json::json!({
                "config": {
                    "channels": {
                        "feishu": {
                            "appId": "cli_synced",
                            "appSecret": "secret_synced"
                        }
                    }
                },
                "commands": []
            })
            .to_string(),
        )
        .expect("write shim state");

        let updated = sync_feishu_gateway_credentials_from_shim_with_pool(&pool, &shim_root)
            .await
            .expect("sync shim credentials");

        assert!(updated);
        assert_eq!(
            get_app_setting(&pool, "feishu_app_id")
                .await
                .expect("load app id")
                .as_deref(),
            Some("cli_synced")
        );
        assert_eq!(
            get_app_setting(&pool, "feishu_app_secret")
                .await
                .expect("load app secret")
                .as_deref(),
            Some("secret_synced")
        );
    }

    #[test]
    fn derive_feishu_credentials_from_openclaw_state_config_reads_plaintext_credentials() {
        let state_root = Path::new("C:\\workclaw\\openclaw-state");
        let config = serde_json::json!({
            "channels": {
                "feishu": {
                    "appId": "cli_created_from_state",
                    "appSecret": "secret_created_from_state"
                }
            }
        });

        let credentials = derive_feishu_credentials_from_openclaw_state_config(&config, state_root)
            .expect("credentials from controlled state config");

        assert_eq!(credentials.0, "cli_created_from_state");
        assert_eq!(credentials.1, "secret_created_from_state");
    }

    #[tokio::test]
    async fn sync_feishu_gateway_credentials_from_controlled_state_reads_env_secret() {
        let pool = setup_memory_pool().await;
        let temp = tempfile::tempdir().expect("tempdir");
        let state_root = temp.path().join("openclaw-state");
        std::fs::create_dir_all(&state_root).expect("create state root");
        std::fs::write(
            state_root.join(".env"),
            "LARK_APP_SECRET=secret_from_env\n",
        )
        .expect("write env file");
        std::fs::write(
            state_root.join("openclaw.json"),
            serde_json::json!({
                "channels": {
                    "feishu": {
                        "appId": "cli_from_state",
                        "appSecret": {
                            "source": "env",
                            "provider": "default",
                            "id": "LARK_APP_SECRET"
                        }
                    }
                }
            })
            .to_string(),
        )
        .expect("write controlled state config");

        let updated = sync_feishu_gateway_credentials_from_openclaw_state_with_pool(&pool, &state_root)
            .await
            .expect("sync credentials from controlled state");

        assert!(updated);
        assert_eq!(
            get_app_setting(&pool, "feishu_app_id")
                .await
                .expect("load app id")
                .as_deref(),
            Some("cli_from_state")
        );
        assert_eq!(
            get_app_setting(&pool, "feishu_app_secret")
                .await
                .expect("load app secret")
                .as_deref(),
            Some("secret_from_env")
        );
    }

    #[test]
    fn resolve_plugin_host_dir_finds_packaged_up_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let exe_dir = temp.path().join("runtime-bin");
        let up_plugin_host = exe_dir.join("_up_").join("plugin-host");
        std::fs::create_dir_all(&up_plugin_host).expect("create packaged plugin host");
        std::fs::write(up_plugin_host.join("marker.txt"), "ok").expect("write marker");

        let candidates = [
            exe_dir.join("resources").join("plugin-host"),
            exe_dir.join("_up_").join("plugin-host"),
            exe_dir.join("plugin-host"),
        ];
        let resolved = candidates
            .into_iter()
            .find(|candidate| candidate.exists())
            .expect("resolved packaged plugin host");
        assert_eq!(resolved, up_plugin_host);
    }

    #[test]
    fn build_plugin_host_fixture_root_uses_app_data_dir() {
        let app_data_dir = Path::new(r"C:\Users\Alice\AppData\Roaming\dev.workclaw.runtime");
        let fixture_root = build_plugin_host_fixture_root_from_app_data_dir(app_data_dir);
        assert_eq!(
            fixture_root,
            PathBuf::from(r"C:\Users\Alice\AppData\Roaming\dev.workclaw.runtime\plugin-host-fixtures")
        );
    }

    #[test]
    fn prepend_env_path_places_shim_first() {
        let mut command = Command::new("node");
        let shim_dir = Path::new("C:\\shim");
        prepend_env_path(&mut command, shim_dir);
        let env_path = command
            .get_envs()
            .find_map(|(key, value)| (key == "PATH").then(|| value))
            .flatten()
            .expect("PATH env");
        let first = std::env::split_paths(env_path)
            .next()
            .expect("first PATH segment");
        assert_eq!(first, shim_dir);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn build_effective_path_entries_keeps_prepend_and_adds_registry_paths() {
        let prepend = vec![PathBuf::from(r"C:\shim")];
        let current_path = std::ffi::OsString::from(r"C:\gui-node;C:\common");
        let extra_entries = vec![PathBuf::from(r"C:\user-node"), PathBuf::from(r"C:\common")];

        let paths = build_effective_path_entries(Some(&current_path), &prepend, &extra_entries);

        assert_eq!(paths.first(), Some(&PathBuf::from(r"C:\shim")));
        assert!(paths.contains(&PathBuf::from(r"C:\gui-node")));
        assert!(paths.contains(&PathBuf::from(r"C:\user-node")));
        assert_eq!(
            paths
                .iter()
                .filter(|entry| entry == &&PathBuf::from(r"C:\common"))
                .count(),
            1
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn parse_windows_registry_path_output_expands_env_segments() {
        std::env::set_var("LOCALAPPDATA", r"C:\Users\Alice\AppData\Local");
        let parsed = parse_windows_registry_path_output(
            "HKEY_CURRENT_USER\\Environment\n    Path    REG_EXPAND_SZ    %LOCALAPPDATA%\\Programs\\nodejs;C:\\Tools\\Node\n",
        );

        assert_eq!(
            parsed,
            vec![
                PathBuf::from(r"C:\Users\Alice\AppData\Local\Programs\nodejs"),
                PathBuf::from(r"C:\Tools\Node"),
            ]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_node_candidates_include_nvm_and_common_install_locations() {
        let temp = tempfile::tempdir().expect("tempdir");
        let nvm_link = temp.path().join("nvm-link");
        let nvm_home = temp.path().join("nvm-home");
        std::fs::create_dir_all(&nvm_link).expect("create nvm_link");
        std::fs::create_dir_all(&nvm_home).expect("create nvm_home");
        std::env::set_var("NVM_SYMLINK", &nvm_link);
        std::env::set_var("NVM_HOME", &nvm_home);

        let candidates = collect_windows_node_command_candidates();
        assert!(candidates.iter().any(|path| path.ends_with(Path::new("node.exe"))));
        assert!(candidates.iter().any(|path| path == &nvm_link.join("node.exe")));
        assert!(candidates.iter().any(|path| path == &nvm_home.join("node.exe")));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_node_candidates_are_deduped_case_insensitively() {
        std::env::set_var("PATH", r"C:\Node;C:\node");
        let candidates = collect_windows_node_command_candidates();
        let lowered: std::collections::HashSet<String> = candidates
            .iter()
            .map(|candidate| candidate.to_string_lossy().to_lowercase())
            .collect();
        assert_eq!(lowered.len(), candidates.len());
    }

    #[tokio::test]
    async fn outbound_send_writes_command_and_receives_structured_send_result() {
        use std::collections::HashMap;
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};
        use std::sync::{Arc, Mutex};

        let temp = tempfile::tempdir().expect("tempdir");
        let script_path = temp.path().join("echo-send-result.mjs");
        std::fs::write(
            &script_path,
            r#"
import readline from 'node:readline';
const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
rl.on('line', (line) => {
  const payload = JSON.parse(line);
  process.stdout.write(JSON.stringify({
    event: 'send_result',
    requestId: payload.requestId,
    request: payload,
    result: {
      delivered: true,
      channel: 'feishu',
      accountId: payload.accountId,
      target: payload.target,
      threadId: payload.threadId,
      text: payload.text,
      mode: payload.mode,
      messageId: 'om_outbound_1',
      chatId: payload.target,
      sequence: 1,
    },
  }) + '\n');
});
"#,
        )
        .expect("write echo runtime script");

        let mut child = Command::new("node")
            .arg(&script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn echo runtime");
        let stdout = child.stdout.take().expect("runtime stdout");
        let runtime_stdin = Arc::new(Mutex::new(child.stdin.take().expect("runtime stdin")));
        let state = OpenClawPluginFeishuRuntimeState(Arc::new(Mutex::new(
            OpenClawPluginFeishuRuntimeStore {
                process: Some(Arc::new(Mutex::new(Some(child)))),
                stdin: Some(runtime_stdin.clone()),
                status: OpenClawPluginFeishuRuntimeStatus {
                    running: true,
                    ..Default::default()
                },
                pending_outbound_send_results: HashMap::new(),
            },
        )));

        let state_clone = state.clone();
        let stdout_thread = std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
                    continue;
                };
                let _ = handle_openclaw_plugin_feishu_runtime_send_result_event(
                    &state_clone,
                    &value,
                );
            }
        });

        let result = send_openclaw_plugin_feishu_runtime_outbound_message_in_state(
            &state,
            OpenClawPluginFeishuOutboundSendRequest {
                request_id: "request-1".to_string(),
                account_id: "default".to_string(),
            target: "oc_chat_123".to_string(),
            thread_id: Some("oc_chat_123".to_string()),
            text: "你好".to_string(),
            mode: "text".to_string(),
        },
        )
        .expect("send outbound message");

        assert_eq!(result.request_id, "request-1");
        assert_eq!(result.request.account_id, "default");
        assert_eq!(result.request.target, "oc_chat_123");
        assert_eq!(result.result.delivered, true);
        assert_eq!(result.result.channel, "feishu");
        assert_eq!(result.result.message_id, "om_outbound_1");
        assert_eq!(result.result.chat_id, "oc_chat_123");

        {
            let mut guard = state.0.lock().expect("runtime state lock");
            guard.stdin = None;
        }
        drop(runtime_stdin);
        {
            let guard = state.0.lock().expect("runtime state lock");
            if let Some(slot) = guard.process.as_ref() {
                if let Ok(mut child_guard) = slot.lock() {
                    if let Some(mut child) = child_guard.take() {
                        let _ = child.wait();
                    }
                }
            }
        }

        stdout_thread.join().expect("stdout reader");
    }

    #[test]
    fn outbound_command_error_fails_pending_request_immediately() {
        use std::collections::HashMap;
        use std::sync::{Arc, Mutex};

        let request_id = "request-command-error";
        let state = OpenClawPluginFeishuRuntimeState(Arc::new(Mutex::new(
            OpenClawPluginFeishuRuntimeStore {
                process: None,
                stdin: None,
                status: OpenClawPluginFeishuRuntimeStatus {
                    running: true,
                    ..Default::default()
                },
                pending_outbound_send_results: HashMap::new(),
            },
        )));

        let receiver = register_pending_feishu_runtime_outbound_send_waiter(&state, request_id)
            .expect("register pending outbound waiter");

        let handled = handle_openclaw_plugin_feishu_runtime_command_error_event(
            &state,
            &serde_json::json!({
                "event": "command_error",
                "requestId": request_id,
                "command": "send_message",
                "error": "outbound target is required",
            }),
        );

        assert!(handled, "expected command_error event to resolve pending waiter");
        let result = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("receive command_error result");
        match result {
            Ok(_) => panic!("expected outbound command to fail"),
            Err(error) => assert!(
                error.contains("outbound target is required"),
                "unexpected command_error: {error}"
            ),
        }
    }

    #[test]
    fn merges_runtime_status_patch_events() {
        let mut status = OpenClawPluginFeishuRuntimeStatus::default();
        merge_feishu_runtime_status_event(
            &mut status,
            &serde_json::json!({
                "event": "status",
                "patch": {
                    "accountId": "workspace",
                    "port": 3100,
                    "lastError": ""
                }
            }),
        );

        assert_eq!(status.account_id, "workspace");
        assert_eq!(status.port, Some(3100));
        assert_eq!(status.last_error, None);
    }

    #[test]
    fn merges_runtime_fatal_events_into_last_error() {
        let mut status = OpenClawPluginFeishuRuntimeStatus::default();
        merge_feishu_runtime_status_event(
            &mut status,
            &serde_json::json!({
                "event": "fatal",
                "error": "runtime crashed"
            }),
        );

        assert_eq!(status.last_error.as_deref(), Some("runtime crashed"));
    }

    #[test]
    fn merges_runtime_log_events_into_recent_logs_and_error_state() {
        let mut status = OpenClawPluginFeishuRuntimeStatus::default();
        merge_feishu_runtime_status_event(
            &mut status,
            &serde_json::json!({
                "event": "log",
                "level": "error",
                "scope": "channel/monitor",
                "message": "failed to dispatch inbound message"
            }),
        );

        assert_eq!(
            status.last_error.as_deref(),
            Some("[error] channel/monitor: failed to dispatch inbound message")
        );
        assert_eq!(
            status.recent_logs.last().map(String::as_str),
            Some("[error] channel/monitor: failed to dispatch inbound message")
        );
        assert!(status.last_event_at.is_some());
    }

    #[test]
    fn matches_feishu_runtime_command_line_by_plugin_root_and_account() {
        let command_line = "\"node\" D:\\code\\WorkClaw\\apps\\runtime\\plugin-host\\scripts\\run-feishu-host.mjs --plugin-root C:\\Users\\36443\\AppData\\Roaming\\dev.workclaw.runtime\\openclaw-plugins\\openclaw-lark\\workspace\\node_modules\\@larksuite\\openclaw-lark --fixture-name openclaw-lark-runtime --account-id default --config-json {}";
        assert!(matches_feishu_runtime_command_line(
            command_line,
            "C:\\Users\\36443\\AppData\\Roaming\\dev.workclaw.runtime\\openclaw-plugins\\openclaw-lark\\workspace\\node_modules\\@larksuite\\openclaw-lark",
            "default"
        ));
        assert!(!matches_feishu_runtime_command_line(
            command_line,
            "C:\\Users\\36443\\AppData\\Roaming\\dev.workclaw.runtime\\openclaw-plugins\\other",
            "default"
        ));
        assert!(!matches_feishu_runtime_command_line(
            command_line,
            "C:\\Users\\36443\\AppData\\Roaming\\dev.workclaw.runtime\\openclaw-plugins\\openclaw-lark\\workspace\\node_modules\\@larksuite\\openclaw-lark",
            "workspace"
        ));
    }

    #[tokio::test]
    async fn parses_runtime_dispatch_events_into_im_events() {
        let pool = setup_memory_pool().await;
        let event = parse_feishu_runtime_dispatch_event_with_pool(
            &pool,
            &serde_json::json!({
                "threadId": "ou_sender",
                "chatId": "oc_chat_123",
                "accountId": "default",
                "senderId": "ou_sender",
                "messageId": "om_123",
                "text": "你好",
                "chatType": "direct"
            }),
        )
        .await
        .expect("parse dispatch event");

        assert_eq!(event.channel, "feishu");
        assert_eq!(event.event_type, ImEventType::MessageCreated);
        assert_eq!(event.thread_id, "oc_chat_123");
        assert_eq!(event.text.as_deref(), Some("你好"));
        assert_eq!(event.sender_id.as_deref(), Some("ou_sender"));
        assert_eq!(event.chat_type.as_deref(), Some("direct"));
    }

    #[tokio::test]
    async fn resolves_runtime_dispatch_thread_id_from_pairing_chat_id() {
        let pool = setup_memory_pool().await;
        let _ = sqlx::query(
            "INSERT INTO feishu_pairing_requests (
                id, channel, account_id, sender_id, chat_id, code, status, created_at, updated_at, resolved_at, resolved_by_user
             ) VALUES (?, 'feishu', ?, ?, ?, ?, 'approved', ?, ?, ?, ?)",
        )
        .bind("req_1")
        .bind("default")
        .bind("ou_sender")
        .bind("oc_chat_123")
        .bind("PAIR1234")
        .bind("2026-03-19T00:00:00Z")
        .bind("2026-03-19T00:00:00Z")
        .bind("2026-03-19T00:00:00Z")
        .bind("tester")
        .execute(&pool)
        .await
        .expect("insert pairing request");

        let event = parse_feishu_runtime_dispatch_event_with_pool(
            &pool,
            &serde_json::json!({
                "threadId": "ou_sender",
                "accountId": "default",
                "senderId": "ou_sender",
                "messageId": "om_124",
                "text": "你好",
                "chatType": "direct"
            }),
        )
        .await
        .expect("parse dispatch event");

        assert_eq!(event.thread_id, "oc_chat_123");
    }

    #[tokio::test]
    async fn upsert_openclaw_plugin_install_records_plugin_metadata() {
        let pool = setup_memory_pool().await;

        let record = upsert_openclaw_plugin_install_with_pool(
            &pool,
            OpenClawPluginInstallInput {
                plugin_id: "openclaw-lark".to_string(),
                npm_spec: "@larksuite/openclaw-lark".to_string(),
                version: "2026.3.17".to_string(),
                install_path: "D:/plugins/openclaw-lark".to_string(),
                source_type: "npm".to_string(),
                manifest_json: "{\"id\":\"openclaw-lark\"}".to_string(),
            },
        )
        .await
        .expect("upsert plugin install");

        assert_eq!(record.plugin_id, "openclaw-lark");
        assert_eq!(record.npm_spec, "@larksuite/openclaw-lark");
        assert_eq!(record.version, "2026.3.17");
        assert_eq!(record.install_path, "D:/plugins/openclaw-lark");
        assert_eq!(record.source_type, "npm");
        assert_eq!(record.manifest_json, "{\"id\":\"openclaw-lark\"}");
        assert!(!record.installed_at.is_empty());
        assert!(!record.updated_at.is_empty());
    }

    #[tokio::test]
    async fn list_openclaw_plugin_installs_is_separate_from_local_skills() {
        let pool = setup_memory_pool().await;

        sqlx::query(
            "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type)
             VALUES ('local-brainstorming', '{}', '2026-03-19T00:00:00Z', '', 'D:/skills/brainstorming', 'local')",
        )
        .execute(&pool)
        .await
        .expect("seed installed skill");

        upsert_openclaw_plugin_install_with_pool(
            &pool,
            OpenClawPluginInstallInput {
                plugin_id: "openclaw-lark".to_string(),
                npm_spec: "@larksuite/openclaw-lark".to_string(),
                version: "2026.3.17".to_string(),
                install_path: "D:/plugins/openclaw-lark".to_string(),
                source_type: "npm".to_string(),
                manifest_json: "{\"id\":\"openclaw-lark\"}".to_string(),
            },
        )
        .await
        .expect("upsert plugin install");

        let records = list_openclaw_plugin_installs_with_pool(&pool)
            .await
            .expect("list plugin installs");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].plugin_id, "openclaw-lark");
    }

    #[tokio::test]
    async fn upsert_openclaw_plugin_install_updates_existing_record() {
        let pool = setup_memory_pool().await;

        upsert_openclaw_plugin_install_with_pool(
            &pool,
            OpenClawPluginInstallInput {
                plugin_id: "openclaw-lark".to_string(),
                npm_spec: "@larksuite/openclaw-lark".to_string(),
                version: "2026.3.15".to_string(),
                install_path: "D:/plugins/openclaw-lark-old".to_string(),
                source_type: "npm".to_string(),
                manifest_json: "{\"id\":\"openclaw-lark\",\"version\":\"2026.3.15\"}".to_string(),
            },
        )
        .await
        .expect("seed plugin install");

        let updated = upsert_openclaw_plugin_install_with_pool(
            &pool,
            OpenClawPluginInstallInput {
                plugin_id: "openclaw-lark".to_string(),
                npm_spec: "@larksuite/openclaw-lark".to_string(),
                version: "2026.3.17".to_string(),
                install_path: "D:/plugins/openclaw-lark".to_string(),
                source_type: "npm".to_string(),
                manifest_json: "{\"id\":\"openclaw-lark\",\"version\":\"2026.3.17\"}".to_string(),
            },
        )
        .await
        .expect("update plugin install");

        let records = list_openclaw_plugin_installs_with_pool(&pool)
            .await
            .expect("list plugin installs");

        assert_eq!(records.len(), 1);
        assert_eq!(updated.version, "2026.3.17");
        assert_eq!(records[0].install_path, "D:/plugins/openclaw-lark");
    }

    #[test]
    fn derive_channel_capabilities_flattens_runtime_flags() {
        let channel = OpenClawPluginChannelInspection {
            id: Some("feishu".to_string()),
            meta: None,
            capabilities: Some(serde_json::json!({
                "chatTypes": ["direct", "group"],
                "media": true,
                "reactions": true,
                "threads": true,
                "nativeCommands": true,
                "blockStreaming": true
            })),
            reload_config_prefixes: vec!["channels.feishu".to_string()],
            has_pairing: true,
            has_setup: true,
            has_onboarding: true,
            has_directory: true,
            has_outbound: true,
            has_threading: true,
            has_actions: true,
            has_status: true,
            target_hint: Some("<chatId|user:openId>".to_string()),
        };

        let capabilities = derive_channel_capabilities(&channel);

        assert!(capabilities.contains(&"chat_type:direct".to_string()));
        assert!(capabilities.contains(&"chat_type:group".to_string()));
        assert!(capabilities.contains(&"media".to_string()));
        assert!(capabilities.contains(&"reactions".to_string()));
        assert!(capabilities.contains(&"threads".to_string()));
        assert!(capabilities.contains(&"native_commands".to_string()));
        assert!(capabilities.contains(&"block_streaming".to_string()));
        assert!(capabilities.contains(&"pairing".to_string()));
        assert!(capabilities.contains(&"setup".to_string()));
        assert!(capabilities.contains(&"onboarding".to_string()));
        assert!(capabilities.contains(&"directory".to_string()));
        assert!(capabilities.contains(&"outbound".to_string()));
        assert!(capabilities.contains(&"threading".to_string()));
        assert!(capabilities.contains(&"actions".to_string()));
        assert!(capabilities.contains(&"status".to_string()));
    }

    #[test]
    fn parse_feishu_app_access_token_response_returns_token_on_success() {
        let token = parse_feishu_app_access_token_response(serde_json::json!({
            "code": 0,
            "msg": "success",
            "app_access_token": "token-123"
        }))
        .expect("token should parse");

        assert_eq!(token, "token-123");
    }

    #[test]
    fn parse_feishu_app_access_token_response_returns_api_error() {
        let error = parse_feishu_app_access_token_response(serde_json::json!({
            "code": 99991663,
            "msg": "invalid app credentials"
        }))
        .expect_err("invalid credentials should fail");

        assert_eq!(error, "API error: invalid app credentials");
    }

    #[test]
    fn parse_feishu_bot_info_response_extracts_identity() {
        let result = parse_feishu_bot_info_response(
            "cli_app",
            serde_json::json!({
                "code": 0,
                "msg": "success",
                "bot": {
                    "bot_name": "WorkClaw Bot",
                    "open_id": "ou_bot_open_id"
                }
            }),
        );

        assert!(result.ok);
        assert_eq!(result.app_id, "cli_app");
        assert_eq!(result.bot_name.as_deref(), Some("WorkClaw Bot"));
        assert_eq!(result.bot_open_id.as_deref(), Some("ou_bot_open_id"));
        assert_eq!(result.error, None);
    }
}
