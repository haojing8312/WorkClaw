pub mod service;
pub mod traits;
pub mod types;

pub use service::{
    classify_model_route_error, compose_system_prompt, infer_capability_from_user_message,
    normalize_permission_mode_for_storage, normalize_session_mode_for_storage,
    normalize_team_id_for_storage, parse_fallback_chain_targets, parse_permission_mode_for_runtime,
    permission_mode_label, retry_backoff_ms, retry_budget_for_error, should_retry_same_candidate,
    ChatExecutionPreparationService, ChatPreparationService,
};
pub use traits::{ChatEmployeeDirectory, ChatSessionContextRepository, ChatSettingsRepository};
pub use types::{
    ChatEmployeeSnapshot, ChatExecutionContext, ChatExecutionGuidance,
    ChatExecutionPreparationRequest, ChatPermissionMode, ChatPreparationRequest,
    ChatRoutePolicySnapshot, ChatRoutingSnapshot, ModelRouteErrorKind, PreparedChatExecution,
    PreparedChatExecutionAssembly, PreparedRouteCandidate, PreparedRouteCandidates,
    PreparedSessionCreation, ProviderConnectionSnapshot, RoutingSettingsSnapshot,
    SessionCreationRequest, SessionExecutionContextSnapshot, SessionModelSnapshot,
};
