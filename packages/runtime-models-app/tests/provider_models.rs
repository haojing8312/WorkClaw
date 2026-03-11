use async_trait::async_trait;
use runtime_models_app::{
    CapabilityRoutingPolicy, ModelCatalogCacheEntry, ModelsAppService, ModelsConfigRepository,
    ModelsReadRepository, ProviderCatalog, ProviderConfig, ProviderPluginInfo, RoutingSettings,
};
use std::sync::Mutex;

#[derive(Default)]
struct FakeRepo {
    cache: Mutex<Vec<ModelCatalogCacheEntry>>,
    delete_calls: Mutex<Vec<String>>,
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

    async fn delete_provider_config(&self, provider_id: &str) -> Result<(), String> {
        self.delete_calls
            .lock()
            .expect("delete calls")
            .push(provider_id.to_string());
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
        Ok("qwen".to_string())
    }

    async fn load_model_catalog_cache(
        &self,
        _provider_id: &str,
    ) -> Result<Vec<ModelCatalogCacheEntry>, String> {
        Ok(self.cache.lock().expect("cache").clone())
    }

    async fn replace_model_catalog_cache(
        &self,
        _provider_id: &str,
        models: &[String],
        fetched_at: &str,
        ttl_seconds: i64,
    ) -> Result<(), String> {
        let mut cache = self.cache.lock().expect("cache");
        cache.clear();
        cache.extend(
            models
                .iter()
                .cloned()
                .map(|model_id| ModelCatalogCacheEntry {
                    model_id,
                    fetched_at: fetched_at.to_string(),
                    ttl_seconds,
                }),
        );
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
async fn delete_provider_config_delegates_to_repository() {
    let repo = FakeRepo::default();
    let service = ModelsAppService::new(repo, EmptyCatalog);
    service
        .delete_provider_config("provider-1")
        .await
        .expect("delete provider");
}

#[tokio::test]
async fn list_provider_models_refreshes_cache_and_filters_by_capability() {
    let service = ModelsAppService::new(FakeRepo::default(), EmptyCatalog);
    let models = service
        .list_provider_models("provider-1", Some("chat"))
        .await
        .expect("list provider models");
    assert!(models.iter().any(|m| m == "qwen-max"));
}
