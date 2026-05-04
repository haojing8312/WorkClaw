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

