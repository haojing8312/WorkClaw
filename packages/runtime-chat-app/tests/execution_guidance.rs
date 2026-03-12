use async_trait::async_trait;
use runtime_chat_app::{
    ChatExecutionGuidance, ChatExecutionPreparationRequest, ChatExecutionPreparationService,
    ChatSettingsRepository, ChatRoutePolicySnapshot, ChatRoutingSnapshot,
    ProviderConnectionSnapshot, RoutingSettingsSnapshot, SessionModelSnapshot,
};

struct FakeGuidanceRepo;

#[async_trait]
impl ChatSettingsRepository for FakeGuidanceRepo {
    async fn load_routing_settings(&self) -> Result<RoutingSettingsSnapshot, String> {
        Ok(RoutingSettingsSnapshot {
            max_call_depth: 4,
            node_timeout_seconds: 60,
            retry_count: 0,
        })
    }

    async fn load_chat_routing(&self) -> Result<Option<ChatRoutingSnapshot>, String> {
        Ok(None)
    }

    async fn resolve_default_model_id(&self) -> Result<Option<String>, String> {
        Ok(None)
    }

    async fn resolve_default_usable_model_id(&self) -> Result<Option<String>, String> {
        Ok(None)
    }

    async fn load_route_policy(
        &self,
        _capability: &str,
    ) -> Result<Option<ChatRoutePolicySnapshot>, String> {
        Ok(None)
    }

    async fn get_provider_connection(
        &self,
        _provider_id: &str,
    ) -> Result<Option<ProviderConnectionSnapshot>, String> {
        Ok(None)
    }

    async fn load_session_model(&self, _model_id: &str) -> Result<SessionModelSnapshot, String> {
        Err("unused".to_string())
    }

    async fn load_default_work_dir(&self) -> Result<Option<String>, String> {
        Ok(Some("E:/default-workdir".to_string()))
    }

    async fn load_imported_mcp_guidance(
        &self,
        imported_mcp_server_ids: &[String],
    ) -> Result<Option<String>, String> {
        Ok(Some(format!(
            "Prefer imported MCPs: {}",
            imported_mcp_server_ids.join(", ")
        )))
    }
}

#[tokio::test]
async fn prepare_execution_guidance_uses_request_context() {
    let guidance = ChatExecutionPreparationService::new()
        .prepare_execution_guidance(
            &FakeGuidanceRepo,
            &ChatExecutionPreparationRequest {
                user_message: "continue".to_string(),
                session_id: Some("session-7".to_string()),
                permission_mode: Some("standard".to_string()),
                session_mode: Some("general".to_string()),
                team_id: None,
                employee_id: None,
                requested_capability: Some("chat".to_string()),
                work_dir: Some("E:/request-workdir".to_string()),
                imported_mcp_server_ids: vec!["mcp-filesystem".to_string()],
            },
        )
        .await
        .expect("execution guidance");

    assert_eq!(
        guidance,
        ChatExecutionGuidance {
            effective_work_dir: "E:/request-workdir".to_string(),
            imported_mcp_guidance: Some("Prefer imported MCPs: mcp-filesystem".to_string()),
        }
    );
}
