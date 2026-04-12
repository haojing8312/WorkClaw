use super::types::EmployeeGroupRule;
use crate::agent::group_orchestrator::GroupRunExecuteTarget;
use crate::employee_runtime_adapter::delegation_policy::{
    resolve_delegation_targets, DelegationKind,
};
use crate::employee_runtime_adapter::team_topology::{
    normalize_member_employee_ids as normalize_runtime_member_employee_ids,
    resolve_planner_employee_id, resolve_reviewer_employee_id, NormalizedTeamRule,
};

pub(super) fn normalize_member_employee_ids(raw: &[String]) -> Vec<String> {
    normalize_runtime_member_employee_ids(raw)
}

pub(super) fn group_rule_matches_relation_types(
    rule: &EmployeeGroupRule,
    relation_types: &[&str],
) -> bool {
    let normalized_relation_type = rule.relation_type.trim().to_lowercase();
    relation_types
        .iter()
        .any(|relation_type| normalized_relation_type == *relation_type)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn resolve_group_planner_employee_id(
    entry_employee_id: &str,
    coordinator_employee_id: &str,
    rules: &[EmployeeGroupRule],
) -> String {
    resolve_planner_employee_id(
        entry_employee_id,
        coordinator_employee_id,
        &normalize_team_rules(rules),
    )
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn resolve_group_reviewer_employee_id(
    review_mode: &str,
    planner_employee_id: &str,
    rules: &[EmployeeGroupRule],
) -> Option<String> {
    resolve_reviewer_employee_id(review_mode, planner_employee_id, &normalize_team_rules(rules))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn select_group_execute_dispatch_targets(
    rules: &[EmployeeGroupRule],
    member_employee_ids: &[String],
    preferred_dispatch_sources: &[String],
) -> (Vec<GroupRunExecuteTarget>, bool) {
    let normalized_rules = normalize_team_rules(rules);
    let has_execute_rules = normalized_rules
        .iter()
        .any(|rule| group_rule_allows_execute_reassignment_from_normalized(rule));
    if !has_execute_rules {
        return (Vec::new(), false);
    }

    let policy = resolve_delegation_targets(
        preferred_dispatch_sources,
        member_employee_ids,
        &normalized_rules,
    );

    (
        policy
            .targets
            .into_iter()
            .filter(|target| matches!(target.kind, DelegationKind::DispatchToOther))
            .map(|target| GroupRunExecuteTarget {
                dispatch_source_employee_id: target.source_employee_id,
                assignee_employee_id: target.target_employee_id,
            })
            .collect(),
        true,
    )
}

#[cfg_attr(not(test), allow(dead_code))]
fn normalize_team_rules(rules: &[EmployeeGroupRule]) -> Vec<NormalizedTeamRule> {
    rules.iter()
        .map(|rule| NormalizedTeamRule {
            from_employee_id: rule.from_employee_id.clone(),
            to_employee_id: rule.to_employee_id.clone(),
            relation_type: rule.relation_type.clone(),
            phase_scope: rule.phase_scope.clone(),
        })
        .collect()
}

#[cfg_attr(not(test), allow(dead_code))]
fn group_rule_allows_execute_reassignment_from_normalized(rule: &NormalizedTeamRule) -> bool {
    let relation_type = rule.relation_type.trim().to_lowercase();
    let phase_scope = rule.phase_scope.trim().to_lowercase();
    let relation_allowed = relation_type == "delegate" || relation_type == "handoff";
    let phase_allowed = phase_scope.is_empty()
        || phase_scope == "execute"
        || phase_scope == "all"
        || phase_scope == "*";
    relation_allowed && phase_allowed
}

#[cfg(test)]
mod tests {
    use super::{normalize_member_employee_ids, select_group_execute_dispatch_targets};
    use crate::commands::employee_agents::EmployeeGroupRule;

    fn build_rule(
        from_employee_id: &str,
        to_employee_id: &str,
        relation_type: &str,
        phase_scope: &str,
    ) -> EmployeeGroupRule {
        EmployeeGroupRule {
            id: "rule-1".to_string(),
            group_id: "group-1".to_string(),
            from_employee_id: from_employee_id.to_string(),
            to_employee_id: to_employee_id.to_string(),
            relation_type: relation_type.to_string(),
            phase_scope: phase_scope.to_string(),
            required: false,
            priority: 100,
            created_at: "2026-03-23T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn normalize_member_employee_ids_trims_dedupes_and_lowercases() {
        assert_eq!(
            normalize_member_employee_ids(&[
                " Alice ".to_string(),
                "alice".to_string(),
                String::new(),
                "BOB".to_string(),
            ]),
            vec!["alice".to_string(), "bob".to_string()]
        );
    }

    #[test]
    fn select_group_execute_dispatch_targets_prefers_matching_dispatch_source() {
        let rules = vec![
            build_rule("planner", "worker-a", "delegate", "execute"),
            build_rule("reviewer", "worker-b", "delegate", "execute"),
        ];

        let (targets, has_execute_rules) = select_group_execute_dispatch_targets(
            &rules,
            &["worker-a".to_string(), "worker-b".to_string()],
            &["reviewer".to_string()],
        );

        assert!(has_execute_rules);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].dispatch_source_employee_id, "reviewer");
        assert_eq!(targets[0].assignee_employee_id, "worker-b");
    }

    #[test]
    fn legacy_select_group_execute_dispatch_targets_filters_out_self_dispatch() {
        let rules = vec![build_rule("planner", "planner", "delegate", "execute")];

        let (targets, has_execute_rules) = select_group_execute_dispatch_targets(
            &rules,
            &["planner".to_string()],
            &["planner".to_string()],
        );

        assert!(has_execute_rules);
        assert!(targets.is_empty());
    }
}
