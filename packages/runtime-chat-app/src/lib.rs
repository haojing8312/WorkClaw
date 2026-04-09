pub mod preparation;
pub mod prompt_assembly;
pub mod routing;
pub mod service;
pub mod traits;
pub mod types;

pub use preparation::{
    infer_capability_from_message_parts, infer_capability_from_user_message,
    normalize_permission_mode_for_storage, normalize_session_mode_for_storage,
    normalize_team_id_for_storage, parse_permission_mode_for_runtime, permission_mode_label,
};
pub use prompt_assembly::{
    build_system_prompt_sections, compose_system_prompt, compose_system_prompt_from_sections,
    compose_system_prompt_from_tool_names, SystemPromptSections,
};
pub use routing::{
    classify_model_route_error, parse_fallback_chain_targets, retry_backoff_ms,
    retry_budget_for_error, should_retry_same_candidate,
};
pub use service::{ChatExecutionPreparationService, ChatPreparationService};
pub use traits::{ChatEmployeeDirectory, ChatSessionContextRepository, ChatSettingsRepository};
pub use types::{
    ChatEmployeeSnapshot, ChatExecutionContext, ChatExecutionGuidance,
    ChatExecutionPreparationRequest, ChatPermissionMode, ChatPreparationRequest,
    ChatRoutePolicySnapshot, ChatRoutingSnapshot, ModelRouteErrorKind, PreparedChatExecution,
    PreparedChatExecutionAssembly, PreparedRouteCandidate, PreparedRouteCandidates,
    PreparedSessionCreation, ProviderConnectionSnapshot, RoutingSettingsSnapshot,
    SessionCreationRequest, SessionExecutionContextSnapshot, SessionModelSnapshot,
};
