use crate::agent::runtime::attempt_runner::RouteExecutionOutcome;
use crate::agent::runtime::kernel::capability_snapshot::CapabilitySnapshot;
use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::RunStopReason;
use crate::agent::runtime::runtime_io::WorkspaceSkillRuntimeEntry;
use crate::agent::runtime::skill_routing::runner::RouteRunPlan;
use crate::agent::runtime::skill_routing::index::SkillRouteIndex;
use runtime_chat_app::ChatExecutionGuidance;

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
        Self {
            lane,
            route_plan,
        }
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
    pub capability_snapshot: CapabilitySnapshot,
    pub system_prompt: String,
    pub permission_mode: PermissionMode,
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
            capability_snapshot: CapabilitySnapshot::default(),
            system_prompt: String::new(),
            permission_mode: PermissionMode::AcceptEdits,
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
}

#[derive(Debug, Clone)]
pub(crate) enum ExecutionOutcome {
    DirectDispatch(String),
    RouteExecution {
        route_execution: RouteExecutionOutcome,
        reconstructed_history_len: usize,
    },
    SkillCommandFailed(String),
    SkillCommandStopped {
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
    use super::{ExecutionContext, ExecutionLane, ExecutionPlan};
    use crate::agent::permissions::PermissionMode;
    use crate::agent::runtime::kernel::capability_snapshot::CapabilitySnapshot;
    use crate::agent::runtime::skill_routing::intent::RouteFallbackReason;
    use crate::agent::runtime::skill_routing::index::SkillRouteIndex;
    use crate::agent::runtime::skill_routing::runner::RouteRunPlan;
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
        assert!(matches!(execution_plan.route_plan, RouteRunPlan::OpenTask {
            fallback_reason: Some(RouteFallbackReason::NoCandidates)
        }));
    }

    #[test]
    fn execution_context_exposes_runtime_snapshot_contract() {
        let execution_context = ExecutionContext {
            capability_snapshot: CapabilitySnapshot {
                allowed_tools: Some(vec!["read".to_string(), "exec".to_string()]),
                resolved_tool_names: vec!["read".to_string(), "exec".to_string()],
                skill_command_specs: Vec::new(),
                runtime_notes: vec!["offline only".to_string()],
            },
            system_prompt: "Prompt".to_string(),
            permission_mode: PermissionMode::AcceptEdits,
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
            execution_context.employee_collaboration_guidance.as_deref(),
            Some("Work with employee-1")
        );
    }
}
