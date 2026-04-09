use crate::agent::runtime::attempt_runner::RouteExecutionOutcome;
use crate::agent::runtime::effective_tool_set::{
    resolve_effective_tool_set, session_tool_policy_input, skill_tool_policy_input,
    EffectiveToolPolicyInput,
};
use crate::agent::runtime::runtime_io::{
    WorkspaceSkillCommandSpec, WorkspaceSkillContent, WorkspaceSkillRuntimeEntry,
};
use crate::agent::runtime::skill_routing::intent::RouteFallbackReason;
use crate::agent::runtime::tool_profiles::ToolProfileName;
use crate::agent::tool_manifest::{ToolCategory, ToolSource};
use crate::agent::permissions::PermissionMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RoutedSkillToolSetup {
    pub skill_id: String,
    pub skill_system_prompt: String,
    pub skill_allowed_tools: Option<Vec<String>>,
    pub skill_denied_tools: Option<Vec<String>>,
    pub skill_allowed_tool_sources: Option<Vec<ToolSource>>,
    pub skill_denied_tool_sources: Option<Vec<ToolSource>>,
    pub skill_allowed_tool_categories: Option<Vec<ToolCategory>>,
    pub skill_denied_tool_categories: Option<Vec<ToolCategory>>,
    pub skill_allowed_mcp_servers: Option<Vec<String>>,
    pub tool_profile: Option<ToolProfileName>,
    pub max_iterations: Option<usize>,
    pub source_type: String,
    pub pack_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RouteRunPlan {
    OpenTask {
        fallback_reason: Option<RouteFallbackReason>,
    },
    PromptSkillInline {
        skill_id: String,
        setup: RoutedSkillToolSetup,
    },
    PromptSkillFork {
        skill_id: String,
        setup: RoutedSkillToolSetup,
    },
    DirectDispatchSkill {
        skill_id: String,
        setup: RoutedSkillToolSetup,
        command_spec: WorkspaceSkillCommandSpec,
        raw_args: String,
    },
}

#[derive(Debug)]
pub(crate) enum RouteRunOutcome {
    OpenTask,
    DirectDispatch(String),
    Prompt {
        route_execution: RouteExecutionOutcome,
        reconstructed_history_len: usize,
    },
}

pub(crate) fn build_routed_skill_tool_setup(
    entry: &WorkspaceSkillRuntimeEntry,
) -> RoutedSkillToolSetup {
    RoutedSkillToolSetup {
        skill_id: entry.skill_id.clone(),
        skill_system_prompt: entry.config.system_prompt.clone(),
        skill_allowed_tools: entry.config.allowed_tools.clone(),
        skill_denied_tools: entry.config.denied_tools.clone(),
        skill_allowed_tool_sources: parse_skill_allowed_tool_sources(&entry.config),
        skill_denied_tool_sources: parse_skill_denied_tool_sources(&entry.config),
        skill_allowed_tool_categories: parse_skill_allowed_tool_categories(&entry.config),
        skill_denied_tool_categories: parse_skill_denied_tool_categories(&entry.config),
        skill_allowed_mcp_servers: skill_allowed_mcp_servers(entry),
        tool_profile: infer_skill_tool_profile(entry),
        max_iterations: entry.config.max_iterations,
        source_type: entry.source_type.clone(),
        pack_path: match &entry.content {
            WorkspaceSkillContent::LocalDir(path) => path.to_string_lossy().to_string(),
            WorkspaceSkillContent::FileTree(_) => String::new(),
        },
    }
}

pub(crate) fn skill_allowed_mcp_servers(
    entry: &WorkspaceSkillRuntimeEntry,
) -> Option<Vec<String>> {
    let servers = entry
        .config
        .mcp_servers
        .iter()
        .map(|server| server.name.trim())
        .filter(|name| !name.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    (!servers.is_empty()).then_some(servers)
}

fn infer_skill_tool_profile(entry: &WorkspaceSkillRuntimeEntry) -> Option<ToolProfileName> {
    if entry.config.allowed_tools.is_some() {
        return None;
    }

    let haystack = format!(
        "{} {} {}",
        entry.skill_id.to_ascii_lowercase(),
        entry.name.to_ascii_lowercase(),
        entry.description.to_ascii_lowercase()
    );

    if haystack.contains("browser") {
        return Some(ToolProfileName::Browser);
    }
    if haystack.contains("employee") || haystack.contains("team") {
        return Some(ToolProfileName::Employee);
    }
    if haystack.contains("coding") || haystack.contains("code") || haystack.contains("developer") {
        return Some(ToolProfileName::Coding);
    }

    None
}

pub(crate) fn parse_skill_denied_tool_categories(
    config: &runtime_skill_core::SkillConfig,
) -> Option<Vec<ToolCategory>> {
    let categories = config
        .denied_tool_categories
        .as_ref()
        .into_iter()
        .flatten()
        .filter_map(|name| parse_tool_category_name(name))
        .collect::<Vec<_>>();

    (!categories.is_empty()).then_some(categories)
}

pub(crate) fn parse_skill_allowed_tool_categories(
    config: &runtime_skill_core::SkillConfig,
) -> Option<Vec<ToolCategory>> {
    let categories = config
        .allowed_tool_categories
        .as_ref()
        .into_iter()
        .flatten()
        .filter_map(|name| parse_tool_category_name(name))
        .collect::<Vec<_>>();

    (!categories.is_empty()).then_some(categories)
}

pub(crate) fn parse_skill_allowed_tool_sources(
    config: &runtime_skill_core::SkillConfig,
) -> Option<Vec<ToolSource>> {
    let sources = config
        .allowed_tool_sources
        .as_ref()
        .into_iter()
        .flatten()
        .filter_map(|name| parse_tool_source_name(name))
        .collect::<Vec<_>>();

    (!sources.is_empty()).then_some(sources)
}

pub(crate) fn parse_skill_denied_tool_sources(
    config: &runtime_skill_core::SkillConfig,
) -> Option<Vec<ToolSource>> {
    let sources = config
        .denied_tool_sources
        .as_ref()
        .into_iter()
        .flatten()
        .filter_map(|name| parse_tool_source_name(name))
        .collect::<Vec<_>>();

    (!sources.is_empty()).then_some(sources)
}

pub(crate) fn parse_tool_category_name(raw: &str) -> Option<ToolCategory> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "file" => Some(ToolCategory::File),
        "shell" => Some(ToolCategory::Shell),
        "web" => Some(ToolCategory::Web),
        "browser" => Some(ToolCategory::Browser),
        "system" => Some(ToolCategory::System),
        "planning" => Some(ToolCategory::Planning),
        "agent" => Some(ToolCategory::Agent),
        "memory" => Some(ToolCategory::Memory),
        "search" => Some(ToolCategory::Search),
        "integration" => Some(ToolCategory::Integration),
        "other" => Some(ToolCategory::Other),
        _ => None,
    }
}

pub(crate) fn parse_tool_source_name(raw: &str) -> Option<ToolSource> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "native" => Some(ToolSource::Native),
        "runtime" => Some(ToolSource::Runtime),
        "sidecar" => Some(ToolSource::Sidecar),
        "mcp" => Some(ToolSource::Mcp),
        "plugin" => Some(ToolSource::Plugin),
        "alias" => Some(ToolSource::Alias),
        _ => None,
    }
}

pub(crate) fn resolve_skill_allowed_tools(
    registry: &crate::agent::ToolRegistry,
    setup: &RoutedSkillToolSetup,
    runtime_default_tool_policy: &EffectiveToolPolicyInput,
    permission_mode: PermissionMode,
) -> Option<Vec<String>> {
    resolve_effective_tool_set(
        registry,
        setup.skill_allowed_tools.clone(),
        setup.tool_profile,
        &[
            runtime_default_tool_policy.clone(),
            session_tool_policy_input(permission_mode),
            skill_tool_policy_input(
                setup.skill_denied_tools.clone().unwrap_or_default(),
                setup
                    .skill_denied_tool_categories
                    .clone()
                    .unwrap_or_default(),
                setup.skill_allowed_tool_categories.clone(),
                setup.skill_allowed_tool_sources.clone(),
                setup.skill_denied_tool_sources.clone().unwrap_or_default(),
                setup.skill_allowed_mcp_servers.clone(),
            ),
        ],
    )
    .allowed_tools
}
