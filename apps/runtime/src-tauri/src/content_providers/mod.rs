mod agent_reach;
mod result;
mod router;
mod types;

pub use agent_reach::{
    build_agent_reach_source_status, detect_agent_reach_mcp_servers, detect_agent_reach_provider,
    inspect_agent_reach_source, CommandResult, DiagnosticsRunner, ProcessDiagnosticsRunner,
};
pub use result::{ContentArtifact, NormalizedContentResult};
pub use router::{route_content_request, RouteDecision};
pub use types::{
    ContentCapability, ContentRequest, DetectedExternalMcpServer, ExternalCapabilityChannel,
    ExternalCapabilitySourceStatus, ProviderAvailability, ProviderStatus,
};
