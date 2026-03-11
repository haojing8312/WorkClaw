use crate::traits::{
    ModelsConfigRepository, ModelsReadRepository, ProviderCatalog, ProviderHealthProbe,
};
use crate::types::{
    CapabilityRoutingPolicy, ChatRoutingPolicy, ModelConfig, ProviderConfig, ProviderHealthInfo,
    ProviderPluginInfo, RoutingSettings,
};
use runtime_routing_core::{
    builtin_capability_route_templates, cache_row_is_fresh, default_model_for_protocol,
    filter_models_by_capability, recommended_models_for_provider,
};

pub struct ModelsAppService<R, C, P = NoopProviderHealthProbe> {
    repo: R,
    catalog: C,
    probe: P,
}

pub struct NoopProviderHealthProbe;

#[async_trait::async_trait]
impl ProviderHealthProbe for NoopProviderHealthProbe {
    async fn test_connection(
        &self,
        _protocol_type: &str,
        _base_url: &str,
        _api_key: &str,
        _model: &str,
    ) -> Result<bool, String> {
        Err("not implemented".to_string())
    }
}

impl<R, C> ModelsAppService<R, C, NoopProviderHealthProbe> {
    pub fn new(repo: R, catalog: C) -> Self {
        Self {
            repo,
            catalog,
            probe: NoopProviderHealthProbe,
        }
    }
}

impl<R, C, P> ModelsAppService<R, C, P> {
    pub fn with_probe(repo: R, catalog: C, probe: P) -> Self {
        Self {
            repo,
            catalog,
            probe,
        }
    }
}

impl<R, C, P> ModelsAppService<R, C, P>
where
    R: ModelsConfigRepository + ModelsReadRepository,
    C: ProviderCatalog,
    P: ProviderHealthProbe,
{
    pub async fn load_routing_settings(&self) -> Result<RoutingSettings, String> {
        let rows = self.repo.load_routing_settings().await?;
        let mut settings = RoutingSettings {
            max_call_depth: 4,
            node_timeout_seconds: 60,
            retry_count: 0,
        };

        for (k, v) in rows {
            match k.as_str() {
                "route_max_call_depth" => {
                    settings.max_call_depth = v.parse::<usize>().unwrap_or(4).clamp(2, 8);
                }
                "route_node_timeout_seconds" => {
                    settings.node_timeout_seconds = v.parse::<u64>().unwrap_or(60).clamp(5, 600);
                }
                "route_retry_count" => {
                    settings.retry_count = v.parse::<usize>().unwrap_or(0).clamp(0, 2);
                }
                _ => {}
            }
        }

        Ok(settings)
    }

    pub async fn save_routing_settings(&self, settings: RoutingSettings) -> Result<(), String> {
        let normalized = RoutingSettings {
            max_call_depth: settings.max_call_depth.clamp(2, 8),
            node_timeout_seconds: settings.node_timeout_seconds.clamp(5, 600),
            retry_count: settings.retry_count.clamp(0, 2),
        };
        self.repo.save_routing_settings(&normalized).await
    }

    pub async fn save_provider_config(&self, config: ProviderConfig) -> Result<String, String> {
        self.repo.save_provider_config(config).await
    }

    pub async fn save_model_config(
        &self,
        config: ModelConfig,
        api_key: String,
    ) -> Result<String, String> {
        self.repo.save_model_config(config, api_key).await
    }

    pub async fn list_provider_configs(&self) -> Result<Vec<ProviderConfig>, String> {
        self.repo.list_provider_configs().await
    }

    pub async fn delete_model_config(&self, model_id: &str) -> Result<(), String> {
        let was_default =
            self.repo.query_candidate_model_id(true, false).await? == Some(model_id.to_string());
        self.repo.delete_model_config(model_id).await?;

        if was_default {
            if let Some(replacement_id) = self.repo.query_candidate_model_id(false, false).await? {
                self.repo.set_default_model(&replacement_id).await?;
            }
        }

        Ok(())
    }

    pub async fn set_default_model(&self, model_id: &str) -> Result<(), String> {
        self.repo.set_default_model(model_id).await
    }

    pub async fn delete_provider_config(&self, provider_id: &str) -> Result<(), String> {
        self.repo.delete_provider_config(provider_id).await
    }

    pub fn list_provider_plugins(&self) -> Result<Vec<ProviderPluginInfo>, String> {
        self.catalog.list_provider_plugins()
    }

    pub async fn apply_capability_route_template(
        &self,
        capability: &str,
        template_id: &str,
    ) -> Result<CapabilityRoutingPolicy, String> {
        let template = builtin_capability_route_templates()
            .into_iter()
            .find(|t| t.template_id == template_id && t.capability == capability)
            .ok_or_else(|| format!("模板不存在: {} / {}", template_id, capability))?;

        let enabled_providers = self.repo.list_enabled_provider_keys().await?;
        let resolve_provider = |keys: &[&str]| -> Option<String> {
            enabled_providers
                .iter()
                .find(|(_, key)| keys.iter().any(|target| target == key))
                .map(|(id, _)| id.clone())
        };

        let primary_provider_id =
            resolve_provider(template.primary_provider_keys).ok_or_else(|| {
                format!(
                    "模板缺少主 Provider（需要其一）: {:?}",
                    template.primary_provider_keys
                )
            })?;

        let mut fallback_items: Vec<serde_json::Value> = Vec::new();
        for item in template.fallback {
            if let Some(provider_id) = resolve_provider(item.provider_keys) {
                fallback_items.push(serde_json::json!({
                    "provider_id": provider_id,
                    "model": item.model,
                }));
            }
        }

        Ok(CapabilityRoutingPolicy {
            capability: capability.to_string(),
            primary_provider_id,
            primary_model: template.primary_model.to_string(),
            fallback_chain_json: serde_json::to_string(&fallback_items)
                .unwrap_or_else(|_| "[]".to_string()),
            timeout_ms: template.timeout_ms,
            retry_count: template.retry_count,
            enabled: true,
        })
    }

    pub async fn set_capability_routing_policy(
        &self,
        policy: CapabilityRoutingPolicy,
    ) -> Result<(), String> {
        self.repo.upsert_capability_routing_policy(policy).await
    }

    pub async fn get_capability_routing_policy(
        &self,
        capability: &str,
    ) -> Result<Option<CapabilityRoutingPolicy>, String> {
        self.repo.get_capability_routing_policy(capability).await
    }

    pub async fn set_chat_routing_policy(&self, policy: ChatRoutingPolicy) -> Result<(), String> {
        self.repo
            .upsert_capability_routing_policy(CapabilityRoutingPolicy {
                capability: "chat".to_string(),
                primary_provider_id: policy.primary_provider_id,
                primary_model: policy.primary_model,
                fallback_chain_json: policy.fallback_chain_json,
                timeout_ms: policy.timeout_ms,
                retry_count: policy.retry_count,
                enabled: policy.enabled,
            })
            .await
    }

    pub async fn get_chat_routing_policy(&self) -> Result<Option<ChatRoutingPolicy>, String> {
        let policy = self.repo.get_capability_routing_policy("chat").await?;
        Ok(policy.map(|p| ChatRoutingPolicy {
            primary_provider_id: p.primary_provider_id,
            primary_model: p.primary_model,
            fallback_chain_json: p.fallback_chain_json,
            timeout_ms: p.timeout_ms,
            retry_count: p.retry_count,
            enabled: p.enabled,
        }))
    }

    pub async fn list_provider_models(
        &self,
        provider_id: &str,
        capability: Option<&str>,
    ) -> Result<Vec<String>, String> {
        let provider_key = self.repo.get_provider_key(provider_id).await?;
        let cache_rows = self.repo.load_model_catalog_cache(provider_id).await?;

        let cached_models = if cache_rows.is_empty() {
            None
        } else if cache_rows
            .iter()
            .all(|row| cache_row_is_fresh(&row.fetched_at, row.ttl_seconds))
        {
            Some(
                cache_rows
                    .into_iter()
                    .map(|row| row.model_id)
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };

        let models = if let Some(models) = cached_models {
            models
        } else {
            let fresh_models = recommended_models_for_provider(&provider_key);
            let now = chrono::Utc::now().to_rfc3339();
            self.repo
                .replace_model_catalog_cache(provider_id, &fresh_models, &now, 3600)
                .await?;
            fresh_models
        };

        let mut out = filter_models_by_capability(models, capability);
        out.sort();
        Ok(out)
    }

    pub async fn list_recent_route_attempt_logs(
        &self,
        session_id: Option<&str>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<crate::types::RouteAttemptLog>, String> {
        let lim = limit.unwrap_or(50).clamp(1, 500);
        let off = offset.unwrap_or(0).max(0);
        self.repo
            .list_recent_route_attempt_logs(session_id, lim, off)
            .await
    }

    pub async fn export_route_attempt_logs_csv(
        &self,
        session_id: Option<&str>,
        hours: Option<i64>,
        capability: Option<&str>,
        result_filter: Option<&str>,
        error_kind: Option<&str>,
    ) -> Result<String, String> {
        let h = hours.unwrap_or(24).clamp(1, 24 * 90);
        let cutoff = (chrono::Utc::now() - chrono::Duration::hours(h)).to_rfc3339();
        let rows = self
            .repo
            .list_route_attempt_logs_since(session_id, &cutoff)
            .await?;

        let mut csv = String::from(
            "created_at,session_id,capability,api_format,model_name,attempt_index,retry_index,error_kind,success,error_message\n",
        );
        for row in rows {
            if let Some(cap_filter) = capability {
                if cap_filter != "all" && row.capability != cap_filter {
                    continue;
                }
            }
            if let Some(result) = result_filter {
                if result == "success" && !row.success {
                    continue;
                }
                if result == "failed" && row.success {
                    continue;
                }
            }
            if let Some(err_filter) = error_kind {
                if err_filter != "all" && row.error_kind != err_filter {
                    continue;
                }
            }
            let escaped_error = row.error_message.replace('"', "\"\"");
            csv.push_str(&format!(
                "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",{},{},\"{}\",{},\"{}\"\n",
                row.created_at,
                row.session_id,
                row.capability,
                row.api_format,
                row.model_name,
                row.attempt_index,
                row.retry_index,
                row.error_kind,
                if row.success { 1 } else { 0 },
                escaped_error
            ));
        }
        Ok(csv)
    }

    pub async fn list_route_attempt_stats(
        &self,
        hours: i64,
        capability: Option<&str>,
    ) -> Result<Vec<crate::types::RouteAttemptStat>, String> {
        let normalized_hours = hours.clamp(1, 24 * 30);
        self.repo
            .list_route_attempt_stats(normalized_hours, capability)
            .await
    }

    async fn resolve_default_model_id_internal(
        &self,
        require_api_key: bool,
    ) -> Result<Option<String>, String> {
        if let Some(id) = self
            .repo
            .query_candidate_model_id(true, require_api_key)
            .await?
        {
            return Ok(Some(id));
        }

        let fallback = self
            .repo
            .query_candidate_model_id(false, require_api_key)
            .await?;
        if let Some(id) = fallback.as_deref() {
            self.repo.set_default_model(id).await?;
        }
        Ok(fallback)
    }

    pub async fn resolve_default_model_id(&self) -> Result<Option<String>, String> {
        self.resolve_default_model_id_internal(false).await
    }

    pub async fn resolve_default_usable_model_id(&self) -> Result<Option<String>, String> {
        self.resolve_default_model_id_internal(true).await
    }

    pub async fn test_provider_health(
        &self,
        provider_id: &str,
    ) -> Result<ProviderHealthInfo, String> {
        let Some(info) = self.repo.get_provider_connection_info(provider_id).await? else {
            return Ok(ProviderHealthInfo {
                provider_id: provider_id.to_string(),
                ok: false,
                protocol_type: String::new(),
                message: "provider 不存在或未启用".to_string(),
            });
        };

        if info.api_key.trim().is_empty() {
            return Ok(ProviderHealthInfo {
                provider_id: provider_id.to_string(),
                ok: false,
                protocol_type: info.protocol_type,
                message: "API Key 为空".to_string(),
            });
        }

        let model = default_model_for_protocol(&info.protocol_type);
        match self
            .probe
            .test_connection(&info.protocol_type, &info.base_url, &info.api_key, model)
            .await
        {
            Ok(true) => Ok(ProviderHealthInfo {
                provider_id: provider_id.to_string(),
                ok: true,
                protocol_type: info.protocol_type,
                message: "连接正常".to_string(),
            }),
            Ok(false) => Ok(ProviderHealthInfo {
                provider_id: provider_id.to_string(),
                ok: false,
                protocol_type: info.protocol_type,
                message: "连接失败".to_string(),
            }),
            Err(err) => Ok(ProviderHealthInfo {
                provider_id: provider_id.to_string(),
                ok: false,
                protocol_type: info.protocol_type,
                message: err,
            }),
        }
    }

    pub async fn test_all_provider_health(&self) -> Result<Vec<ProviderHealthInfo>, String> {
        let provider_ids = self.repo.list_enabled_provider_ids().await?;
        let mut results = Vec::with_capacity(provider_ids.len());
        for provider_id in provider_ids {
            results.push(self.test_provider_health(&provider_id).await?);
        }
        Ok(results)
    }
}
