use super::types::{
    ContentCapability, DetectedExternalMcpServer, ExternalCapabilityChannel,
    ExternalCapabilitySourceStatus, ProviderAvailability, ProviderStatus,
};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub status_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub trait DiagnosticsRunner {
    fn command_exists(&self, program: &str) -> bool;
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandResult, String>;
}

pub struct ProcessDiagnosticsRunner;

impl DiagnosticsRunner for ProcessDiagnosticsRunner {
    fn command_exists(&self, program: &str) -> bool {
        Command::new(program)
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn run(&self, program: &str, args: &[&str]) -> Result<CommandResult, String> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|err| err.to_string())?;

        Ok(CommandResult {
            status_code: output.status.code().unwrap_or_default(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

pub fn detect_agent_reach_provider(runner: &dyn DiagnosticsRunner) -> ProviderStatus {
    if !runner.command_exists("agent-reach") {
        return ProviderStatus {
            provider_id: "agent-reach".to_string(),
            availability: ProviderAvailability::NotFound,
            capabilities: Vec::new(),
            detail: Some("agent-reach command not found".to_string()),
        };
    }

    match runner.run("agent-reach", &["doctor"]) {
        Ok(output) => parse_doctor_output(&output),
        Err(err) => ProviderStatus {
            provider_id: "agent-reach".to_string(),
            availability: ProviderAvailability::Partial,
            capabilities: Vec::new(),
            detail: Some(err),
        },
    }
}

pub fn inspect_agent_reach_source(
    runner: &dyn DiagnosticsRunner,
) -> (ProviderStatus, Vec<ExternalCapabilityChannel>) {
    let provider = detect_agent_reach_provider(runner);

    if !runner.command_exists("agent-reach") {
        return (provider, Vec::new());
    }

    let channels = runner
        .run("agent-reach", &["doctor"])
        .ok()
        .map(|output| parse_doctor_channels(&output.stdout, &output.stderr))
        .unwrap_or_default();

    (provider, channels)
}

pub fn build_agent_reach_source_status(
    runner: &dyn DiagnosticsRunner,
) -> ExternalCapabilitySourceStatus {
    let (provider, channels) = inspect_agent_reach_source(runner);
    let mcp_count = channels
        .iter()
        .filter(|item| item.backend_type == "mcp")
        .count();
    let summary = if channels.is_empty() {
        "No external channels detected".to_string()
    } else {
        format!(
            "{} channels detected, {} MCP-backed",
            channels.len(),
            mcp_count
        )
    };

    ExternalCapabilitySourceStatus {
        source_id: "agent-reach".to_string(),
        display_name: "Agent-Reach".to_string(),
        availability: provider.availability,
        summary,
        channels,
        detail: provider.detail,
    }
}

pub fn detect_agent_reach_mcp_servers(
    runner: &dyn DiagnosticsRunner,
) -> Vec<DetectedExternalMcpServer> {
    let source = build_agent_reach_source_status(runner);
    source
        .channels
        .into_iter()
        .filter(|channel| channel.backend_type == "mcp")
        .map(|channel| {
            let (command, args, env) =
                build_mcp_server_template(&channel.channel, &channel.backend_name);
            DetectedExternalMcpServer {
                source_id: "agent-reach".to_string(),
                server_name: format!("agent-reach-{}", channel.channel),
                display_name: channel.channel.clone(),
                channel: channel.channel,
                status: channel.status,
                backend_name: channel.backend_name,
                command,
                args,
                env,
                managed_by_workclaw: false,
            }
        })
        .collect()
}

fn parse_doctor_output(output: &CommandResult) -> ProviderStatus {
    let combined = format!("{}\n{}", output.stdout, output.stderr);
    let lower = combined.to_lowercase();

    let mut capabilities = Vec::new();
    if lower.contains("read: ok") {
        capabilities.push(ContentCapability::ReadUrl);
    }
    if lower.contains("search: ok") {
        capabilities.push(ContentCapability::SearchContent);
    }
    if lower.contains("media: ok") || lower.contains("video: ok") {
        capabilities.push(ContentCapability::ExtractMediaContext);
    }

    let availability = if output.status_code != 0
        || lower.contains("missing")
        || lower.contains("failed")
        || lower.contains("not found")
    {
        ProviderAvailability::Partial
    } else {
        ProviderAvailability::Available
    };

    ProviderStatus {
        provider_id: "agent-reach".to_string(),
        availability,
        capabilities,
        detail: Some(combined.trim().to_string()).filter(|value| !value.is_empty()),
    }
}

fn parse_doctor_channels(stdout: &str, stderr: &str) -> Vec<ExternalCapabilityChannel> {
    let mut channels = Vec::new();
    for line in stdout.lines().chain(stderr.lines()) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some((channel, remainder)) = trimmed.split_once(':') else {
            continue;
        };

        let lower = remainder.to_lowercase();
        let backend_type = if lower.contains("mcp") {
            "mcp"
        } else if lower.contains("http") {
            "http"
        } else if lower.contains("cli") {
            "cli"
        } else {
            continue;
        };

        let status = if lower.contains("ok") || lower.contains("available") {
            "available"
        } else if lower.contains("missing")
            || lower.contains("not found")
            || lower.contains("failed")
        {
            "partial"
        } else {
            "unknown"
        };

        let backend_name = remainder
            .split_whitespace()
            .collect::<Vec<_>>()
            .windows(2)
            .find_map(|parts| {
                if parts[0].eq_ignore_ascii_case("via") {
                    Some(parts[1].trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| backend_type.to_string());

        channels.push(ExternalCapabilityChannel {
            channel: channel.trim().to_string(),
            status: status.to_string(),
            backend_type: backend_type.to_string(),
            backend_name,
            detail: Some(trimmed.to_string()),
        });
    }
    channels
}

fn build_mcp_server_template(
    channel: &str,
    backend_name: &str,
) -> (String, Vec<String>, Vec<String>) {
    match backend_name {
        "mcporter" => (
            "mcporter".to_string(),
            vec!["serve".to_string(), channel.to_string()],
            Vec::new(),
        ),
        "linkedin-mcp" => ("linkedin-mcp".to_string(), Vec::new(), Vec::new()),
        other => (other.to_string(), Vec::new(), Vec::new()),
    }
}
