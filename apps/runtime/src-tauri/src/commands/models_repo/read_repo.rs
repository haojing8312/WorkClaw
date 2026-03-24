use super::{NullModelsRepository, PoolModelsRepository};
use async_trait::async_trait;
use runtime_models_app::{
    ModelCatalogCacheEntry, ModelsReadRepository, ProviderConnectionInfo, RouteAttemptLog,
    RouteAttemptStat,
};

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
