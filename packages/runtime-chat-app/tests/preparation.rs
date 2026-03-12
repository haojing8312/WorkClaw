use async_trait::async_trait;
use runtime_chat_app::{
    ChatExecutionContext, ChatPreparationRequest, ChatPreparationService, ChatRoutePolicySnapshot,
    ChatRoutingSnapshot, ChatSettingsRepository, PreparedChatExecution, ProviderConnectionSnapshot,
    RoutingSettingsSnapshot, SessionModelSnapshot,
};

struct FakeSettingsRepo {
    routing: RoutingSettingsSnapshot,
    chat: Option<ChatRoutingSnapshot>,
    default_model_id: Option<String>,
    default_usable_model_id: Option<String>,
}

#[async_trait]
impl ChatSettingsRepository for FakeSettingsRepo {
    async fn load_routing_settings(&self) -> Result<RoutingSettingsSnapshot, String> {
        Ok(self.routing.clone())
    }

    async fn load_chat_routing(&self) -> Result<Option<ChatRoutingSnapshot>, String> {
        Ok(self.chat.clone())
    }

    async fn resolve_default_model_id(&self) -> Result<Option<String>, String> {
        Ok(self.default_model_id.clone())
    }

    async fn resolve_default_usable_model_id(&self) -> Result<Option<String>, String> {
        Ok(self.default_usable_model_id.clone())
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

    async fn load_session_model(&self, model_id: &str) -> Result<SessionModelSnapshot, String> {
        Ok(SessionModelSnapshot {
            model_id: model_id.to_string(),
            api_format: "openai".to_string(),
            base_url: String::new(),
            model_name: String::new(),
            api_key: String::new(),
        })
    }
}

#[tokio::test]
async fn prepare_chat_execution_normalizes_request_and_settings() {
    let repo = FakeSettingsRepo {
        routing: RoutingSettingsSnapshot {
            max_call_depth: 4,
            node_timeout_seconds: 45,
            retry_count: 1,
        },
        chat: Some(ChatRoutingSnapshot {
            primary_provider_id: "provider-1".to_string(),
            primary_model: "gpt-4.1".to_string(),
            fallback_chain_json: r#"[{"provider_id":"provider-2","model":"claude-3-5-sonnet"}]"#
                .to_string(),
            timeout_ms: 12_000,
            retry_count: 1,
            enabled: true,
        }),
        default_model_id: Some("model-default".to_string()),
        default_usable_model_id: Some("model-usable".to_string()),
    };

    let prepared = ChatPreparationService::new()
        .prepare_chat_execution(
            &repo,
            ChatPreparationRequest {
                user_message: "帮我识图并解释内容".to_string(),
                permission_mode: Some("accept_edits".to_string()),
                session_mode: Some("general".to_string()),
                team_id: Some("team-1".to_string()),
            },
        )
        .await
        .expect("prepared");

    assert_eq!(
        prepared,
        PreparedChatExecution {
            capability: "vision".to_string(),
            permission_mode_storage: "standard".to_string(),
            session_mode_storage: "general".to_string(),
            normalized_team_id: String::new(),
            permission_label: "标准模式".to_string(),
            max_call_depth: 4,
            node_timeout_seconds: 45,
            retry_count: 1,
            primary_provider_id: Some("provider-1".to_string()),
            primary_model: Some("gpt-4.1".to_string()),
            fallback_targets: vec![("provider-2".to_string(), "claude-3-5-sonnet".to_string())],
            default_model_id: Some("model-default".to_string()),
            default_usable_model_id: Some("model-usable".to_string()),
            execution_context: ChatExecutionContext {
                session_id: String::new(),
                session_mode_storage: "general".to_string(),
                normalized_team_id: String::new(),
                employee_id: String::new(),
                work_dir: String::new(),
                imported_mcp_server_ids: Vec::new(),
            },
        }
    );
}

#[tokio::test]
async fn prepare_chat_execution_falls_back_to_defaults_without_route() {
    let repo = FakeSettingsRepo {
        routing: RoutingSettingsSnapshot {
            max_call_depth: 3,
            node_timeout_seconds: 60,
            retry_count: 0,
        },
        chat: None,
        default_model_id: None,
        default_usable_model_id: Some("usable-1".to_string()),
    };

    let prepared = ChatPreparationService::new()
        .prepare_chat_execution(
            &repo,
            ChatPreparationRequest {
                user_message: "普通聊天".to_string(),
                permission_mode: None,
                session_mode: Some("team_entry".to_string()),
                team_id: Some(" team-a ".to_string()),
            },
        )
        .await
        .expect("prepared");

    assert_eq!(prepared.capability, "chat");
    assert_eq!(prepared.permission_mode_storage, "standard");
    assert_eq!(prepared.session_mode_storage, "team_entry");
    assert_eq!(prepared.normalized_team_id, "team-a");
    assert_eq!(prepared.primary_provider_id, None);
    assert!(prepared.fallback_targets.is_empty());
    assert_eq!(
        prepared.default_usable_model_id.as_deref(),
        Some("usable-1")
    );
}
