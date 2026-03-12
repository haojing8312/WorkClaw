use runtime_lib::content_providers::{
    detect_agent_reach_provider, CommandResult, ContentCapability, DiagnosticsRunner,
    ProviderAvailability,
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
fn missing_command_reports_not_found() {
    let status = detect_agent_reach_provider(&FakeRunner::default());

    assert_eq!(status.provider_id, "agent-reach");
    assert_eq!(status.availability, ProviderAvailability::NotFound);
    assert!(status.capabilities.is_empty());
}

#[test]
fn doctor_output_with_missing_dependencies_reports_partial() {
    let runner = FakeRunner::default()
        .with_existing("agent-reach")
        .with_output(
            "agent-reach",
            Ok(CommandResult {
                status_code: 0,
                stdout: "search: ok\nvideo: missing dependency".to_string(),
                stderr: String::new(),
            }),
        );

    let status = detect_agent_reach_provider(&runner);

    assert_eq!(status.availability, ProviderAvailability::Partial);
    assert!(status
        .capabilities
        .contains(&ContentCapability::SearchContent));
    assert!(status.detail.unwrap_or_default().contains("missing"));
}

#[test]
fn healthy_doctor_output_reports_available() {
    let runner = FakeRunner::default()
        .with_existing("agent-reach")
        .with_output(
            "agent-reach",
            Ok(CommandResult {
                status_code: 0,
                stdout: "read: ok\nsearch: ok\nmedia: ok".to_string(),
                stderr: String::new(),
            }),
        );

    let status = detect_agent_reach_provider(&runner);

    assert_eq!(status.availability, ProviderAvailability::Available);
    assert_eq!(
        status.capabilities,
        vec![
            ContentCapability::ReadUrl,
            ContentCapability::SearchContent,
            ContentCapability::ExtractMediaContext
        ]
    );
}
