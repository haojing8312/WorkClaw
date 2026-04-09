use crate::agent::AgentExecutor;

pub(crate) fn load_memory_content(memory_dir: &std::path::Path) -> String {
    let memory_file = memory_dir.join("MEMORY.md");
    if memory_file.exists() {
        std::fs::read_to_string(memory_file).unwrap_or_default()
    } else {
        String::new()
    }
}

pub(crate) fn resolve_tool_name_list(
    allowed_tools: &Option<Vec<String>>,
    agent_executor: &AgentExecutor,
) -> Vec<String> {
    match allowed_tools {
        Some(whitelist) => whitelist.clone(),
        None => agent_executor
            .registry()
            .get_tool_definitions()
            .iter()
            .filter_map(|t| t["name"].as_str().map(String::from))
            .collect(),
    }
}

pub(crate) fn resolve_tool_names(
    allowed_tools: &Option<Vec<String>>,
    agent_executor: &AgentExecutor,
) -> String {
    resolve_tool_name_list(allowed_tools, agent_executor).join(", ")
}
pub(crate) fn sanitize_memory_bucket_component(raw: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut prev_sep = false;
    for ch in raw.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_sep = false;
            continue;
        }
        if !prev_sep {
            out.push('_');
            prev_sep = true;
        }
    }
    let normalized = out.trim_matches('_').to_string();
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized
    }
}

pub(crate) fn build_memory_dir_for_session(
    memory_root: &std::path::Path,
    skill_id: &str,
    employee_id: &str,
) -> std::path::PathBuf {
    if employee_id.trim().is_empty() {
        return memory_root.join(skill_id);
    }
    let employee_bucket = sanitize_memory_bucket_component(employee_id, "employee");
    memory_root
        .join("employees")
        .join(employee_bucket)
        .join("skills")
        .join(skill_id)
}

pub(crate) fn tool_ctx_from_work_dir(work_dir: &str) -> Option<std::path::PathBuf> {
    if work_dir.trim().is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(work_dir))
    }
}
