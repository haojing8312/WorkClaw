use crate::agent::group_orchestrator::GroupRunExecuteTarget;
use crate::agent_catalog::agent_definition::{
    default_memory_scope_for_role, normalize_agent_id, AgentDefinition, AgentRoleKind,
};
use crate::agent_catalog::agent_permissions::{
    derive_allowed_tools_for_role, derive_capabilities_for_role,
};
use crate::agent_catalog::agent_workspace::build_agent_profile_context;
use crate::agent_core::spawn_policy::{evaluate_spawn_policy, SpawnPolicyInput};
use crate::commands::employee_agents::{AgentEmployee, EmployeeGroupRule};
use crate::employee_runtime_adapter::delegation_policy::{
    resolve_delegation_targets, DelegationPolicy, DelegationTarget,
};
use crate::employee_runtime_adapter::team_topology::{
    normalize_member_employee_ids, resolve_team_topology, NormalizedTeamRule, TeamTopology,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EmployeeRuntimeView {
    pub employee_id: String,
    pub display_name: String,
    pub agent_definition: AgentDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TeamRuntimeView {
    pub topology: TeamTopology,
    pub delegation_policy: DelegationPolicy,
    pub employees: Vec<EmployeeRuntimeView>,
}

pub(crate) fn infer_role_kind_for_employee(
    employee_id: &str,
    topology: &TeamTopology,
) -> AgentRoleKind {
    let normalized_employee_id = employee_id.trim().to_lowercase();
    if normalized_employee_id == topology.coordinator_employee_id {
        AgentRoleKind::Coordinator
    } else if normalized_employee_id == topology.planner_employee_id {
        AgentRoleKind::Planner
    } else if topology
        .reviewer_employee_id
        .as_ref()
        .is_some_and(|reviewer| reviewer == &normalized_employee_id)
    {
        AgentRoleKind::Reviewer
    } else if topology
        .executor_employee_ids
        .iter()
        .any(|executor| executor == &normalized_employee_id)
    {
        AgentRoleKind::Executor
    } else {
        AgentRoleKind::General
    }
}

pub(crate) fn build_agent_definition_from_employee(
    employee: &AgentEmployee,
    role_kind: AgentRoleKind,
) -> AgentDefinition {
    let profile = build_agent_profile_context(&employee.default_work_dir, &employee.persona);
    AgentDefinition {
        agent_id: normalize_agent_id(&employee.employee_id),
        display_name: employee.name.trim().to_string(),
        role_kind: role_kind.clone(),
        workspace_dir: profile.workspace_dir,
        persona_text: profile.persona_text,
        allowed_tools: derive_allowed_tools_for_role(&role_kind),
        permission_mode: "default".to_string(),
        model_id: None,
        memory_scope: default_memory_scope_for_role(&role_kind),
        capabilities: derive_capabilities_for_role(&role_kind),
    }
}

pub(crate) fn build_employee_runtime_view(
    employee: &AgentEmployee,
    topology: &TeamTopology,
) -> EmployeeRuntimeView {
    let role_kind = infer_role_kind_for_employee(&employee.employee_id, topology);
    EmployeeRuntimeView {
        employee_id: employee.employee_id.trim().to_lowercase(),
        display_name: employee.name.trim().to_string(),
        agent_definition: build_agent_definition_from_employee(employee, role_kind),
    }
}

pub(crate) fn normalize_team_rules(rules: &[EmployeeGroupRule]) -> Vec<NormalizedTeamRule> {
    rules.iter()
        .map(|rule| NormalizedTeamRule {
            from_employee_id: rule.from_employee_id.clone(),
            to_employee_id: rule.to_employee_id.clone(),
            relation_type: rule.relation_type.clone(),
            phase_scope: rule.phase_scope.clone(),
        })
        .collect()
}

pub(crate) fn build_team_runtime_view(
    employees: &[AgentEmployee],
    coordinator_employee_id: &str,
    entry_employee_id: &str,
    member_employee_ids: &[String],
    review_mode: &str,
    rules: &[EmployeeGroupRule],
    preferred_dispatch_sources: &[String],
) -> TeamRuntimeView {
    let normalized_rules = normalize_team_rules(rules);
    let topology = resolve_team_topology(
        entry_employee_id,
        coordinator_employee_id,
        member_employee_ids,
        review_mode,
        &normalized_rules,
    );
    let delegation_policy = resolve_delegation_targets(
        preferred_dispatch_sources,
        member_employee_ids,
        &normalized_rules,
    );
    let employees = employees
        .iter()
        .filter(|employee| {
            let employee_id = employee.employee_id.trim().to_lowercase();
            normalize_member_employee_ids(member_employee_ids)
                .iter()
                .any(|member_id| member_id == &employee_id)
        })
        .map(|employee| build_employee_runtime_view(employee, &topology))
        .collect::<Vec<_>>();

    TeamRuntimeView {
        topology,
        delegation_policy,
        employees,
    }
}

pub(crate) fn build_group_run_execute_targets(
    team_runtime_view: &TeamRuntimeView,
) -> Vec<GroupRunExecuteTarget> {
    let targets = team_runtime_view
        .delegation_policy
        .targets
        .iter()
        .filter_map(|target| {
            build_group_run_execute_target_from_policy(team_runtime_view, target)
        })
        .collect::<Vec<_>>();
    if !targets.is_empty() {
        return targets;
    }

    team_runtime_view
        .topology
        .executor_employee_ids
        .iter()
        .map(|employee_id| GroupRunExecuteTarget {
            dispatch_source_employee_id: String::new(),
            assignee_employee_id: employee_id.clone(),
        })
        .collect()
}

fn build_group_run_execute_target_from_policy(
    team_runtime_view: &TeamRuntimeView,
    target: &DelegationTarget,
) -> Option<GroupRunExecuteTarget> {
    let source_agent = team_runtime_view.employees.iter().find(|employee| {
        employee.employee_id == target.source_employee_id
    })?;
    let decision = evaluate_spawn_policy(SpawnPolicyInput {
        source_agent: &source_agent.agent_definition,
        target,
    });
    if !decision.allowed {
        return None;
    }

    Some(GroupRunExecuteTarget {
        dispatch_source_employee_id: decision.normalized_dispatch_source_employee_id,
        assignee_employee_id: target.target_employee_id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::{build_group_run_execute_targets, build_team_runtime_view};
    use crate::commands::employee_agents::{AgentEmployee, EmployeeGroupRule};

    fn employee(employee_id: &str, name: &str) -> AgentEmployee {
        AgentEmployee {
            id: employee_id.to_string(),
            employee_id: employee_id.to_string(),
            name: name.to_string(),
            role_id: employee_id.to_string(),
            persona: format!("{name} persona"),
            feishu_open_id: String::new(),
            feishu_app_id: String::new(),
            feishu_app_secret: String::new(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "D:/work".to_string(),
            openclaw_agent_id: employee_id.to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["app".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec![],
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn rule(from_employee_id: &str, to_employee_id: &str, relation_type: &str, phase_scope: &str) -> EmployeeGroupRule {
        EmployeeGroupRule {
            id: "r1".to_string(),
            group_id: "g1".to_string(),
            from_employee_id: from_employee_id.to_string(),
            to_employee_id: to_employee_id.to_string(),
            relation_type: relation_type.to_string(),
            phase_scope: phase_scope.to_string(),
            required: true,
            priority: 100,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn build_group_run_execute_targets_keeps_self_execute_without_fake_dispatch_source() {
        let team_runtime_view = build_team_runtime_view(
            &[employee("lead", "Lead")],
            "lead",
            "lead",
            &["lead".to_string()],
            "none",
            &[rule("lead", "lead", "delegate", "execute")],
            &["lead".to_string()],
        );

        let execute_targets = build_group_run_execute_targets(&team_runtime_view);
        assert_eq!(execute_targets.len(), 1);
        assert_eq!(execute_targets[0].assignee_employee_id, "lead");
        assert!(execute_targets[0].dispatch_source_employee_id.is_empty());
    }

    #[test]
    fn build_group_run_execute_targets_falls_back_to_topology_executors_when_rules_missing() {
        let team_runtime_view = build_team_runtime_view(
            &[
                employee("lead", "Lead"),
                employee("worker", "Worker"),
            ],
            "lead",
            "lead",
            &["lead".to_string(), "worker".to_string()],
            "none",
            &[],
            &["lead".to_string()],
        );

        let execute_targets = build_group_run_execute_targets(&team_runtime_view);
        assert_eq!(execute_targets.len(), 1);
        assert_eq!(execute_targets[0].assignee_employee_id, "worker");
        assert!(execute_targets[0].dispatch_source_employee_id.is_empty());
    }
}
