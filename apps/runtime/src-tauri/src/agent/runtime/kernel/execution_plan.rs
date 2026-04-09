use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::RunStopReason;
use crate::agent::runtime::attempt_runner::RouteExecutionOutcome;
use crate::agent::runtime::effective_tool_set::{
    EffectiveToolDecisionRecord, EffectiveToolPolicyInput, EffectiveToolSet,
};
use crate::agent::runtime::kernel::capability_snapshot::CapabilitySnapshot;
use crate::agent::runtime::kernel::route_lane::RouteRunPlan;
use crate::agent::runtime::kernel::session_profile::SessionExecutionProfile;
use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
use crate::agent::runtime::runtime_io::WorkspaceSkillRuntimeEntry;
use crate::agent::runtime::skill_routing::index::SkillRouteIndex;
use runtime_chat_app::ChatExecutionGuidance;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionLane {
    OpenTask,
    PromptInline,
    PromptFork,
    DirectDispatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutionPlan {
    pub lane: ExecutionLane,
    pub route_plan: RouteRunPlan,
}

impl ExecutionPlan {
    pub(crate) fn from_route_plan(route_plan: RouteRunPlan) -> Self {
        let lane = Self::lane_for_route_plan(&route_plan);
        Self { lane, route_plan }
    }

    pub(crate) fn lane_for_route_plan(route_plan: &RouteRunPlan) -> ExecutionLane {
        match route_plan {
            RouteRunPlan::OpenTask { .. } => ExecutionLane::OpenTask,
            RouteRunPlan::PromptSkillInline { .. } => ExecutionLane::PromptInline,
            RouteRunPlan::PromptSkillFork { .. } => ExecutionLane::PromptFork,
            RouteRunPlan::DirectDispatchSkill { .. } => ExecutionLane::DirectDispatch,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ExecutionContext {
    pub session_profile: SessionExecutionProfile,
    pub capability_snapshot: CapabilitySnapshot,
    pub system_prompt: String,
    pub continuation_runtime_notes: Vec<String>,
    pub permission_mode: PermissionMode,
    pub runtime_default_tool_policy: EffectiveToolPolicyInput,
    pub executor_work_dir: Option<String>,
    pub max_iterations: Option<usize>,
    pub max_call_depth: usize,
    pub node_timeout_seconds: u64,
    pub route_retry_count: usize,
    pub execution_guidance: ChatExecutionGuidance,
    pub memory_bucket_employee_id: String,
    pub employee_collaboration_guidance: Option<String>,
    pub workspace_skill_entries: Vec<WorkspaceSkillRuntimeEntry>,
    pub route_index: SkillRouteIndex,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            session_profile: SessionExecutionProfile::default(),
            capability_snapshot: CapabilitySnapshot::default(),
            system_prompt: String::new(),
            continuation_runtime_notes: Vec::new(),
            permission_mode: PermissionMode::AcceptEdits,
            runtime_default_tool_policy: EffectiveToolPolicyInput {
                source:
                    crate::agent::runtime::effective_tool_set::EffectiveToolPolicyInputSource::RuntimeDefault,
                label: "default".to_string(),
                denied_tool_names: Vec::new(),
                denied_categories: Vec::new(),
                allowed_categories: None,
                allowed_sources: None,
                denied_sources: Vec::new(),
                allowed_mcp_servers: None,
            },
            executor_work_dir: None,
            max_iterations: None,
            max_call_depth: 0,
            node_timeout_seconds: 0,
            route_retry_count: 0,
            execution_guidance: ChatExecutionGuidance {
                effective_work_dir: String::new(),
                local_timezone: String::new(),
                local_date: String::new(),
                local_tomorrow: String::new(),
                local_month_range: String::new(),
            },
            memory_bucket_employee_id: String::new(),
            employee_collaboration_guidance: None,
            workspace_skill_entries: Vec::new(),
            route_index: SkillRouteIndex::default(),
        }
    }
}

impl ExecutionContext {
    pub(crate) fn allowed_tools(&self) -> Option<&[String]> {
        self.capability_snapshot.allowed_tools.as_deref()
    }

    pub(crate) fn skill_command_specs(
        &self,
    ) -> &[crate::agent::runtime::runtime_io::WorkspaceSkillCommandSpec] {
        &self.capability_snapshot.skill_command_specs
    }

    pub(crate) fn full_allowed_tools(&self) -> Option<&[String]> {
        (!self.capability_snapshot.full_allowed_tools.is_empty())
            .then_some(self.capability_snapshot.full_allowed_tools.as_slice())
            .or_else(|| self.allowed_tools())
    }

    pub(crate) fn effective_tool_plan(&self) -> Option<&EffectiveToolSet> {
        self.capability_snapshot.effective_tool_plan.as_ref()
    }

    pub(crate) fn tool_plan_record(&self) -> Option<EffectiveToolDecisionRecord> {
        self.capability_snapshot.tool_plan_record()
    }

    pub(crate) fn has_deferred_tools(&self) -> bool {
        self.capability_snapshot.has_deferred_tools()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ContinuationKind {
    #[default]
    Standard,
    CompactionRecovery,
    HiddenChildSession,
    EmployeeStepSession,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ContinuationTurnPolicy {
    pub per_candidate_retry_count: Option<usize>,
    pub route_retry_count: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ContinuationPreference {
    pub kind: ContinuationKind,
    pub selected_skill: Option<String>,
    pub selected_runner: Option<String>,
    pub reconstructed_history_len: Option<usize>,
    pub turn_policy: ContinuationTurnPolicy,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TurnContext {
    pub user_message: String,
    pub requested_capability: String,
    pub route_candidates: Vec<(String, String, String, String, String)>,
    pub per_candidate_retry_count: usize,
    pub messages: Vec<Value>,
    pub continuation_preference: Option<ContinuationPreference>,
}

impl TurnContext {
    pub(crate) fn primary_route_candidate(
        &self,
    ) -> Option<&(String, String, String, String, String)> {
        self.route_candidates.first()
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ExecutionOutcome {
    DirectDispatch {
        output: String,
        turn_state: TurnStateSnapshot,
    },
    RouteExecution {
        route_execution: RouteExecutionOutcome,
        reconstructed_history_len: usize,
        turn_state: TurnStateSnapshot,
    },
    SkillCommandFailed {
        error: String,
        turn_state: TurnStateSnapshot,
    },
    SkillCommandStopped {
        turn_state: TurnStateSnapshot,
        stop_reason: RunStopReason,
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SessionEngineError {
    Generic(String),
}

#[cfg(test)]
mod tests {
    use super::{
        ContinuationKind, ContinuationPreference, ContinuationTurnPolicy, ExecutionContext,
        EffectiveToolPolicyInput, ExecutionLane, ExecutionPlan, TurnContext,
    };
    use crate::agent::permissions::PermissionMode;
    use crate::agent::runtime::kernel::capability_snapshot::CapabilitySnapshot;
    use crate::agent::runtime::kernel::route_lane::RouteRunPlan;
    use crate::agent::runtime::kernel::session_profile::{
        SessionExecutionProfile, SessionSurfaceKind,
    };
    use crate::agent::runtime::skill_routing::index::SkillRouteIndex;
    use crate::agent::runtime::skill_routing::intent::RouteFallbackReason;
    use runtime_chat_app::ChatExecutionGuidance;

    #[test]
    fn execution_plan_supports_all_desktop_runtime_lanes() {
        let lanes = [
            ExecutionLane::OpenTask,
            ExecutionLane::PromptInline,
            ExecutionLane::PromptFork,
            ExecutionLane::DirectDispatch,
        ];

        assert_eq!(lanes.len(), 4);
    }

    #[test]
    fn execution_plan_captures_lane_and_route_plan() {
        let route_plan = RouteRunPlan::OpenTask {
            fallback_reason: Some(RouteFallbackReason::NoCandidates),
        };

        let execution_plan = ExecutionPlan::from_route_plan(route_plan.clone());

        assert_eq!(execution_plan.lane, ExecutionLane::OpenTask);
        assert!(matches!(
            execution_plan.route_plan,
            RouteRunPlan::OpenTask {
                fallback_reason: Some(RouteFallbackReason::NoCandidates)
            }
        ));
    }

    #[test]
    fn execution_context_exposes_runtime_snapshot_contract() {
        let execution_context = ExecutionContext {
            session_profile: SessionExecutionProfile::for_surface(SessionSurfaceKind::LocalChat),
            capability_snapshot: CapabilitySnapshot {
                allowed_tools: Some(vec!["read".to_string(), "exec".to_string()]),
                resolved_tool_names: vec!["read".to_string(), "exec".to_string()],
                full_allowed_tools: vec!["read".to_string(), "exec".to_string()],
                tool_manifest: Vec::new(),
                effective_tool_plan: None,
                discovery_candidates: Vec::new(),
                skill_command_specs: Vec::new(),
                runtime_notes: vec!["offline only".to_string()],
            },
            system_prompt: "Prompt".to_string(),
            continuation_runtime_notes: vec!["resume from compacted context".to_string()],
            permission_mode: PermissionMode::AcceptEdits,
            runtime_default_tool_policy: EffectiveToolPolicyInput {
                source:
                    crate::agent::runtime::effective_tool_set::EffectiveToolPolicyInputSource::RuntimeDefault,
                label: "default".to_string(),
                denied_tool_names: Vec::new(),
                denied_categories: Vec::new(),
                allowed_categories: None,
                allowed_sources: None,
                denied_sources: Vec::new(),
                allowed_mcp_servers: None,
            },
            executor_work_dir: Some("E:/workspace/demo".to_string()),
            max_iterations: Some(12),
            max_call_depth: 4,
            node_timeout_seconds: 90,
            route_retry_count: 2,
            execution_guidance: ChatExecutionGuidance {
                effective_work_dir: "E:/workspace/demo".to_string(),
                local_timezone: "Asia/Shanghai".to_string(),
                local_date: "2026-04-07".to_string(),
                local_tomorrow: "2026-04-08".to_string(),
                local_month_range: "2026-04-01 ~ 2026-04-30".to_string(),
            },
            memory_bucket_employee_id: "employee-1".to_string(),
            employee_collaboration_guidance: Some("Work with employee-1".to_string()),
            workspace_skill_entries: Vec::new(),
            route_index: SkillRouteIndex::default(),
        };

        assert_eq!(
            execution_context.allowed_tools(),
            Some(&["read".to_string(), "exec".to_string()][..])
        );
        assert!(execution_context.skill_command_specs().is_empty());
        assert_eq!(execution_context.system_prompt, "Prompt");
        assert_eq!(
            execution_context
                .full_allowed_tools()
                .expect("full allowed tools"),
            &["read".to_string(), "exec".to_string()][..]
        );
        assert!(execution_context.effective_tool_plan().is_none());
        assert_eq!(
            execution_context.continuation_runtime_notes,
            vec!["resume from compacted context".to_string()]
        );
        assert_eq!(
            execution_context.employee_collaboration_guidance.as_deref(),
            Some("Work with employee-1")
        );
        assert_eq!(
            execution_context.session_profile.surface,
            SessionSurfaceKind::LocalChat
        );
    }

    #[test]
    fn session_execution_profile_defaults_to_local_chat_surface() {
        let profile = SessionExecutionProfile::default();

        assert_eq!(profile.surface, SessionSurfaceKind::LocalChat);
        assert_eq!(
            profile.continuation_mode,
            crate::agent::runtime::kernel::session_profile::SessionContinuationProfile::LocalChat
        );
    }

    #[test]
    fn turn_context_keeps_request_and_candidate_state_together() {
        let turn_context = TurnContext {
            user_message: "请总结今天的变更".to_string(),
            requested_capability: "chat".to_string(),
            route_candidates: vec![(
                "provider-1".to_string(),
                "openai".to_string(),
                "https://example.invalid".to_string(),
                "gpt-4.1".to_string(),
                "key".to_string(),
            )],
            per_candidate_retry_count: 2,
            messages: vec![serde_json::json!({
                "role": "user",
                "content": "请总结今天的变更"
            })],
            continuation_preference: Some(ContinuationPreference {
                kind: ContinuationKind::CompactionRecovery,
                selected_skill: Some("feishu-pm-weekly-work-summary".to_string()),
                selected_runner: Some("prompt_skill_inline".to_string()),
                reconstructed_history_len: Some(4),
                turn_policy: ContinuationTurnPolicy {
                    per_candidate_retry_count: Some(0),
                    route_retry_count: Some(0),
                },
            }),
        };

        assert_eq!(turn_context.requested_capability, "chat");
        assert_eq!(turn_context.user_message, "请总结今天的变更");
        assert_eq!(turn_context.route_candidates.len(), 1);
        assert_eq!(turn_context.per_candidate_retry_count, 2);
        assert_eq!(turn_context.messages.len(), 1);
        assert_eq!(
            turn_context
                .continuation_preference
                .as_ref()
                .and_then(|preference| preference.selected_skill.as_deref()),
            Some("feishu-pm-weekly-work-summary")
        );
        assert_eq!(
            turn_context
                .continuation_preference
                .as_ref()
                .and_then(|preference| preference.turn_policy.route_retry_count),
            Some(0)
        );
        let primary = turn_context
            .primary_route_candidate()
            .expect("primary route candidate");
        assert_eq!(primary.0, "provider-1");
        assert_eq!(primary.3, "gpt-4.1");
    }
}
