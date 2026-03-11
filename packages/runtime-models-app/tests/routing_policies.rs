use async_trait::async_trait;
use runtime_models_app::{
    CapabilityRoutingPolicy, ChatRoutingPolicy, ModelsAppService, ModelsConfigRepository,
    ModelsReadRepository, ProviderCatalog, ProviderConfig, ProviderPluginInfo, RoutingSettings,
};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
struct FakeRepo {
    policies: Mutex<HashMap<String, CapabilityRoutingPolicy>>,
}

#[async_trait]
impl ModelsConfigRepository for FakeRepo {
    async fn load_routing_settings(&self) -> Result<Vec<(String, String)>, String> {
        Ok(Vec::new())
    }

    async fn save_routing_settings(&self, _settings: &RoutingSettings) -> Result<(), String> {
        Ok(())
    }

    async fn save_provider_config(&self, _config: ProviderConfig) -> Result<String, String> {
        unreachable!("not used in this test")
    }

    async fn list_provider_configs(&self) -> Result<Vec<ProviderConfig>, String> {
        unreachable!("not used in this test")
    }

    async fn delete_provider_config(&self, _provider_id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn upsert_capability_routing_policy(
        &self,
        policy: CapabilityRoutingPolicy,
    ) -> Result<(), String> {
        self.policies
            .lock()
            .expect("policies lock")
            .insert(policy.capability.clone(), policy);
        Ok(())
    }

    async fn get_capability_routing_policy(
        &self,
        capability: &str,
    ) -> Result<Option<CapabilityRoutingPolicy>, String> {
        Ok(self
            .policies
            .lock()
            .expect("policies lock")
            .get(capability)
            .cloned())
    }
}

#[async_trait]
impl ModelsReadRepository for FakeRepo {
    async fn list_enabled_provider_keys(&self) -> Result<Vec<(String, String)>, String> {
        Ok(Vec::new())
    }

    async fn get_provider_key(&self, _provider_id: &str) -> Result<String, String> {
        Ok(String::new())
    }

    async fn load_model_catalog_cache(
        &self,
        _provider_id: &str,
    ) -> Result<Vec<runtime_models_app::ModelCatalogCacheEntry>, String> {
        Ok(Vec::new())
    }

    async fn replace_model_catalog_cache(
        &self,
        _provider_id: &str,
        _models: &[String],
        _fetched_at: &str,
        _ttl_seconds: i64,
    ) -> Result<(), String> {
        Ok(())
    }
}

struct EmptyCatalog;

impl ProviderCatalog for EmptyCatalog {
    fn list_provider_plugins(&self) -> Result<Vec<ProviderPluginInfo>, String> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn chat_routing_policy_round_trips_through_service() {
    let service = ModelsAppService::new(FakeRepo::default(), EmptyCatalog);
    service
        .set_chat_routing_policy(ChatRoutingPolicy {
            primary_provider_id: "provider-1".to_string(),
            primary_model: "deepseek-chat".to_string(),
            fallback_chain_json: "[{\"provider_id\":\"provider-2\",\"model\":\"qwen-max\"}]"
                .to_string(),
            timeout_ms: 45_000,
            retry_count: 1,
            enabled: true,
        })
        .await
        .expect("save chat policy");

    let loaded = service
        .get_chat_routing_policy()
        .await
        .expect("load chat policy")
        .expect("chat policy exists");
    assert_eq!(loaded.primary_provider_id, "provider-1");
    assert_eq!(loaded.primary_model, "deepseek-chat");
    assert!(loaded.fallback_chain_json.contains("qwen-max"));
}

#[tokio::test]
async fn capability_routing_policy_round_trips_through_service() {
    let service = ModelsAppService::new(FakeRepo::default(), EmptyCatalog);
    service
        .set_capability_routing_policy(CapabilityRoutingPolicy {
            capability: "vision".to_string(),
            primary_provider_id: "provider-vision".to_string(),
            primary_model: "qwen-vl-max".to_string(),
            fallback_chain_json: "[]".to_string(),
            timeout_ms: 30_000,
            retry_count: 1,
            enabled: true,
        })
        .await
        .expect("save capability policy");

    let loaded = service
        .get_capability_routing_policy("vision")
        .await
        .expect("load capability policy")
        .expect("vision policy exists");
    assert_eq!(loaded.capability, "vision");
    assert_eq!(loaded.primary_model, "qwen-vl-max");
}
