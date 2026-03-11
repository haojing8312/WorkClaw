use async_trait::async_trait;
use runtime_models_app::{
    CapabilityRoutingPolicy, ModelsAppService, ModelsConfigRepository, ModelsReadRepository,
    ProviderCatalog, ProviderConfig, ProviderPluginInfo, RoutingSettings,
};

#[derive(Default)]
struct FakeRepo;

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

struct FakeCatalog;

impl ProviderCatalog for FakeCatalog {
    fn list_provider_plugins(&self) -> Result<Vec<ProviderPluginInfo>, String> {
        Ok(vec![ProviderPluginInfo {
            key: "deepseek".to_string(),
            display_name: "DeepSeek".to_string(),
            capabilities: vec!["chat".to_string(), "reasoning".to_string()],
        }])
    }
}

#[tokio::test]
async fn list_provider_plugins_returns_catalog_data() {
    let service = ModelsAppService::new(FakeRepo, FakeCatalog);
    let plugins = service.list_provider_plugins().expect("list plugins");
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].key, "deepseek");
    assert!(plugins[0].capabilities.iter().any(|c| c == "chat"));
}
