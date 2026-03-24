use super::{NullModelsRepository, PoolModelsRepository};
use async_trait::async_trait;
use crate::commands::models::{CapabilityRoutingPolicy, ModelConfig, ProviderConfig, RoutingSettings};
use runtime_models_app::ModelsConfigRepository;
use uuid::Uuid;

#[async_trait]
impl ModelsConfigRepository for PoolModelsRepository<'_> {
    async fn load_routing_settings(&self) -> Result<Vec<(String, String)>, String> {
        sqlx::query_as::<_, (String, String)>(
            "SELECT key, value FROM app_settings WHERE key IN ('route_max_call_depth', 'route_node_timeout_seconds', 'route_retry_count')",
        )
        .fetch_all(self.db)
        .await
        .map_err(|e| format!("读取路由设置失败: {e}"))
    }

    async fn save_routing_settings(&self, settings: &RoutingSettings) -> Result<(), String> {
        sqlx::query(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('route_max_call_depth', ?)",
        )
        .bind(settings.max_call_depth.to_string())
        .execute(self.db)
        .await
        .map_err(|e| format!("保存路由深度设置失败: {e}"))?;
        sqlx::query(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('route_node_timeout_seconds', ?)",
        )
        .bind(settings.node_timeout_seconds.to_string())
        .execute(self.db)
        .await
        .map_err(|e| format!("保存路由超时设置失败: {e}"))?;
        sqlx::query(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('route_retry_count', ?)",
        )
        .bind(settings.retry_count.to_string())
        .execute(self.db)
        .await
        .map_err(|e| format!("保存路由重试设置失败: {e}"))?;
        Ok(())
    }

    async fn save_provider_config(&self, config: ProviderConfig) -> Result<String, String> {
        let id = if config.id.trim().is_empty() {
            Uuid::new_v4().to_string()
        } else {
            config.id.clone()
        };
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT OR REPLACE INTO provider_configs (id, provider_key, display_name, protocol_type, base_url, auth_type, api_key_encrypted, org_id, extra_json, enabled, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, COALESCE((SELECT created_at FROM provider_configs WHERE id = ?), ?), ?)",
        )
        .bind(&id)
        .bind(&config.provider_key)
        .bind(&config.display_name)
        .bind(&config.protocol_type)
        .bind(&config.base_url)
        .bind(&config.auth_type)
        .bind(&config.api_key_encrypted)
        .bind(&config.org_id)
        .bind(&config.extra_json)
        .bind(config.enabled)
        .bind(&id)
        .bind(&now)
        .bind(&now)
        .execute(self.db)
        .await
        .map_err(|e| format!("保存 Provider 配置失败: {e}"))?;
        Ok(id)
    }

    async fn save_model_config(
        &self,
        config: ModelConfig,
        api_key: String,
    ) -> Result<String, String> {
        let id = if config.id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            config.id.clone()
        };
        sqlx::query(
            "INSERT OR REPLACE INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&config.name)
        .bind(&config.api_format)
        .bind(&config.base_url)
        .bind(&config.model_name)
        .bind(config.is_default)
        .bind(&api_key)
        .execute(self.db)
        .await
        .map_err(|e| format!("保存模型配置失败: {e}"))?;
        Ok(id)
    }

    async fn list_provider_configs(&self) -> Result<Vec<ProviderConfig>, String> {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, String, bool)>(
            "SELECT id, provider_key, display_name, protocol_type, base_url, auth_type, api_key_encrypted, org_id, extra_json, CAST(enabled AS BOOLEAN)
             FROM provider_configs ORDER BY updated_at DESC",
        )
        .fetch_all(self.db)
        .await
        .map_err(|e| format!("读取 Provider 配置失败: {e}"))?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    provider_key,
                    display_name,
                    protocol_type,
                    base_url,
                    auth_type,
                    api_key_encrypted,
                    org_id,
                    extra_json,
                    enabled,
                )| ProviderConfig {
                    id,
                    provider_key,
                    display_name,
                    protocol_type,
                    base_url,
                    auth_type,
                    api_key_encrypted,
                    org_id,
                    extra_json,
                    enabled,
                },
            )
            .collect())
    }

    async fn delete_provider_config(&self, provider_id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM provider_configs WHERE id = ?")
            .bind(provider_id)
            .execute(self.db)
            .await
            .map_err(|e| format!("删除 Provider 配置失败: {e}"))?;
        Ok(())
    }

    async fn delete_model_config(&self, model_id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM model_configs WHERE id = ?")
            .bind(model_id)
            .execute(self.db)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn set_default_model(&self, model_id: &str) -> Result<(), String> {
        sqlx::query("UPDATE model_configs SET is_default = 0 WHERE api_format NOT LIKE 'search_%'")
            .execute(self.db)
            .await
            .map_err(|e| e.to_string())?;

        sqlx::query(
            "UPDATE model_configs SET is_default = 1 WHERE id = ? AND api_format NOT LIKE 'search_%'",
        )
        .bind(model_id)
        .execute(self.db)
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    async fn upsert_capability_routing_policy(
        &self,
        policy: CapabilityRoutingPolicy,
    ) -> Result<(), String> {
        sqlx::query(
            "INSERT OR REPLACE INTO routing_policies (capability, primary_provider_id, primary_model, fallback_chain_json, timeout_ms, retry_count, enabled)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&policy.capability)
        .bind(&policy.primary_provider_id)
        .bind(&policy.primary_model)
        .bind(&policy.fallback_chain_json)
        .bind(policy.timeout_ms)
        .bind(policy.retry_count)
        .bind(policy.enabled)
        .execute(self.db)
        .await
        .map_err(|e| format!("保存能力路由策略失败: {e}"))?;
        Ok(())
    }

    async fn get_capability_routing_policy(
        &self,
        capability: &str,
    ) -> Result<Option<CapabilityRoutingPolicy>, String> {
        let row = sqlx::query_as::<_, (String, String, String, i64, i64, bool)>(
            "SELECT primary_provider_id, primary_model, fallback_chain_json, timeout_ms, retry_count, CAST(enabled AS BOOLEAN)
             FROM routing_policies WHERE capability = ? LIMIT 1",
        )
        .bind(capability)
        .fetch_optional(self.db)
        .await
        .map_err(|e| format!("读取能力路由策略失败: {e}"))?;

        Ok(row.map(
            |(
                primary_provider_id,
                primary_model,
                fallback_chain_json,
                timeout_ms,
                retry_count,
                enabled,
            )| CapabilityRoutingPolicy {
                capability: capability.to_string(),
                primary_provider_id,
                primary_model,
                fallback_chain_json,
                timeout_ms,
                retry_count,
                enabled,
            },
        ))
    }
}

#[async_trait]
impl ModelsConfigRepository for NullModelsRepository {
    async fn load_routing_settings(&self) -> Result<Vec<(String, String)>, String> {
        Ok(Vec::new())
    }

    async fn save_routing_settings(&self, _settings: &RoutingSettings) -> Result<(), String> {
        Err("not used".to_string())
    }

    async fn save_provider_config(&self, _config: ProviderConfig) -> Result<String, String> {
        Err("not used".to_string())
    }

    async fn save_model_config(
        &self,
        _config: ModelConfig,
        _api_key: String,
    ) -> Result<String, String> {
        Err("not used".to_string())
    }

    async fn list_provider_configs(&self) -> Result<Vec<ProviderConfig>, String> {
        Err("not used".to_string())
    }

    async fn delete_provider_config(&self, _provider_id: &str) -> Result<(), String> {
        Err("not used".to_string())
    }

    async fn delete_model_config(&self, _model_id: &str) -> Result<(), String> {
        Err("not used".to_string())
    }

    async fn set_default_model(&self, _model_id: &str) -> Result<(), String> {
        Err("not used".to_string())
    }

    async fn upsert_capability_routing_policy(
        &self,
        _policy: CapabilityRoutingPolicy,
    ) -> Result<(), String> {
        Err("not used".to_string())
    }

    async fn get_capability_routing_policy(
        &self,
        _capability: &str,
    ) -> Result<Option<CapabilityRoutingPolicy>, String> {
        Err("not used".to_string())
    }
}
