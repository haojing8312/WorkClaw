#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    File,
    Shell,
    Web,
    Browser,
    System,
    Planning,
    Agent,
    Memory,
    Search,
    Integration,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSource {
    Native,
    Runtime,
    Sidecar,
    Mcp,
    Plugin,
    Alias,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolMetadata {
    pub display_name: Option<String>,
    pub category: ToolCategory,
    pub read_only: bool,
    pub destructive: bool,
    pub concurrency_safe: bool,
    pub open_world: bool,
    pub requires_approval: bool,
    pub source: ToolSource,
}

impl Default for ToolMetadata {
    fn default() -> Self {
        Self {
            display_name: None,
            category: ToolCategory::Other,
            read_only: false,
            destructive: false,
            concurrency_safe: false,
            open_world: false,
            requires_approval: false,
            source: ToolSource::Native,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolManifestEntry {
    pub name: String,
    pub description: String,
    pub display_name: String,
    pub category: ToolCategory,
    pub read_only: bool,
    pub destructive: bool,
    pub concurrency_safe: bool,
    pub open_world: bool,
    pub requires_approval: bool,
    pub source: ToolSource,
}

impl ToolManifestEntry {
    pub fn from_parts(name: &str, description: &str, metadata: ToolMetadata) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            display_name: metadata
                .display_name
                .clone()
                .unwrap_or_else(|| name.to_string()),
            category: metadata.category,
            read_only: metadata.read_only,
            destructive: metadata.destructive,
            concurrency_safe: metadata.concurrency_safe,
            open_world: metadata.open_world,
            requires_approval: metadata.requires_approval,
            source: metadata.source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ToolCategory, ToolManifestEntry, ToolMetadata, ToolSource};
    use serde_json::json;

    #[test]
    fn manifest_entry_falls_back_to_tool_name_for_display_name() {
        let entry = ToolManifestEntry::from_parts(
            "read_file",
            "Read a file",
            ToolMetadata {
                category: ToolCategory::File,
                read_only: true,
                ..ToolMetadata::default()
            },
        );

        assert_eq!(entry.display_name, "read_file");
        assert_eq!(entry.category, ToolCategory::File);
        assert!(entry.read_only);
    }

    #[test]
    fn manifest_entry_preserves_explicit_display_name_and_flags() {
        let entry = ToolManifestEntry::from_parts(
            "bash",
            "Run a shell command",
            ToolMetadata {
                display_name: Some("Shell".to_string()),
                category: ToolCategory::Shell,
                destructive: true,
                requires_approval: true,
                source: ToolSource::Runtime,
                ..ToolMetadata::default()
            },
        );

        assert_eq!(entry.display_name, "Shell");
        assert!(entry.destructive);
        assert!(entry.requires_approval);
        assert_eq!(entry.source, ToolSource::Runtime);
    }

    #[test]
    fn metadata_serializes_category_and_source_as_snake_case() {
        let value = serde_json::to_value(ToolMetadata {
            category: ToolCategory::Browser,
            source: ToolSource::Sidecar,
            ..ToolMetadata::default()
        })
        .expect("serialize metadata");

        assert_eq!(
            value,
            json!({
                "display_name": null,
                "category": "browser",
                "read_only": false,
                "destructive": false,
                "concurrency_safe": false,
                "open_world": false,
                "requires_approval": false,
                "source": "sidecar"
            })
        );
    }
}
