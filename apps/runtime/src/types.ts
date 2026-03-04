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
  tags: string[];
  stars: number;
  downloads: number;
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
  role: "user" | "assistant" | "system";
  content: string;
  created_at: string;
  toolCalls?: ToolCallInfo[];
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

export interface SessionInfo {
  id: string;
  title: string;
  created_at: string;
  model_id: string;
  work_dir?: string;
  permission_mode?: "default" | "accept_edits" | "unrestricted";
  permission_mode_label?: string;
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
  status: "running" | "completed" | "failed" | string;
  summary?: string;
  duration_ms?: number;
}

export interface ImRoleDispatchRequest {
  session_id: string;
  thread_id: string;
  role_id: string;
  role_name: string;
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

export interface FeishuWsStatus {
  running: boolean;
  started_at?: string | null;
  queued_events: number;
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

export interface ThreadEmployeeBinding {
  thread_id: string;
  employee_ids: string[];
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
}

/// 文件附件（用于 File Upload 功能）
export interface FileAttachment {
  name: string;
  size: number;
  type: string;
  content: string; // 文件文本内容或 base64
}
