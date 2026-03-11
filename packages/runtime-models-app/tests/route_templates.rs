use async_trait::async_trait;
use runtime_models_app::{
    CapabilityRoutingPolicy, ModelsAppService, ModelsConfigRepository, ModelsReadRepository,
    ProviderCatalog, ProviderPluginInfo, RoutingSettings,
};

struct FakeRepo {
    enabled: Vec<(String, String)>,
}

#[async_trait]
impl ModelsConfigRepository for FakeRepo {
    async fn load_routing_settings(&self) -> Result<Vec<(String, String)>, String> {
        Ok(Vec::new())
    }

    async fn save_routing_settings(&self, _settings: &RoutingSettings) -> Result<(), String> {
        Ok(())
    }

    async fn save_provider_config(
        &self,
        _config: runtime_models_app::ProviderConfig,
    ) -> Result<String, String> {
        unreachable!("not used in this test")
    }

    async fn list_provider_configs(
        &self,
    ) -> Result<Vec<runtime_models_app::ProviderConfig>, String> {
        unreachable!("not used in this test")
    }

    async fn delete_provider_config(&self, _provider_id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn upsert_capability_routing_policy(
        &self,
        _policy: CapabilityRoutingPolicy,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn get_capability_routing_policy(
        &self,
        _capability: &str,
    ) -> Result<Option<CapabilityRoutingPolicy>, String> {
        Ok(None)
    }
}

#[async_trait]
impl ModelsReadRepository for FakeRepo {
    async fn list_enabled_provider_keys(&self) -> Result<Vec<(String, String)>, String> {
        Ok(self.enabled.clone())
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
async fn apply_template_uses_enabled_provider_keys() {
    let service = ModelsAppService::new(
        FakeRepo {
            enabled: vec![
                ("provider-primary".to_string(), "deepseek".to_string()),
                ("provider-fallback".to_string(), "qwen".to_string()),
            ],
        },
        EmptyCatalog,
    );

    let policy = service
        .apply_capability_route_template("chat", "china-first-p0")
        .await
        .expect("apply template");

    assert_eq!(policy.capability, "chat");
    assert_eq!(policy.primary_provider_id, "provider-primary");
    assert!(policy.fallback_chain_json.contains("provider-fallback"));
}

#[tokio::test]
async fn apply_template_fails_when_required_provider_is_missing() {
    let service = ModelsAppService::new(
        FakeRepo {
            enabled: Vec::new(),
        },
        EmptyCatalog,
    );
    let err = service
        .apply_capability_route_template("chat", "china-first-p0")
        .await
        .expect_err("missing provider should fail");
    assert!(err.contains("主 Provider") || err.contains("模板不存在"));
}
