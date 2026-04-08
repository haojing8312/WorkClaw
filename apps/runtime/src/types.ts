export interface SkillManifest {
  id: string;
  name: string;
  description: string;
  version: string;
  author: string;
  recommended_model: string;
  tags: string[];
  created_at: string;
  username_hint?: string;
  source_type?: string;
}

export interface ClawhubSkillSummary {
  name: string;
  slug: string;
  description: string;
  github_url?: string | null;
  source_url?: string | null;
  stars: number;
}

export interface ClawhubLibraryItem {
  slug: string;
  name: string;
  summary: string;
  github_url?: string | null;
  source_url?: string | null;
  tags: string[];
  stars: number;
  downloads: number;
}

export interface ClawhubLibraryResponse {
  items: ClawhubLibraryItem[];
  next_cursor?: string | null;
  last_synced_at?: string | null;
}

export interface SkillhubCatalogSyncStatus {
  total_skills: number;
  last_synced_at?: string | null;
  refreshed: boolean;
}

export interface ClawhubSkillDetail {
  slug: string;
  name: string;
  summary: string;
  description: string;
  author?: string | null;
  github_url?: string | null;
  source_url?: string | null;
  updated_at?: string | null;
  stars: number;
  downloads: number;
  tags: string[];
  readme?: string | null;
}

export interface ClawhubSkillRecommendation {
  slug: string;
  name: string;
  description: string;
  stars: number;
  score: number;
  reason: string;
  github_url?: string | null;
  source_url?: string | null;
}

export interface ClawhubInstallRequest {
  slug: string;
  githubUrl?: string | null;
  sourceUrl?: string | null;
}

export interface ModelConfig {
  id: string;
  name: string;
  api_format: string;
  base_url: string;
  model_name: string;
  is_default: boolean;
}

export interface ProviderConfig {
  id: string;
  provider_key: string;
  display_name: string;
  protocol_type: string;
  base_url: string;
  auth_type: string;
  api_key_encrypted: string;
  org_id: string;
  extra_json: string;
  enabled: boolean;
}

export interface ProviderPluginInfo {
  key: string;
  display_name: string;
  capabilities: string[];
}

export interface ChatRoutingPolicy {
  primary_provider_id: string;
  primary_model: string;
  fallback_chain_json: string;
  timeout_ms: number;
  retry_count: number;
  enabled: boolean;
}

export interface CapabilityRoutingPolicy {
  capability: string;
  primary_provider_id: string;
  primary_model: string;
  fallback_chain_json: string;
  timeout_ms: number;
  retry_count: number;
  enabled: boolean;
}

export interface ProviderHealthInfo {
  provider_id: string;
  ok: boolean;
  protocol_type: string;
  message: string;
}

export type ModelErrorKind = "billing" | "auth" | "rate_limit" | "timeout" | "network" | "unknown";

export interface ModelConnectionTestResult {
  ok: boolean;
  kind: ModelErrorKind;
  title: string;
  message: string;
  raw_message?: string | null;
}

export interface RouteAttemptLog {
  session_id: string;
  capability: string;
  api_format: string;
  model_name: string;
  attempt_index: number;
  retry_index: number;
  error_kind: string;
  success: boolean;
  error_message: string;
  created_at: string;
}

export interface RouteAttemptStat {
  capability: string;
  error_kind: string;
  success: boolean;
  count: number;
}

export interface CapabilityRouteTemplateInfo {
  template_id: string;
  name: string;
  description: string;
  capability: string;
}

export interface FrontMatter {
  name?: string;
  description?: string;
  version?: string;
  model?: string;
}

export interface SkillDirInfo {
  files: string[];
  front_matter: FrontMatter;
}

/// 有序的流式输出项：文字和工具调用按发生顺序排列
export interface StreamItem {
  type: "text" | "tool_call";
  content?: string;          // type === "text" 时的文字内容
  toolCall?: ToolCallInfo;   // type === "tool_call" 时的工具信息
}

export interface Message {
  id?: string;
  role: "user" | "assistant" | "system";
  content: string;
  contentParts?: ChatMessagePart[];
  created_at: string;
  runId?: string | null;
  toolCalls?: ToolCallInfo[];
  reasoning?: {
    status: "thinking" | "completed" | "interrupted";
    duration_ms?: number;
    content: string;
  };
  /// 有序的展示项（新格式），优先使用此字段渲染
  streamItems?: StreamItem[];
}

export interface ToolCallInfo {
  id: string;
  name: string;
  input: Record<string, unknown>;
  output?: string;
  status: "running" | "completed" | "error";
}

export interface ChatRuntimeAgentState {
  state: string;
  detail?: string;
  iteration: number;
  stopReasonKind?: string;
  stopReasonTitle?: string;
  stopReasonMessage?: string;
  stopReasonLastCompletedStep?: string;
}

export interface ChatDelegationCardState {
  id: string;
  fromRole: string;
  toRole: string;
  status: "running" | "completed" | "failed";
  taskId?: string;
}

export interface PersistedChatRuntimeState {
  streaming: boolean;
  streamItems: StreamItem[];
  streamReasoning: {
    status: "thinking" | "completed" | "interrupted";
    content: string;
    durationMs?: number;
  } | null;
  agentState: ChatRuntimeAgentState | null;
  subAgentBuffer: string;
  subAgentRoleName: string;
  mainRoleName: string;
  mainSummaryDelivered: boolean;
  delegationCards: ChatDelegationCardState[];
}

export interface SessionRunProjection {
  id: string;
  session_id: string;
  user_message_id: string;
  assistant_message_id?: string | null;
  status: string;
  buffered_text: string;
  error_kind?: string | null;
  error_message?: string | null;
  created_at: string;
  updated_at: string;
  turn_state?: SessionRunTurnStateSnapshot | null;
}

export interface SessionRunTurnStateCompactionBoundary {
  transcript_path: string;
  original_tokens: number;
  compacted_tokens: number;
  summary: string;
}

export interface SessionRunTurnStateSnapshot {
  execution_lane?: string | null;
  selected_runner?: string | null;
  selected_skill?: string | null;
  fallback_reason?: string | null;
  allowed_tools?: string[];
  invoked_skills?: string[];
  partial_assistant_text?: string;
  tool_failure_streak?: number;
  reconstructed_history_len?: number | null;
  compaction_boundary?: SessionRunTurnStateCompactionBoundary | null;
}

export interface SessionInfo {
  id: string;
  skill_id?: string;
  title: string;
  display_title?: string;
  created_at: string;
  model_id: string;
  work_dir?: string;
  employee_id?: string;
  employee_name?: string;
  optimistic?: boolean;
  permission_mode?: "standard" | "full_access" | "default" | "accept_edits" | "unrestricted";
  session_mode?: "general" | "employee_direct" | "team_entry";
  team_id?: string;
  permission_mode_label?: string;
  source_channel?: "local" | "app" | "feishu" | "wecom" | string;
  source_label?: string;
  runtime_status?: "running" | "waiting_approval" | "completed" | "failed" | string | null;
}

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
}

export interface FeishuPluginEnvironmentStatus {
  node_available: boolean;
  npm_available: boolean;
  node_version?: string | null;
  npm_version?: string | null;
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

export interface AgentEmployee {
  id: string;
  employee_id: string;
  name: string;
  role_id: string;
  persona: string;
  feishu_open_id: string;
  feishu_app_id: string;
  feishu_app_secret: string;
  primary_skill_id: string;
  default_work_dir: string;
  openclaw_agent_id: string;
  routing_priority: number;
  enabled_scopes: string[];
  enabled: boolean;
  is_default: boolean;
  skill_ids: string[];
  created_at: string;
  updated_at: string;
}

export interface EmployeeGroup {
  id: string;
  name: string;
  coordinator_employee_id: string;
  member_employee_ids: string[];
  member_count: number;
  template_id: string;
  entry_employee_id: string;
  review_mode: string;
  execution_mode: string;
  visibility_mode: string;
  is_bootstrap_seeded: boolean;
  config_json: string;
  created_at: string;
  updated_at: string;
}

export interface EmployeeGroupRule {
  id: string;
  group_id: string;
  from_employee_id: string;
  to_employee_id: string;
  relation_type: string;
  phase_scope: string;
  required: boolean;
  priority: number;
  created_at: string;
}

export interface EmployeeGroupRunStep {
  id: string;
  round_no: number;
  step_type: string;
  assignee_employee_id: string;
  dispatch_source_employee_id?: string;
  session_id?: string;
  attempt_no?: number;
  status: "running" | "completed" | "failed" | string;
  output_summary?: string;
  output: string;
}

export interface EmployeeGroupRunResult {
  run_id: string;
  group_id: string;
  session_id: string;
  session_skill_id: string;
  state: string;
  current_round: number;
  final_report: string;
  steps: EmployeeGroupRunStep[];
}

export interface EmployeeGroupRunSummary {
  id: string;
  group_id: string;
  group_name: string;
  goal: string;
  status: string;
  started_at: string;
  finished_at: string;
  session_id: string;
  session_skill_id: string;
}

export interface EmployeeGroupRunSnapshot {
  run_id: string;
  group_id: string;
  session_id: string;
  state: string;
  current_round: number;
  current_phase: string;
  review_round: number;
  status_reason: string;
  waiting_for_employee_id: string;
  waiting_for_user: boolean;
  final_report: string;
  steps: EmployeeGroupRunStep[];
  events: EmployeeGroupRunEvent[];
}

export interface EmployeeGroupRunEvent {
  id: string;
  step_id: string;
  event_type: string;
  payload_json: string;
  created_at: string;
}

export interface EmployeeMemorySkillStats {
  skill_id: string;
  total_files: number;
  total_bytes: number;
}

export interface EmployeeMemoryStats {
  employee_id: string;
  total_files: number;
  total_bytes: number;
  skills: EmployeeMemorySkillStats[];
}

export interface EmployeeMemoryExportFile {
  skill_id: string;
  relative_path: string;
  size_bytes: number;
  modified_at?: string | null;
  content: string;
}

export interface EmployeeMemoryExport {
  employee_id: string;
  skill_id?: string | null;
  exported_at: string;
  total_files: number;
  total_bytes: number;
  files: EmployeeMemoryExportFile[];
}

export interface UpsertAgentEmployeeInput {
  id?: string;
  employee_id: string;
  name: string;
  role_id: string;
  persona: string;
  feishu_open_id: string;
  feishu_app_id: string;
  feishu_app_secret: string;
  primary_skill_id: string;
  default_work_dir: string;
  openclaw_agent_id: string;
  routing_priority: number;
  enabled_scopes: string[];
  enabled: boolean;
  is_default: boolean;
  skill_ids: string[];
}

export interface SaveFeishuEmployeeAssociationInput {
  employee_db_id: string;
  enabled: boolean;
  mode: "default" | "scoped";
  peer_kind: "group" | "channel" | "direct";
  peer_id: string;
  priority: number;
}

export interface AgentProfileAnswerInput {
  key: string;
  question: string;
  answer: string;
}

export interface AgentProfilePayload {
  employee_db_id: string;
  answers: AgentProfileAnswerInput[];
}

export interface AgentProfileDraft {
  employee_id: string;
  employee_name: string;
  agents_md: string;
  soul_md: string;
  user_md: string;
}

export interface AgentProfileFileResult {
  path: string;
  ok: boolean;
  error?: string | null;
}

export interface ApplyAgentProfileResult {
  files: AgentProfileFileResult[];
}

export interface AgentProfileFileView {
  name: string;
  path: string;
  exists: boolean;
  content: string;
  error?: string | null;
}

export interface AgentProfileFilesView {
  employee_id: string;
  employee_name: string;
  profile_dir: string;
  files: AgentProfileFileView[];
}

export interface ImRoutingBinding {
  id: string;
  agent_id: string;
  channel: string;
  account_id: string;
  peer_kind: string;
  peer_id: string;
  guild_id: string;
  team_id: string;
  role_ids: string[];
  connector_meta?: Record<string, unknown>;
  priority: number;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface UpsertImRoutingBindingInput {
  id?: string;
  agent_id: string;
  channel: string;
  account_id: string;
  peer_kind: string;
  peer_id: string;
  guild_id: string;
  team_id: string;
  role_ids: string[];
  connector_meta?: Record<string, unknown>;
  priority: number;
  enabled: boolean;
}

export interface ImRouteSimulationPayload {
  channel: string;
  account_id: string;
  peer: {
    kind: "group" | "channel" | "direct";
    id: string;
  };
  default_agent_id: string;
  bindings: Array<{
    agentId: string;
    match: {
      channel: string;
      accountId?: string;
      peer?: { kind: "group" | "channel" | "direct"; id: string };
      guildId?: string;
      teamId?: string;
      roles?: string[];
    };
  }>;
}

export interface RuntimePreferences {
  default_work_dir: string;
  default_language: string;
  immersive_translation_enabled: boolean;
  immersive_translation_display: "translated_only" | "bilingual_inline" | string;
  immersive_translation_trigger: "auto" | "manual" | string;
  translation_engine: "model_then_free" | "model_only" | "free_only" | string;
  translation_model_id: string;
  launch_at_login: boolean;
  launch_minimized: boolean;
  close_to_tray: boolean;
  operation_permission_mode: "standard" | "full_access" | string;
}

export type PendingAttachment =
  | {
      id: string;
      kind: "image";
      name: string;
      mimeType: string;
      size: number;
      data: string;
      previewUrl: string;
    }
  | {
      id: string;
      kind: "text-file";
      name: string;
      mimeType: string;
      size: number;
      text: string;
      truncated?: boolean;
    }
  | {
      id: string;
      kind: "pdf-file";
      name: string;
      mimeType: string;
      size: number;
      data: string;
      extractedText?: string;
      truncated?: boolean;
    };

export interface LandingSessionLaunchInput {
  initialMessage: string;
  attachments: PendingAttachment[];
  workDir: string;
}

export type ChatMessagePart =
  | { type: "text"; text: string }
  | {
      type: "image";
      name: string;
      mimeType: string;
      size: number;
      data: string;
    }
  | {
      type: "file_text";
      name: string;
      mimeType: string;
      size: number;
      text: string;
      truncated?: boolean;
    }
  | {
      type: "pdf_file";
      name: string;
      mimeType: string;
      size: number;
      data?: string;
      extractedText?: string;
      truncated?: boolean;
    };

export interface SendMessageRequest {
  sessionId: string;
  parts: ChatMessagePart[];
  maxIterations?: number;
}

/// 兼容旧附件实现，待迁移到 PendingAttachment/ChatMessagePart。
export interface FileAttachment {
  name: string;
  size: number;
  type: string;
  content: string;
}
