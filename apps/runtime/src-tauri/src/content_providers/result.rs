use super::types::ContentCapability;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentArtifact {
    pub kind: String,
    pub uri: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedContentResult {
    pub source_provider: String,
    pub capability: ContentCapability,
    pub title: Option<String>,
    pub url: Option<String>,
    pub text: String,
    pub markdown: Option<String>,
    pub metadata: Value,
    pub artifacts: Vec<ContentArtifact>,
}
