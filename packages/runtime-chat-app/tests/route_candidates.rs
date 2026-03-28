use async_trait::async_trait;
use runtime_chat_app::{
    ChatPreparationRequest, ChatPreparationService, ChatRoutePolicySnapshot,
    ChatSettingsRepository, PreparedRouteCandidate, ProviderConnectionSnapshot,
    RoutingSettingsSnapshot, SessionModelSnapshot,
};
use serde_json::json;
use std::collections::HashMap;

struct FakeRouteRepo {
    routing: RoutingSettingsSnapshot,
    requested_route: Option<ChatRoutePolicySnapshot>,
    chat_route: Option<ChatRoutePolicySnapshot>,
    providers: Vec<ProviderConnectionSnapshot>,
    session_models: HashMap<String, SessionModelSnapshot>,
    default_usable_model_id: Option<String>,
}

#[async_trait]
impl ChatSettingsRepository for FakeRouteRepo {
    async fn load_routing_settings(&self) -> Result<RoutingSettingsSnapshot, String> {
        Ok(self.routing.clone())
    }

    async fn load_chat_routing(
        &self,
    ) -> Result<Option<runtime_chat_app::ChatRoutingSnapshot>, String> {
        Ok(None)
    }

    async fn resolve_default_model_id(&self) -> Result<Option<String>, String> {
        Ok(None)
    }

    async fn resolve_default_usable_model_id(&self) -> Result<Option<String>, String> {
        Ok(self.default_usable_model_id.clone())
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

    async fn load_session_model(&self, model_id: &str) -> Result<SessionModelSnapshot, String> {
        self.session_models
            .get(model_id)
            .cloned()
            .ok_or_else(|| format!("模型配置不存在 (model_id={model_id})"))
    }
}

#[tokio::test]
async fn prepare_route_candidates_prefers_requested_capability_route() {
    let repo = FakeRouteRepo {
        routing: RoutingSettingsSnapshot {
            max_call_depth: 4,
            node_timeout_seconds: 60,
            retry_count: 0,
        },
        requested_route: Some(ChatRoutePolicySnapshot {
            primary_provider_id: "provider-1".to_string(),
            primary_model: "gpt-4.1".to_string(),
            fallback_chain_json: r#"[{"provider_id":"provider-2","model":"claude-3-5-sonnet"}]"#
                .to_string(),
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
                provider_id: "provider-1".to_string(),
                provider_key: "openai".to_string(),
                protocol_type: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: "sk-openai".to_string(),
            },
            ProviderConnectionSnapshot {
                provider_id: "provider-2".to_string(),
                provider_key: "anthropic".to_string(),
                protocol_type: "anthropic".to_string(),
                base_url: "https://api.anthropic.com".to_string(),
                api_key: "sk-anthropic".to_string(),
            },
        ],
        session_models: HashMap::from([(
            "model-1".to_string(),
            SessionModelSnapshot {
                model_id: "model-1".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://fallback.example.com".to_string(),
                model_name: "session-model".to_string(),
                api_key: "sk-session".to_string(),
            },
        )]),
        default_usable_model_id: None,
    };

    let prepared = ChatPreparationService::new()
        .prepare_route_candidates(
            &repo,
            "model-1",
            &ChatPreparationRequest {
                user_message: "帮我识图".to_string(),
                user_message_parts: None,
                permission_mode: None,
                session_mode: None,
                team_id: None,
            },
        )
        .await
        .expect("candidates");

    assert_eq!(prepared.retry_count_per_candidate, 2);
    assert_eq!(
        prepared.candidates,
        vec![
            PreparedRouteCandidate {
                provider_key: "openai".to_string(),
                protocol_type: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model_name: "gpt-4.1".to_string(),
                api_key: "sk-openai".to_string(),
            },
            PreparedRouteCandidate {
                provider_key: "anthropic".to_string(),
                protocol_type: "anthropic".to_string(),
                base_url: "https://api.anthropic.com".to_string(),
                model_name: "claude-3-5-sonnet".to_string(),
                api_key: "sk-anthropic".to_string(),
            },
            PreparedRouteCandidate {
                provider_key: String::new(),
                protocol_type: "openai".to_string(),
                base_url: "https://fallback.example.com".to_string(),
                model_name: "session-model".to_string(),
                api_key: "sk-session".to_string(),
            }
        ]
    );
}

#[tokio::test]
async fn prepare_route_candidates_falls_back_to_chat_route_when_capability_missing() {
    let repo = FakeRouteRepo {
        routing: RoutingSettingsSnapshot {
            max_call_depth: 4,
            node_timeout_seconds: 60,
            retry_count: 0,
        },
        requested_route: None,
        chat_route: Some(ChatRoutePolicySnapshot {
            primary_provider_id: "provider-chat".to_string(),
            primary_model: String::new(),
            fallback_chain_json: "[]".to_string(),
            retry_count: 1,
            enabled: true,
        }),
        providers: vec![ProviderConnectionSnapshot {
            provider_id: "provider-chat".to_string(),
            provider_key: "openai".to_string(),
            protocol_type: "openai".to_string(),
            base_url: "https://chat.example.com/v1".to_string(),
            api_key: "sk-chat".to_string(),
        }],
        session_models: HashMap::from([(
            "model-1".to_string(),
            SessionModelSnapshot {
                model_id: "model-1".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://fallback.example.com".to_string(),
                model_name: "session-model".to_string(),
                api_key: "sk-session".to_string(),
            },
        )]),
        default_usable_model_id: None,
    };

    let prepared = ChatPreparationService::new()
        .prepare_route_candidates(
            &repo,
            "model-1",
            &ChatPreparationRequest {
                user_message: "帮我识图".to_string(),
                user_message_parts: None,
                permission_mode: None,
                session_mode: None,
                team_id: None,
            },
        )
        .await
        .expect("candidates");

    assert_eq!(prepared.retry_count_per_candidate, 1);
    assert_eq!(prepared.candidates[0].model_name, "session-model");
}

#[tokio::test]
async fn stale_model_id_falls_back_to_default_usable_model() {
    let repo = FakeRouteRepo {
        routing: RoutingSettingsSnapshot {
            max_call_depth: 4,
            node_timeout_seconds: 60,
            retry_count: 0,
        },
        requested_route: None,
        chat_route: None,
        providers: vec![],
        session_models: HashMap::from([(
            "model-live".to_string(),
            SessionModelSnapshot {
                model_id: "model-live".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://proxy.example.com/v1".to_string(),
                model_name: "MiniMax-M2.5".to_string(),
                api_key: "sk-live".to_string(),
            },
        )]),
        default_usable_model_id: Some("model-live".to_string()),
    };

    let prepared = ChatPreparationService::new()
        .prepare_route_candidates(
            &repo,
            "model-stale",
            &ChatPreparationRequest {
                user_message: "你好".to_string(),
                user_message_parts: None,
                permission_mode: None,
                session_mode: None,
                team_id: None,
            },
        )
        .await
        .expect("fallback candidates");

    assert_eq!(prepared.retry_count_per_candidate, 0);
    assert_eq!(
        prepared.candidates,
        vec![PreparedRouteCandidate {
            provider_key: String::new(),
            protocol_type: "openai".to_string(),
            base_url: "https://proxy.example.com/v1".to_string(),
            model_name: "MiniMax-M2.5".to_string(),
            api_key: "sk-live".to_string(),
        }]
    );
}

#[tokio::test]
async fn image_parts_do_not_fall_back_to_chat_route_when_vision_route_missing() {
    let repo = FakeRouteRepo {
        routing: RoutingSettingsSnapshot {
            max_call_depth: 4,
            node_timeout_seconds: 60,
            retry_count: 0,
        },
        requested_route: None,
        chat_route: Some(ChatRoutePolicySnapshot {
            primary_provider_id: "provider-chat".to_string(),
            primary_model: String::new(),
            fallback_chain_json: "[]".to_string(),
            retry_count: 1,
            enabled: true,
        }),
        providers: vec![ProviderConnectionSnapshot {
            provider_id: "provider-chat".to_string(),
            provider_key: "openai".to_string(),
            protocol_type: "openai".to_string(),
            base_url: "https://chat.example.com/v1".to_string(),
            api_key: "sk-chat".to_string(),
        }],
        session_models: HashMap::from([(
            "model-1".to_string(),
            SessionModelSnapshot {
                model_id: "model-1".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://fallback.example.com".to_string(),
                model_name: "session-model".to_string(),
                api_key: "sk-session".to_string(),
            },
        )]),
        default_usable_model_id: None,
    };

    let prepared = ChatPreparationService::new()
        .prepare_route_candidates(
            &repo,
            "model-1",
            &ChatPreparationRequest {
                user_message: "请分析图片".to_string(),
                user_message_parts: Some(vec![
                    json!({ "type": "text", "text": "请分析图片" }),
                    json!({ "type": "image", "name": "screen.png", "mimeType": "image/png", "data": "abcd" }),
                ]),
                permission_mode: None,
                session_mode: None,
                team_id: None,
            },
        )
        .await
        .expect("candidates");

    assert_eq!(prepared.retry_count_per_candidate, 0);
    assert!(prepared.candidates.is_empty());
}
