export type SkillRouteNodeStatus =
  | "routing"
  | "executing"
  | "waiting_user"
  | "confirm_required"
  | "completed"
  | "failed"
  | "cancelled";

export interface SkillRouteEvent {
  session_id: string;
  route_run_id: string;
  node_id: string;
  parent_node_id?: string;
  skill_name: string;
  depth: number;
  status: SkillRouteNodeStatus | string;
  duration_ms?: number;
  error_code?: string;
  error_message?: string;
}

export interface ImRoleTimelineEvent {
  session_id: string;
  thread_id: string;
  role_id: string;
  role_name: string;
  message_type?: string;
  sender_role?: "user" | "main_agent" | "sub_agent" | "system" | string;
  sender_employee_id?: string;
  target_employee_id?: string;
  task_id?: string;
  parent_task_id?: string;
  source_channel?: "desktop" | "app" | "feishu" | "wecom" | string;
  status: "running" | "completed" | "failed" | string;
  summary?: string;
  duration_ms?: number;
}

export interface ImRoleDispatchRequest {
  session_id: string;
  thread_id: string;
  message_id?: string;
  role_id: string;
  role_name: string;
  message_type?: string;
  sender_role?: "user" | "main_agent" | "sub_agent" | "system" | string;
  sender_employee_id?: string;
  target_employee_id?: string;
  task_id?: string;
  parent_task_id?: string;
  source_channel?: "desktop" | "app" | "feishu" | "wecom" | string;
  prompt: string;
  agent_type: string;
}

export interface ImRouteDecisionEvent {
  session_id?: string;
  thread_id: string;
  agent_id: string;
  session_key: string;
  matched_by: string;
}

export interface FeishuGatewaySettings {
  app_id: string;
  app_secret: string;
  ingress_token: string;
  encrypt_key: string;
  sidecar_base_url: string;
}

export interface OpenClawPluginFeishuAdvancedSettings {
  groups_json: string;
  dms_json: string;
  footer_json: string;
  account_overrides_json: string;
  render_mode: string;
  streaming: string;
  text_chunk_limit: string;
  chunk_mode: string;
  reply_in_thread: string;
  group_session_scope: string;
  topic_session_mode: string;
  markdown_mode: string;
  markdown_table_mode: string;
  heartbeat_visibility: string;
  heartbeat_interval_ms: string;
  media_max_mb: string;
  http_timeout_ms: string;
  config_writes: string;
  webhook_host: string;
  webhook_port: string;
  dynamic_agent_creation_enabled: string;
  dynamic_agent_creation_workspace_template: string;
  dynamic_agent_creation_agent_dir_template: string;
  dynamic_agent_creation_max_agents: string;
}

export interface WecomGatewaySettings {
  corp_id: string;
  agent_id: string;
  agent_secret: string;
  sidecar_base_url: string;
}

export interface FeishuWsStatus {
  running: boolean;
  started_at?: string | null;
  queued_events: number;
}

export interface OpenClawPluginFeishuRuntimeStatus {
  plugin_id: string;
  account_id: string;
  running: boolean;
  started_at?: string | null;
  last_stop_at?: string | null;
  last_event_at?: string | null;
  last_error?: string | null;
  pid?: number | null;
  port?: number | null;
  recent_logs?: string[];
  recent_reply_lifecycle?: {
    logicalReplyId: string;
    phase: string;
    channel: string;
    accountId?: string | null;
    threadId?: string | null;
    chatId?: string | null;
    messageId?: string | null;
    queuedCounts?: unknown;
  }[];
  latest_reply_completion?: {
    logicalReplyId: string;
    phase: string;
    state:
      | "running"
      | "waiting_for_idle"
      | "idle_reached"
      | "awaiting_user"
      | "awaiting_approval"
      | "interrupted"
      | "completed"
      | "failed"
      | "stopped";
    updatedAt?: string | null;
  } | null;
}

export interface FeishuPluginEnvironmentStatus {
  node_available: boolean;
  npm_available: boolean;
  node_version?: string | null;
  npm_version?: string | null;
  node_version_supported?: boolean;
  required_node_major?: number;
  can_install_plugin: boolean;
  can_start_runtime: boolean;
  error?: string | null;
}

export interface FeishuSetupProgress {
  environment: FeishuPluginEnvironmentStatus;
  credentials_configured: boolean;
  plugin_installed: boolean;
  plugin_version?: string | null;
  runtime_running: boolean;
  runtime_last_error?: string | null;
  auth_status: string;
  pending_pairings: number;
  default_routing_employee_name?: string | null;
  scoped_routing_count: number;
  summary_state: string;
}

export interface OpenClawPluginFeishuCredentialProbeResult {
  ok: boolean;
  app_id: string;
  bot_name?: string | null;
  bot_open_id?: string | null;
  error?: string | null;
}

export type OpenClawLarkInstallerMode = "create" | "link";

export interface OpenClawLarkInstallerSessionStatus {
  running: boolean;
  mode?: OpenClawLarkInstallerMode | null;
  started_at?: string | null;
  last_output_at?: string | null;
  last_error?: string | null;
  prompt_hint?: string | null;
  recent_output: string[];
}

export interface WecomConnectorStatus {
  running: boolean;
  state: string;
  started_at?: string | null;
  last_error?: string | null;
  reconnect_attempts: number;
  queue_depth: number;
  instance_id: string;
}

export interface ChannelConnectorIssue {
  code: string;
  category: string;
  user_message: string;
  technical_message: string;
  retryable: boolean;
  occurred_at?: string | null;
}

export interface ChannelConnectorDescriptor {
  channel: string;
  display_name: string;
  capabilities: string[];
}

export interface ChannelConnectorHealth {
  adapter_name: string;
  instance_id: string;
  state: string;
  last_ok_at?: string | null;
  last_error?: string | null;
  reconnect_attempts: number;
  queue_depth: number;
  issue?: ChannelConnectorIssue | null;
}

export interface ChannelConnectorReplayStats {
  retained_events: number;
  acked_events: number;
}

export interface ChannelConnectorDiagnostics {
  connector: ChannelConnectorDescriptor;
  status: string;
  health: ChannelConnectorHealth;
  replay: ChannelConnectorReplayStats;
}

export interface ChannelConnectorMonitorStatus {
  running: boolean;
  generation: number;
  interval_ms: number;
  limit: number;
  total_synced: number;
  monitored_instance_id?: string | null;
  ack_status?: string | null;
  last_synced_at?: string | null;
  last_error?: string | null;
}

export interface ImChannelRestoreEntry {
  channel: string;
  host_kind: string;
  should_restore: boolean;
  restored: boolean;
  monitor_restored: boolean;
  detail: string;
  error?: string | null;
}

export interface ImChannelRestoreReport {
  feishu_runtime_restored: boolean;
  wecom_connector_restored: boolean;
  wecom_monitor_restored: boolean;
  entries: ImChannelRestoreEntry[];
}

export interface ImChannelHostActionRecord {
  channel: string;
  action: string;
  desired_running: boolean;
  ok: boolean;
  detail: string;
  error?: string | null;
  source: string;
  occurred_at: string;
}

export interface ImChannelHostRuntimeSnapshot {
  last_restore_report?: ImChannelRestoreReport | null;
  recent_actions: ImChannelHostActionRecord[];
}

export type ImChannelHostKind = "openclaw_plugin" | "connector";

export type ImChannelRegistryStatus =
  | "running"
  | "ready"
  | "degraded"
  | "stopped"
  | "not_configured";

export interface ImChannelRegistryEntry {
  channel: string;
  display_name: string;
  host_kind: ImChannelHostKind;
  status: ImChannelRegistryStatus;
  summary: string;
  detail: string;
  capabilities: string[];
  instance_id?: string | null;
  last_error?: string | null;
  plugin_host?: OpenClawPluginChannelHost | null;
  runtime_status?: OpenClawPluginFeishuRuntimeStatus | WecomConnectorStatus | null;
  diagnostics?: ChannelConnectorDiagnostics | null;
  monitor_status?: ChannelConnectorMonitorStatus | null;
  connector_settings?: Record<string, string> | null;
  automation_status?: ImChannelRestoreEntry | null;
  recent_action?: ImChannelHostActionRecord | null;
}

export interface OpenClawPluginChannelHost {
  plugin_id: string;
  npm_spec: string;
  version: string;
  channel: string;
  display_name: string;
  capabilities: string[];
  reload_config_prefixes: string[];
  target_hint?: string | null;
  docs_path?: string | null;
  status: string;
  error?: string | null;
}

export interface OpenClawPluginInstallRecord {
  plugin_id: string;
  npm_spec: string;
  version: string;
  install_path: string;
  source_type: string;
  manifest_json: string;
  installed_at: string;
  updated_at: string;
}

export interface OpenClawPluginChannelAccountSnapshot {
  accountId: string;
  account: Record<string, unknown> | null;
  describedAccount: Record<string, unknown> | null;
  allowFrom: string[];
  warnings: string[];
}

export interface OpenClawPluginChannelSnapshot {
  channelId: string;
  defaultAccountId?: string | null;
  accountIds: string[];
  accounts: OpenClawPluginChannelAccountSnapshot[];
  reloadConfigPrefixes: string[];
  targetHint?: string | null;
}

export interface OpenClawPluginChannelSnapshotResult {
  pluginRoot: string;
  preparedRoot: string;
  manifest: Record<string, unknown>;
  entryPath: string;
  snapshot: OpenClawPluginChannelSnapshot;
  logRecordCount: number;
}

export interface FeishuPairingRequestRecord {
  id: string;
  channel: string;
  account_id: string;
  sender_id: string;
  chat_id: string;
  code: string;
  status: string;
  created_at: string;
  updated_at: string;
  resolved_at?: string | null;
  resolved_by_user: string;
}

export interface FeishuEmployeeWsStatus {
  employee_id: string;
  running: boolean;
  started_at?: string | null;
  queued_events: number;
  last_event_at?: string | null;
  last_error?: string | null;
  reconnect_attempts: number;
}

export interface FeishuWsStatusSummary {
  running: boolean;
  started_at?: string | null;
  queued_events: number;
  running_count: number;
  items: FeishuEmployeeWsStatus[];
}

export interface FeishuEventRelayStatus {
  running: boolean;
  generation: number;
  interval_ms: number;
  total_accepted: number;
  last_error?: string | null;
}

export interface FeishuEmployeeConnectionStatuses {
  relay: FeishuEventRelayStatus;
  sidecar: FeishuWsStatusSummary;
}

export interface FeishuChatInfo {
  chat_id: string;
  name: string;
  description?: string;
  owner_id?: string;
}

export interface RecentImThread {
  thread_id: string;
  source: string;
  last_text_preview: string;
  last_seen_at: string;
}

export interface ThreadEmployeeBinding {
  thread_id: string;
  employee_ids: string[];
}

export interface ThreadRoleConfig {
  thread_id: string;
  tenant_id: string;
  scenario_template: string;
  status: string;
  roles: string[];
}

