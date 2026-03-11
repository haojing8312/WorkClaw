use crate::types::{
    CapabilityRoutingPolicy, ModelCatalogCacheEntry, ModelConfig, ProviderConfig,
    ProviderConnectionInfo, ProviderPluginInfo, RouteAttemptLog, RouteAttemptStat, RoutingSettings,
};
use async_trait::async_trait;

#[async_trait]
pub trait ModelsConfigRepository: Send + Sync {
    async fn load_routing_settings(&self) -> Result<Vec<(String, String)>, String>;
    async fn save_routing_settings(&self, settings: &RoutingSettings) -> Result<(), String>;
    async fn save_provider_config(&self, config: ProviderConfig) -> Result<String, String>;
    async fn list_provider_configs(&self) -> Result<Vec<ProviderConfig>, String>;
    async fn save_model_config(
        &self,
        config: ModelConfig,
        api_key: String,
    ) -> Result<String, String> {
        let _ = (config, api_key);
        Err("not implemented".to_string())
    }
    async fn delete_model_config(&self, model_id: &str) -> Result<(), String> {
        let _ = model_id;
        Err("not implemented".to_string())
    }
    async fn set_default_model(&self, model_id: &str) -> Result<(), String> {
        let _ = model_id;
        Err("not implemented".to_string())
    }
    async fn delete_provider_config(&self, provider_id: &str) -> Result<(), String> {
        let _ = provider_id;
        Err("not implemented".to_string())
    }
    async fn upsert_capability_routing_policy(
        &self,
        policy: CapabilityRoutingPolicy,
    ) -> Result<(), String>;
    async fn get_capability_routing_policy(
        &self,
        capability: &str,
    ) -> Result<Option<CapabilityRoutingPolicy>, String>;
}

#[async_trait]
pub trait ModelsReadRepository: Send + Sync {
    async fn list_enabled_provider_keys(&self) -> Result<Vec<(String, String)>, String>;
    async fn list_enabled_provider_ids(&self) -> Result<Vec<String>, String> {
        Err("not implemented".to_string())
    }
    async fn query_candidate_model_id(
        &self,
        default_only: bool,
        require_api_key: bool,
    ) -> Result<Option<String>, String> {
        let _ = (default_only, require_api_key);
        Err("not implemented".to_string())
    }
    async fn get_provider_key(&self, provider_id: &str) -> Result<String, String> {
        let _ = provider_id;
        Err("not implemented".to_string())
    }
    async fn load_model_catalog_cache(
        &self,
        provider_id: &str,
    ) -> Result<Vec<ModelCatalogCacheEntry>, String> {
        let _ = provider_id;
        Err("not implemented".to_string())
    }
    async fn replace_model_catalog_cache(
        &self,
        provider_id: &str,
        models: &[String],
        fetched_at: &str,
        ttl_seconds: i64,
    ) -> Result<(), String> {
        let _ = (provider_id, models, fetched_at, ttl_seconds);
        Err("not implemented".to_string())
    }
    async fn list_recent_route_attempt_logs(
        &self,
        session_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<RouteAttemptLog>, String> {
        let _ = (session_id, limit, offset);
        Err("not implemented".to_string())
    }
    async fn list_route_attempt_logs_since(
        &self,
        session_id: Option<&str>,
        cutoff_rfc3339: &str,
    ) -> Result<Vec<RouteAttemptLog>, String> {
        let _ = (session_id, cutoff_rfc3339);
        Err("not implemented".to_string())
    }
    async fn list_route_attempt_stats(
        &self,
        hours: i64,
        capability: Option<&str>,
    ) -> Result<Vec<RouteAttemptStat>, String> {
        let _ = (hours, capability);
        Err("not implemented".to_string())
    }
    async fn get_provider_connection_info(
        &self,
        provider_id: &str,
    ) -> Result<Option<ProviderConnectionInfo>, String> {
        let _ = provider_id;
        Err("not implemented".to_string())
    }
}

pub trait ProviderCatalog: Send + Sync {
    fn list_provider_plugins(&self) -> Result<Vec<ProviderPluginInfo>, String>;
}

#[async_trait]
pub trait ProviderHealthProbe: Send + Sync {
    async fn test_connection(
        &self,
        protocol_type: &str,
        base_url: &str,
        api_key: &str,
        model: &str,
    ) -> Result<bool, String>;
}
