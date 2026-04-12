#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TeamTopology {
    pub coordinator_employee_id: String,
    pub planner_employee_id: String,
    pub reviewer_employee_id: Option<String>,
    pub executor_employee_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NormalizedTeamRule {
    pub from_employee_id: String,
    pub to_employee_id: String,
    pub relation_type: String,
    pub phase_scope: String,
}

fn normalize_value(raw: &str) -> String {
    raw.trim().to_lowercase()
}

fn group_rule_matches_relation_types(rule: &NormalizedTeamRule, relation_types: &[&str]) -> bool {
    let relation_type = normalize_value(&rule.relation_type);
    relation_types
        .iter()
        .any(|relation_type_candidate| relation_type == *relation_type_candidate)
}

fn group_rule_matches_phase_scope(rule: &NormalizedTeamRule, phase_scope: &str) -> bool {
    let normalized_phase_scope = normalize_value(&rule.phase_scope);
    normalized_phase_scope.is_empty()
        || normalized_phase_scope == phase_scope
        || normalized_phase_scope == "all"
        || normalized_phase_scope == "*"
}

pub(crate) fn normalize_member_employee_ids(raw: &[String]) -> Vec<String> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in raw {
        let normalized = normalize_value(item);
        if normalized.is_empty() {
            continue;
        }
        if seen.insert(normalized.clone()) {
            out.push(normalized);
        }
    }
    out
}

pub(crate) fn resolve_planner_employee_id(
    entry_employee_id: &str,
    coordinator_employee_id: &str,
    rules: &[NormalizedTeamRule],
) -> String {
    let normalized_entry_employee_id = normalize_value(entry_employee_id);
    if !normalized_entry_employee_id.is_empty() {
        if let Some(planner_employee_id) = rules
            .iter()
            .find(|rule| {
                group_rule_matches_relation_types(rule, &["delegate", "handoff"])
                    && group_rule_matches_phase_scope(rule, "intake")
                    && normalize_value(&rule.from_employee_id) == normalized_entry_employee_id
                    && !normalize_value(&rule.to_employee_id).is_empty()
            })
            .map(|rule| normalize_value(&rule.to_employee_id))
        {
            return planner_employee_id;
        }
        return normalized_entry_employee_id;
    }

    if let Some(planner_employee_id) = rules
        .iter()
        .find(|rule| {
            group_rule_matches_relation_types(rule, &["review"])
                && group_rule_matches_phase_scope(rule, "plan")
                && !normalize_value(&rule.from_employee_id).is_empty()
        })
        .map(|rule| normalize_value(&rule.from_employee_id))
    {
        return planner_employee_id;
    }

    normalize_value(coordinator_employee_id)
}

pub(crate) fn resolve_reviewer_employee_id(
    review_mode: &str,
    planner_employee_id: &str,
    rules: &[NormalizedTeamRule],
) -> Option<String> {
    if review_mode.trim().eq_ignore_ascii_case("none") {
        return None;
    }

    let normalized_planner_employee_id = normalize_value(planner_employee_id);
    rules
        .iter()
        .find(|rule| {
            group_rule_matches_relation_types(rule, &["review"])
                && group_rule_matches_phase_scope(rule, "plan")
                && !normalized_planner_employee_id.is_empty()
                && normalize_value(&rule.from_employee_id) == normalized_planner_employee_id
                && !normalize_value(&rule.to_employee_id).is_empty()
        })
        .map(|rule| normalize_value(&rule.to_employee_id))
        .or_else(|| {
            rules
                .iter()
                .find(|rule| {
                    group_rule_matches_relation_types(rule, &["review"])
                        && group_rule_matches_phase_scope(rule, "plan")
                        && !normalize_value(&rule.to_employee_id).is_empty()
                })
                .map(|rule| normalize_value(&rule.to_employee_id))
        })
}

pub(crate) fn resolve_executor_employee_ids(
    coordinator_employee_id: &str,
    member_employee_ids: &[String],
    planner_employee_id: &str,
    reviewer_employee_id: Option<&str>,
) -> Vec<String> {
    let normalized_members = normalize_member_employee_ids(member_employee_ids);
    if normalized_members.is_empty() {
        return vec![normalize_value(coordinator_employee_id)];
    }
    if normalized_members.len() == 1 {
        return normalized_members;
    }

    let planner_employee_id = normalize_value(planner_employee_id);
    let reviewer_employee_id = reviewer_employee_id.map(normalize_value);
    let coordinator_employee_id = normalize_value(coordinator_employee_id);
    let filtered = normalized_members
        .iter()
        .filter(|employee_id| **employee_id != planner_employee_id)
        .filter(|employee_id| {
            reviewer_employee_id
                .as_ref()
                .map(|reviewer| **employee_id != *reviewer)
                .unwrap_or(true)
        })
        .filter(|employee_id| **employee_id != coordinator_employee_id)
        .cloned()
        .collect::<Vec<_>>();

    if filtered.is_empty() {
        normalized_members
    } else {
        filtered
    }
}

pub(crate) fn resolve_team_topology(
    entry_employee_id: &str,
    coordinator_employee_id: &str,
    member_employee_ids: &[String],
    review_mode: &str,
    rules: &[NormalizedTeamRule],
) -> TeamTopology {
    let coordinator_employee_id = normalize_value(coordinator_employee_id);
    let planner_employee_id =
        resolve_planner_employee_id(entry_employee_id, &coordinator_employee_id, rules);
    let reviewer_employee_id =
        resolve_reviewer_employee_id(review_mode, &planner_employee_id, rules);
    let executor_employee_ids = resolve_executor_employee_ids(
        &coordinator_employee_id,
        member_employee_ids,
        &planner_employee_id,
        reviewer_employee_id.as_deref(),
    );

    TeamTopology {
        coordinator_employee_id,
        planner_employee_id,
        reviewer_employee_id,
        executor_employee_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_member_employee_ids, resolve_team_topology, NormalizedTeamRule, TeamTopology,
    };

    fn build_rule(
        from_employee_id: &str,
        to_employee_id: &str,
        relation_type: &str,
        phase_scope: &str,
    ) -> NormalizedTeamRule {
        NormalizedTeamRule {
            from_employee_id: from_employee_id.to_string(),
            to_employee_id: to_employee_id.to_string(),
            relation_type: relation_type.to_string(),
            phase_scope: phase_scope.to_string(),
        }
    }

    #[test]
    fn normalize_member_employee_ids_dedupes_and_trims() {
        assert_eq!(
            normalize_member_employee_ids(&[
                " Alice ".to_string(),
                "alice".to_string(),
                "".to_string(),
                "BOB".to_string(),
            ]),
            vec!["alice".to_string(), "bob".to_string()]
        );
    }

    #[test]
    fn resolve_team_topology_handles_single_member_team() {
        let topology = resolve_team_topology(
            "",
            "lead",
            &["lead".to_string()],
            "none",
            &[],
        );

        assert_eq!(
            topology,
            TeamTopology {
                coordinator_employee_id: "lead".to_string(),
                planner_employee_id: "lead".to_string(),
                reviewer_employee_id: None,
                executor_employee_ids: vec!["lead".to_string()],
            }
        );
    }

    #[test]
    fn resolve_team_topology_prefers_entry_then_review_rules() {
        let topology = resolve_team_topology(
            "intake",
            "lead",
            &[
                "lead".to_string(),
                "planner".to_string(),
                "reviewer".to_string(),
                "worker".to_string(),
            ],
            "required",
            &[
                build_rule("intake", "planner", "delegate", "intake"),
                build_rule("planner", "reviewer", "review", "plan"),
            ],
        );

        assert_eq!(topology.planner_employee_id, "planner");
        assert_eq!(topology.reviewer_employee_id.as_deref(), Some("reviewer"));
        assert!(topology.executor_employee_ids.contains(&"worker".to_string()));
    }
}
