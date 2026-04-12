use crate::agent::run_guard::RunStopReason;
use crate::agent::runtime::attempt_runner::RouteExecutionOutcome;
use crate::agent::runtime::compaction_pipeline::RuntimeCompactionOutcome;
use crate::agent::runtime::kernel::execution_plan::ExecutionLane;
use crate::agent::runtime::kernel::session_profile::SessionSurfaceKind;
use crate::agent::runtime::skill_routing::observability::ImplicitRouteObservation;
use crate::agent::runtime::task_state::{
    TaskBackendKind, TaskIdentity, TaskKind, TaskState, TaskSurfaceKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TurnCompactionBoundary {
    pub transcript_path: String,
    pub original_tokens: usize,
    pub compacted_tokens: usize,
    pub summary: String,
}

impl From<&RuntimeCompactionOutcome> for TurnCompactionBoundary {
    fn from(value: &RuntimeCompactionOutcome) -> Self {
        Self {
            transcript_path: value.transcript_path.to_string_lossy().to_string(),
            original_tokens: value.original_tokens,
            compacted_tokens: value.new_tokens,
            summary: value.summary.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TurnStateSnapshot {
    pub task_identity: Option<TaskIdentity>,
    pub task_kind: Option<TaskKind>,
    pub task_surface: Option<TaskSurfaceKind>,
    pub task_backend: Option<TaskBackendKind>,
    pub session_surface: Option<SessionSurfaceKind>,
    pub route_observation: Option<ImplicitRouteObservation>,
    pub execution_lane: Option<ExecutionLane>,
    pub allowed_tools: Vec<String>,
    pub invoked_skills: Vec<String>,
    pub partial_assistant_text: String,
    pub tool_failure_streak: usize,
    pub compaction_boundary: Option<TurnCompactionBoundary>,
    pub stop_reason: Option<RunStopReason>,
    pub reconstructed_history_len: Option<usize>,
}

impl TurnStateSnapshot {
    pub(crate) fn new(allowed_tools: Option<Vec<String>>) -> Self {
        Self {
            allowed_tools: allowed_tools.unwrap_or_default(),
            ..Self::default()
        }
    }

    pub(crate) fn with_session_surface(mut self, surface: SessionSurfaceKind) -> Self {
        self.session_surface = Some(surface);
        self
    }

    pub(crate) fn with_task_state(mut self, task_state: &TaskState) -> Self {
        self.task_identity = Some(task_state.task_identity.clone());
        self.task_kind = Some(task_state.task_kind);
        self.task_surface = Some(task_state.surface_kind);
        self.task_backend = Some(task_state.backend_kind);
        self
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn resolved_session_surface(&self) -> SessionSurfaceKind {
        self.session_surface.unwrap_or_default()
    }

    pub(crate) fn with_allowed_tools(mut self, allowed_tools: Option<Vec<String>>) -> Self {
        self.allowed_tools = allowed_tools.unwrap_or_default();
        self
    }

    pub(crate) fn with_route_observation(mut self, observation: ImplicitRouteObservation) -> Self {
        self.route_observation = Some(observation);
        self
    }

    pub(crate) fn with_execution_lane(mut self, lane: ExecutionLane) -> Self {
        self.execution_lane = Some(lane);
        self
    }

    pub(crate) fn with_invoked_skill(mut self, skill_id: impl Into<String>) -> Self {
        let skill_id = skill_id.into();
        if !skill_id.trim().is_empty() && !self.invoked_skills.iter().any(|id| id == &skill_id) {
            self.invoked_skills.push(skill_id);
        }
        self
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn with_partial_assistant_text(mut self, text: impl Into<String>) -> Self {
        self.partial_assistant_text = text.into();
        self
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn with_tool_failure_streak(mut self, streak: usize) -> Self {
        self.tool_failure_streak = streak;
        self
    }

    pub(crate) fn with_stop_reason(mut self, stop_reason: RunStopReason) -> Self {
        self.stop_reason = Some(stop_reason);
        self
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn with_compaction_boundary(
        mut self,
        compaction_boundary: TurnCompactionBoundary,
    ) -> Self {
        self.compaction_boundary = Some(compaction_boundary);
        self
    }

    pub(crate) fn with_reconstructed_history_len(
        mut self,
        reconstructed_history_len: usize,
    ) -> Self {
        self.reconstructed_history_len = Some(reconstructed_history_len);
        self
    }

    pub(crate) fn with_route_execution(
        mut self,
        route_execution: &RouteExecutionOutcome,
        reconstructed_history_len: usize,
    ) -> Self {
        if !route_execution.partial_text.is_empty() {
            self.partial_assistant_text = route_execution.partial_text.clone();
        }
        if let Some(stop_reason) = route_execution.last_stop_reason.clone() {
            self.stop_reason = Some(stop_reason);
        }
        if let Some(compaction_boundary) = route_execution.compaction_boundary.clone() {
            self.compaction_boundary = Some(compaction_boundary);
        }
        self.with_reconstructed_history_len(reconstructed_history_len)
    }
}

#[cfg(test)]
mod tests {
    use super::{TurnCompactionBoundary, TurnStateSnapshot};
    use crate::agent::run_guard::RunStopReason;
    use crate::agent::runtime::attempt_runner::RouteExecutionOutcome;
    use crate::agent::runtime::kernel::execution_plan::ExecutionLane;
    use crate::agent::runtime::kernel::session_profile::SessionSurfaceKind;
    use crate::agent::runtime::skill_routing::intent::RouteFallbackReason;
    use crate::agent::runtime::skill_routing::observability::ImplicitRouteObservation;
    use crate::agent::runtime::task_state::{TaskKind, TaskState, TaskSurfaceKind};

    #[test]
    fn turn_state_snapshot_keeps_route_and_tool_state_together() {
        let snapshot = TurnStateSnapshot::new(Some(vec!["read".to_string(), "exec".to_string()]))
            .with_session_surface(SessionSurfaceKind::LocalChat)
            .with_route_observation(ImplicitRouteObservation {
                route_latency_ms: 18,
                candidate_count: 2,
                selected_runner: "prompt_skill_inline".to_string(),
                selected_skill: Some("pm-summary".to_string()),
                fallback_reason: None,
                tool_recommendation_summary: None,
                tool_recommendation_aligned: None,
            })
            .with_execution_lane(ExecutionLane::PromptInline)
            .with_invoked_skill("pm-summary")
            .with_partial_assistant_text("partial summary")
            .with_tool_failure_streak(2)
            .with_stop_reason(RunStopReason::max_turns(8))
            .with_compaction_boundary(TurnCompactionBoundary {
                transcript_path: "temp/transcripts/session-1.json".to_string(),
                original_tokens: 1200,
                compacted_tokens: 320,
                summary: "summary".to_string(),
            });

        assert_eq!(
            snapshot.allowed_tools,
            vec!["read".to_string(), "exec".to_string()]
        );
        assert_eq!(
            snapshot.session_surface,
            Some(SessionSurfaceKind::LocalChat)
        );
        assert_eq!(snapshot.execution_lane, Some(ExecutionLane::PromptInline));
        assert_eq!(snapshot.invoked_skills, vec!["pm-summary".to_string()]);
        assert_eq!(snapshot.partial_assistant_text, "partial summary");
        assert_eq!(snapshot.tool_failure_streak, 2);
        assert!(snapshot.stop_reason.is_some());
        assert_eq!(
            snapshot
                .route_observation
                .as_ref()
                .and_then(|observation| observation.selected_skill.as_deref()),
            Some("pm-summary")
        );
        assert_eq!(
            snapshot
                .compaction_boundary
                .as_ref()
                .map(|boundary| boundary.original_tokens),
            Some(1200)
        );
    }

    #[test]
    fn turn_state_snapshot_preserves_open_task_fallback_route_metadata() {
        let snapshot =
            TurnStateSnapshot::default().with_route_observation(ImplicitRouteObservation {
                route_latency_ms: 7,
                candidate_count: 0,
                selected_runner: "open_task".to_string(),
                selected_skill: None,
                fallback_reason: Some(RouteFallbackReason::NoCandidates),
                tool_recommendation_summary: None,
                tool_recommendation_aligned: None,
            });

        assert_eq!(
            snapshot
                .route_observation
                .as_ref()
                .and_then(|observation| observation.fallback_reason),
            Some(RouteFallbackReason::NoCandidates)
        );
    }

    #[test]
    fn turn_state_snapshot_absorbs_compaction_boundary_from_route_execution() {
        let route_execution = RouteExecutionOutcome {
            final_messages: None,
            last_error: None,
            last_error_kind: None,
            last_stop_reason: Some(RunStopReason::max_turns(6)),
            partial_text: "partial".to_string(),
            reasoning_text: String::new(),
            reasoning_duration_ms: None,
            tool_exposure_expanded: false,
            tool_exposure_expansion_reason: None,
            compaction_boundary: Some(TurnCompactionBoundary {
                transcript_path: "temp/transcripts/route.json".to_string(),
                original_tokens: 4096,
                compacted_tokens: 1024,
                summary: "summary".to_string(),
            }),
        };

        let snapshot = TurnStateSnapshot::default().with_route_execution(&route_execution, 3);

        assert_eq!(snapshot.partial_assistant_text, "partial");
        assert_eq!(snapshot.reconstructed_history_len, Some(3));
        assert_eq!(
            snapshot
                .compaction_boundary
                .as_ref()
                .map(|boundary| boundary.original_tokens),
            Some(4096)
        );
        assert!(snapshot.stop_reason.is_some());
    }

    #[test]
    fn turn_state_snapshot_keeps_session_surface_with_lane_state() {
        let snapshot = TurnStateSnapshot::default()
            .with_session_surface(SessionSurfaceKind::EmployeeStepSession)
            .with_execution_lane(ExecutionLane::PromptFork);

        assert_eq!(
            snapshot.session_surface,
            Some(SessionSurfaceKind::EmployeeStepSession)
        );
        assert_eq!(snapshot.execution_lane, Some(ExecutionLane::PromptFork));
    }

    #[test]
    fn turn_state_snapshot_defaults_missing_surface_to_local_chat() {
        let snapshot = TurnStateSnapshot::default();

        assert_eq!(
            snapshot.resolved_session_surface(),
            SessionSurfaceKind::LocalChat
        );
    }

    #[test]
    fn turn_state_snapshot_can_attach_task_state_reference() {
        let task_state = TaskState::new_primary_local_chat("session-1", "user-1", "run-1");

        let snapshot = TurnStateSnapshot::default().with_task_state(&task_state);

        assert_eq!(
            snapshot
                .task_identity
                .as_ref()
                .map(|identity| identity.task_id.as_str()),
            Some(task_state.task_identity.task_id.as_str())
        );
        assert_eq!(snapshot.task_kind, Some(TaskKind::PrimaryUserTask));
        assert_eq!(
            snapshot.task_surface,
            Some(TaskSurfaceKind::LocalChatSurface)
        );
        assert_eq!(
            snapshot.task_backend,
            Some(crate::agent::runtime::task_state::TaskBackendKind::InteractiveChatBackend)
        );
    }
}
