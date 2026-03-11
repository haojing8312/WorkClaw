use async_trait::async_trait;
use runtime_models_app::{
    ModelsAppService, ModelsConfigRepository, ModelsReadRepository, ProviderCatalog,
    ProviderPluginInfo, RoutingSettings,
};
use std::sync::Mutex;

#[derive(Default)]
struct FakeRepo {
    rows: Vec<(String, String)>,
    saved: Mutex<Vec<RoutingSettings>>,
}

#[async_trait]
impl ModelsConfigRepository for FakeRepo {
    async fn load_routing_settings(&self) -> Result<Vec<(String, String)>, String> {
        Ok(self.rows.clone())
    }

    async fn save_routing_settings(&self, settings: &RoutingSettings) -> Result<(), String> {
        self.saved
            .lock()
            .expect("saved lock")
            .push(settings.clone());
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
        unreachable!("not used in this test")
    }

    async fn upsert_capability_routing_policy(
        &self,
        _policy: runtime_models_app::CapabilityRoutingPolicy,
    ) -> Result<(), String> {
        unreachable!("not used in this test")
    }

    async fn get_capability_routing_policy(
        &self,
        _capability: &str,
    ) -> Result<Option<runtime_models_app::CapabilityRoutingPolicy>, String> {
        unreachable!("not used in this test")
    }
}

#[async_trait]
impl ModelsReadRepository for FakeRepo {
    async fn list_enabled_provider_keys(&self) -> Result<Vec<(String, String)>, String> {
        Ok(Vec::new())
    }

    async fn get_provider_key(&self, _provider_id: &str) -> Result<String, String> {
        unreachable!("not used in this test")
    }

    async fn load_model_catalog_cache(
        &self,
        _provider_id: &str,
    ) -> Result<Vec<runtime_models_app::ModelCatalogCacheEntry>, String> {
        unreachable!("not used in this test")
    }

    async fn replace_model_catalog_cache(
        &self,
        _provider_id: &str,
        _models: &[String],
        _fetched_at: &str,
        _ttl_seconds: i64,
    ) -> Result<(), String> {
        unreachable!("not used in this test")
    }
}

struct EmptyCatalog;

impl ProviderCatalog for EmptyCatalog {
    fn list_provider_plugins(&self) -> Result<Vec<ProviderPluginInfo>, String> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn load_routing_settings_uses_defaults_when_repo_has_no_rows() {
    let service = ModelsAppService::new(FakeRepo::default(), EmptyCatalog);
    let settings = service.load_routing_settings().await.expect("settings");
    assert_eq!(
        settings,
        RoutingSettings {
            max_call_depth: 4,
            node_timeout_seconds: 60,
            retry_count: 0
        }
    );
}

#[tokio::test]
async fn save_routing_settings_clamps_values_before_persisting() {
    let repo = FakeRepo::default();
    let service = ModelsAppService::new(repo, EmptyCatalog);
    service
        .save_routing_settings(RoutingSettings {
            max_call_depth: 99,
            node_timeout_seconds: 1,
            retry_count: 10,
        })
        .await
        .expect("save settings");
}
