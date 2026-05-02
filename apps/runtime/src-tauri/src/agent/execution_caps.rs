use crate::agent::types::ExecutionCaps;

#[cfg(target_os = "windows")]
fn preferred_shell() -> &'static str {
    "powershell"
}

#[cfg(not(target_os = "windows"))]
fn preferred_shell() -> &'static str {
    "bash"
}

pub fn detect_execution_caps() -> ExecutionCaps {
    let preferred_shell = preferred_shell().to_string();
    ExecutionCaps {
        platform: Some(std::env::consts::OS.to_string()),
        preferred_shell: Some(preferred_shell.clone()),
        python_candidates: Vec::new(),
        node_candidates: Vec::new(),
        notes: vec![format!(
            "static P0 detection; command execution should prefer exec ({preferred_shell})"
        )],
    }
}
