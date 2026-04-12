use crate::employee_runtime_adapter::team_topology::NormalizedTeamRule;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DelegationKind {
    DispatchToOther,
    SelfExecute,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DelegationTarget {
    pub source_employee_id: String,
    pub target_employee_id: String,
    pub kind: DelegationKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DelegationPolicy {
    pub targets: Vec<DelegationTarget>,
    pub has_dispatch_targets: bool,
}

fn normalize_value(raw: &str) -> String {
    raw.trim().to_lowercase()
}

fn is_execute_dispatch_rule(rule: &NormalizedTeamRule) -> bool {
    let relation_type = normalize_value(&rule.relation_type);
    let phase_scope = normalize_value(&rule.phase_scope);
    let relation_allowed = relation_type == "delegate" || relation_type == "handoff";
    let phase_allowed = phase_scope.is_empty()
        || phase_scope == "execute"
        || phase_scope == "all"
        || phase_scope == "*";
    relation_allowed && phase_allowed
}

pub(crate) fn resolve_delegation_targets(
    preferred_dispatch_sources: &[String],
    member_employee_ids: &[String],
    execute_rules: &[NormalizedTeamRule],
) -> DelegationPolicy {
    use std::collections::HashSet;

    let member_ids = crate::employee_runtime_adapter::team_topology::normalize_member_employee_ids(
        member_employee_ids,
    );
    let member_set = member_ids.iter().cloned().collect::<HashSet<_>>();
    let eligible_rules = execute_rules
        .iter()
        .filter(|rule| is_execute_dispatch_rule(rule))
        .filter_map(|rule| {
            let source_employee_id = normalize_value(&rule.from_employee_id);
            let target_employee_id = normalize_value(&rule.to_employee_id);
            if source_employee_id.is_empty() || target_employee_id.is_empty() {
                return None;
            }
            if !member_set.is_empty() && !member_set.contains(&target_employee_id) {
                return None;
            }
            Some((source_employee_id, target_employee_id))
        })
        .collect::<Vec<_>>();

    let preferred_sources = preferred_dispatch_sources
        .iter()
        .map(|employee_id| normalize_value(employee_id))
        .filter(|employee_id| !employee_id.is_empty())
        .collect::<Vec<_>>();

    let selected_rules = preferred_sources
        .iter()
        .find_map(|preferred_source| {
            let matching_rules = eligible_rules
                .iter()
                .filter(|(source_employee_id, _)| source_employee_id == preferred_source)
                .cloned()
                .collect::<Vec<_>>();
            if matching_rules.is_empty() {
                None
            } else {
                Some(matching_rules)
            }
        })
        .unwrap_or(eligible_rules);

    let mut seen = HashSet::new();
    let targets = selected_rules
        .into_iter()
        .filter_map(|(source_employee_id, target_employee_id)| {
            if source_employee_id == target_employee_id {
                if member_set.len() == 1 {
                    let key = format!("self:{source_employee_id}");
                    if seen.insert(key) {
                        return Some(DelegationTarget {
                            source_employee_id,
                            target_employee_id,
                            kind: DelegationKind::SelfExecute,
                        });
                    }
                }
                return None;
            }

            let key = format!("dispatch:{target_employee_id}");
            if seen.insert(key) {
                Some(DelegationTarget {
                    source_employee_id,
                    target_employee_id,
                    kind: DelegationKind::DispatchToOther,
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let has_dispatch_targets = targets
        .iter()
        .any(|target| matches!(target.kind, DelegationKind::DispatchToOther));

    DelegationPolicy {
        targets,
        has_dispatch_targets,
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_delegation_targets, DelegationKind};
    use crate::employee_runtime_adapter::team_topology::NormalizedTeamRule;

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
    fn resolve_delegation_targets_returns_dispatch_to_other_for_valid_pair() {
        let policy = resolve_delegation_targets(
            &["planner".to_string()],
            &["worker-a".to_string(), "worker-b".to_string()],
            &[build_rule("planner", "worker-b", "delegate", "execute")],
        );

        assert_eq!(policy.targets.len(), 1);
        assert_eq!(policy.targets[0].source_employee_id, "planner");
        assert_eq!(policy.targets[0].target_employee_id, "worker-b");
        assert_eq!(policy.targets[0].kind, DelegationKind::DispatchToOther);
        assert!(policy.has_dispatch_targets);
    }

    #[test]
    fn resolve_delegation_targets_downgrades_self_dispatch_to_self_execute() {
        let policy = resolve_delegation_targets(
            &["planner".to_string()],
            &["planner".to_string()],
            &[build_rule("planner", "planner", "delegate", "execute")],
        );

        assert_eq!(policy.targets.len(), 1);
        assert_eq!(policy.targets[0].source_employee_id, "planner");
        assert_eq!(policy.targets[0].target_employee_id, "planner");
        assert_eq!(policy.targets[0].kind, DelegationKind::SelfExecute);
        assert!(!policy.has_dispatch_targets);
    }

    #[test]
    fn resolve_delegation_targets_does_not_fallback_to_self_dispatch_for_multi_member_team() {
        let policy = resolve_delegation_targets(
            &["planner".to_string()],
            &["planner".to_string(), "worker".to_string()],
            &[build_rule("planner", "planner", "delegate", "execute")],
        );

        assert_eq!(policy.targets.len(), 0);
        assert!(!policy.has_dispatch_targets);
    }
}
