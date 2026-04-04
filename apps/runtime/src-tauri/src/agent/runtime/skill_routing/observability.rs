use super::intent::RouteFallbackReason;
use super::runner::RouteRunPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImplicitRouteObservation {
    pub route_latency_ms: u64,
    pub candidate_count: usize,
    pub selected_runner: String,
    pub selected_skill: Option<String>,
    pub fallback_reason: Option<RouteFallbackReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedImplicitRoute {
    pub route_plan: RouteRunPlan,
    pub observation: ImplicitRouteObservation,
}

pub(crate) fn build_implicit_route_observation(
    route_plan: &RouteRunPlan,
    candidate_count: usize,
    route_latency_ms: u64,
) -> ImplicitRouteObservation {
    let (selected_runner, selected_skill, fallback_reason) = match route_plan {
        RouteRunPlan::OpenTask { fallback_reason } => {
            ("open_task".to_string(), None, *fallback_reason)
        }
        RouteRunPlan::PromptSkillInline { skill_id, .. } => (
            "prompt_skill_inline".to_string(),
            Some(skill_id.clone()),
            None,
        ),
        RouteRunPlan::PromptSkillFork { skill_id, .. } => (
            "prompt_skill_fork".to_string(),
            Some(skill_id.clone()),
            None,
        ),
        RouteRunPlan::DirectDispatchSkill { skill_id, .. } => (
            "direct_dispatch_skill".to_string(),
            Some(skill_id.clone()),
            None,
        ),
    };

    ImplicitRouteObservation {
        route_latency_ms,
        candidate_count,
        selected_runner,
        selected_skill,
        fallback_reason,
    }
}

pub(crate) fn route_fallback_reason_key(reason: RouteFallbackReason) -> &'static str {
    match reason {
        RouteFallbackReason::ExplicitOpenTask => "explicit_open_task",
        RouteFallbackReason::NoCandidates => "no_candidates",
        RouteFallbackReason::AmbiguousCandidates => "ambiguous_candidates",
        RouteFallbackReason::InvalidSkillContract => "invalid_skill_contract",
        RouteFallbackReason::DispatchArgumentResolutionFailed => {
            "dispatch_argument_resolution_failed"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::runtime::runtime_io::WorkspaceSkillCommandSpec;
    use crate::agent::runtime::skill_routing::intent::RouteFallbackReason;
    use crate::agent::runtime::skill_routing::runner::RoutedSkillToolSetup;

    fn build_setup(skill_id: &str) -> RoutedSkillToolSetup {
        RoutedSkillToolSetup {
            skill_id: skill_id.to_string(),
            skill_system_prompt: "prompt".to_string(),
            skill_allowed_tools: Some(vec!["read_file".to_string()]),
            max_iterations: Some(4),
            source_type: "local".to_string(),
            pack_path: String::new(),
        }
    }

    #[test]
    fn build_observation_captures_open_task_fallback_metadata() {
        let observation = build_implicit_route_observation(
            &RouteRunPlan::OpenTask {
                fallback_reason: Some(RouteFallbackReason::AmbiguousCandidates),
            },
            3,
            17,
        );

        assert_eq!(observation.route_latency_ms, 17);
        assert_eq!(observation.candidate_count, 3);
        assert_eq!(observation.selected_runner, "open_task");
        assert_eq!(observation.selected_skill, None);
        assert_eq!(
            observation.fallback_reason,
            Some(RouteFallbackReason::AmbiguousCandidates)
        );
    }

    #[test]
    fn build_observation_captures_direct_dispatch_metadata() {
        let observation = build_implicit_route_observation(
            &RouteRunPlan::DirectDispatchSkill {
                skill_id: "feishu-pm-task-dispatch".to_string(),
                setup: build_setup("feishu-pm-task-dispatch"),
                command_spec: WorkspaceSkillCommandSpec {
                    name: "pm_task_dispatch".to_string(),
                    skill_id: "feishu-pm-task-dispatch".to_string(),
                    skill_name: "PM Task Dispatch".to_string(),
                    description: "dispatch".to_string(),
                    dispatch: None,
                },
                raw_args: "--employee 郝敬".to_string(),
            },
            2,
            9,
        );

        assert_eq!(observation.route_latency_ms, 9);
        assert_eq!(observation.candidate_count, 2);
        assert_eq!(observation.selected_runner, "direct_dispatch_skill");
        assert_eq!(
            observation.selected_skill.as_deref(),
            Some("feishu-pm-task-dispatch")
        );
        assert_eq!(observation.fallback_reason, None);
    }
}
