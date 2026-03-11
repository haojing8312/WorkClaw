use super::models::{
    CapabilityRoutingPolicy, ModelConfig, ProviderConfig, ProviderPluginInfo, RoutingSettings,
};
use crate::providers::ProviderRegistry;
use async_trait::async_trait;
use runtime_models_app::{
    ModelCatalogCacheEntry, ModelsConfigRepository, ModelsReadRepository, ProviderCatalog,
    ProviderConnectionInfo, ProviderHealthProbe, RouteAttemptLog, RouteAttemptStat,
};
use sqlx::SqlitePool;
use uuid::Uuid;

pub struct PoolModelsRepository<'a> {
    db: &'a SqlitePool,
}

impl<'a> PoolModelsRepository<'a> {
    pub fn new(db: &'a SqlitePool) -> Self {
        Self { db }
    }
}

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
impl ModelsReadRepository for PoolModelsRepository<'_> {
    async fn list_enabled_provider_keys(&self) -> Result<Vec<(String, String)>, String> {
        sqlx::query_as::<_, (String, String, bool)>(
            "SELECT id, provider_key, CAST(enabled AS BOOLEAN) FROM provider_configs",
        )
        .fetch_all(self.db)
        .await
        .map(|rows| {
            rows.into_iter()
                .filter(|(_, _, enabled)| *enabled)
                .map(|(id, key, _)| (id, key))
                .collect()
        })
        .map_err(|e| format!("读取 Provider 配置失败: {e}"))
    }

    async fn list_enabled_provider_ids(&self) -> Result<Vec<String>, String> {
        sqlx::query_scalar::<_, String>(
            "SELECT id FROM provider_configs WHERE enabled = 1 ORDER BY updated_at DESC",
        )
        .fetch_all(self.db)
        .await
        .map_err(|e| format!("读取 Provider 列表失败: {e}"))
    }

    async fn query_candidate_model_id(
        &self,
        default_only: bool,
        require_api_key: bool,
    ) -> Result<Option<String>, String> {
        let mut sql =
            String::from("SELECT id FROM model_configs WHERE api_format NOT LIKE 'search_%'");
        if default_only {
            sql.push_str(" AND is_default = 1");
        }
        if require_api_key {
            sql.push_str(" AND TRIM(api_key) != ''");
        }
        sql.push_str(" ORDER BY rowid ASC LIMIT 1");

        sqlx::query_as::<_, (String,)>(&sql)
            .fetch_optional(self.db)
            .await
            .map_err(|e| e.to_string())
            .map(|row| row.map(|(id,)| id))
    }

    async fn get_provider_key(&self, provider_id: &str) -> Result<String, String> {
        sqlx::query_scalar::<_, String>(
            "SELECT provider_key FROM provider_configs WHERE id = ? LIMIT 1",
        )
        .bind(provider_id)
        .fetch_optional(self.db)
        .await
        .map_err(|e| format!("读取 Provider Key 失败: {e}"))?
        .ok_or_else(|| "Provider 配置不存在".to_string())
    }

    async fn load_model_catalog_cache(
        &self,
        provider_id: &str,
    ) -> Result<Vec<ModelCatalogCacheEntry>, String> {
        let rows = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT model_id, fetched_at, ttl_seconds FROM model_catalog_cache WHERE provider_id = ?",
        )
        .bind(provider_id)
        .fetch_all(self.db)
        .await
        .map_err(|e| format!("读取模型缓存失败: {e}"))?;

        Ok(rows
            .into_iter()
            .map(
                |(model_id, fetched_at, ttl_seconds)| ModelCatalogCacheEntry {
                    model_id,
                    fetched_at,
                    ttl_seconds,
                },
            )
            .collect())
    }

    async fn replace_model_catalog_cache(
        &self,
        provider_id: &str,
        models: &[String],
        fetched_at: &str,
        ttl_seconds: i64,
    ) -> Result<(), String> {
        sqlx::query("DELETE FROM model_catalog_cache WHERE provider_id = ?")
            .bind(provider_id)
            .execute(self.db)
            .await
            .map_err(|e| format!("清理模型缓存失败: {e}"))?;
        for model in models {
            let raw_json = serde_json::json!({ "model": model }).to_string();
            sqlx::query(
                "INSERT OR REPLACE INTO model_catalog_cache (provider_id, model_id, raw_json, fetched_at, ttl_seconds) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(provider_id)
            .bind(model)
            .bind(raw_json)
            .bind(fetched_at)
            .bind(ttl_seconds)
            .execute(self.db)
            .await
            .map_err(|e| format!("写入模型缓存失败: {e}"))?;
        }
        Ok(())
    }

    async fn list_recent_route_attempt_logs(
        &self,
        session_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<RouteAttemptLog>, String> {
        let rows = if let Some(sid) = session_id {
            sqlx::query_as::<_, (String, String, String, String, i64, i64, String, bool, String, String)>(
                "SELECT session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, CAST(success AS BOOLEAN), error_message, created_at
                 FROM route_attempt_logs WHERE session_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(sid)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db)
            .await
            .map_err(|e| format!("读取路由尝试日志失败: {e}"))?
        } else {
            sqlx::query_as::<_, (String, String, String, String, i64, i64, String, bool, String, String)>(
                "SELECT session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, CAST(success AS BOOLEAN), error_message, created_at
                 FROM route_attempt_logs ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db)
            .await
            .map_err(|e| format!("读取路由尝试日志失败: {e}"))?
        };

        Ok(rows
            .into_iter()
            .map(
                |(
                    session_id,
                    capability,
                    api_format,
                    model_name,
                    attempt_index,
                    retry_index,
                    error_kind,
                    success,
                    error_message,
                    created_at,
                )| RouteAttemptLog {
                    session_id,
                    capability,
                    api_format,
                    model_name,
                    attempt_index,
                    retry_index,
                    error_kind,
                    success,
                    error_message,
                    created_at,
                },
            )
            .collect())
    }

    async fn list_route_attempt_logs_since(
        &self,
        session_id: Option<&str>,
        cutoff_rfc3339: &str,
    ) -> Result<Vec<RouteAttemptLog>, String> {
        let rows = if let Some(sid) = session_id {
            sqlx::query_as::<_, (String, String, String, String, i64, i64, String, bool, String, String)>(
                "SELECT session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, CAST(success AS BOOLEAN), error_message, created_at
                 FROM route_attempt_logs
                 WHERE created_at >= ? AND session_id = ?
                 ORDER BY created_at DESC",
            )
            .bind(cutoff_rfc3339)
            .bind(sid)
            .fetch_all(self.db)
            .await
            .map_err(|e| format!("读取路由日志失败: {e}"))?
        } else {
            sqlx::query_as::<_, (String, String, String, String, i64, i64, String, bool, String, String)>(
                "SELECT session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, CAST(success AS BOOLEAN), error_message, created_at
                 FROM route_attempt_logs
                 WHERE created_at >= ?
                 ORDER BY created_at DESC",
            )
            .bind(cutoff_rfc3339)
            .fetch_all(self.db)
            .await
            .map_err(|e| format!("读取路由日志失败: {e}"))?
        };

        Ok(rows
            .into_iter()
            .map(
                |(
                    session_id,
                    capability,
                    api_format,
                    model_name,
                    attempt_index,
                    retry_index,
                    error_kind,
                    success,
                    error_message,
                    created_at,
                )| RouteAttemptLog {
                    session_id,
                    capability,
                    api_format,
                    model_name,
                    attempt_index,
                    retry_index,
                    error_kind,
                    success,
                    error_message,
                    created_at,
                },
            )
            .collect())
    }

    async fn list_route_attempt_stats(
        &self,
        hours: i64,
        capability: Option<&str>,
    ) -> Result<Vec<RouteAttemptStat>, String> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::hours(hours)).to_rfc3339();
        let rows = if let Some(cap) = capability {
            sqlx::query_as::<_, (String, String, bool, i64)>(
                "SELECT capability, error_kind, CAST(success AS BOOLEAN), COUNT(*) as cnt
                 FROM route_attempt_logs
                 WHERE created_at >= ? AND capability = ?
                 GROUP BY capability, error_kind, success
                 ORDER BY cnt DESC",
            )
            .bind(cutoff)
            .bind(cap)
            .fetch_all(self.db)
            .await
            .map_err(|e| format!("读取路由统计失败: {e}"))?
        } else {
            sqlx::query_as::<_, (String, String, bool, i64)>(
                "SELECT capability, error_kind, CAST(success AS BOOLEAN), COUNT(*) as cnt
                 FROM route_attempt_logs
                 WHERE created_at >= ?
                 GROUP BY capability, error_kind, success
                 ORDER BY cnt DESC",
            )
            .bind(cutoff)
            .fetch_all(self.db)
            .await
            .map_err(|e| format!("读取路由统计失败: {e}"))?
        };

        Ok(rows
            .into_iter()
            .map(
                |(capability, error_kind, success, count)| RouteAttemptStat {
                    capability,
                    error_kind,
                    success,
                    count,
                },
            )
            .collect())
    }

    async fn get_provider_connection_info(
        &self,
        provider_id: &str,
    ) -> Result<Option<ProviderConnectionInfo>, String> {
        let row = sqlx::query_as::<_, (String, String, String)>(
            "SELECT protocol_type, base_url, api_key_encrypted FROM provider_configs WHERE id = ? AND enabled = 1 LIMIT 1",
        )
        .bind(provider_id)
        .fetch_optional(self.db)
        .await
        .map_err(|e| format!("读取 Provider 配置失败: {e}"))?;

        Ok(row.map(
            |(protocol_type, base_url, api_key)| ProviderConnectionInfo {
                provider_id: provider_id.to_string(),
                protocol_type,
                base_url,
                api_key,
            },
        ))
    }
}

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

#[async_trait]
impl ModelsReadRepository for NullModelsRepository {
    async fn list_enabled_provider_keys(&self) -> Result<Vec<(String, String)>, String> {
        Err("not used".to_string())
    }

    async fn list_enabled_provider_ids(&self) -> Result<Vec<String>, String> {
        Err("not used".to_string())
    }

    async fn query_candidate_model_id(
        &self,
        _default_only: bool,
        _require_api_key: bool,
    ) -> Result<Option<String>, String> {
        Err("not used".to_string())
    }

    async fn get_provider_key(&self, _provider_id: &str) -> Result<String, String> {
        Err("not used".to_string())
    }

    async fn load_model_catalog_cache(
        &self,
        _provider_id: &str,
    ) -> Result<Vec<ModelCatalogCacheEntry>, String> {
        Err("not used".to_string())
    }

    async fn replace_model_catalog_cache(
        &self,
        _provider_id: &str,
        _models: &[String],
        _fetched_at: &str,
        _ttl_seconds: i64,
    ) -> Result<(), String> {
        Err("not used".to_string())
    }

    async fn list_recent_route_attempt_logs(
        &self,
        _session_id: Option<&str>,
        _limit: i64,
        _offset: i64,
    ) -> Result<Vec<RouteAttemptLog>, String> {
        Err("not used".to_string())
    }

    async fn list_route_attempt_logs_since(
        &self,
        _session_id: Option<&str>,
        _cutoff_rfc3339: &str,
    ) -> Result<Vec<RouteAttemptLog>, String> {
        Err("not used".to_string())
    }

    async fn list_route_attempt_stats(
        &self,
        _hours: i64,
        _capability: Option<&str>,
    ) -> Result<Vec<RouteAttemptStat>, String> {
        Err("not used".to_string())
    }

    async fn get_provider_connection_info(
        &self,
        _provider_id: &str,
    ) -> Result<Option<ProviderConnectionInfo>, String> {
        Err("not used".to_string())
    }
}

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
