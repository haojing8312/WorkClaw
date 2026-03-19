use crate::agent::types::ExecutionCaps;

#[cfg(target_os = "windows")]
fn preferred_shell() -> &'static str {
    "cmd"
}

#[cfg(not(target_os = "windows"))]
fn preferred_shell() -> &'static str {
    "bash"
}

pub fn detect_execution_caps() -> ExecutionCaps {
    ExecutionCaps {
        platform: Some(std::env::consts::OS.to_string()),
        preferred_shell: Some(preferred_shell().to_string()),
        python_candidates: Vec::new(),
        node_candidates: Vec::new(),
        notes: vec!["static P0 detection".to_string()],
    }
}
