use crate::commands::skills::DbState;
use crate::windows_process::hide_console_window;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Child, ChildStdin};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

#[path = "openclaw_plugins/tauri_commands.rs"]
mod tauri_commands;
#[path = "openclaw_plugins/settings_service.rs"]
mod settings_service;
#[path = "openclaw_plugins/setup_service.rs"]
mod setup_service;
#[path = "openclaw_plugins/runtime_service.rs"]
mod runtime_service;
#[path = "openclaw_plugins/install_repo.rs"]
mod install_repo;
#[path = "openclaw_plugins/install_service.rs"]
mod install_service;
#[path = "openclaw_plugins/plugin_host_service.rs"]
mod plugin_host_service;
#[path = "openclaw_plugins/installer_session.rs"]
mod installer_session;

use tauri_commands::{
    delete_openclaw_plugin_install_command, get_feishu_plugin_environment_status_command,
    get_feishu_setup_progress_command, get_openclaw_lark_installer_session_status_command,
    get_openclaw_plugin_feishu_advanced_settings_command,
    get_openclaw_plugin_feishu_channel_snapshot_command,
    get_openclaw_plugin_feishu_runtime_status_command, inspect_openclaw_plugin_command,
    install_openclaw_plugin_from_npm_command, list_openclaw_plugin_channel_hosts_command,
    list_openclaw_plugin_installs_command, probe_openclaw_plugin_feishu_credentials_command,
    send_openclaw_lark_installer_input_command,
    set_openclaw_plugin_feishu_advanced_settings_command,
    start_openclaw_lark_installer_session_command, start_openclaw_plugin_feishu_runtime_command,
    stop_openclaw_lark_installer_session_command, stop_openclaw_plugin_feishu_runtime_command,
    upsert_openclaw_plugin_install_command,
};
use settings_service::{
    app_setting_string_or_default, build_feishu_openclaw_config_with_pool,
    get_openclaw_plugin_feishu_advanced_settings_with_pool,
    set_openclaw_plugin_feishu_advanced_settings_with_pool,
};
use setup_service::{
    build_openclaw_shim_state_file_path, get_feishu_setup_progress_with_pool,
    probe_openclaw_plugin_feishu_credentials_with_app_secret, resolve_controlled_openclaw_state_root,
    resolve_openclaw_shim_root, should_auto_restore_feishu_runtime,
    sync_feishu_gateway_credentials_from_openclaw_state_with_pool,
    sync_feishu_gateway_credentials_from_shim_with_pool,
};
pub(crate) use runtime_service::{
    current_feishu_runtime_status, maybe_restore_openclaw_plugin_feishu_runtime_with_pool,
    send_openclaw_plugin_feishu_runtime_outbound_message_in_state,
    start_openclaw_plugin_feishu_runtime_with_pool, stop_openclaw_plugin_feishu_runtime_in_state,
};
pub(crate) use install_repo::{
    delete_openclaw_plugin_install_with_pool, get_openclaw_plugin_install_by_id_with_pool,
    list_openclaw_plugin_installs_with_pool, upsert_openclaw_plugin_install_with_pool,
};
pub(crate) use plugin_host_service::{
    append_disable_dep0190_node_option, apply_command_search_path,
    build_openclaw_lark_tools_npx_args, get_feishu_plugin_environment_status_internal,
    get_openclaw_plugin_feishu_channel_snapshot_with_pool,
    get_openclaw_plugin_feishu_channel_snapshot_with_pool_and_app_public as get_openclaw_plugin_feishu_channel_snapshot_with_pool_and_app,
    inspect_openclaw_plugin_with_pool_and_app_public as inspect_openclaw_plugin_with_pool_and_app,
    list_openclaw_plugin_channel_hosts_with_pool_and_app_public as list_openclaw_plugin_channel_hosts_with_pool_and_app,
    resolve_npm_command, resolve_npx_command, resolve_openclaw_plugin_workspace_root,
    resolve_plugin_host_dir, resolve_plugin_host_fixture_root,
    resolve_plugin_host_run_feishu_script, resolve_windows_node_command_path,
};
pub(crate) use installer_session::{
    current_openclaw_lark_installer_session_status, send_openclaw_lark_installer_input_in_state,
    start_openclaw_lark_installer_session_with_pool,
    stop_openclaw_lark_installer_session_in_state,
};

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
pub(crate) struct OpenClawLarkInstallerAutoInputState {
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

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
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

#[tauri::command]
pub async fn start_openclaw_plugin_feishu_runtime(
    plugin_id: String,
    account_id: Option<String>,
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    runtime: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<OpenClawPluginFeishuRuntimeStatus, String> {
    start_openclaw_plugin_feishu_runtime_command(plugin_id, account_id, app, db, runtime).await
}

#[tauri::command]
pub async fn stop_openclaw_plugin_feishu_runtime(
    runtime: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<OpenClawPluginFeishuRuntimeStatus, String> {
    stop_openclaw_plugin_feishu_runtime_command(runtime).await
}

#[tauri::command]
pub async fn get_openclaw_plugin_feishu_runtime_status(
    runtime: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<OpenClawPluginFeishuRuntimeStatus, String> {
    get_openclaw_plugin_feishu_runtime_status_command(runtime).await
}

#[tauri::command]
pub async fn get_feishu_plugin_environment_status() -> Result<FeishuPluginEnvironmentStatus, String> {
    get_feishu_plugin_environment_status_command().await
}

#[tauri::command]
pub async fn get_feishu_setup_progress(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    runtime: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<FeishuSetupProgress, String> {
    get_feishu_setup_progress_command(app, db, runtime).await
}

#[tauri::command]
pub async fn get_openclaw_plugin_feishu_advanced_settings(
    db: State<'_, DbState>,
) -> Result<OpenClawPluginFeishuAdvancedSettings, String> {
    get_openclaw_plugin_feishu_advanced_settings_command(db).await
}

#[tauri::command]
pub async fn set_openclaw_plugin_feishu_advanced_settings(
    settings: OpenClawPluginFeishuAdvancedSettings,
    db: State<'_, DbState>,
) -> Result<OpenClawPluginFeishuAdvancedSettings, String> {
    set_openclaw_plugin_feishu_advanced_settings_command(settings, db).await
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
    start_openclaw_lark_installer_session_command(mode, app_id, app_secret, app, db, installer)
        .await
}

#[tauri::command]
pub async fn get_openclaw_lark_installer_session_status(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    installer: State<'_, OpenClawLarkInstallerSessionState>,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    get_openclaw_lark_installer_session_status_command(app, db, installer).await
}

#[tauri::command]
pub async fn send_openclaw_lark_installer_input(
    input: String,
    installer: State<'_, OpenClawLarkInstallerSessionState>,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    send_openclaw_lark_installer_input_command(input, installer).await
}

#[tauri::command]
pub async fn stop_openclaw_lark_installer_session(
    installer: State<'_, OpenClawLarkInstallerSessionState>,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    stop_openclaw_lark_installer_session_command(installer).await
}

#[tauri::command]
pub async fn probe_openclaw_plugin_feishu_credentials(
    app_id: String,
    app_secret: String,
) -> Result<OpenClawPluginFeishuCredentialProbeResult, String> {
    probe_openclaw_plugin_feishu_credentials_command(app_id, app_secret).await
}

#[tauri::command]
pub async fn upsert_openclaw_plugin_install(
    input: OpenClawPluginInstallInput,
    db: State<'_, DbState>,
) -> Result<OpenClawPluginInstallRecord, String> {
    upsert_openclaw_plugin_install_command(input, db).await
}

#[tauri::command]
pub async fn list_openclaw_plugin_installs(
    db: State<'_, DbState>,
) -> Result<Vec<OpenClawPluginInstallRecord>, String> {
    list_openclaw_plugin_installs_command(db).await
}

#[tauri::command]
pub async fn delete_openclaw_plugin_install(
    plugin_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    delete_openclaw_plugin_install_command(plugin_id, db).await
}

#[tauri::command]
pub async fn inspect_openclaw_plugin(
    plugin_id: String,
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<OpenClawPluginInspectionResult, String> {
    inspect_openclaw_plugin_command(plugin_id, app, db).await
}

#[tauri::command]
pub async fn list_openclaw_plugin_channel_hosts(
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<Vec<OpenClawPluginChannelHost>, String> {
    list_openclaw_plugin_channel_hosts_command(app, db).await
}

#[tauri::command]
pub async fn get_openclaw_plugin_feishu_channel_snapshot(
    plugin_id: String,
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<OpenClawPluginChannelSnapshotResult, String> {
    get_openclaw_plugin_feishu_channel_snapshot_command(plugin_id, app, db).await
}

#[tauri::command]
pub async fn install_openclaw_plugin_from_npm(
    plugin_id: String,
    npm_spec: String,
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<OpenClawPluginInstallRecord, String> {
    install_openclaw_plugin_from_npm_command(plugin_id, npm_spec, app, db).await
}

#[cfg(test)]
#[path = "openclaw_plugins/tests.rs"]
mod tests;
