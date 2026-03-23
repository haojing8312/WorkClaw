#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AgentEmployee {
    pub id: String,
    pub employee_id: String,
    pub name: String,
    pub role_id: String,
    pub persona: String,
    pub feishu_open_id: String,
    pub feishu_app_id: String,
    pub feishu_app_secret: String,
    pub primary_skill_id: String,
    pub default_work_dir: String,
    pub openclaw_agent_id: String,
    pub routing_priority: i64,
    pub enabled_scopes: Vec<String>,
    pub enabled: bool,
    pub is_default: bool,
    pub skill_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct UpsertAgentEmployeeInput {
    pub id: Option<String>,
    #[serde(default)]
    pub employee_id: String,
    pub name: String,
    pub role_id: String,
    pub persona: String,
    pub feishu_open_id: String,
    pub feishu_app_id: String,
    pub feishu_app_secret: String,
    pub primary_skill_id: String,
    pub default_work_dir: String,
    pub openclaw_agent_id: String,
    #[serde(default = "default_routing_priority")]
    pub routing_priority: i64,
    pub enabled_scopes: Vec<String>,
    pub enabled: bool,
    pub is_default: bool,
    pub skill_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SaveFeishuEmployeeAssociationInput {
    pub employee_db_id: String,
    pub enabled: bool,
    pub mode: String,
    pub peer_kind: String,
    pub peer_id: String,
    pub priority: i64,
}

fn default_routing_priority() -> i64 {
    100
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EnsuredEmployeeSession {
    pub employee_id: String,
    pub role_id: String,
    pub employee_name: String,
    pub session_id: String,
    pub created: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeInboundDispatchSession {
    pub session_id: String,
    pub thread_id: String,
    pub employee_id: String,
    pub role_id: String,
    pub employee_name: String,
    pub route_agent_id: String,
    pub route_session_key: String,
    pub matched_by: String,
    pub prompt: String,
    pub message_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeGroup {
    pub id: String,
    pub name: String,
    pub coordinator_employee_id: String,
    pub member_employee_ids: Vec<String>,
    pub member_count: i64,
    pub template_id: String,
    pub entry_employee_id: String,
    pub review_mode: String,
    pub execution_mode: String,
    pub visibility_mode: String,
    pub is_bootstrap_seeded: bool,
    pub config_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeGroupRule {
    pub id: String,
    pub group_id: String,
    pub from_employee_id: String,
    pub to_employee_id: String,
    pub relation_type: String,
    pub phase_scope: String,
    pub required: bool,
    pub priority: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CreateEmployeeGroupInput {
    pub name: String,
    pub coordinator_employee_id: String,
    pub member_employee_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CreateEmployeeTeamRuleInput {
    pub from_employee_id: String,
    pub to_employee_id: String,
    pub relation_type: String,
    pub phase_scope: String,
    pub required: bool,
    pub priority: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CreateEmployeeTeamInput {
    pub name: String,
    pub coordinator_employee_id: String,
    pub member_employee_ids: Vec<String>,
    #[serde(default)]
    pub entry_employee_id: String,
    #[serde(default)]
    pub planner_employee_id: String,
    #[serde(default)]
    pub reviewer_employee_id: String,
    #[serde(default = "default_team_review_mode")]
    pub review_mode: String,
    #[serde(default = "default_team_execution_mode")]
    pub execution_mode: String,
    #[serde(default = "default_team_visibility_mode")]
    pub visibility_mode: String,
    #[serde(default)]
    pub rules: Vec<CreateEmployeeTeamRuleInput>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CloneEmployeeGroupTemplateInput {
    pub source_group_id: String,
    pub name: String,
}

pub(super) fn default_group_execution_window() -> usize {
    3
}

pub(super) fn default_group_max_retry() -> usize {
    1
}

fn default_team_review_mode() -> String {
    "none".to_string()
}

fn default_team_execution_mode() -> String {
    "sequential".to_string()
}

fn default_team_visibility_mode() -> String {
    "internal".to_string()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StartEmployeeGroupRunInput {
    pub group_id: String,
    pub user_goal: String,
    #[serde(default = "default_group_execution_window")]
    pub execution_window: usize,
    #[serde(default)]
    pub timeout_employee_ids: Vec<String>,
    #[serde(default = "default_group_max_retry")]
    pub max_retry_per_step: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeGroupRunStep {
    pub id: String,
    pub round_no: i64,
    pub step_type: String,
    pub assignee_employee_id: String,
    pub dispatch_source_employee_id: String,
    pub session_id: String,
    pub attempt_no: i64,
    pub status: String,
    pub output_summary: String,
    pub output: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeGroupRunResult {
    pub run_id: String,
    pub group_id: String,
    pub session_id: String,
    pub session_skill_id: String,
    pub state: String,
    pub current_round: i64,
    pub final_report: String,
    pub steps: Vec<EmployeeGroupRunStep>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeGroupRunSummary {
    pub id: String,
    pub group_id: String,
    pub group_name: String,
    pub goal: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: String,
    pub session_id: String,
    pub session_skill_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeGroupRunSnapshot {
    pub run_id: String,
    pub group_id: String,
    pub session_id: String,
    pub state: String,
    pub current_round: i64,
    pub current_phase: String,
    pub review_round: i64,
    pub status_reason: String,
    pub waiting_for_employee_id: String,
    pub waiting_for_user: bool,
    pub final_report: String,
    pub steps: Vec<EmployeeGroupRunStep>,
    pub events: Vec<EmployeeGroupRunEvent>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeGroupRunEvent {
    pub id: String,
    pub step_id: String,
    pub event_type: String,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GroupStepExecutionResult {
    pub step_id: String,
    pub run_id: String,
    pub assignee_employee_id: String,
    pub session_id: String,
    pub status: String,
    pub output: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeMemorySkillStats {
    pub skill_id: String,
    pub total_files: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeMemoryStats {
    pub employee_id: String,
    pub total_files: u64,
    pub total_bytes: u64,
    pub skills: Vec<EmployeeMemorySkillStats>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeMemoryExportFile {
    pub skill_id: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EmployeeMemoryExport {
    pub employee_id: String,
    pub skill_id: Option<String>,
    pub exported_at: String,
    pub total_files: u64,
    pub total_bytes: u64,
    pub files: Vec<EmployeeMemoryExportFile>,
}
