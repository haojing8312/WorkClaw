use super::intent::RouteFallbackReason;
use crate::agent::runtime::effective_tool_set::{EffectiveToolDecisionRecord, ToolLoadingPolicy};
use crate::agent::runtime::kernel::execution_plan::ExecutionPlan;
use crate::agent::runtime::kernel::route_lane::{RouteRunPlan, RoutedSkillToolSetup};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImplicitRouteObservation {
    pub route_latency_ms: u64,
    pub candidate_count: usize,
    pub selected_runner: String,
    pub selected_skill: Option<String>,
    pub fallback_reason: Option<RouteFallbackReason>,
    pub tool_recommendation_summary: Option<String>,
    pub tool_recommendation_aligned: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedImplicitRoute {
    pub execution_plan: ExecutionPlan,
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
        tool_recommendation_summary: None,
        tool_recommendation_aligned: None,
    }
}

pub(crate) fn attach_tool_recommendation_observation(
    mut observation: ImplicitRouteObservation,
    plan: &EffectiveToolDecisionRecord,
) -> ImplicitRouteObservation {
    if plan.recommended_tool_count == 0 {
        return observation;
    }

    let recommendation_sample = plan
        .recommended_tools
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let supporting_sample = plan
        .supporting_tools
        .iter()
        .take(2)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let loading_label = match plan.loading_policy {
        ToolLoadingPolicy::Full => "full",
        ToolLoadingPolicy::RecommendedOnly => "recommended_only",
        ToolLoadingPolicy::RecommendedPlusCoreSafeTools => "recommended_plus_core_safe_tools",
    };
    observation.tool_recommendation_summary = Some(format!(
        "tool_recommendation={} supporting={} active={} deferred={} loading_policy={}",
        recommendation_sample,
        supporting_sample,
        plan.active_tool_count,
        plan.deferred_tool_count,
        loading_label
    ));
    observation.tool_recommendation_aligned =
        Some(observation.selected_runner != "open_task" || plan.expanded_to_full);
    observation
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

    fn build_setup(skill_id: &str) -> RoutedSkillToolSetup {
        RoutedSkillToolSetup {
            skill_id: skill_id.to_string(),
            skill_system_prompt: "prompt".to_string(),
            skill_allowed_tools: Some(vec!["read_file".to_string()]),
            skill_denied_tools: None,
            skill_allowed_tool_sources: None,
            skill_denied_tool_sources: None,
            skill_allowed_tool_categories: None,
            skill_denied_tool_categories: None,
            skill_allowed_mcp_servers: None,
            tool_profile: None,
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
        assert!(observation.tool_recommendation_summary.is_none());
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

    #[test]
    fn attach_tool_recommendation_observation_adds_summary() {
        let observation = build_implicit_route_observation(
            &RouteRunPlan::PromptSkillInline {
                skill_id: "repo-skill".to_string(),
                setup: build_setup("repo-skill"),
            },
            2,
            15,
        );

        let enriched = attach_tool_recommendation_observation(
            observation,
            &EffectiveToolDecisionRecord {
                source: crate::agent::runtime::effective_tool_set::EffectiveToolSetSource::ExplicitAllowList,
                allowed_tool_count: 4,
                active_tool_count: 2,
                recommended_tool_count: 2,
                supporting_tool_count: 1,
                deferred_tool_count: 2,
                excluded_tool_count: 0,
                active_tools: vec!["read_file".to_string(), "web_search".to_string()],
                recommended_tools: vec!["web_search".to_string(), "web_fetch".to_string()],
                supporting_tools: vec!["read_file".to_string()],
                deferred_tools: vec!["bash".to_string(), "edit".to_string()],
                missing_tools: Vec::new(),
                filtered_out_tools: Vec::new(),
                excluded_tools: Vec::new(),
                source_counts: Vec::new(),
                exclusion_counts: Vec::new(),
                policy: crate::agent::runtime::effective_tool_set::EffectiveToolPolicySummary {
                    denied_tool_names: Vec::new(),
                    denied_categories: Vec::new(),
                    allowed_categories: None,
                    allowed_sources: None,
                    denied_sources: Vec::new(),
                    allowed_mcp_servers: None,
                    inputs: Vec::new(),
                },
                loading_policy: ToolLoadingPolicy::RecommendedPlusCoreSafeTools,
                expanded_to_full: false,
                expansion_reason: None,
                discovery_candidates: Vec::new(),
            },
        );

        assert!(enriched
            .tool_recommendation_summary
            .as_deref()
            .unwrap_or_default()
            .contains("web_search"));
        assert_eq!(enriched.tool_recommendation_aligned, Some(true));
    }
}
