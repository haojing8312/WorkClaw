use async_trait::async_trait;
use runtime_models_app::{
    ModelsAppService, ModelsConfigRepository, ModelsReadRepository, ProviderCatalog,
    ProviderConnectionInfo, ProviderHealthInfo, ProviderHealthProbe, ProviderPluginInfo,
};
use std::collections::HashMap;

#[derive(Default)]
struct FakeRepo {
    providers: HashMap<String, ProviderConnectionInfo>,
    enabled_ids: Vec<String>,
}

#[async_trait]
impl ModelsConfigRepository for FakeRepo {
    async fn load_routing_settings(&self) -> Result<Vec<(String, String)>, String> {
        Ok(Vec::new())
    }

    async fn save_routing_settings(
        &self,
        _settings: &runtime_models_app::RoutingSettings,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn save_provider_config(
        &self,
        _config: runtime_models_app::ProviderConfig,
    ) -> Result<String, String> {
        Err("not used".to_string())
    }

    async fn list_provider_configs(
        &self,
    ) -> Result<Vec<runtime_models_app::ProviderConfig>, String> {
        Err("not used".to_string())
    }

    async fn upsert_capability_routing_policy(
        &self,
        _policy: runtime_models_app::CapabilityRoutingPolicy,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn get_capability_routing_policy(
        &self,
        _capability: &str,
    ) -> Result<Option<runtime_models_app::CapabilityRoutingPolicy>, String> {
        Ok(None)
    }
}

#[async_trait]
impl ModelsReadRepository for FakeRepo {
    async fn list_enabled_provider_keys(&self) -> Result<Vec<(String, String)>, String> {
        Ok(Vec::new())
    }

    async fn list_enabled_provider_ids(&self) -> Result<Vec<String>, String> {
        Ok(self.enabled_ids.clone())
    }

    async fn get_provider_connection_info(
        &self,
        provider_id: &str,
    ) -> Result<Option<ProviderConnectionInfo>, String> {
        Ok(self.providers.get(provider_id).cloned())
    }
}

struct EmptyCatalog;

impl ProviderCatalog for EmptyCatalog {
    fn list_provider_plugins(&self) -> Result<Vec<ProviderPluginInfo>, String> {
        Ok(Vec::new())
    }
}

struct FakeProbe;

#[async_trait]
impl ProviderHealthProbe for FakeProbe {
    async fn test_connection(
        &self,
        protocol_type: &str,
        _base_url: &str,
        _api_key: &str,
        _model: &str,
    ) -> Result<bool, String> {
        if protocol_type == "anthropic" {
            Err("anthropic down".to_string())
        } else {
            Ok(true)
        }
    }
}

#[tokio::test]
async fn missing_provider_returns_not_found_health_status() {
    let service = ModelsAppService::with_probe(FakeRepo::default(), EmptyCatalog, FakeProbe);
    let result = service
        .test_provider_health("missing")
        .await
        .expect("health");
    assert_eq!(
        result,
        ProviderHealthInfo {
            provider_id: "missing".to_string(),
            ok: false,
            protocol_type: String::new(),
            message: "provider 不存在或未启用".to_string(),
        }
    );
}

#[tokio::test]
async fn empty_api_key_returns_invalid_health_status() {
    let mut repo = FakeRepo::default();
    repo.providers.insert(
        "provider-1".to_string(),
        ProviderConnectionInfo {
            provider_id: "provider-1".to_string(),
            protocol_type: "openai".to_string(),
            base_url: "https://api.example.com/v1".to_string(),
            api_key: String::new(),
        },
    );
    let service = ModelsAppService::with_probe(repo, EmptyCatalog, FakeProbe);
    let result = service
        .test_provider_health("provider-1")
        .await
        .expect("health");
    assert_eq!(result.message, "API Key 为空");
    assert!(!result.ok);
}

#[tokio::test]
async fn test_all_provider_health_aggregates_probe_results() {
    let mut repo = FakeRepo::default();
    repo.enabled_ids = vec!["provider-1".to_string(), "provider-2".to_string()];
    repo.providers.insert(
        "provider-1".to_string(),
        ProviderConnectionInfo {
            provider_id: "provider-1".to_string(),
            protocol_type: "openai".to_string(),
            base_url: "https://api.example.com/v1".to_string(),
            api_key: "sk-1".to_string(),
        },
    );
    repo.providers.insert(
        "provider-2".to_string(),
        ProviderConnectionInfo {
            provider_id: "provider-2".to_string(),
            protocol_type: "anthropic".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            api_key: "sk-2".to_string(),
        },
    );

    let service = ModelsAppService::with_probe(repo, EmptyCatalog, FakeProbe);
    let results = service
        .test_all_provider_health()
        .await
        .expect("all health");
    assert_eq!(results.len(), 2);
    assert!(results
        .iter()
        .any(|r| r.provider_id == "provider-1" && r.ok));
    assert!(results
        .iter()
        .any(|r| r.provider_id == "provider-2" && !r.ok && r.message == "anthropic down"));
}
