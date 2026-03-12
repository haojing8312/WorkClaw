use crate::types::{
    ChatEmployeeSnapshot, ChatRoutePolicySnapshot, ChatRoutingSnapshot,
    ProviderConnectionSnapshot, RoutingSettingsSnapshot, SessionExecutionContextSnapshot,
    SessionModelSnapshot,
};
use async_trait::async_trait;

#[async_trait]
pub trait ChatSettingsRepository: Send + Sync {
    async fn load_routing_settings(&self) -> Result<RoutingSettingsSnapshot, String>;
    async fn load_chat_routing(&self) -> Result<Option<ChatRoutingSnapshot>, String>;
    async fn resolve_default_model_id(&self) -> Result<Option<String>, String>;
    async fn resolve_default_usable_model_id(&self) -> Result<Option<String>, String>;
    async fn load_route_policy(
        &self,
        capability: &str,
    ) -> Result<Option<ChatRoutePolicySnapshot>, String>;
    async fn get_provider_connection(
        &self,
        provider_id: &str,
    ) -> Result<Option<ProviderConnectionSnapshot>, String>;
    async fn load_session_model(&self, model_id: &str) -> Result<SessionModelSnapshot, String>;
    async fn load_default_work_dir(&self) -> Result<Option<String>, String> {
        Ok(None)
    }
    async fn load_imported_mcp_guidance(
        &self,
        _imported_mcp_server_ids: &[String],
    ) -> Result<Option<String>, String> {
        Ok(None)
    }
}

#[async_trait]
pub trait ChatSessionContextRepository: Send + Sync {
    async fn load_session_execution_context(
        &self,
        session_id: Option<&str>,
    ) -> Result<SessionExecutionContextSnapshot, String>;
}

#[async_trait]
pub trait ChatEmployeeDirectory: Send + Sync {
    async fn list_collaboration_candidates(&self) -> Result<Vec<ChatEmployeeSnapshot>, String>;
}
