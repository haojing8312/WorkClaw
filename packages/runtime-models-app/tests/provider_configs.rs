use async_trait::async_trait;
use runtime_models_app::{
    CapabilityRoutingPolicy, ModelsAppService, ModelsConfigRepository, ModelsReadRepository,
    ProviderCatalog, ProviderConfig, ProviderPluginInfo, RoutingSettings,
};
use std::sync::Mutex;

#[derive(Default)]
struct FakeRepo {
    configs: Mutex<Vec<ProviderConfig>>,
}

#[async_trait]
impl ModelsConfigRepository for FakeRepo {
    async fn load_routing_settings(&self) -> Result<Vec<(String, String)>, String> {
        Ok(Vec::new())
    }

    async fn save_routing_settings(&self, _settings: &RoutingSettings) -> Result<(), String> {
        Ok(())
    }

    async fn save_provider_config(&self, config: ProviderConfig) -> Result<String, String> {
        self.configs
            .lock()
            .expect("configs lock")
            .push(config.clone());
        Ok(config.id)
    }

    async fn list_provider_configs(&self) -> Result<Vec<ProviderConfig>, String> {
        Ok(self.configs.lock().expect("configs lock").clone())
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

struct EmptyCatalog;

impl ProviderCatalog for EmptyCatalog {
    fn list_provider_plugins(&self) -> Result<Vec<ProviderPluginInfo>, String> {
        Ok(Vec::new())
    }
}

fn sample_provider_config() -> ProviderConfig {
    ProviderConfig {
        id: "provider-1".to_string(),
        provider_key: "deepseek".to_string(),
        display_name: "DeepSeek CN".to_string(),
        protocol_type: "openai".to_string(),
        base_url: "https://deepseek.example.com/v1".to_string(),
        auth_type: "bearer".to_string(),
        api_key_encrypted: "sk-encrypted".to_string(),
        org_id: String::new(),
        extra_json: "{}".to_string(),
        enabled: true,
    }
}

#[tokio::test]
async fn save_provider_config_returns_repository_id() {
    let service = ModelsAppService::new(FakeRepo::default(), EmptyCatalog);
    let id = service
        .save_provider_config(sample_provider_config())
        .await
        .expect("save provider");
    assert_eq!(id, "provider-1");
}

#[tokio::test]
async fn list_provider_configs_returns_saved_configs() {
    let repo = FakeRepo::default();
    repo.configs
        .lock()
        .expect("configs lock")
        .push(sample_provider_config());
    let service = ModelsAppService::new(repo, EmptyCatalog);
    let configs = service
        .list_provider_configs()
        .await
        .expect("list providers");
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].provider_key, "deepseek");
}
