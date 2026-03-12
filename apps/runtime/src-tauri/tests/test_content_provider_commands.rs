use runtime_lib::commands::content_providers::{
    is_known_safe_external_mcp_template, list_content_providers_for,
    run_content_provider_diagnostics_for,
};
use runtime_lib::content_providers::{
    CommandResult, DetectedExternalMcpServer, DiagnosticsRunner, ProviderAvailability,
};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
struct FakeRunner {
    existing: HashSet<String>,
    outputs: HashMap<String, Result<CommandResult, String>>,
}

impl FakeRunner {
    fn with_existing(mut self, command: &str) -> Self {
        self.existing.insert(command.to_string());
        self
    }

    fn with_output(mut self, command: &str, result: Result<CommandResult, String>) -> Self {
        self.outputs.insert(command.to_string(), result);
        self
    }
}

impl DiagnosticsRunner for FakeRunner {
    fn command_exists(&self, program: &str) -> bool {
        self.existing.contains(program)
    }

    fn run(&self, program: &str, _args: &[&str]) -> Result<CommandResult, String> {
        self.outputs
            .get(program)
            .cloned()
            .unwrap_or_else(|| Err(format!("missing stub for {program}")))
    }
}

#[test]
fn list_content_providers_includes_builtin_and_agent_reach() {
    let providers = list_content_providers_for(&FakeRunner::default());

    assert_eq!(providers.len(), 2);
    assert_eq!(providers[0].provider_id, "builtin-web");
    assert_eq!(providers[0].availability, ProviderAvailability::Available);
    assert_eq!(providers[1].provider_id, "agent-reach");
}

#[test]
fn diagnostics_can_target_agent_reach() {
    let runner = FakeRunner::default()
        .with_existing("agent-reach")
        .with_output(
            "agent-reach",
            Ok(CommandResult {
                status_code: 0,
                stdout: "read: ok\nsearch: ok".to_string(),
                stderr: String::new(),
            }),
        );

    let status = run_content_provider_diagnostics_for("agent-reach", &runner).expect("diagnostics");

    assert_eq!(status.provider_id, "agent-reach");
    assert_eq!(status.availability, ProviderAvailability::Available);
}

#[test]
fn diagnostics_reject_unknown_provider_id() {
    let error = run_content_provider_diagnostics_for("unknown", &FakeRunner::default())
        .expect_err("unknown provider should error");

    assert!(error.contains("unknown content provider"));
}

#[test]
fn import_rejects_unknown_external_mcp_templates() {
    let server = DetectedExternalMcpServer {
        source_id: "agent-reach".to_string(),
        channel: "custom".to_string(),
        server_name: "agent-reach-custom".to_string(),
        display_name: "custom".to_string(),
        status: "available".to_string(),
        backend_name: "unknown".to_string(),
        command: "custom-mcp".to_string(),
        args: vec!["serve".to_string()],
        env: vec![],
        managed_by_workclaw: false,
    };

    assert!(!is_known_safe_external_mcp_template(&server));
}

#[test]
fn import_accepts_known_safe_external_mcp_templates() {
    let server = DetectedExternalMcpServer {
        source_id: "agent-reach".to_string(),
        channel: "xiaohongshu".to_string(),
        server_name: "agent-reach-xiaohongshu".to_string(),
        display_name: "xiaohongshu".to_string(),
        status: "available".to_string(),
        backend_name: "mcporter".to_string(),
        command: "mcporter".to_string(),
        args: vec!["serve".to_string(), "xiaohongshu".to_string()],
        env: vec![],
        managed_by_workclaw: false,
    };

    assert!(is_known_safe_external_mcp_template(&server));
}
