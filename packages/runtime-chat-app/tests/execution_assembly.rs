use async_trait::async_trait;
use runtime_chat_app::{
    ChatEmployeeDirectory, ChatEmployeeSnapshot, ChatExecutionPreparationRequest,
    ChatExecutionPreparationService, ChatRoutePolicySnapshot, ChatRoutingSnapshot,
    ChatSessionContextRepository, ChatSettingsRepository, ProviderConnectionSnapshot,
    RoutingSettingsSnapshot, SessionExecutionContextSnapshot, SessionModelSnapshot,
};

struct FakeExecutionAssemblyRepo;

#[async_trait]
impl ChatSettingsRepository for FakeExecutionAssemblyRepo {
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
        capability: &str,
    ) -> Result<Option<ChatRoutePolicySnapshot>, String> {
        if capability == "vision" {
            Ok(Some(ChatRoutePolicySnapshot {
                primary_provider_id: "provider-primary".to_string(),
                primary_model: "vision-primary".to_string(),
                fallback_chain_json:
                    r#"[{"provider_id":"provider-fallback","model":"vision-fallback"}]"#
                        .to_string(),
                retry_count: 2,
                enabled: true,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_provider_connection(
        &self,
        provider_id: &str,
    ) -> Result<Option<ProviderConnectionSnapshot>, String> {
        Ok(match provider_id {
            "provider-primary" => Some(ProviderConnectionSnapshot {
                provider_id: provider_id.to_string(),
                protocol_type: "openai".to_string(),
                base_url: "https://primary.example.com/v1".to_string(),
                api_key: "sk-primary".to_string(),
            }),
            "provider-fallback" => Some(ProviderConnectionSnapshot {
                provider_id: provider_id.to_string(),
                protocol_type: "anthropic".to_string(),
                base_url: "https://fallback.example.com".to_string(),
                api_key: "sk-fallback".to_string(),
            }),
            _ => None,
        })
    }

    async fn load_session_model(&self, model_id: &str) -> Result<SessionModelSnapshot, String> {
        Ok(SessionModelSnapshot {
            model_id: model_id.to_string(),
            api_format: "openai".to_string(),
            base_url: "https://session.example.com/v1".to_string(),
            model_name: "session-model".to_string(),
            api_key: "sk-session".to_string(),
        })
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

#[async_trait]
impl ChatSessionContextRepository for FakeExecutionAssemblyRepo {
    async fn load_session_execution_context(
        &self,
        session_id: Option<&str>,
    ) -> Result<SessionExecutionContextSnapshot, String> {
        Ok(SessionExecutionContextSnapshot {
            session_id: session_id.unwrap_or_default().to_string(),
            session_mode: "team_entry".to_string(),
            team_id: "team-from-session".to_string(),
            employee_id: "emp-from-session".to_string(),
            work_dir: "E:/session-workdir".to_string(),
            imported_mcp_server_ids: vec!["session-mcp".to_string()],
        })
    }
}

#[async_trait]
impl ChatEmployeeDirectory for FakeExecutionAssemblyRepo {
    async fn list_collaboration_candidates(&self) -> Result<Vec<ChatEmployeeSnapshot>, String> {
        Ok(vec![
            ChatEmployeeSnapshot {
                id: "current".to_string(),
                employee_id: "emp-from-session".to_string(),
                name: "Current Agent".to_string(),
                role_id: "current-role".to_string(),
                feishu_open_id: String::new(),
                enabled: true,
            },
            ChatEmployeeSnapshot {
                id: "other".to_string(),
                employee_id: "emp-other".to_string(),
                name: "Other Agent".to_string(),
                role_id: "other-role".to_string(),
                feishu_open_id: "ou_xxx".to_string(),
                enabled: true,
            },
        ])
    }
}

#[tokio::test]
async fn prepare_execution_assembles_context_guidance_and_routes() {
    let prepared = ChatExecutionPreparationService::new()
        .prepare_execution_with_directory(
            &FakeExecutionAssemblyRepo,
            &FakeExecutionAssemblyRepo,
            "model-1",
            &ChatExecutionPreparationRequest {
                user_message: "请帮我识图".to_string(),
                session_id: Some("session-42".to_string()),
                permission_mode: Some("standard".to_string()),
                session_mode: None,
                team_id: None,
                employee_id: None,
                requested_capability: Some("vision".to_string()),
                work_dir: None,
                imported_mcp_server_ids: vec!["imported-mcp".to_string()],
            },
        )
        .await
        .expect("prepared execution assembly");

    assert_eq!(prepared.execution_context.session_id, "session-42");
    assert_eq!(prepared.chat_preparation.capability, "vision".to_string());
    assert_eq!(
        prepared.execution_context.session_mode_storage,
        "team_entry".to_string()
    );
    assert_eq!(
        prepared.execution_context.normalized_team_id,
        "team-from-session".to_string()
    );
    assert_eq!(
        prepared.execution_guidance.effective_work_dir,
        "E:/session-workdir".to_string()
    );
    assert_eq!(
        prepared.execution_guidance.imported_mcp_guidance.as_deref(),
        Some("Prefer imported MCPs: imported-mcp")
    );
    assert!(
        prepared
            .employee_collaboration_guidance
            .as_deref()
            .unwrap_or_default()
            .contains("Other Agent")
    );
    assert_eq!(prepared.route_decisions.retry_count_per_candidate, 2);
    assert_eq!(prepared.route_decisions.candidates.len(), 3);
}
