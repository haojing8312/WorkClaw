use crate::agent::tool_manifest::{ToolCategory, ToolSource};
use crate::agent::{ToolManifestEntry, ToolRegistry};

use super::tool_catalog::{ToolDiscoveryCandidateRecord, ToolRecommendationStage};
use super::tool_profiles::{resolve_tool_profile, ToolProfileName};
use crate::agent::permissions::PermissionMode;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveToolSetSource {
    RegistryDefault,
    ExplicitAllowList,
    Profile(ToolProfileName),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EffectiveToolSet {
    pub source: EffectiveToolSetSource,
    pub allowed_tools: Option<Vec<String>>,
    pub tool_names: Vec<String>,
    pub tool_manifest: Vec<ToolManifestEntry>,
    pub active_tools: Vec<String>,
    pub active_tool_manifest: Vec<ToolManifestEntry>,
    pub recommended_tools: Vec<String>,
    pub supporting_tools: Vec<String>,
    pub deferred_tools: Vec<String>,
    pub loading_policy: ToolLoadingPolicy,
    pub expanded_to_full: bool,
    pub expansion_reason: Option<String>,
    pub missing_tools: Vec<String>,
    pub filtered_out_tools: Vec<String>,
    pub excluded_tools: Vec<EffectiveToolExclusion>,
    pub source_counts: Vec<EffectiveToolSourceCount>,
    pub policy: EffectiveToolPolicySummary,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ToolFilterReason {
    MissingFromRegistry,
    McpServerFiltered,
    ExplicitDenyList,
    CategoryFiltered,
    AllowedCategoryFiltered,
    SourceFiltered,
    DeniedSourceFiltered,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EffectiveToolExclusion {
    pub name: String,
    pub source: Option<ToolSource>,
    pub category: Option<ToolCategory>,
    pub reason: ToolFilterReason,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EffectiveToolSourceCount {
    pub source: ToolSource,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EffectiveToolReasonCount {
    pub reason: ToolFilterReason,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolLoadingPolicy {
    Full,
    RecommendedOnly,
    RecommendedPlusCoreSafeTools,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EffectiveToolDecisionRecord {
    pub source: EffectiveToolSetSource,
    pub allowed_tool_count: usize,
    pub active_tool_count: usize,
    pub recommended_tool_count: usize,
    pub supporting_tool_count: usize,
    pub deferred_tool_count: usize,
    pub excluded_tool_count: usize,
    pub active_tools: Vec<String>,
    pub recommended_tools: Vec<String>,
    pub supporting_tools: Vec<String>,
    pub deferred_tools: Vec<String>,
    pub missing_tools: Vec<String>,
    pub filtered_out_tools: Vec<String>,
    pub excluded_tools: Vec<EffectiveToolExclusion>,
    pub source_counts: Vec<EffectiveToolSourceCount>,
    pub exclusion_counts: Vec<EffectiveToolReasonCount>,
    pub policy: EffectiveToolPolicySummary,
    pub loading_policy: ToolLoadingPolicy,
    pub expanded_to_full: bool,
    pub expansion_reason: Option<String>,
    pub discovery_candidates: Vec<ToolDiscoveryCandidateRecord>,
}

#[cfg_attr(not(test), allow(dead_code))]
pub type EffectiveToolPlanSummary = EffectiveToolDecisionRecord;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EffectiveToolSetFilters {
    pub denied_tool_names: Vec<String>,
    pub denied_categories: Vec<ToolCategory>,
    pub allowed_categories: Option<Vec<ToolCategory>>,
    pub allowed_sources: Option<Vec<ToolSource>>,
    pub denied_sources: Vec<ToolSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveToolPolicyInputSource {
    Session,
    Skill,
    RuntimeDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EffectiveToolPolicyInputSummary {
    pub source: EffectiveToolPolicyInputSource,
    pub label: String,
    pub denied_tool_names: Vec<String>,
    pub denied_categories: Vec<ToolCategory>,
    pub allowed_categories: Option<Vec<ToolCategory>>,
    pub allowed_sources: Option<Vec<ToolSource>>,
    pub denied_sources: Vec<ToolSource>,
    pub allowed_mcp_servers: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EffectiveToolPolicyInput {
    pub source: EffectiveToolPolicyInputSource,
    pub label: String,
    pub denied_tool_names: Vec<String>,
    pub denied_categories: Vec<ToolCategory>,
    pub allowed_categories: Option<Vec<ToolCategory>>,
    pub allowed_sources: Option<Vec<ToolSource>>,
    pub denied_sources: Vec<ToolSource>,
    pub allowed_mcp_servers: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EffectiveToolPolicySummary {
    pub denied_tool_names: Vec<String>,
    pub denied_categories: Vec<ToolCategory>,
    pub allowed_categories: Option<Vec<ToolCategory>>,
    pub allowed_sources: Option<Vec<ToolSource>>,
    pub denied_sources: Vec<ToolSource>,
    pub allowed_mcp_servers: Option<Vec<String>>,
    pub inputs: Vec<EffectiveToolPolicyInputSummary>,
}

impl EffectiveToolSet {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn tool_names_csv(&self) -> String {
        self.active_tools.join(", ")
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn decision_record(&self) -> EffectiveToolDecisionRecord {
        self.decision_record_with_candidates(Vec::new())
    }

    pub(crate) fn decision_record_with_candidates(
        &self,
        discovery_candidates: Vec<ToolDiscoveryCandidateRecord>,
    ) -> EffectiveToolDecisionRecord {
        EffectiveToolDecisionRecord {
            source: self.source.clone(),
            allowed_tool_count: self.tool_names.len(),
            active_tool_count: self.active_tools.len(),
            recommended_tool_count: self.recommended_tools.len(),
            supporting_tool_count: self.supporting_tools.len(),
            deferred_tool_count: self.deferred_tools.len(),
            excluded_tool_count: self.excluded_tools.len(),
            active_tools: self.active_tools.clone(),
            recommended_tools: self.recommended_tools.clone(),
            supporting_tools: self.supporting_tools.clone(),
            deferred_tools: self.deferred_tools.clone(),
            missing_tools: self.missing_tools.clone(),
            filtered_out_tools: self.filtered_out_tools.clone(),
            excluded_tools: self.excluded_tools.clone(),
            source_counts: self.source_counts.clone(),
            exclusion_counts: build_reason_counts(&self.excluded_tools),
            policy: self.policy.clone(),
            loading_policy: self.loading_policy,
            expanded_to_full: self.expanded_to_full,
            expansion_reason: self.expansion_reason.clone(),
            discovery_candidates,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn summary(&self) -> EffectiveToolPlanSummary {
        self.decision_record()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn exclusions_for_reason(
        &self,
        reason: ToolFilterReason,
    ) -> Vec<&EffectiveToolExclusion> {
        self.excluded_tools
            .iter()
            .filter(|entry| entry.reason == reason)
            .collect()
    }

    pub(crate) fn has_deferred_tools(&self) -> bool {
        !self.deferred_tools.is_empty()
    }

    pub(crate) fn full_allowed_tools(&self) -> Vec<String> {
        self.tool_names.clone()
    }

    pub(crate) fn apply_recommended_tools(
        &mut self,
        discovery_candidates: &[ToolDiscoveryCandidateRecord],
    ) {
        if self.tool_names.len() < 8 {
            self.allowed_tools = if self.allowed_tools.is_some() {
                Some(self.tool_names.clone())
            } else {
                None
            };
            return;
        }

        let recommended_tools = discovery_candidates
            .iter()
            .filter(|candidate| candidate.stage == ToolRecommendationStage::Primary)
            .map(|candidate| candidate.name.clone())
            .filter(|name| self.tool_names.contains(name))
            .fold(Vec::new(), |mut acc, name| {
                if !acc.contains(&name) {
                    acc.push(name);
                }
                acc
            });

        if recommended_tools.is_empty() {
            self.loading_policy = ToolLoadingPolicy::Full;
            self.allowed_tools = if self.allowed_tools.is_some() {
                Some(self.tool_names.clone())
            } else {
                None
            };
            return;
        }

        let supporting_tools = discovery_candidates
            .iter()
            .filter(|candidate| candidate.stage == ToolRecommendationStage::Supporting)
            .map(|candidate| candidate.name.clone())
            .filter(|name| self.tool_names.contains(name))
            .fold(Vec::new(), |mut acc, name| {
                if !acc.contains(&name) {
                    acc.push(name);
                }
                acc
            });

        let core_safe_tools = self
            .tool_manifest
            .iter()
            .filter(|entry| entry.read_only && !entry.requires_approval && !entry.destructive)
            .map(|entry| entry.name.clone())
            .fold(Vec::new(), |mut acc, name| {
                if !acc.contains(&name) {
                    acc.push(name);
                }
                acc
            });

        let mut active_tools = recommended_tools.clone();
        for tool_name in supporting_tools.iter().take(2) {
            if !active_tools.contains(&tool_name) {
                active_tools.push(tool_name.clone());
            }
        }
        for tool_name in core_safe_tools {
            if !active_tools.contains(&tool_name) {
                active_tools.push(tool_name);
            }
        }

        if active_tools.len() >= self.tool_names.len() {
            self.loading_policy = ToolLoadingPolicy::Full;
            self.allowed_tools = if self.allowed_tools.is_some() {
                Some(self.tool_names.clone())
            } else {
                None
            };
            return;
        }

        self.recommended_tools = recommended_tools;
        self.supporting_tools = supporting_tools;
        self.active_tools = active_tools.clone();
        self.active_tool_manifest = self
            .tool_manifest
            .iter()
            .filter(|entry| active_tools.contains(&entry.name))
            .cloned()
            .collect();
        self.deferred_tools = self
            .tool_names
            .iter()
            .filter(|name| !active_tools.contains(name))
            .cloned()
            .collect();
        self.loading_policy = ToolLoadingPolicy::RecommendedPlusCoreSafeTools;
        self.allowed_tools = Some(active_tools);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn expand_to_full(&mut self, reason: impl Into<String>) {
        if !self.has_deferred_tools() {
            return;
        }

        self.active_tools = self.tool_names.clone();
        self.active_tool_manifest = self.tool_manifest.clone();
        self.deferred_tools.clear();
        self.loading_policy = ToolLoadingPolicy::Full;
        self.expanded_to_full = true;
        self.expansion_reason = Some(reason.into());
        self.allowed_tools = Some(self.tool_names.clone());
    }
}

pub(crate) fn session_tool_policy_input(
    permission_mode: PermissionMode,
) -> EffectiveToolPolicyInput {
    EffectiveToolPolicyInput {
        source: EffectiveToolPolicyInputSource::Session,
        label: match permission_mode {
            PermissionMode::Default => "permission_mode:default".to_string(),
            PermissionMode::AcceptEdits => "permission_mode:accept_edits".to_string(),
            PermissionMode::Unrestricted => "permission_mode:unrestricted".to_string(),
        },
        denied_tool_names: Vec::new(),
        denied_categories: Vec::new(),
        allowed_categories: None,
        allowed_sources: None,
        denied_sources: Vec::new(),
        allowed_mcp_servers: None,
    }
}

pub(crate) fn skill_tool_policy_input(
    denied_tool_names: Vec<String>,
    denied_categories: Vec<ToolCategory>,
    allowed_categories: Option<Vec<ToolCategory>>,
    allowed_sources: Option<Vec<ToolSource>>,
    denied_sources: Vec<ToolSource>,
    allowed_mcp_servers: Option<Vec<String>>,
) -> EffectiveToolPolicyInput {
    EffectiveToolPolicyInput {
        source: EffectiveToolPolicyInputSource::Skill,
        label: "skill_declared_filters".to_string(),
        denied_tool_names,
        denied_categories,
        allowed_categories,
        allowed_sources,
        denied_sources,
        allowed_mcp_servers,
    }
}

pub(crate) fn runtime_default_tool_policy_input(
    label: String,
    denied_tool_names: Vec<String>,
    denied_categories: Vec<ToolCategory>,
    allowed_sources: Option<Vec<ToolSource>>,
    allowed_mcp_servers: Option<Vec<String>>,
) -> EffectiveToolPolicyInput {
    EffectiveToolPolicyInput {
        source: EffectiveToolPolicyInputSource::RuntimeDefault,
        label,
        denied_tool_names,
        denied_categories,
        allowed_categories: None,
        allowed_sources,
        denied_sources: Vec::new(),
        allowed_mcp_servers,
    }
}

fn normalize_vec_strings(values: &[String]) -> Vec<String> {
    let mut normalized = values
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn normalize_allowed_sources(values: Option<&[ToolSource]>) -> Option<Vec<ToolSource>> {
    values.map(|sources| {
        let mut normalized = sources.to_vec();
        normalized.sort();
        normalized.dedup();
        normalized
    })
}

fn normalize_allowed_categories(values: Option<&[ToolCategory]>) -> Option<Vec<ToolCategory>> {
    values.map(|categories| {
        let mut normalized = categories.to_vec();
        normalized.sort();
        normalized.dedup();
        normalized
    })
}

fn intersect_sources(
    left: Option<Vec<ToolSource>>,
    right: Option<Vec<ToolSource>>,
) -> Option<Vec<ToolSource>> {
    match (left, right) {
        (Some(left), Some(right)) => {
            let values = left
                .into_iter()
                .filter(|source| right.contains(source))
                .collect::<Vec<_>>();
            Some(values)
        }
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn intersect_servers(left: Option<Vec<String>>, right: Option<Vec<String>>) -> Option<Vec<String>> {
    match (left, right) {
        (Some(left), Some(right)) => {
            let values = left
                .into_iter()
                .filter(|server| right.contains(server))
                .collect::<Vec<_>>();
            Some(values)
        }
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn merge_policy_inputs(
    policy_inputs: &[EffectiveToolPolicyInput],
) -> (
    EffectiveToolSetFilters,
    Option<Vec<String>>,
    EffectiveToolPolicySummary,
) {
    let mut denied_tool_names = Vec::new();
    let mut denied_categories = Vec::new();
    let mut allowed_categories: Option<Vec<ToolCategory>> = None;
    let mut allowed_sources = None;
    let mut denied_sources = Vec::new();
    let mut allowed_mcp_servers = None;
    let mut inputs = Vec::new();

    for input in policy_inputs {
        denied_tool_names.extend(input.denied_tool_names.clone());
        denied_categories.extend(input.denied_categories.clone());
        allowed_categories = match (
            allowed_categories,
            normalize_allowed_categories(input.allowed_categories.as_deref()),
        ) {
            (Some(left), Some(right)) => Some(
                left.into_iter()
                    .filter(|category| right.contains(category))
                    .collect::<Vec<_>>(),
            ),
            (Some(left), None) => Some(left),
            (None, Some(right)) => Some(right),
            (None, None) => None,
        };
        allowed_sources = intersect_sources(
            allowed_sources,
            normalize_allowed_sources(input.allowed_sources.as_deref()),
        );
        denied_sources.extend(input.denied_sources.clone());
        allowed_mcp_servers = intersect_servers(
            allowed_mcp_servers,
            Some(normalize_vec_strings(
                input.allowed_mcp_servers.as_deref().unwrap_or(&[]),
            ))
            .filter(|servers| !servers.is_empty()),
        );
        inputs.push(EffectiveToolPolicyInputSummary {
            source: input.source.clone(),
            label: input.label.clone(),
            denied_tool_names: normalize_vec_strings(&input.denied_tool_names),
            denied_categories: {
                let mut normalized = input.denied_categories.clone();
                normalized.sort();
                normalized.dedup();
                normalized
            },
            allowed_categories: normalize_allowed_categories(input.allowed_categories.as_deref()),
            allowed_sources: normalize_allowed_sources(input.allowed_sources.as_deref()),
            denied_sources: {
                let mut normalized = input.denied_sources.clone();
                normalized.sort();
                normalized.dedup();
                normalized
            },
            allowed_mcp_servers: input
                .allowed_mcp_servers
                .as_ref()
                .map(|servers| normalize_vec_strings(servers))
                .filter(|servers| !servers.is_empty()),
        });
    }

    denied_tool_names = normalize_vec_strings(&denied_tool_names);
    denied_categories.sort();
    denied_categories.dedup();
    denied_sources.sort();
    denied_sources.dedup();
    if let Some(categories) = allowed_categories.as_mut() {
        categories.sort();
        categories.dedup();
    }
    if let Some(sources) = allowed_sources.as_mut() {
        sources.sort();
        sources.dedup();
    }
    if let Some(servers) = allowed_mcp_servers.as_mut() {
        servers.sort();
        servers.dedup();
    }

    (
        EffectiveToolSetFilters {
            denied_tool_names: denied_tool_names.clone(),
            denied_categories: denied_categories.clone(),
            allowed_categories: allowed_categories.clone(),
            allowed_sources: allowed_sources.clone(),
            denied_sources: denied_sources.clone(),
        },
        allowed_mcp_servers.clone(),
        EffectiveToolPolicySummary {
            denied_tool_names,
            denied_categories,
            allowed_categories,
            allowed_sources,
            denied_sources,
            allowed_mcp_servers,
            inputs,
        },
    )
}

pub(crate) fn resolve_effective_tool_set(
    registry: &ToolRegistry,
    explicit_allowed_tools: Option<Vec<String>>,
    profile: Option<ToolProfileName>,
    policy_inputs: &[EffectiveToolPolicyInput],
) -> EffectiveToolSet {
    let (filters, allowed_mcp_servers, policy) = merge_policy_inputs(policy_inputs);
    if let Some(requested_tools) = explicit_allowed_tools {
        let (tool_names, tool_manifest, missing_tools, filtered_out_tools, excluded_tools) =
            resolve_requested_tools_with_filters(
                registry,
                requested_tools,
                allowed_mcp_servers.as_deref(),
                &filters,
            );
        return EffectiveToolSet {
            source: EffectiveToolSetSource::ExplicitAllowList,
            allowed_tools: Some(tool_names.clone()),
            active_tools: tool_names.clone(),
            active_tool_manifest: tool_manifest.clone(),
            recommended_tools: Vec::new(),
            supporting_tools: Vec::new(),
            deferred_tools: Vec::new(),
            loading_policy: ToolLoadingPolicy::Full,
            expanded_to_full: false,
            expansion_reason: None,
            source_counts: build_source_counts(&tool_manifest),
            tool_names,
            tool_manifest,
            missing_tools,
            filtered_out_tools,
            excluded_tools,
            policy,
        };
    }

    if let Some(profile_name) = profile {
        let requested_tools = resolve_tool_profile(registry, profile_name);
        let (tool_names, tool_manifest, missing_tools, filtered_out_tools, excluded_tools) =
            resolve_requested_tools_with_filters(
                registry,
                requested_tools,
                allowed_mcp_servers.as_deref(),
                &filters,
            );
        return EffectiveToolSet {
            source: EffectiveToolSetSource::Profile(profile_name),
            allowed_tools: Some(tool_names.clone()),
            active_tools: tool_names.clone(),
            active_tool_manifest: tool_manifest.clone(),
            recommended_tools: Vec::new(),
            supporting_tools: Vec::new(),
            deferred_tools: Vec::new(),
            loading_policy: ToolLoadingPolicy::Full,
            expanded_to_full: false,
            expansion_reason: None,
            source_counts: build_source_counts(&tool_manifest),
            tool_names,
            tool_manifest,
            missing_tools,
            filtered_out_tools,
            excluded_tools,
            policy,
        };
    }

    let (tool_names, tool_manifest, filtered_out_tools, excluded_tools) = filter_mcp_tools(
        registry.tool_names(),
        registry.tool_manifest_entries(),
        allowed_mcp_servers.as_deref(),
        &filters,
    );
    EffectiveToolSet {
        source: EffectiveToolSetSource::RegistryDefault,
        allowed_tools: (!filtered_out_tools.is_empty()).then(|| tool_names.clone()),
        active_tools: tool_names.clone(),
        active_tool_manifest: tool_manifest.clone(),
        recommended_tools: Vec::new(),
        supporting_tools: Vec::new(),
        deferred_tools: Vec::new(),
        loading_policy: ToolLoadingPolicy::Full,
        expanded_to_full: false,
        expansion_reason: None,
        source_counts: build_source_counts(&tool_manifest),
        tool_names,
        tool_manifest,
        missing_tools: Vec::new(),
        filtered_out_tools,
        excluded_tools,
        policy,
    }
}

fn resolve_requested_tools_with_filters(
    registry: &ToolRegistry,
    requested_tools: Vec<String>,
    allowed_mcp_servers: Option<&[String]>,
    filters: &EffectiveToolSetFilters,
) -> (
    Vec<String>,
    Vec<ToolManifestEntry>,
    Vec<String>,
    Vec<String>,
    Vec<EffectiveToolExclusion>,
) {
    let mut tool_names = Vec::new();
    let mut tool_manifest = Vec::new();
    let mut missing_tools = Vec::new();
    let mut filtered_out_tools = Vec::new();
    let mut excluded_tools = Vec::new();

    for tool_name in requested_tools {
        if tool_names.contains(&tool_name)
            || missing_tools.contains(&tool_name)
            || filtered_out_tools.contains(&tool_name)
        {
            continue;
        }

        if let Some(tool) = registry.get(&tool_name) {
            let manifest_entry =
                ToolManifestEntry::from_parts(&tool_name, tool.description(), tool.metadata());
            if let Some(reason) = exclusion_reason(&manifest_entry, allowed_mcp_servers, filters) {
                excluded_tools.push(EffectiveToolExclusion {
                    name: tool_name.clone(),
                    source: Some(manifest_entry.source),
                    category: Some(manifest_entry.category),
                    reason,
                });
                filtered_out_tools.push(tool_name);
            } else {
                tool_manifest.push(manifest_entry);
                tool_names.push(tool_name);
            }
        } else {
            excluded_tools.push(EffectiveToolExclusion {
                name: tool_name.clone(),
                source: None,
                category: None,
                reason: ToolFilterReason::MissingFromRegistry,
            });
            missing_tools.push(tool_name);
        }
    }

    (
        tool_names,
        tool_manifest,
        missing_tools,
        filtered_out_tools,
        excluded_tools,
    )
}

fn filter_mcp_tools(
    tool_names: Vec<String>,
    tool_manifest: Vec<ToolManifestEntry>,
    allowed_mcp_servers: Option<&[String]>,
    filters: &EffectiveToolSetFilters,
) -> (
    Vec<String>,
    Vec<ToolManifestEntry>,
    Vec<String>,
    Vec<EffectiveToolExclusion>,
) {
    let mut filtered_names = Vec::new();
    let mut filtered_manifest = Vec::new();
    let mut filtered_out = Vec::new();
    let mut excluded_tools = Vec::new();
    let manifest_by_name = tool_manifest
        .iter()
        .map(|entry| (entry.name.clone(), (entry.source, entry.category)))
        .collect::<std::collections::HashMap<_, _>>();

    for name in tool_names {
        let exclusion_reason =
            manifest_by_name
                .get(&name)
                .copied()
                .and_then(|(source, category)| {
                    exclusion_reason(
                        &ToolManifestEntry {
                            name: name.clone(),
                            description: String::new(),
                            display_name: name.clone(),
                            category,
                            read_only: false,
                            destructive: false,
                            concurrency_safe: false,
                            open_world: false,
                            requires_approval: false,
                            source,
                        },
                        allowed_mcp_servers,
                        filters,
                    )
                });
        if exclusion_reason.is_none() {
            filtered_names.push(name);
        } else {
            excluded_tools.push(EffectiveToolExclusion {
                source: manifest_by_name.get(&name).map(|(source, _)| *source),
                category: manifest_by_name.get(&name).map(|(_, category)| *category),
                name: name.clone(),
                reason: exclusion_reason.expect("checked is_some"),
            });
            filtered_out.push(name);
        }
    }

    for entry in tool_manifest {
        if exclusion_reason(&entry, allowed_mcp_servers, filters).is_none() {
            filtered_manifest.push(entry);
        }
    }

    (
        filtered_names,
        filtered_manifest,
        filtered_out,
        excluded_tools,
    )
}

fn is_mcp_tool_allowed(tool_name: &str, allowed_mcp_servers: Option<&[String]>) -> bool {
    if !tool_name.starts_with("mcp_") {
        return true;
    }

    let Some(allowed_servers) = allowed_mcp_servers else {
        return true;
    };
    if allowed_servers.is_empty() {
        return false;
    }

    allowed_servers
        .iter()
        .any(|server_name| matches_mcp_server(tool_name, server_name))
}

fn exclusion_reason(
    manifest_entry: &ToolManifestEntry,
    allowed_mcp_servers: Option<&[String]>,
    filters: &EffectiveToolSetFilters,
) -> Option<ToolFilterReason> {
    if filters
        .denied_tool_names
        .iter()
        .any(|name| name == &manifest_entry.name)
    {
        return Some(ToolFilterReason::ExplicitDenyList);
    }
    if filters
        .denied_categories
        .iter()
        .any(|category| category == &manifest_entry.category)
    {
        return Some(ToolFilterReason::CategoryFiltered);
    }
    if let Some(allowed_categories) = filters.allowed_categories.as_deref() {
        if !allowed_categories.contains(&manifest_entry.category) {
            return Some(ToolFilterReason::AllowedCategoryFiltered);
        }
    }
    if filters
        .denied_sources
        .iter()
        .any(|source| source == &manifest_entry.source)
    {
        return Some(ToolFilterReason::DeniedSourceFiltered);
    }
    if let Some(allowed_sources) = filters.allowed_sources.as_deref() {
        if !allowed_sources.contains(&manifest_entry.source) {
            return Some(ToolFilterReason::SourceFiltered);
        }
    }
    if !is_mcp_tool_allowed(&manifest_entry.name, allowed_mcp_servers) {
        return Some(ToolFilterReason::McpServerFiltered);
    }
    None
}

fn matches_mcp_server(tool_name: &str, server_name: &str) -> bool {
    let normalized_server = normalize_mcp_server_name(server_name);
    if normalized_server.is_empty() {
        return false;
    }
    tool_name == format!("mcp_{normalized_server}")
        || tool_name.starts_with(&format!("mcp_{normalized_server}_"))
}

fn normalize_mcp_server_name(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

fn build_source_counts(tool_manifest: &[ToolManifestEntry]) -> Vec<EffectiveToolSourceCount> {
    let mut counts = std::collections::BTreeMap::new();
    for entry in tool_manifest {
        *counts.entry(entry.source).or_insert(0usize) += 1;
    }

    counts
        .into_iter()
        .map(|(source, count)| EffectiveToolSourceCount { source, count })
        .collect()
}

fn build_reason_counts(excluded_tools: &[EffectiveToolExclusion]) -> Vec<EffectiveToolReasonCount> {
    let mut counts = std::collections::BTreeMap::new();
    for entry in excluded_tools {
        *counts.entry(entry.reason).or_insert(0usize) += 1;
    }

    counts
        .into_iter()
        .map(|(reason, count)| EffectiveToolReasonCount { reason, count })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_effective_tool_set, session_tool_policy_input, skill_tool_policy_input,
        EffectiveToolSetSource, ToolFilterReason, ToolLoadingPolicy,
    };
    use crate::agent::permissions::PermissionMode;
    use crate::agent::runtime::tool_catalog::ToolDiscoveryCandidateRecord;
    use crate::agent::runtime::tool_profiles::ToolProfileName;
    use crate::agent::runtime::tool_registry_builder::{
        RuntimeToolRegistryBuilder, DEFAULT_BROWSER_SIDECAR_URL,
    };
    use crate::agent::tool_manifest::{ToolCategory, ToolSource};
    use crate::agent::tools::{ProcessManager, TaskTool};
    use crate::agent::ToolRegistry;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn effective_tool_set_uses_registry_default_when_no_filters_are_present() {
        let registry = ToolRegistry::with_standard_tools();

        let effective = resolve_effective_tool_set(&registry, None, None, &[]);

        assert_eq!(effective.source, EffectiveToolSetSource::RegistryDefault);
        assert!(effective.allowed_tools.is_none());
        assert!(effective.tool_names.contains(&"read_file".to_string()));
        assert!(effective.missing_tools.is_empty());
        assert!(effective.filtered_out_tools.is_empty());
        assert!(effective.excluded_tools.is_empty());
        assert_eq!(effective.active_tools, effective.tool_names);
        assert!(effective.deferred_tools.is_empty());
        assert_eq!(effective.loading_policy, ToolLoadingPolicy::Full);
    }

    #[test]
    fn effective_tool_set_filters_missing_explicit_tools_and_preserves_order() {
        let registry = ToolRegistry::with_standard_tools();

        let effective = resolve_effective_tool_set(
            &registry,
            Some(vec![
                "write_file".to_string(),
                "missing_tool".to_string(),
                "read_file".to_string(),
                "write_file".to_string(),
            ]),
            Some(ToolProfileName::Browser),
            &[],
        );

        assert_eq!(effective.source, EffectiveToolSetSource::ExplicitAllowList);
        assert_eq!(
            effective.allowed_tools,
            Some(vec!["write_file".to_string(), "read_file".to_string()])
        );
        assert_eq!(
            effective.tool_names,
            vec!["write_file".to_string(), "read_file".to_string()]
        );
        assert_eq!(effective.missing_tools, vec!["missing_tool".to_string()]);
        assert!(effective.filtered_out_tools.is_empty());
        assert_eq!(
            effective
                .exclusions_for_reason(ToolFilterReason::MissingFromRegistry)
                .len(),
            1
        );
        assert_eq!(effective.tool_manifest.len(), 2);
        assert_eq!(effective.tool_manifest[0].name, "write_file");
        assert_eq!(effective.tool_manifest[1].name, "read_file");
        assert_eq!(effective.active_tools, effective.tool_names);
    }

    #[tokio::test]
    async fn effective_tool_set_can_resolve_named_profiles_into_runtime_tools() {
        let registry = Arc::new(ToolRegistry::with_standard_tools());
        let builder = RuntimeToolRegistryBuilder::new(registry.as_ref());
        builder.register_process_shell_tools(Arc::new(ProcessManager::new()));
        builder.register_browser_and_alias_tools(DEFAULT_BROWSER_SIDECAR_URL);
        builder.register_skill_and_compaction_tools("sess-effective", Vec::new(), 2);
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let memory_dir = tempdir().expect("temp dir");
        let task_tool = TaskTool::new(
            Arc::clone(&registry),
            "openai".to_string(),
            "https://example.com".to_string(),
            "test-key".to_string(),
            "gpt-4o-mini".to_string(),
        );
        builder.register_runtime_support_tools(task_tool, db, memory_dir.path().to_path_buf());

        let effective = resolve_effective_tool_set(
            registry.as_ref(),
            None,
            Some(ToolProfileName::Browser),
            &[],
        );

        assert_eq!(
            effective.source,
            EffectiveToolSetSource::Profile(ToolProfileName::Browser)
        );
        assert_eq!(effective.allowed_tools, Some(effective.tool_names.clone()));
        assert!(effective.tool_names.contains(&"browser_launch".to_string()));
        assert!(effective
            .tool_names
            .contains(&"browser_snapshot".to_string()));
        assert!(effective.missing_tools.is_empty());
    }

    #[test]
    fn effective_tool_set_filters_mcp_tools_by_allowed_server_names() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(FakeTool {
            name: "mcp_repo_files_read".to_string(),
            description: "Read from repo MCP".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            source: ToolSource::Mcp,
            category: ToolCategory::Integration,
        }));
        registry.register(Arc::new(FakeTool {
            name: "mcp_brave_search_web".to_string(),
            description: "Search from brave MCP".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            source: ToolSource::Mcp,
            category: ToolCategory::Search,
        }));
        registry.register(Arc::new(FakeTool {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            source: ToolSource::Native,
            category: ToolCategory::File,
        }));

        let effective = resolve_effective_tool_set(
            &registry,
            None,
            None,
            &[skill_tool_policy_input(
                Vec::new(),
                Vec::new(),
                None,
                None,
                Vec::new(),
                Some(vec!["repo-files".to_string()]),
            )],
        );

        assert!(effective
            .tool_names
            .contains(&"mcp_repo_files_read".to_string()));
        assert!(effective.tool_names.contains(&"read_file".to_string()));
        assert!(!effective
            .tool_names
            .contains(&"mcp_brave_search_web".to_string()));
        assert!(effective
            .filtered_out_tools
            .contains(&"mcp_brave_search_web".to_string()));
        assert_eq!(
            effective.allowed_tools,
            Some(vec![
                "mcp_repo_files_read".to_string(),
                "read_file".to_string()
            ])
        );
        assert_eq!(
            effective
                .exclusions_for_reason(ToolFilterReason::McpServerFiltered)
                .len(),
            1
        );
        assert_eq!(effective.source_counts.len(), 2);
    }

    #[test]
    fn effective_tool_set_respects_explicit_deny_list() {
        let registry = ToolRegistry::with_standard_tools();
        let effective = resolve_effective_tool_set(
            &registry,
            Some(vec![
                "read_file".to_string(),
                "bash".to_string(),
                "write_file".to_string(),
            ]),
            None,
            &[skill_tool_policy_input(
                vec!["bash".to_string(), "write_file".to_string()],
                Vec::new(),
                None,
                None,
                Vec::new(),
                None,
            )],
        );

        assert_eq!(effective.tool_names, vec!["read_file".to_string()]);
        assert_eq!(
            effective
                .exclusions_for_reason(ToolFilterReason::ExplicitDenyList)
                .len(),
            2
        );
    }

    #[test]
    fn effective_tool_set_respects_allowed_sources_filter() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(FakeTool {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            source: ToolSource::Native,
            category: ToolCategory::File,
        }));
        registry.register(Arc::new(FakeTool {
            name: "mcp_repo_files_read".to_string(),
            description: "Read from repo MCP".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            source: ToolSource::Mcp,
            category: ToolCategory::File,
        }));
        let effective = resolve_effective_tool_set(
            &registry,
            None,
            None,
            &[skill_tool_policy_input(
                Vec::new(),
                Vec::new(),
                None,
                Some(vec![ToolSource::Mcp]),
                Vec::new(),
                None,
            )],
        );

        assert_eq!(
            effective.tool_names,
            vec!["mcp_repo_files_read".to_string()]
        );
        assert_eq!(
            effective
                .exclusions_for_reason(ToolFilterReason::SourceFiltered)
                .len(),
            1
        );
    }

    #[test]
    fn effective_tool_set_respects_denied_categories_filter() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(FakeTool {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            source: ToolSource::Native,
            category: ToolCategory::File,
        }));
        registry.register(Arc::new(FakeTool {
            name: "bash".to_string(),
            description: "Run shell".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            source: ToolSource::Runtime,
            category: ToolCategory::Shell,
        }));
        let effective = resolve_effective_tool_set(
            &registry,
            None,
            None,
            &[skill_tool_policy_input(
                Vec::new(),
                vec![ToolCategory::Shell],
                None,
                None,
                Vec::new(),
                None,
            )],
        );

        assert_eq!(effective.tool_names, vec!["read_file".to_string()]);
        assert_eq!(
            effective
                .exclusions_for_reason(ToolFilterReason::CategoryFiltered)
                .len(),
            1
        );
    }

    #[test]
    fn effective_tool_set_respects_allowed_categories_filter() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(FakeTool {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            source: ToolSource::Native,
            category: ToolCategory::File,
        }));
        registry.register(Arc::new(FakeTool {
            name: "web_fetch".to_string(),
            description: "Fetch a webpage".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            source: ToolSource::Native,
            category: ToolCategory::Web,
        }));

        let effective = resolve_effective_tool_set(
            &registry,
            None,
            None,
            &[skill_tool_policy_input(
                Vec::new(),
                Vec::new(),
                Some(vec![ToolCategory::File]),
                None,
                Vec::new(),
                None,
            )],
        );

        assert_eq!(effective.tool_names, vec!["read_file".to_string()]);
        assert_eq!(
            effective
                .exclusions_for_reason(ToolFilterReason::AllowedCategoryFiltered)
                .len(),
            1
        );
    }

    #[test]
    fn effective_tool_set_respects_denied_sources_filter() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(FakeTool {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            source: ToolSource::Native,
            category: ToolCategory::File,
        }));
        registry.register(Arc::new(FakeTool {
            name: "bash".to_string(),
            description: "Run shell".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            source: ToolSource::Runtime,
            category: ToolCategory::Shell,
        }));

        let effective = resolve_effective_tool_set(
            &registry,
            None,
            None,
            &[skill_tool_policy_input(
                Vec::new(),
                Vec::new(),
                None,
                None,
                vec![ToolSource::Runtime],
                None,
            )],
        );

        assert_eq!(effective.tool_names, vec!["read_file".to_string()]);
        assert_eq!(
            effective
                .exclusions_for_reason(ToolFilterReason::DeniedSourceFiltered)
                .len(),
            1
        );
    }

    #[test]
    fn effective_tool_set_summary_keeps_policy_filters() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(FakeTool {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            source: ToolSource::Native,
            category: ToolCategory::File,
        }));
        registry.register(Arc::new(FakeTool {
            name: "mcp_repo_files_read".to_string(),
            description: "Read from repo MCP".to_string(),
            schema: serde_json::json!({ "type": "object" }),
            source: ToolSource::Mcp,
            category: ToolCategory::Integration,
        }));
        let summary = resolve_effective_tool_set(
            &registry,
            None,
            None,
            &[skill_tool_policy_input(
                vec!["read_file".to_string()],
                vec![ToolCategory::Shell],
                None,
                Some(vec![ToolSource::Mcp]),
                Vec::new(),
                Some(vec!["repo-files".to_string()]),
            )],
        )
        .summary();

        assert_eq!(
            summary.policy.denied_tool_names,
            vec!["read_file".to_string()]
        );
        assert_eq!(summary.policy.denied_categories, vec![ToolCategory::Shell]);
        assert_eq!(summary.policy.allowed_sources, Some(vec![ToolSource::Mcp]));
        assert_eq!(
            summary.policy.allowed_mcp_servers,
            Some(vec!["repo-files".to_string()])
        );
        assert_eq!(summary.policy.inputs.len(), 1);
        assert_eq!(summary.active_tool_count, summary.allowed_tool_count);
        assert_eq!(summary.loading_policy, ToolLoadingPolicy::Full);
    }

    #[test]
    fn effective_tool_set_summary_keeps_multiple_policy_inputs() {
        let registry = ToolRegistry::with_standard_tools();

        let summary = resolve_effective_tool_set(
            &registry,
            Some(vec!["read_file".to_string()]),
            None,
            &[
                session_tool_policy_input(PermissionMode::AcceptEdits),
                skill_tool_policy_input(
                    vec!["bash".to_string()],
                    vec![ToolCategory::Shell],
                    None,
                    Some(vec![ToolSource::Native, ToolSource::Mcp]),
                    Vec::new(),
                    Some(vec!["repo-files".to_string()]),
                ),
            ],
        )
        .summary();

        assert_eq!(summary.policy.inputs.len(), 2);
        assert_eq!(
            summary.policy.inputs[0].label,
            "permission_mode:accept_edits"
        );
        assert_eq!(summary.policy.inputs[1].label, "skill_declared_filters");
        assert_eq!(
            summary.policy.allowed_mcp_servers,
            Some(vec!["repo-files".to_string()])
        );
        assert_eq!(summary.active_tool_count, summary.allowed_tool_count);
        assert!(summary.deferred_tools.is_empty());
    }

    #[test]
    fn effective_tool_set_can_apply_recommended_tool_loading_plan() {
        let registry = ToolRegistry::with_standard_tools();
        let mut effective = resolve_effective_tool_set(&registry, None, None, &[]);
        let candidates = vec![
            ToolDiscoveryCandidateRecord {
                name: "web_fetch".to_string(),
                category: ToolCategory::Web,
                source: ToolSource::Native,
                score: 12,
                stage: crate::agent::runtime::tool_catalog::ToolRecommendationStage::Primary,
                matched_terms: vec!["fetch".to_string()],
                matched_fields: vec!["name".to_string()],
            },
            ToolDiscoveryCandidateRecord {
                name: "read_file".to_string(),
                category: ToolCategory::File,
                source: ToolSource::Native,
                score: 10,
                stage: crate::agent::runtime::tool_catalog::ToolRecommendationStage::Primary,
                matched_terms: vec!["read".to_string()],
                matched_fields: vec!["name".to_string()],
            },
            ToolDiscoveryCandidateRecord {
                name: "glob".to_string(),
                category: ToolCategory::File,
                source: ToolSource::Alias,
                score: 8,
                stage: crate::agent::runtime::tool_catalog::ToolRecommendationStage::Supporting,
                matched_terms: vec!["glob".to_string()],
                matched_fields: vec!["name".to_string()],
            },
        ];

        effective.apply_recommended_tools(&candidates);

        assert_eq!(
            effective.loading_policy,
            ToolLoadingPolicy::RecommendedPlusCoreSafeTools
        );
        assert!(effective.active_tools.contains(&"web_fetch".to_string()));
        assert!(effective
            .recommended_tools
            .contains(&"read_file".to_string()));
        assert!(effective.supporting_tools.contains(&"glob".to_string()));
        assert!(effective.active_tools.contains(&"glob".to_string()));
        assert!(!effective.deferred_tools.is_empty());
        assert_eq!(
            effective.allowed_tools,
            Some(effective.active_tools.clone())
        );
    }

    #[test]
    fn effective_tool_set_falls_back_to_full_when_recommendation_is_empty() {
        let registry = ToolRegistry::with_standard_tools();
        let mut effective = resolve_effective_tool_set(&registry, None, None, &[]);

        effective.apply_recommended_tools(&[]);

        assert_eq!(effective.loading_policy, ToolLoadingPolicy::Full);
        assert_eq!(effective.active_tools, effective.tool_names);
        assert!(effective.deferred_tools.is_empty());
    }

    struct FakeTool {
        name: String,
        description: String,
        schema: serde_json::Value,
        source: ToolSource,
        category: ToolCategory,
    }

    impl crate::agent::Tool for FakeTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        fn input_schema(&self) -> serde_json::Value {
            self.schema.clone()
        }

        fn execute(
            &self,
            _input: serde_json::Value,
            _ctx: &crate::agent::ToolContext,
        ) -> anyhow::Result<String> {
            Ok("ok".to_string())
        }

        fn metadata(&self) -> crate::agent::tool_manifest::ToolMetadata {
            crate::agent::tool_manifest::ToolMetadata {
                source: self.source,
                category: self.category,
                ..Default::default()
            }
        }
    }
}
