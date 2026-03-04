use super::skills::DbState;
use crate::providers::ProviderRegistry;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub api_format: String,
    pub base_url: String,
    pub model_name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingSettings {
    pub max_call_depth: usize,
    pub node_timeout_seconds: u64,
    pub retry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPluginInfo {
    pub key: String,
    pub display_name: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub provider_key: String,
    pub display_name: String,
    pub protocol_type: String,
    pub base_url: String,
    pub auth_type: String,
    pub api_key_encrypted: String,
    pub org_id: String,
    pub extra_json: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRoutingPolicy {
    pub primary_provider_id: String,
    pub primary_model: String,
    pub fallback_chain_json: String,
    pub timeout_ms: i64,
    pub retry_count: i64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRoutingPolicy {
    pub capability: String,
    pub primary_provider_id: String,
    pub primary_model: String,
    pub fallback_chain_json: String,
    pub timeout_ms: i64,
    pub retry_count: i64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealthInfo {
    pub provider_id: String,
    pub ok: bool,
    pub protocol_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteAttemptLog {
    pub session_id: String,
    pub capability: String,
    pub api_format: String,
    pub model_name: String,
    pub attempt_index: i64,
    pub retry_index: i64,
    pub error_kind: String,
    pub success: bool,
    pub error_message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteAttemptStat {
    pub capability: String,
    pub error_kind: String,
    pub success: bool,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRouteTemplateInfo {
    pub template_id: String,
    pub name: String,
    pub description: String,
    pub capability: String,
}

struct TemplateFallbackDef {
    provider_keys: &'static [&'static str],
    model: &'static str,
}

struct CapabilityRouteTemplateDef {
    template_id: &'static str,
    name: &'static str,
    description: &'static str,
    capability: &'static str,
    primary_provider_keys: &'static [&'static str],
    primary_model: &'static str,
    fallback: &'static [TemplateFallbackDef],
    timeout_ms: i64,
    retry_count: i64,
}

fn builtin_capability_route_templates() -> Vec<CapabilityRouteTemplateDef> {
    vec![
        CapabilityRouteTemplateDef {
            template_id: "china-first-p0",
            name: "中国优先 P0",
            description: "优先使用 DeepSeek/Qwen/Kimi，兼顾成本与可用性",
            capability: "chat",
            primary_provider_keys: &["deepseek", "qwen", "moonshot"],
            primary_model: "deepseek-chat",
            fallback: &[
                TemplateFallbackDef {
                    provider_keys: &["qwen"],
                    model: "qwen-plus",
                },
                TemplateFallbackDef {
                    provider_keys: &["moonshot"],
                    model: "kimi-k2",
                },
            ],
            timeout_ms: 60000,
            retry_count: 1,
        },
        CapabilityRouteTemplateDef {
            template_id: "china-first-p0",
            name: "中国优先 P0",
            description: "视觉任务优先走国内多模态",
            capability: "vision",
            primary_provider_keys: &["qwen"],
            primary_model: "qwen-vl-max",
            fallback: &[TemplateFallbackDef {
                provider_keys: &["deepseek"],
                model: "deepseek-chat",
            }],
            timeout_ms: 90000,
            retry_count: 1,
        },
        CapabilityRouteTemplateDef {
            template_id: "china-first-p0",
            name: "中国优先 P0",
            description: "生图优先国内，海外作为兜底",
            capability: "image_gen",
            primary_provider_keys: &["qwen"],
            primary_model: "wanx2.1-t2i-plus",
            fallback: &[TemplateFallbackDef {
                provider_keys: &["openai"],
                model: "gpt-image-1",
            }],
            timeout_ms: 120000,
            retry_count: 1,
        },
        CapabilityRouteTemplateDef {
            template_id: "china-first-p0",
            name: "中国优先 P0",
            description: "语音转写优先国内 ASR",
            capability: "audio_stt",
            primary_provider_keys: &["qwen"],
            primary_model: "paraformer-v2",
            fallback: &[TemplateFallbackDef {
                provider_keys: &["openai"],
                model: "gpt-4o-mini-transcribe",
            }],
            timeout_ms: 90000,
            retry_count: 1,
        },
        CapabilityRouteTemplateDef {
            template_id: "china-first-p0",
            name: "中国优先 P0",
            description: "语音合成优先国内 TTS",
            capability: "audio_tts",
            primary_provider_keys: &["qwen"],
            primary_model: "cosyvoice-v1",
            fallback: &[TemplateFallbackDef {
                provider_keys: &["openai"],
                model: "gpt-4o-mini-tts",
            }],
            timeout_ms: 60000,
            retry_count: 1,
        },
    ]
}

pub fn list_capability_route_templates_for(
    capability: Option<&str>,
) -> Vec<CapabilityRouteTemplateInfo> {
    builtin_capability_route_templates()
        .into_iter()
        .filter(|t| capability.map(|c| c == t.capability).unwrap_or(true))
        .map(|t| CapabilityRouteTemplateInfo {
            template_id: t.template_id.to_string(),
            name: t.name.to_string(),
            description: t.description.to_string(),
            capability: t.capability.to_string(),
        })
        .collect()
}

pub async fn apply_capability_route_template_from_pool(
    db: &SqlitePool,
    capability: &str,
    template_id: &str,
) -> Result<CapabilityRoutingPolicy, String> {
    let template = builtin_capability_route_templates()
        .into_iter()
        .find(|t| t.template_id == template_id && t.capability == capability)
        .ok_or_else(|| format!("模板不存在: {} / {}", template_id, capability))?;

    let providers = sqlx::query_as::<_, (String, String, bool)>(
        "SELECT id, provider_key, CAST(enabled AS BOOLEAN) FROM provider_configs",
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("读取 Provider 配置失败: {e}"))?;

    let enabled_providers: Vec<(String, String)> = providers
        .into_iter()
        .filter(|(_, _, enabled)| *enabled)
        .map(|(id, key, _)| (id, key))
        .collect();

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

pub async fn load_routing_settings_from_pool(db: &SqlitePool) -> Result<RoutingSettings, String> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT key, value FROM app_settings WHERE key IN ('route_max_call_depth', 'route_node_timeout_seconds', 'route_retry_count')"
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("读取路由设置失败: {e}"))?;

    let mut max_call_depth = 4usize;
    let mut node_timeout_seconds = 60u64;
    let mut retry_count = 0usize;

    for (k, v) in rows {
        match k.as_str() {
            "route_max_call_depth" => {
                max_call_depth = v.parse::<usize>().unwrap_or(4).clamp(2, 8);
            }
            "route_node_timeout_seconds" => {
                node_timeout_seconds = v.parse::<u64>().unwrap_or(60).clamp(5, 600);
            }
            "route_retry_count" => {
                retry_count = v.parse::<usize>().unwrap_or(0).clamp(0, 2);
            }
            _ => {}
        }
    }

    Ok(RoutingSettings {
        max_call_depth,
        node_timeout_seconds,
        retry_count,
    })
}

pub async fn save_provider_config_to_pool(
    db: &SqlitePool,
    config: ProviderConfig,
) -> Result<String, String> {
    let id = if config.id.trim().is_empty() {
        Uuid::new_v4().to_string()
    } else {
        config.id.clone()
    };
    let now = Utc::now().to_rfc3339();
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
    .execute(db)
    .await
    .map_err(|e| format!("保存 Provider 配置失败: {e}"))?;
    Ok(id)
}

pub async fn list_provider_configs_from_pool(
    db: &SqlitePool,
) -> Result<Vec<ProviderConfig>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, String, bool)>(
        "SELECT id, provider_key, display_name, protocol_type, base_url, auth_type, api_key_encrypted, org_id, extra_json, CAST(enabled AS BOOLEAN)
         FROM provider_configs ORDER BY updated_at DESC",
    )
    .fetch_all(db)
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

pub async fn upsert_capability_routing_policy_to_pool(
    db: &SqlitePool,
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
    .execute(db)
    .await
    .map_err(|e| format!("保存能力路由策略失败: {e}"))?;
    Ok(())
}

pub async fn get_capability_routing_policy_from_pool(
    db: &SqlitePool,
    capability: &str,
) -> Result<Option<CapabilityRoutingPolicy>, String> {
    let row = sqlx::query_as::<_, (String, String, String, i64, i64, bool)>(
        "SELECT primary_provider_id, primary_model, fallback_chain_json, timeout_ms, retry_count, CAST(enabled AS BOOLEAN)
         FROM routing_policies WHERE capability = ? LIMIT 1",
    )
    .bind(capability)
    .fetch_optional(db)
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

pub async fn upsert_chat_routing_policy_to_pool(
    db: &SqlitePool,
    policy: ChatRoutingPolicy,
) -> Result<(), String> {
    upsert_capability_routing_policy_to_pool(
        db,
        CapabilityRoutingPolicy {
            capability: "chat".to_string(),
            primary_provider_id: policy.primary_provider_id,
            primary_model: policy.primary_model,
            fallback_chain_json: policy.fallback_chain_json,
            timeout_ms: policy.timeout_ms,
            retry_count: policy.retry_count,
            enabled: policy.enabled,
        },
    )
    .await
}

pub async fn get_chat_routing_policy_from_pool(
    db: &SqlitePool,
) -> Result<Option<ChatRoutingPolicy>, String> {
    let policy = get_capability_routing_policy_from_pool(db, "chat").await?;
    Ok(policy.map(|p| ChatRoutingPolicy {
        primary_provider_id: p.primary_provider_id,
        primary_model: p.primary_model,
        fallback_chain_json: p.fallback_chain_json,
        timeout_ms: p.timeout_ms,
        retry_count: p.retry_count,
        enabled: p.enabled,
    }))
}

fn default_model_for_protocol(protocol_type: &str) -> &str {
    if protocol_type == "anthropic" {
        "claude-3-5-haiku-20241022"
    } else {
        "gpt-4o-mini"
    }
}

async fn check_provider_health_from_pool(
    db: &SqlitePool,
    provider_id: &str,
) -> Result<ProviderHealthInfo, String> {
    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT protocol_type, base_url, api_key_encrypted FROM provider_configs WHERE id = ? AND enabled = 1 LIMIT 1",
    )
    .bind(provider_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("读取 Provider 配置失败: {e}"))?;

    let Some((protocol_type, base_url, api_key)) = row else {
        return Ok(ProviderHealthInfo {
            provider_id: provider_id.to_string(),
            ok: false,
            protocol_type: String::new(),
            message: "provider 不存在或未启用".to_string(),
        });
    };

    if api_key.trim().is_empty() {
        return Ok(ProviderHealthInfo {
            provider_id: provider_id.to_string(),
            ok: false,
            protocol_type,
            message: "API Key 为空".to_string(),
        });
    }

    let model = default_model_for_protocol(&protocol_type);
    let ok = if protocol_type == "anthropic" {
        crate::adapters::anthropic::test_connection(&base_url, &api_key, model).await
    } else {
        crate::adapters::openai::test_connection(&base_url, &api_key, model).await
    };

    match ok {
        Ok(true) => Ok(ProviderHealthInfo {
            provider_id: provider_id.to_string(),
            ok: true,
            protocol_type,
            message: "连接正常".to_string(),
        }),
        Ok(false) => Ok(ProviderHealthInfo {
            provider_id: provider_id.to_string(),
            ok: false,
            protocol_type,
            message: "连接失败".to_string(),
        }),
        Err(err) => Ok(ProviderHealthInfo {
            provider_id: provider_id.to_string(),
            ok: false,
            protocol_type,
            message: err.to_string(),
        }),
    }
}

fn recommended_models_for_provider(provider_key: &str) -> Vec<String> {
    match provider_key {
        "deepseek" => vec!["deepseek-chat".to_string(), "deepseek-reasoner".to_string()],
        "qwen" => vec![
            "qwen-max".to_string(),
            "qwen-plus".to_string(),
            "qwen-vl-max".to_string(),
            "qwen-vl-plus".to_string(),
            "qwen-tts".to_string(),
            "qwen-omni".to_string(),
        ],
        "moonshot" => vec![
            "kimi-k2".to_string(),
            "moonshot-v1-32k".to_string(),
            "moonshot-v1-128k".to_string(),
        ],
        "anthropic" => vec![
            "claude-3-5-haiku-20241022".to_string(),
            "claude-3-5-sonnet-20241022".to_string(),
            "claude-sonnet-4-5-20250929".to_string(),
        ],
        _ => vec![
            "gpt-4o-mini".to_string(),
            "gpt-4.1-mini".to_string(),
            "gpt-image-1".to_string(),
            "whisper-1".to_string(),
            "tts-1".to_string(),
        ],
    }
}

fn filter_models_by_capability(models: Vec<String>, capability: Option<&str>) -> Vec<String> {
    let Some(capability) = capability else {
        return models;
    };
    let original = models.clone();
    let filtered = if capability == "vision" {
        models
            .into_iter()
            .filter(|m| {
                m.contains("vl") || m.contains("4o") || m.contains("claude") || m.contains("omni")
            })
            .collect::<Vec<_>>()
    } else if capability == "reasoning" {
        models
            .into_iter()
            .filter(|m| m.contains("reasoner") || m.contains("k2") || m.contains("sonnet"))
            .collect::<Vec<_>>()
    } else if capability == "image_gen" {
        models
            .into_iter()
            .filter(|m| {
                m.contains("image")
                    || m.contains("vl")
                    || m.contains("omni")
                    || m.contains("cogview")
            })
            .collect::<Vec<_>>()
    } else if capability == "audio_stt" {
        models
            .into_iter()
            .filter(|m| m.contains("whisper") || m.contains("stt") || m.contains("omni"))
            .collect::<Vec<_>>()
    } else if capability == "audio_tts" {
        models
            .into_iter()
            .filter(|m| m.contains("tts") || m.contains("omni"))
            .collect::<Vec<_>>()
    } else {
        models
    };
    if filtered.is_empty() {
        original
    } else {
        filtered
    }
}

fn cache_row_is_fresh(fetched_at: &str, ttl_seconds: i64) -> bool {
    let Ok(parsed) = DateTime::parse_from_rfc3339(fetched_at) else {
        return false;
    };
    let age = Utc::now()
        .signed_duration_since(parsed.with_timezone(&Utc))
        .num_seconds();
    age >= 0 && age < ttl_seconds
}

pub async fn list_provider_models_from_pool(
    db: &SqlitePool,
    provider_id: &str,
    capability: Option<&str>,
) -> Result<Vec<String>, String> {
    let provider_key = sqlx::query_scalar::<_, String>(
        "SELECT provider_key FROM provider_configs WHERE id = ? LIMIT 1",
    )
    .bind(provider_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("读取 Provider Key 失败: {e}"))?
    .ok_or_else(|| "Provider 配置不存在".to_string())?;

    let cache_rows = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT model_id, fetched_at, ttl_seconds FROM model_catalog_cache WHERE provider_id = ?",
    )
    .bind(provider_id)
    .fetch_all(db)
    .await
    .map_err(|e| format!("读取模型缓存失败: {e}"))?;

    let cached_models: Option<Vec<String>> = if cache_rows.is_empty() {
        None
    } else if cache_rows
        .iter()
        .all(|(_, fetched_at, ttl)| cache_row_is_fresh(fetched_at, *ttl))
    {
        Some(
            cache_rows
                .into_iter()
                .map(|(model_id, _, _)| model_id)
                .collect(),
        )
    } else {
        None
    };

    let models = if let Some(models) = cached_models {
        models
    } else {
        let fresh_models = recommended_models_for_provider(&provider_key);
        let now = Utc::now().to_rfc3339();
        let ttl_seconds = 3600i64;
        sqlx::query("DELETE FROM model_catalog_cache WHERE provider_id = ?")
            .bind(provider_id)
            .execute(db)
            .await
            .map_err(|e| format!("清理模型缓存失败: {e}"))?;
        for model in &fresh_models {
            let raw_json = serde_json::json!({ "model": model }).to_string();
            sqlx::query(
                "INSERT OR REPLACE INTO model_catalog_cache (provider_id, model_id, raw_json, fetched_at, ttl_seconds) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(provider_id)
            .bind(model)
            .bind(raw_json)
            .bind(&now)
            .bind(ttl_seconds)
            .execute(db)
            .await
            .map_err(|e| format!("写入模型缓存失败: {e}"))?;
        }
        fresh_models
    };

    let mut out = filter_models_by_capability(models, capability);
    out.sort();
    Ok(out)
}

#[tauri::command]
pub async fn get_routing_settings(db: State<'_, DbState>) -> Result<RoutingSettings, String> {
    load_routing_settings_from_pool(&db.0).await
}

#[tauri::command]
pub async fn set_routing_settings(
    settings: RoutingSettings,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let max_call_depth = settings.max_call_depth.clamp(2, 8).to_string();
    let node_timeout_seconds = settings.node_timeout_seconds.clamp(5, 600).to_string();
    let retry_count = settings.retry_count.clamp(0, 2).to_string();

    sqlx::query(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('route_max_call_depth', ?)",
    )
    .bind(&max_call_depth)
    .execute(&db.0)
    .await
    .map_err(|e| format!("保存路由深度设置失败: {e}"))?;
    sqlx::query(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('route_node_timeout_seconds', ?)",
    )
    .bind(&node_timeout_seconds)
    .execute(&db.0)
    .await
    .map_err(|e| format!("保存路由超时设置失败: {e}"))?;
    sqlx::query("INSERT OR REPLACE INTO app_settings (key, value) VALUES ('route_retry_count', ?)")
        .bind(&retry_count)
        .execute(&db.0)
        .await
        .map_err(|e| format!("保存路由重试设置失败: {e}"))?;

    Ok(())
}

#[tauri::command]
pub async fn save_provider_config(
    config: ProviderConfig,
    db: State<'_, DbState>,
) -> Result<String, String> {
    save_provider_config_to_pool(&db.0, config).await
}

#[tauri::command]
pub async fn list_provider_configs(db: State<'_, DbState>) -> Result<Vec<ProviderConfig>, String> {
    list_provider_configs_from_pool(&db.0).await
}

#[tauri::command]
pub async fn delete_provider_config(
    provider_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM provider_configs WHERE id = ?")
        .bind(&provider_id)
        .execute(&db.0)
        .await
        .map_err(|e| format!("删除 Provider 配置失败: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn set_chat_routing_policy(
    policy: ChatRoutingPolicy,
    db: State<'_, DbState>,
) -> Result<(), String> {
    upsert_chat_routing_policy_to_pool(&db.0, policy).await
}

#[tauri::command]
pub async fn get_chat_routing_policy(
    db: State<'_, DbState>,
) -> Result<Option<ChatRoutingPolicy>, String> {
    get_chat_routing_policy_from_pool(&db.0).await
}

#[tauri::command]
pub async fn set_capability_routing_policy(
    policy: CapabilityRoutingPolicy,
    db: State<'_, DbState>,
) -> Result<(), String> {
    upsert_capability_routing_policy_to_pool(&db.0, policy).await
}

#[tauri::command]
pub async fn get_capability_routing_policy(
    capability: String,
    db: State<'_, DbState>,
) -> Result<Option<CapabilityRoutingPolicy>, String> {
    get_capability_routing_policy_from_pool(&db.0, &capability).await
}

#[tauri::command]
pub async fn test_provider_health(
    provider_id: String,
    db: State<'_, DbState>,
) -> Result<ProviderHealthInfo, String> {
    check_provider_health_from_pool(&db.0, &provider_id).await
}

#[tauri::command]
pub async fn test_all_provider_health(
    db: State<'_, DbState>,
) -> Result<Vec<ProviderHealthInfo>, String> {
    let ids = sqlx::query_scalar::<_, String>(
        "SELECT id FROM provider_configs WHERE enabled = 1 ORDER BY updated_at DESC",
    )
    .fetch_all(&db.0)
    .await
    .map_err(|e| format!("读取 Provider 列表失败: {e}"))?;

    let mut results = Vec::with_capacity(ids.len());
    for provider_id in ids {
        results.push(check_provider_health_from_pool(&db.0, &provider_id).await?);
    }
    Ok(results)
}

#[tauri::command]
pub async fn list_provider_recommended_models(
    provider_key: String,
    capability: Option<String>,
) -> Result<Vec<String>, String> {
    Ok(filter_models_by_capability(
        recommended_models_for_provider(&provider_key),
        capability.as_deref(),
    ))
}

#[tauri::command]
pub async fn list_provider_models(
    provider_id: String,
    capability: Option<String>,
    db: State<'_, DbState>,
) -> Result<Vec<String>, String> {
    list_provider_models_from_pool(&db.0, &provider_id, capability.as_deref()).await
}

#[tauri::command]
pub async fn list_capability_route_templates(
    capability: Option<String>,
) -> Result<Vec<CapabilityRouteTemplateInfo>, String> {
    Ok(list_capability_route_templates_for(capability.as_deref()))
}

#[tauri::command]
pub async fn apply_capability_route_template(
    capability: String,
    template_id: String,
    db: State<'_, DbState>,
) -> Result<CapabilityRoutingPolicy, String> {
    apply_capability_route_template_from_pool(&db.0, &capability, &template_id).await
}

#[tauri::command]
pub async fn list_recent_route_attempt_logs(
    session_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    db: State<'_, DbState>,
) -> Result<Vec<RouteAttemptLog>, String> {
    let lim = limit.unwrap_or(50).clamp(1, 500);
    let off = offset.unwrap_or(0).max(0);
    let rows = if let Some(sid) = session_id {
        sqlx::query_as::<_, (String, String, String, String, i64, i64, String, bool, String, String)>(
            "SELECT session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, CAST(success AS BOOLEAN), error_message, created_at
             FROM route_attempt_logs WHERE session_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(sid)
        .bind(lim)
        .bind(off)
        .fetch_all(&db.0)
        .await
        .map_err(|e| format!("读取路由尝试日志失败: {e}"))?
    } else {
        sqlx::query_as::<_, (String, String, String, String, i64, i64, String, bool, String, String)>(
            "SELECT session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, CAST(success AS BOOLEAN), error_message, created_at
             FROM route_attempt_logs ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(lim)
        .bind(off)
        .fetch_all(&db.0)
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

#[tauri::command]
pub async fn export_route_attempt_logs_csv(
    session_id: Option<String>,
    hours: Option<i64>,
    capability: Option<String>,
    result_filter: Option<String>,
    error_kind: Option<String>,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let h = hours.unwrap_or(24).clamp(1, 24 * 90);
    let cutoff = (Utc::now() - chrono::Duration::hours(h)).to_rfc3339();
    let rows = if let Some(sid) = session_id {
        sqlx::query_as::<_, (String, String, String, String, i64, i64, String, bool, String, String)>(
            "SELECT session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, CAST(success AS BOOLEAN), error_message, created_at
             FROM route_attempt_logs
             WHERE created_at >= ? AND session_id = ?
             ORDER BY created_at DESC",
        )
        .bind(cutoff)
        .bind(sid)
        .fetch_all(&db.0)
        .await
        .map_err(|e| format!("读取路由日志失败: {e}"))?
    } else {
        sqlx::query_as::<_, (String, String, String, String, i64, i64, String, bool, String, String)>(
            "SELECT session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, CAST(success AS BOOLEAN), error_message, created_at
             FROM route_attempt_logs
             WHERE created_at >= ?
             ORDER BY created_at DESC",
        )
        .bind(cutoff)
        .fetch_all(&db.0)
        .await
        .map_err(|e| format!("读取路由日志失败: {e}"))?
    };

    let mut csv = String::from("created_at,session_id,capability,api_format,model_name,attempt_index,retry_index,error_kind,success,error_message\n");
    for (
        session_id,
        capability_value,
        api_format,
        model_name,
        attempt_index,
        retry_index,
        error_kind_value,
        success,
        error_message,
        created_at,
    ) in rows
    {
        if let Some(ref cap_filter) = capability {
            if cap_filter != "all" && capability_value != *cap_filter {
                continue;
            }
        }
        if let Some(ref result) = result_filter {
            if result == "success" && !success {
                continue;
            }
            if result == "failed" && success {
                continue;
            }
        }
        if let Some(ref err_filter) = error_kind {
            if err_filter != "all" && error_kind_value != *err_filter {
                continue;
            }
        }
        let escaped_error = error_message.replace('\"', "\"\"");
        csv.push_str(&format!(
            "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",{},{},\"{}\",{},\"{}\"\n",
            created_at,
            session_id,
            capability_value,
            api_format,
            model_name,
            attempt_index,
            retry_index,
            error_kind_value,
            if success { 1 } else { 0 },
            escaped_error
        ));
    }
    Ok(csv)
}

pub async fn list_route_attempt_stats_from_pool(
    db: &SqlitePool,
    hours: i64,
    capability: Option<&str>,
) -> Result<Vec<RouteAttemptStat>, String> {
    let h = hours.clamp(1, 24 * 30);
    let cutoff = (Utc::now() - chrono::Duration::hours(h)).to_rfc3339();
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
        .fetch_all(db)
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
        .fetch_all(db)
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

#[tauri::command]
pub async fn list_route_attempt_stats(
    hours: Option<i64>,
    capability: Option<String>,
    db: State<'_, DbState>,
) -> Result<Vec<RouteAttemptStat>, String> {
    list_route_attempt_stats_from_pool(&db.0, hours.unwrap_or(24), capability.as_deref()).await
}

#[tauri::command]
pub async fn save_model_config(
    config: ModelConfig,
    api_key: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let id = if config.id.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        config.id.clone()
    };

    sqlx::query(
        "INSERT OR REPLACE INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(&config.name)
    .bind(&config.api_format)
    .bind(&config.base_url)
    .bind(&config.model_name)
    .bind(config.is_default)
    .bind(&api_key)
    .execute(&db.0)
    .await
    .map_err(|e| format!("保存模型配置失败: {e}"))?;

    eprintln!(
        "[models] 模型已保存: id={id}, name={}, api_key={}...{}",
        config.name,
        &api_key[..6.min(api_key.len())],
        &api_key[api_key.len().saturating_sub(4)..]
    );

    Ok(())
}

#[tauri::command]
pub async fn list_model_configs(db: State<'_, DbState>) -> Result<Vec<ModelConfig>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, bool)>(
        "SELECT id, name, api_format, base_url, model_name, CAST(is_default AS BOOLEAN) FROM model_configs WHERE api_format NOT LIKE 'search_%'"
    )
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(
            |(id, name, api_format, base_url, model_name, is_default)| ModelConfig {
                id,
                name,
                api_format,
                base_url,
                model_name,
                is_default,
            },
        )
        .collect())
}

/// 获取指定配置的 API Key（编辑时用）
#[tauri::command]
pub async fn get_model_api_key(model_id: String, db: State<'_, DbState>) -> Result<String, String> {
    let row = sqlx::query_as::<_, (String,)>("SELECT api_key FROM model_configs WHERE id = ?")
        .bind(&model_id)
        .fetch_optional(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    match row {
        Some((key,)) => Ok(key),
        None => Err("配置不存在".to_string()),
    }
}

#[tauri::command]
pub async fn delete_model_config(model_id: String, db: State<'_, DbState>) -> Result<(), String> {
    sqlx::query("DELETE FROM model_configs WHERE id = ?")
        .bind(&model_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn test_connection_cmd(config: ModelConfig, api_key: String) -> Result<bool, String> {
    if config.api_format == "anthropic" {
        crate::adapters::anthropic::test_connection(&config.base_url, &api_key, &config.model_name)
            .await
            .map_err(|e| e.to_string())
    } else {
        crate::adapters::openai::test_connection(&config.base_url, &api_key, &config.model_name)
            .await
            .map_err(|e| e.to_string())
    }
}

/// 列出所有搜索 Provider 配置
#[tauri::command]
pub async fn list_search_configs(db: State<'_, DbState>) -> Result<Vec<ModelConfig>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, bool)>(
        "SELECT id, name, api_format, base_url, model_name, CAST(is_default AS BOOLEAN) FROM model_configs WHERE api_format LIKE 'search_%'"
    )
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(
            |(id, name, api_format, base_url, model_name, is_default)| ModelConfig {
                id,
                name,
                api_format,
                base_url,
                model_name,
                is_default,
            },
        )
        .collect())
}

/// 测试搜索 Provider 连接（执行一次最小化搜索请求）
#[tauri::command]
pub async fn test_search_connection(config: ModelConfig, api_key: String) -> Result<bool, String> {
    use crate::agent::tools::search_providers::{create_provider, SearchParams};

    let provider = create_provider(
        &config.api_format,
        &config.base_url,
        &api_key,
        &config.model_name,
    )
    .map_err(|e| format!("创建 Provider 失败: {}", e))?;

    let result = tokio::task::spawn_blocking(move || {
        provider.search(&SearchParams {
            query: "test".to_string(),
            count: 1,
            freshness: None,
        })
    })
    .await
    .map_err(|e| format!("测试线程异常: {}", e))?;

    match result {
        Ok(_) => Ok(true),
        Err(e) => Err(format!("连接测试失败: {}", e)),
    }
}

/// 设置默认搜索 Provider（同时取消同类其他配置的默认状态）
#[tauri::command]
pub async fn set_default_search(config_id: String, db: State<'_, DbState>) -> Result<(), String> {
    // 先清除所有搜索配置的默认标记
    sqlx::query("UPDATE model_configs SET is_default = 0 WHERE api_format LIKE 'search_%'")
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    // 再将指定配置设为默认
    sqlx::query("UPDATE model_configs SET is_default = 1 WHERE id = ?")
        .bind(&config_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// 列出内置 Provider 插件能力（用于设置页初始化和能力路由配置）
#[tauri::command]
pub async fn list_builtin_provider_plugins() -> Result<Vec<ProviderPluginInfo>, String> {
    let registry = ProviderRegistry::with_china_first_p0();
    let mut providers: Vec<ProviderPluginInfo> = registry
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
