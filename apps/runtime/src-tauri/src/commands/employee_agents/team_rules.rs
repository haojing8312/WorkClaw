use super::types::EmployeeGroupRule;
use crate::agent::group_orchestrator::GroupRunExecuteTarget;

pub(super) fn normalize_member_employee_ids(raw: &[String]) -> Vec<String> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in raw {
        let normalized = item.trim().to_lowercase();
        if normalized.is_empty() {
            continue;
        }
        if seen.insert(normalized.clone()) {
            out.push(normalized);
        }
    }
    out
}

fn group_rule_allows_execute_reassignment(rule: &EmployeeGroupRule) -> bool {
    let relation_type = rule.relation_type.trim().to_lowercase();
    let phase_scope = rule.phase_scope.trim().to_lowercase();
    let relation_allowed = relation_type == "delegate" || relation_type == "handoff";
    let phase_allowed = phase_scope.is_empty()
        || phase_scope == "execute"
        || phase_scope == "all"
        || phase_scope == "*";
    relation_allowed && phase_allowed
}

fn group_rule_matches_phase_scope(rule: &EmployeeGroupRule, phase_scope: &str) -> bool {
    let normalized_phase_scope = rule.phase_scope.trim().to_lowercase();
    normalized_phase_scope.is_empty()
        || normalized_phase_scope == phase_scope
        || normalized_phase_scope == "all"
        || normalized_phase_scope == "*"
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

pub(super) fn resolve_group_planner_employee_id(
    entry_employee_id: &str,
    coordinator_employee_id: &str,
    rules: &[EmployeeGroupRule],
) -> String {
    if let Some(planner_employee_id) = rules
        .iter()
        .find(|rule| {
            group_rule_matches_relation_types(rule, &["review"])
                && group_rule_matches_phase_scope(rule, "plan")
                && !rule.from_employee_id.trim().is_empty()
        })
        .map(|rule| rule.from_employee_id.trim().to_lowercase())
    {
        return planner_employee_id;
    }

    let normalized_entry_employee_id = entry_employee_id.trim().to_lowercase();
    if !normalized_entry_employee_id.is_empty() {
        if let Some(planner_employee_id) = rules
            .iter()
            .find(|rule| {
                group_rule_matches_relation_types(rule, &["delegate", "handoff"])
                    && group_rule_matches_phase_scope(rule, "intake")
                    && rule
                        .from_employee_id
                        .trim()
                        .eq_ignore_ascii_case(&normalized_entry_employee_id)
                    && !rule.to_employee_id.trim().is_empty()
            })
            .map(|rule| rule.to_employee_id.trim().to_lowercase())
        {
            return planner_employee_id;
        }
        return normalized_entry_employee_id;
    }

    coordinator_employee_id.trim().to_lowercase()
}

pub(super) fn resolve_group_reviewer_employee_id(
    review_mode: &str,
    planner_employee_id: &str,
    rules: &[EmployeeGroupRule],
) -> Option<String> {
    if review_mode.trim().eq_ignore_ascii_case("none") {
        return None;
    }

    let normalized_planner_employee_id = planner_employee_id.trim().to_lowercase();
    rules
        .iter()
        .find(|rule| {
            group_rule_matches_relation_types(rule, &["review"])
                && group_rule_matches_phase_scope(rule, "plan")
                && (!normalized_planner_employee_id.is_empty()
                    && rule
                        .from_employee_id
                        .trim()
                        .eq_ignore_ascii_case(&normalized_planner_employee_id))
                && !rule.to_employee_id.trim().is_empty()
        })
        .map(|rule| rule.to_employee_id.trim().to_lowercase())
        .or_else(|| {
            rules
                .iter()
                .find(|rule| {
                    group_rule_matches_relation_types(rule, &["review"])
                        && group_rule_matches_phase_scope(rule, "plan")
                        && !rule.to_employee_id.trim().is_empty()
                })
                .map(|rule| rule.to_employee_id.trim().to_lowercase())
        })
}

pub(super) fn select_group_execute_dispatch_targets(
    rules: &[EmployeeGroupRule],
    member_employee_ids: &[String],
    preferred_dispatch_sources: &[String],
) -> (Vec<GroupRunExecuteTarget>, bool) {
    let member_set = normalize_member_employee_ids(member_employee_ids)
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    let execute_rules = rules
        .iter()
        .filter(|rule| group_rule_allows_execute_reassignment(rule))
        .filter_map(|rule| {
            let assignee_employee_id = rule.to_employee_id.trim().to_lowercase();
            if assignee_employee_id.is_empty() {
                return None;
            }
            if !member_set.is_empty() && !member_set.contains(&assignee_employee_id) {
                return None;
            }
            let dispatch_source_employee_id = rule.from_employee_id.trim().to_lowercase();
            if dispatch_source_employee_id.is_empty() {
                return None;
            }
            Some(GroupRunExecuteTarget {
                dispatch_source_employee_id,
                assignee_employee_id,
            })
        })
        .collect::<Vec<_>>();

    if execute_rules.is_empty() {
        return (Vec::new(), false);
    }

    let preferred_sources = preferred_dispatch_sources
        .iter()
        .map(|employee_id| employee_id.trim().to_lowercase())
        .filter(|employee_id| !employee_id.is_empty())
        .collect::<Vec<_>>();

    let selected_rules = preferred_sources
        .iter()
        .find_map(|dispatch_source_employee_id| {
            let matching_rules = execute_rules
                .iter()
                .filter(|target| target.dispatch_source_employee_id == *dispatch_source_employee_id)
                .cloned()
                .collect::<Vec<_>>();
            if matching_rules.is_empty() {
                None
            } else {
                Some(matching_rules)
            }
        })
        .unwrap_or_else(|| execute_rules.clone());

    let mut seen_assignees = std::collections::HashSet::new();
    (
        selected_rules
            .into_iter()
            .filter(|target| seen_assignees.insert(target.assignee_employee_id.clone()))
            .collect(),
        true,
    )
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
}
