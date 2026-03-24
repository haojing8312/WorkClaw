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

fn default_source_type() -> String {
    "npm".to_string()
}
