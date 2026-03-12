use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentCapability {
    ReadUrl,
    SearchContent,
    ExtractMediaContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentRequest {
    ReadUrl {
        url: String,
    },
    SearchContent {
        query: String,
        platform: Option<String>,
    },
    ExtractMediaContext {
        url: String,
    },
    BrowserInteract {
        action: String,
    },
}

impl ContentRequest {
    pub fn capability(&self) -> Option<ContentCapability> {
        match self {
            Self::ReadUrl { .. } => Some(ContentCapability::ReadUrl),
            Self::SearchContent { .. } => Some(ContentCapability::SearchContent),
            Self::ExtractMediaContext { .. } => Some(ContentCapability::ExtractMediaContext),
            Self::BrowserInteract { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAvailability {
    NotFound,
    Partial,
    Available,
}

impl ProviderAvailability {
    pub fn is_usable(self) -> bool {
        matches!(self, Self::Partial | Self::Available)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub provider_id: String,
    pub availability: ProviderAvailability,
    pub capabilities: Vec<ContentCapability>,
    pub detail: Option<String>,
}

impl ProviderStatus {
    pub fn supports(&self, capability: &ContentCapability) -> bool {
        self.availability.is_usable() && self.capabilities.contains(capability)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalCapabilityChannel {
    pub channel: String,
    pub status: String,
    pub backend_type: String,
    pub backend_name: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalCapabilitySourceStatus {
    pub source_id: String,
    pub display_name: String,
    pub availability: ProviderAvailability,
    pub summary: String,
    pub channels: Vec<ExternalCapabilityChannel>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedExternalMcpServer {
    pub source_id: String,
    pub channel: String,
    pub server_name: String,
    pub display_name: String,
    pub status: String,
    pub backend_name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<String>,
    pub managed_by_workclaw: bool,
}
