use async_trait::async_trait;
use runtime_models_app::{
    ModelConfig, ModelsAppService, ModelsConfigRepository, ModelsReadRepository, ProviderCatalog,
    ProviderPluginInfo,
};
use std::sync::Mutex;

#[derive(Default)]
struct FakeRepo {
    saved_ids: Mutex<Vec<String>>,
    deleted_ids: Mutex<Vec<String>>,
    default_target: Mutex<Option<String>>,
    current_default: Mutex<Option<String>>,
    fallback_models: Mutex<Vec<String>>,
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

    async fn save_model_config(
        &self,
        config: ModelConfig,
        _api_key: String,
    ) -> Result<String, String> {
        self.saved_ids
            .lock()
            .expect("saved ids")
            .push(config.id.clone());
        Ok(config.id)
    }

    async fn delete_model_config(&self, model_id: &str) -> Result<(), String> {
        self.deleted_ids
            .lock()
            .expect("deleted ids")
            .push(model_id.to_string());
        Ok(())
    }

    async fn set_default_model(&self, model_id: &str) -> Result<(), String> {
        *self.default_target.lock().expect("default target") = Some(model_id.to_string());
        Ok(())
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

    async fn query_candidate_model_id(
        &self,
        default_only: bool,
        _require_api_key: bool,
    ) -> Result<Option<String>, String> {
        if default_only {
            Ok(self
                .current_default
                .lock()
                .expect("current default")
                .clone())
        } else {
            Ok(self
                .fallback_models
                .lock()
                .expect("fallback")
                .first()
                .cloned())
        }
    }
}

struct EmptyCatalog;

impl ProviderCatalog for EmptyCatalog {
    fn list_provider_plugins(&self) -> Result<Vec<ProviderPluginInfo>, String> {
        Ok(Vec::new())
    }
}

fn sample_model(id: &str, is_default: bool) -> ModelConfig {
    ModelConfig {
        id: id.to_string(),
        name: format!("Model {id}"),
        api_format: "openai".to_string(),
        base_url: format!("https://{id}.example.com/v1"),
        model_name: "gpt-test".to_string(),
        is_default,
    }
}

#[tokio::test]
async fn save_model_config_returns_repository_id() {
    let service = ModelsAppService::new(FakeRepo::default(), EmptyCatalog);
    let id = service
        .save_model_config(sample_model("model-1", false), "sk-test".to_string())
        .await
        .expect("save model");
    assert_eq!(id, "model-1");
}

#[tokio::test]
async fn delete_default_model_promotes_fallback() {
    let repo = FakeRepo::default();
    *repo.current_default.lock().expect("current default") = Some("model-1".to_string());
    repo.fallback_models
        .lock()
        .expect("fallback")
        .push("model-2".to_string());
    let service = ModelsAppService::new(repo, EmptyCatalog);
    service
        .delete_model_config("model-1")
        .await
        .expect("delete model");
}

#[tokio::test]
async fn resolve_default_model_self_heals_with_first_candidate() {
    let repo = FakeRepo::default();
    repo.fallback_models
        .lock()
        .expect("fallback")
        .push("model-1".to_string());
    let service = ModelsAppService::new(repo, EmptyCatalog);
    let resolved = service
        .resolve_default_model_id()
        .await
        .expect("resolve default");
    assert_eq!(resolved.as_deref(), Some("model-1"));
}
