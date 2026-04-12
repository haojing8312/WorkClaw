use crate::agent::tool_manifest::{ToolCategory, ToolSource};
use crate::agent::ToolManifestEntry;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolDiscoveryCandidateRecord {
    pub name: String,
    pub category: ToolCategory,
    pub source: ToolSource,
    pub score: i32,
    pub stage: ToolRecommendationStage,
    pub matched_terms: Vec<String>,
    pub matched_fields: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRecommendationStage {
    Primary,
    Supporting,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ToolCatalogCategorySummary {
    pub category: ToolCategory,
    pub count: usize,
    pub sample_tools: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ToolCatalogSearchHit {
    pub name: String,
    pub category: ToolCategory,
    pub source: ToolSource,
    pub score: i32,
    pub stage: ToolRecommendationStage,
    pub matched_terms: Vec<String>,
    pub matched_fields: Vec<String>,
}

pub(crate) fn summarize_tool_catalog(
    entries: &[ToolManifestEntry],
    max_categories: usize,
    max_tools_per_category: usize,
) -> Vec<ToolCatalogCategorySummary> {
    let mut grouped = BTreeMap::<ToolCategory, Vec<&ToolManifestEntry>>::new();
    for entry in entries {
        grouped.entry(entry.category).or_default().push(entry);
    }

    grouped
        .into_iter()
        .map(|(category, mut items)| {
            items.sort_by(|left, right| left.name.cmp(&right.name));
            ToolCatalogCategorySummary {
                category,
                count: items.len(),
                sample_tools: items
                    .into_iter()
                    .take(max_tools_per_category)
                    .map(|entry| entry.name.clone())
                    .collect(),
            }
        })
        .filter(|entry| entry.count > 0)
        .take(max_categories)
        .collect()
}

pub(crate) fn format_tool_discovery_index(entries: &[ToolManifestEntry]) -> Option<String> {
    if entries.len() < 8 {
        return None;
    }

    let categories = summarize_tool_catalog(entries, 4, 4);
    if categories.is_empty() {
        return None;
    }

    let mut lines = vec![
        "[工具发现索引]".to_string(),
        "当任务只需要某一类能力时，优先选择对应类别的工具。".to_string(),
    ];

    for category in categories {
        let sample = category.sample_tools.join(", ");
        lines.push(format!(
            "- {}: {} 个，例如 {}",
            category_label(category.category),
            category.count,
            sample
        ));
    }

    Some(lines.join("\n"))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn format_tool_candidate_hints(
    entries: &[ToolManifestEntry],
    query: &str,
    limit: usize,
) -> Option<String> {
    if entries.len() < 8 {
        return None;
    }

    let hits = discover_tool_candidates(entries, query, limit);
    if hits.is_empty() {
        return None;
    }

    format_tool_candidate_record_hints(
        &hits
            .into_iter()
            .map(|hit| ToolDiscoveryCandidateRecord {
                name: hit.name,
                category: hit.category,
                source: hit.source,
                score: hit.score,
                stage: hit.stage,
                matched_terms: hit.matched_terms,
                matched_fields: hit.matched_fields,
            })
            .collect::<Vec<_>>(),
        &[],
        limit,
    )
}

pub(crate) fn build_tool_candidate_records(
    entries: &[ToolManifestEntry],
    query: &str,
    limit: usize,
) -> Vec<ToolDiscoveryCandidateRecord> {
    discover_tool_candidates(entries, query, limit)
        .into_iter()
        .map(|hit| ToolDiscoveryCandidateRecord {
            name: hit.name,
            category: hit.category,
            source: hit.source,
            score: hit.score,
            stage: hit.stage,
            matched_terms: hit.matched_terms,
            matched_fields: hit.matched_fields,
        })
        .collect()
}

pub(crate) fn format_tool_candidate_record_hints(
    records: &[ToolDiscoveryCandidateRecord],
    active_tools: &[String],
    limit: usize,
) -> Option<String> {
    if records.is_empty() || limit == 0 {
        return None;
    }

    let mut lines = vec![
        "[当前任务候选工具]".to_string(),
        "基于当前任务描述，优先考虑以下工具；若不适合，再退回其它工具。".to_string(),
    ];

    for record in records.iter().take(limit) {
        let exposure = if active_tools.is_empty() || active_tools.contains(&record.name) {
            "首轮暴露"
        } else {
            "延后暴露"
        };
        lines.push(format!(
            "- {} [{} / {}]: {} / {}",
            record.name,
            stage_label(record.stage),
            exposure,
            category_label(record.category),
            source_label(record.source)
        ));
    }

    Some(lines.join("\n"))
}

pub(crate) fn discover_tool_candidates(
    entries: &[ToolManifestEntry],
    query: &str,
    limit: usize,
) -> Vec<ToolCatalogSearchHit> {
    let tokens = tokenize(query);
    if tokens.is_empty() || limit == 0 {
        return Vec::new();
    }

    let mut hits = entries
        .iter()
        .filter_map(|entry| {
            let detail = score_tool_match(entry, &tokens);
            (detail.score > 0).then(|| ToolCatalogSearchHit {
                name: entry.name.clone(),
                category: entry.category,
                source: entry.source,
                score: detail.score,
                stage: ToolRecommendationStage::Supporting,
                matched_terms: detail.matched_terms,
                matched_fields: detail.matched_fields,
            })
        })
        .collect::<Vec<_>>();

    hits.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.name.cmp(&right.name))
    });
    for (index, hit) in hits.iter_mut().enumerate() {
        hit.stage = if index < 2 || hit.score >= 14 {
            ToolRecommendationStage::Primary
        } else if index < 5 || hit.score >= 8 {
            ToolRecommendationStage::Supporting
        } else {
            ToolRecommendationStage::Fallback
        };
    }
    hits.truncate(limit);
    hits
}

fn stage_label(stage: ToolRecommendationStage) -> &'static str {
    match stage {
        ToolRecommendationStage::Primary => "主推荐",
        ToolRecommendationStage::Supporting => "补充",
        ToolRecommendationStage::Fallback => "扩展",
    }
}

#[derive(Debug, Default)]
struct ToolMatchDetail {
    score: i32,
    matched_terms: Vec<String>,
    matched_fields: Vec<String>,
}

fn score_tool_match(entry: &ToolManifestEntry, tokens: &[String]) -> ToolMatchDetail {
    let name = entry.name.to_ascii_lowercase();
    let display_name = entry.display_name.to_ascii_lowercase();
    let description = entry.description.to_ascii_lowercase();
    let category = category_label(entry.category).to_ascii_lowercase();
    let source = source_label(entry.source).to_ascii_lowercase();
    let mut detail = ToolMatchDetail::default();
    let category_hints = infer_category_hints(tokens);

    for token in tokens {
        let mut matched = false;
        if name == *token {
            detail.score += 12;
            push_unique(&mut detail.matched_fields, "name");
            matched = true;
        } else if name.contains(token) {
            detail.score += 8;
            push_unique(&mut detail.matched_fields, "name");
            matched = true;
        }
        if display_name.contains(token) {
            detail.score += 6;
            push_unique(&mut detail.matched_fields, "display_name");
            matched = true;
        }
        if description.contains(token) {
            detail.score += 4;
            push_unique(&mut detail.matched_fields, "description");
            matched = true;
        }
        if category.contains(token) {
            detail.score += 5;
            push_unique(&mut detail.matched_fields, "category");
            matched = true;
        }
        if source.contains(token) {
            detail.score += 3;
            push_unique(&mut detail.matched_fields, "source");
            matched = true;
        }
        if matched {
            push_unique_owned(&mut detail.matched_terms, token.clone());
        }
    }

    if category_hints.contains(&entry.category) {
        detail.score += 7;
        push_unique(&mut detail.matched_fields, "intent_category");
    }

    detail
}

fn infer_category_hints(tokens: &[String]) -> Vec<ToolCategory> {
    let mut categories = Vec::new();
    for token in tokens {
        let mapped = match token.as_str() {
            "file" | "files" | "read" | "write" | "edit" | "folder" | "directory" | "dir" => {
                Some(ToolCategory::File)
            }
            "bash" | "shell" | "terminal" | "command" | "commands" | "script" => {
                Some(ToolCategory::Shell)
            }
            "search" | "latest" | "news" | "lookup" | "find" => Some(ToolCategory::Search),
            "fetch" | "web" | "url" | "page" | "website" => Some(ToolCategory::Web),
            "browser" | "click" | "open" | "snapshot" | "submit" => Some(ToolCategory::Browser),
            "memory" | "remember" | "recall" => Some(ToolCategory::Memory),
            "plan" | "todo" | "steps" => Some(ToolCategory::Planning),
            "mcp" | "integration" | "plugin" => Some(ToolCategory::Integration),
            _ => None,
        };
        if let Some(category) = mapped {
            if !categories.contains(&category) {
                categories.push(category);
            }
        }
    }
    categories
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|item| item == value) {
        values.push(value.to_string());
    }
}

fn push_unique_owned(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|item| item == &value) {
        values.push(value);
    }
}

fn tokenize(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .map(|token| token.trim().to_ascii_lowercase())
        .filter(|token| !token.is_empty())
        .collect()
}

fn category_label(category: ToolCategory) -> &'static str {
    match category {
        ToolCategory::File => "文件类",
        ToolCategory::Shell => "Shell 类",
        ToolCategory::Web => "Web 类",
        ToolCategory::Browser => "浏览器类",
        ToolCategory::System => "系统类",
        ToolCategory::Planning => "规划类",
        ToolCategory::Agent => "Agent 类",
        ToolCategory::Memory => "记忆类",
        ToolCategory::Search => "搜索类",
        ToolCategory::Integration => "集成类",
        ToolCategory::Other => "其他类",
    }
}

fn source_label(source: ToolSource) -> &'static str {
    match source {
        ToolSource::Native => "native",
        ToolSource::Runtime => "runtime",
        ToolSource::Sidecar => "sidecar",
        ToolSource::Mcp => "mcp",
        ToolSource::Plugin => "plugin",
        ToolSource::Alias => "alias",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tool_candidate_records, discover_tool_candidates, format_tool_candidate_hints,
        format_tool_candidate_record_hints, format_tool_discovery_index, summarize_tool_catalog,
        ToolDiscoveryCandidateRecord, ToolRecommendationStage,
    };
    use crate::agent::tool_manifest::{ToolCategory, ToolMetadata, ToolSource};
    use crate::agent::ToolManifestEntry;

    fn entry(
        name: &str,
        description: &str,
        category: ToolCategory,
        source: ToolSource,
    ) -> ToolManifestEntry {
        ToolManifestEntry::from_parts(
            name,
            description,
            ToolMetadata {
                category,
                source,
                ..ToolMetadata::default()
            },
        )
    }

    #[test]
    fn summarize_tool_catalog_groups_by_category() {
        let entries = vec![
            entry(
                "read_file",
                "Read file",
                ToolCategory::File,
                ToolSource::Native,
            ),
            entry(
                "write_file",
                "Write file",
                ToolCategory::File,
                ToolSource::Native,
            ),
            entry(
                "browser_launch",
                "Launch browser",
                ToolCategory::Browser,
                ToolSource::Sidecar,
            ),
        ];

        let summary = summarize_tool_catalog(&entries, 4, 4);

        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].category, ToolCategory::File);
        assert_eq!(summary[0].count, 2);
        assert!(summary[0].sample_tools.contains(&"read_file".to_string()));
    }

    #[test]
    fn discover_tool_candidates_scores_name_and_category_matches() {
        let entries = vec![
            entry(
                "read_file",
                "Read file",
                ToolCategory::File,
                ToolSource::Native,
            ),
            entry(
                "browser_launch",
                "Launch browser",
                ToolCategory::Browser,
                ToolSource::Sidecar,
            ),
            entry(
                "mcp_repo_files_read",
                "Read repo via MCP",
                ToolCategory::Integration,
                ToolSource::Mcp,
            ),
        ];

        let hits = discover_tool_candidates(&entries, "browser", 3);

        assert_eq!(
            hits.first().map(|hit| hit.name.as_str()),
            Some("browser_launch")
        );
        assert!(!hits.is_empty());
        assert!(hits
            .first()
            .map(|hit| hit.matched_fields.contains(&"name".to_string()))
            .unwrap_or(false));
        assert_eq!(
            hits.first().map(|hit| hit.stage),
            Some(ToolRecommendationStage::Primary)
        );
    }

    #[test]
    fn format_tool_discovery_index_skips_small_tool_sets() {
        let entries = vec![
            entry(
                "read_file",
                "Read file",
                ToolCategory::File,
                ToolSource::Native,
            ),
            entry(
                "write_file",
                "Write file",
                ToolCategory::File,
                ToolSource::Native,
            ),
        ];

        assert!(format_tool_discovery_index(&entries).is_none());
    }

    #[test]
    fn format_tool_candidate_hints_returns_ranked_matches_for_large_tool_sets() {
        let entries = vec![
            entry(
                "read_file",
                "Read file",
                ToolCategory::File,
                ToolSource::Native,
            ),
            entry(
                "write_file",
                "Write file",
                ToolCategory::File,
                ToolSource::Native,
            ),
            entry("edit", "Edit file", ToolCategory::File, ToolSource::Native),
            entry(
                "list_dir",
                "List dir",
                ToolCategory::File,
                ToolSource::Native,
            ),
            entry(
                "browser_launch",
                "Launch browser",
                ToolCategory::Browser,
                ToolSource::Sidecar,
            ),
            entry(
                "browser_snapshot",
                "Snapshot browser",
                ToolCategory::Browser,
                ToolSource::Sidecar,
            ),
            entry(
                "web_search",
                "Search web",
                ToolCategory::Search,
                ToolSource::Runtime,
            ),
            entry(
                "web_fetch",
                "Fetch web",
                ToolCategory::Web,
                ToolSource::Native,
            ),
        ];

        let hints = format_tool_candidate_hints(&entries, "search the web for latest pricing", 3)
            .expect("candidate hints");
        assert!(hints.contains("[当前任务候选工具]"));
        assert!(hints.contains("web_search"));
        assert!(hints.contains("主推荐"));
    }

    #[test]
    fn build_tool_candidate_records_keeps_ranked_hits() {
        let entries = vec![
            entry(
                "read_file",
                "Read file",
                ToolCategory::File,
                ToolSource::Native,
            ),
            entry(
                "write_file",
                "Write file",
                ToolCategory::File,
                ToolSource::Native,
            ),
            entry("edit", "Edit file", ToolCategory::File, ToolSource::Native),
            entry(
                "list_dir",
                "List dir",
                ToolCategory::File,
                ToolSource::Native,
            ),
            entry(
                "browser_launch",
                "Launch browser",
                ToolCategory::Browser,
                ToolSource::Sidecar,
            ),
            entry(
                "browser_snapshot",
                "Snapshot browser",
                ToolCategory::Browser,
                ToolSource::Sidecar,
            ),
            entry(
                "web_search",
                "Search web",
                ToolCategory::Search,
                ToolSource::Runtime,
            ),
            entry(
                "web_fetch",
                "Fetch web",
                ToolCategory::Web,
                ToolSource::Native,
            ),
        ];

        let records = build_tool_candidate_records(&entries, "search latest web news", 2);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].name, "web_search");
        assert_eq!(records[0].stage, ToolRecommendationStage::Primary);
        assert!(records[0].matched_terms.contains(&"search".to_string()));
    }

    #[test]
    fn discover_tool_candidates_uses_intent_category_boosts() {
        let entries = vec![
            entry(
                "read_file",
                "Read file",
                ToolCategory::File,
                ToolSource::Native,
            ),
            entry(
                "web_search",
                "Search web",
                ToolCategory::Search,
                ToolSource::Runtime,
            ),
        ];

        let hits = discover_tool_candidates(&entries, "latest news", 2);
        assert_eq!(
            hits.first().map(|hit| hit.name.as_str()),
            Some("web_search")
        );
        assert!(hits[0]
            .matched_fields
            .contains(&"intent_category".to_string()));
    }

    #[test]
    fn discover_tool_candidates_assigns_fallback_stage_to_lower_ranked_matches() {
        let entries = vec![
            entry(
                "read_file",
                "Read file",
                ToolCategory::File,
                ToolSource::Native,
            ),
            entry(
                "write_file",
                "Write file",
                ToolCategory::File,
                ToolSource::Native,
            ),
            entry(
                "edit_file",
                "Edit file",
                ToolCategory::File,
                ToolSource::Native,
            ),
            entry(
                "list_dir",
                "List dir",
                ToolCategory::File,
                ToolSource::Native,
            ),
            entry(
                "web_fetch",
                "Fetch web",
                ToolCategory::Web,
                ToolSource::Native,
            ),
            entry(
                "web_search",
                "Search web",
                ToolCategory::Search,
                ToolSource::Runtime,
            ),
            entry(
                "browser_snapshot",
                "Snapshot browser",
                ToolCategory::Browser,
                ToolSource::Sidecar,
            ),
        ];

        let hits = discover_tool_candidates(&entries, "file read write edit list", 7);

        assert!(hits
            .iter()
            .any(|hit| hit.stage == ToolRecommendationStage::Fallback));
    }

    #[test]
    fn format_tool_candidate_record_hints_marks_deferred_exposure() {
        let hints = format_tool_candidate_record_hints(
            &[
                ToolDiscoveryCandidateRecord {
                    name: "web_search".to_string(),
                    category: ToolCategory::Search,
                    source: ToolSource::Runtime,
                    score: 18,
                    stage: ToolRecommendationStage::Primary,
                    matched_terms: vec!["search".to_string()],
                    matched_fields: vec!["name".to_string()],
                },
                ToolDiscoveryCandidateRecord {
                    name: "bash".to_string(),
                    category: ToolCategory::Shell,
                    source: ToolSource::Runtime,
                    score: 9,
                    stage: ToolRecommendationStage::Supporting,
                    matched_terms: vec!["command".to_string()],
                    matched_fields: vec!["description".to_string()],
                },
            ],
            &["web_search".to_string()],
            4,
        )
        .expect("record hints");

        assert!(hints.contains("主推荐 / 首轮暴露"));
        assert!(hints.contains("补充 / 延后暴露"));
    }
}
