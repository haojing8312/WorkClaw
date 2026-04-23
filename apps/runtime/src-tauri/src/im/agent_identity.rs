pub fn resolve_agent_id(agent_id: &str, employee_id: &str, role_id: &str) -> String {
    let normalized_agent_id = agent_id.trim();
    if !normalized_agent_id.is_empty() {
        return normalized_agent_id.to_string();
    }

    let normalized_employee_id = employee_id.trim();
    if !normalized_employee_id.is_empty() {
        return normalized_employee_id.to_string();
    }

    role_id.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::resolve_agent_id;

    #[test]
    fn resolve_agent_id_prefers_explicit_agent_id() {
        assert_eq!(
            resolve_agent_id("agent-main", "employee-main", "planner"),
            "agent-main"
        );
    }

    #[test]
    fn resolve_agent_id_falls_back_to_employee_alias_then_role_id() {
        assert_eq!(
            resolve_agent_id("", "employee-main", "planner"),
            "employee-main"
        );
        assert_eq!(resolve_agent_id("", "", "planner"), "planner");
    }
}
