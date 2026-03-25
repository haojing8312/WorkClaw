use async_trait::async_trait;
use runtime_chat_app::{
    ChatExecutionPreparationRequest, ChatExecutionPreparationService, ChatRoutePolicySnapshot,
    ChatRoutingSnapshot, ChatSettingsRepository, PreparedRouteCandidate,
    ProviderConnectionSnapshot, RoutingSettingsSnapshot, SessionModelSnapshot,
};
use serde_json::json;

struct FakeRouteDecisionRepo {
    requested_route: Option<ChatRoutePolicySnapshot>,
    chat_route: Option<ChatRoutePolicySnapshot>,
    providers: Vec<ProviderConnectionSnapshot>,
    session_model: SessionModelSnapshot,
}

#[async_trait]
impl ChatSettingsRepository for FakeRouteDecisionRepo {
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
        Ok(match capability {
            "vision" => self.requested_route.clone(),
            "chat" => self.chat_route.clone(),
            _ => None,
        })
    }

    async fn get_provider_connection(
        &self,
        provider_id: &str,
    ) -> Result<Option<ProviderConnectionSnapshot>, String> {
        Ok(self
            .providers
            .iter()
            .find(|provider| provider.provider_id == provider_id)
            .cloned())
    }

    async fn load_session_model(&self, _model_id: &str) -> Result<SessionModelSnapshot, String> {
        Ok(self.session_model.clone())
    }
}

#[tokio::test]
async fn prepare_route_decisions_prefers_explicit_capability() {
    let repo = FakeRouteDecisionRepo {
        requested_route: Some(ChatRoutePolicySnapshot {
            primary_provider_id: "provider-vision".to_string(),
            primary_model: "qwen-vl-max".to_string(),
            fallback_chain_json:
                r#"[{"provider_id":"provider-fallback","model":"claude-3-5-sonnet"}]"#.to_string(),
            retry_count: 2,
            enabled: true,
        }),
        chat_route: Some(ChatRoutePolicySnapshot {
            primary_provider_id: "provider-chat".to_string(),
            primary_model: "chat-model".to_string(),
            fallback_chain_json: "[]".to_string(),
            retry_count: 1,
            enabled: true,
        }),
        providers: vec![
            ProviderConnectionSnapshot {
                provider_id: "provider-vision".to_string(),
                protocol_type: "openai".to_string(),
                base_url: "https://vision.example.com/v1".to_string(),
                api_key: "sk-vision".to_string(),
            },
            ProviderConnectionSnapshot {
                provider_id: "provider-fallback".to_string(),
                protocol_type: "anthropic".to_string(),
                base_url: "https://api.anthropic.com".to_string(),
                api_key: "sk-anthropic".to_string(),
            },
        ],
        session_model: SessionModelSnapshot {
            model_id: "model-1".to_string(),
            api_format: "openai".to_string(),
            base_url: "https://fallback.example.com".to_string(),
            model_name: "session-model".to_string(),
            api_key: "sk-session".to_string(),
        },
    };

    let prepared = ChatExecutionPreparationService::new()
        .prepare_route_decisions(
            &repo,
            "model-1",
            &ChatExecutionPreparationRequest {
                user_message: "just continue".to_string(),
                user_message_parts: None,
                session_id: Some("session-9".to_string()),
                permission_mode: Some("standard".to_string()),
                session_mode: Some("general".to_string()),
                team_id: None,
                employee_id: None,
                requested_capability: Some("vision".to_string()),
                work_dir: None,
                imported_mcp_server_ids: vec![],
            },
        )
        .await
        .expect("route decisions");

    assert_eq!(prepared.retry_count_per_candidate, 2);
    assert_eq!(
        prepared.candidates[0],
        PreparedRouteCandidate {
            protocol_type: "openai".to_string(),
            base_url: "https://vision.example.com/v1".to_string(),
            model_name: "qwen-vl-max".to_string(),
            api_key: "sk-vision".to_string(),
        }
    );
}

#[tokio::test]
async fn prepare_route_decisions_ignores_image_parts_and_keeps_session_fallback() {
    let repo = FakeRouteDecisionRepo {
        requested_route: None,
        chat_route: Some(ChatRoutePolicySnapshot {
            primary_provider_id: "provider-chat".to_string(),
            primary_model: "chat-model".to_string(),
            fallback_chain_json: "[]".to_string(),
            retry_count: 1,
            enabled: true,
        }),
        providers: vec![ProviderConnectionSnapshot {
            provider_id: "provider-chat".to_string(),
            protocol_type: "openai".to_string(),
            base_url: "https://chat.example.com/v1".to_string(),
            api_key: "sk-chat".to_string(),
        }],
        session_model: SessionModelSnapshot {
            model_id: "model-1".to_string(),
            api_format: "openai".to_string(),
            base_url: "https://fallback.example.com".to_string(),
            model_name: "session-model".to_string(),
            api_key: "sk-session".to_string(),
        },
    };

    let prepared = ChatExecutionPreparationService::new()
        .prepare_route_decisions(
            &repo,
            "model-1",
            &ChatExecutionPreparationRequest {
                user_message: "just continue".to_string(),
                user_message_parts: Some(vec![
                    json!({ "type": "text", "text": "just continue" }),
                    json!({ "type": "image", "name": "screen.png", "mimeType": "image/png", "data": "abcd" }),
                ]),
                session_id: Some("session-9".to_string()),
                permission_mode: Some("standard".to_string()),
                session_mode: Some("general".to_string()),
                team_id: None,
                employee_id: None,
                requested_capability: None,
                work_dir: None,
                imported_mcp_server_ids: vec![],
            },
        )
        .await
        .expect("route decisions");

    assert_eq!(prepared.retry_count_per_candidate, 1);
    assert_eq!(prepared.candidates.len(), 2);
    assert_eq!(
        prepared.candidates[0],
        PreparedRouteCandidate {
            protocol_type: "openai".to_string(),
            base_url: "https://chat.example.com/v1".to_string(),
            model_name: "chat-model".to_string(),
            api_key: "sk-chat".to_string(),
        }
    );
    assert_eq!(
        prepared.candidates[1],
        PreparedRouteCandidate {
            protocol_type: "openai".to_string(),
            base_url: "https://fallback.example.com".to_string(),
            model_name: "session-model".to_string(),
            api_key: "sk-session".to_string(),
        }
    );
}
