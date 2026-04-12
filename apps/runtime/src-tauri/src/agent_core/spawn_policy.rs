use crate::agent_catalog::agent_definition::AgentDefinition;
use crate::employee_runtime_adapter::delegation_policy::{DelegationKind, DelegationTarget};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpawnPolicyInput<'a> {
    pub source_agent: &'a AgentDefinition,
    pub target: &'a DelegationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpawnPolicyDecision {
    pub allowed: bool,
    pub should_spawn: bool,
    pub normalized_dispatch_source_employee_id: String,
}

pub(crate) fn evaluate_spawn_policy(input: SpawnPolicyInput<'_>) -> SpawnPolicyDecision {
    match input.target.kind {
        DelegationKind::DispatchToOther => SpawnPolicyDecision {
            allowed: input.source_agent.capabilities.can_delegate,
            should_spawn: input.source_agent.capabilities.can_delegate,
            normalized_dispatch_source_employee_id: input.target.source_employee_id.clone(),
        },
        DelegationKind::SelfExecute => SpawnPolicyDecision {
            allowed: true,
            should_spawn: false,
            normalized_dispatch_source_employee_id: String::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{evaluate_spawn_policy, SpawnPolicyInput};
    use crate::agent_catalog::agent_definition::{
        AgentCapabilityFlags, AgentDefinition, AgentMemoryScope, AgentRoleKind,
    };
    use crate::employee_runtime_adapter::delegation_policy::{DelegationKind, DelegationTarget};

    fn agent(can_delegate: bool) -> AgentDefinition {
        AgentDefinition {
            agent_id: "planner".to_string(),
            display_name: "Planner".to_string(),
            role_kind: AgentRoleKind::Planner,
            workspace_dir: "D:/work".to_string(),
            persona_text: "plans".to_string(),
            allowed_tools: vec![],
            permission_mode: "default".to_string(),
            model_id: None,
            memory_scope: AgentMemoryScope::Session,
            capabilities: AgentCapabilityFlags {
                can_delegate,
                can_spawn_subagents: false,
                can_review: false,
                background_capable: true,
            },
        }
    }

    #[test]
    fn evaluate_spawn_policy_keeps_self_execute_local() {
        let decision = evaluate_spawn_policy(SpawnPolicyInput {
            source_agent: &agent(true),
            target: &DelegationTarget {
                source_employee_id: "planner".to_string(),
                target_employee_id: "planner".to_string(),
                kind: DelegationKind::SelfExecute,
            },
        });

        assert!(decision.allowed);
        assert!(!decision.should_spawn);
        assert!(decision.normalized_dispatch_source_employee_id.is_empty());
    }
}
