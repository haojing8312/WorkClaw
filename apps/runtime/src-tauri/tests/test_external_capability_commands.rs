use runtime_lib::commands::content_providers::{
    list_detected_external_mcp_servers_for, list_external_capability_sources_for,
    mark_imported_external_mcp_servers,
};
use runtime_lib::content_providers::{CommandResult, DiagnosticsRunner};
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
fn external_capability_sources_include_agent_reach_channels() {
    let runner = FakeRunner::default()
        .with_existing("agent-reach")
        .with_output(
            "agent-reach",
            Ok(CommandResult {
                status_code: 0,
                stdout: [
                    "github: cli ok via gh",
                    "youtube: cli ok via yt-dlp",
                    "xiaohongshu: mcp ok via mcporter",
                    "douyin: mcp ok via mcporter",
                ]
                .join("\n"),
                stderr: String::new(),
            }),
        );

    let sources = list_external_capability_sources_for(&runner);
    let agent_reach = sources
        .into_iter()
        .find(|item| item.source_id == "agent-reach")
        .expect("agent-reach source");

    assert_eq!(agent_reach.channels.len(), 4);
    assert_eq!(agent_reach.channels[0].channel, "github");
    assert_eq!(agent_reach.channels[0].backend_type, "cli");
    assert_eq!(agent_reach.channels[2].backend_type, "mcp");
}

#[test]
fn detected_external_mcp_servers_filter_mcp_backed_channels() {
    let runner = FakeRunner::default()
        .with_existing("agent-reach")
        .with_output(
            "agent-reach",
            Ok(CommandResult {
                status_code: 0,
                stdout: [
                    "github: cli ok via gh",
                    "xiaohongshu: mcp ok via mcporter",
                    "linkedin: mcp ok via linkedin-mcp",
                ]
                .join("\n"),
                stderr: String::new(),
            }),
        );

    let detected = list_detected_external_mcp_servers_for(&runner);

    assert_eq!(detected.len(), 2);
    assert_eq!(detected[0].source_id, "agent-reach");
    assert_eq!(detected[0].backend_name, "mcporter");
    assert_eq!(detected[0].command, "mcporter");
    assert_eq!(
        detected[0].args,
        vec!["serve".to_string(), "xiaohongshu".to_string()]
    );
    assert_eq!(detected[1].channel, "linkedin");
}

#[test]
fn imported_external_mcp_servers_are_marked_as_managed() {
    let runner = FakeRunner::default()
        .with_existing("agent-reach")
        .with_output(
            "agent-reach",
            Ok(CommandResult {
                status_code: 0,
                stdout: "xiaohongshu: mcp ok via mcporter".to_string(),
                stderr: String::new(),
            }),
        );

    let detected = list_detected_external_mcp_servers_for(&runner);
    let imported = HashSet::from([("agent-reach".to_string(), "xiaohongshu".to_string())]);
    let merged = mark_imported_external_mcp_servers(detected, &imported);

    assert_eq!(merged.len(), 1);
    assert!(merged[0].managed_by_workclaw);
}
