use super::models::{
    ProviderPluginInfo,
};
use crate::providers::ProviderRegistry;
use async_trait::async_trait;
use runtime_models_app::{ProviderCatalog, ProviderHealthProbe};
use sqlx::SqlitePool;

pub struct PoolModelsRepository<'a> {
    db: &'a SqlitePool,
}

impl<'a> PoolModelsRepository<'a> {
    pub fn new(db: &'a SqlitePool) -> Self {
        Self { db }
    }
}

#[path = "models_repo/config_repo.rs"]
mod config_repo;

#[path = "models_repo/read_repo.rs"]
mod read_repo;

pub struct RegistryProviderCatalog<'a> {
    registry: &'a ProviderRegistry,
}

impl<'a> RegistryProviderCatalog<'a> {
    pub fn new(registry: &'a ProviderRegistry) -> Self {
        Self { registry }
    }
}

impl ProviderCatalog for RegistryProviderCatalog<'_> {
    fn list_provider_plugins(&self) -> Result<Vec<ProviderPluginInfo>, String> {
        let mut providers: Vec<ProviderPluginInfo> = self
            .registry
            .list()
            .into_iter()
            .map(|provider| ProviderPluginInfo {
                key: provider.key().to_string(),
                display_name: provider.display_name().to_string(),
                capabilities: provider.capabilities(),
            })
            .collect();
        providers.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(providers)
    }
}

pub struct BuiltinProviderCatalog {
    registry: ProviderRegistry,
}

impl BuiltinProviderCatalog {
    pub fn china_first_p0() -> Self {
        Self {
            registry: ProviderRegistry::with_china_first_p0(),
        }
    }
}

impl ProviderCatalog for BuiltinProviderCatalog {
    fn list_provider_plugins(&self) -> Result<Vec<ProviderPluginInfo>, String> {
        RegistryProviderCatalog::new(&self.registry).list_provider_plugins()
    }
}

pub struct NullModelsRepository;

pub struct NullProviderCatalog;

impl ProviderCatalog for NullProviderCatalog {
    fn list_provider_plugins(&self) -> Result<Vec<ProviderPluginInfo>, String> {
        Ok(Vec::new())
    }
}

pub struct RuntimeProviderHealthProbe;

#[async_trait]
impl ProviderHealthProbe for RuntimeProviderHealthProbe {
    async fn test_connection(
        &self,
        protocol_type: &str,
        base_url: &str,
        api_key: &str,
        model: &str,
    ) -> Result<bool, String> {
        if protocol_type == "anthropic" {
            crate::adapters::anthropic::test_connection(base_url, api_key, model)
                .await
                .map_err(|e| e.to_string())
        } else {
            crate::adapters::openai::test_connection(base_url, api_key, model)
                .await
                .map_err(|e| e.to_string())
        }
    }
}
