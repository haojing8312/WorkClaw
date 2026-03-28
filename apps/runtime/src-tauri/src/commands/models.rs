use super::models_repo::{
    BuiltinProviderCatalog, NullModelsRepository, NullProviderCatalog, PoolModelsRepository,
    RuntimeProviderHealthProbe,
};
use super::skills::DbState;
use crate::model_errors::{
    build_failed_connection_test_result, build_success_connection_test_result,
    ModelConnectionTestResult,
};
use crate::model_transport::{resolve_model_transport, ModelTransportKind};
use runtime_models_app::ModelsAppService;
use runtime_routing_core::{
    filter_models_by_capability, recommended_models_for_provider, CapabilityRouteTemplateInfo,
};
use sqlx::SqlitePool;
use tauri::State;

pub use runtime_models_app::{
    CapabilityRoutingPolicy, ChatRoutingPolicy, ModelConfig, ProviderConfig, ProviderHealthInfo,
    ProviderPluginInfo, RouteAttemptLog, RouteAttemptStat, RoutingSettings,
};
pub use runtime_routing_core::list_capability_route_templates_for;

pub async fn apply_capability_route_template_from_pool(
    db: &SqlitePool,
    capability: &str,
    template_id: &str,
) -> Result<CapabilityRoutingPolicy, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(db), NullProviderCatalog);
    service
        .apply_capability_route_template(capability, template_id)
        .await
}

pub async fn load_routing_settings_from_pool(db: &SqlitePool) -> Result<RoutingSettings, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(db), NullProviderCatalog);
    service.load_routing_settings().await
}

pub async fn save_provider_config_to_pool(
    db: &SqlitePool,
    config: ProviderConfig,
) -> Result<String, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(db), NullProviderCatalog);
    service.save_provider_config(config).await
}

pub async fn list_provider_configs_from_pool(
    db: &SqlitePool,
) -> Result<Vec<ProviderConfig>, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(db), NullProviderCatalog);
    service.list_provider_configs().await
}

pub async fn upsert_capability_routing_policy_to_pool(
    db: &SqlitePool,
    policy: CapabilityRoutingPolicy,
) -> Result<(), String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(db), NullProviderCatalog);
    service.set_capability_routing_policy(policy).await
}

pub async fn get_capability_routing_policy_from_pool(
    db: &SqlitePool,
    capability: &str,
) -> Result<Option<CapabilityRoutingPolicy>, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(db), NullProviderCatalog);
    service.get_capability_routing_policy(capability).await
}

pub async fn upsert_chat_routing_policy_to_pool(
    db: &SqlitePool,
    policy: ChatRoutingPolicy,
) -> Result<(), String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(db), NullProviderCatalog);
    service.set_chat_routing_policy(policy).await
}

pub async fn get_chat_routing_policy_from_pool(
    db: &SqlitePool,
) -> Result<Option<ChatRoutingPolicy>, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(db), NullProviderCatalog);
    service.get_chat_routing_policy().await
}

async fn check_provider_health_from_pool(
    db: &SqlitePool,
    provider_id: &str,
) -> Result<ProviderHealthInfo, String> {
    let service = ModelsAppService::with_probe(
        PoolModelsRepository::new(db),
        NullProviderCatalog,
        RuntimeProviderHealthProbe,
    );
    service.test_provider_health(provider_id).await
}

pub async fn list_provider_models_from_pool(
    db: &SqlitePool,
    provider_id: &str,
    capability: Option<&str>,
) -> Result<Vec<String>, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(db), NullProviderCatalog);
    service.list_provider_models(provider_id, capability).await
}

#[tauri::command]
pub async fn get_routing_settings(db: State<'_, DbState>) -> Result<RoutingSettings, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service.load_routing_settings().await
}

#[tauri::command]
pub async fn set_routing_settings(
    settings: RoutingSettings,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service.save_routing_settings(settings).await
}

#[tauri::command]
pub async fn save_provider_config(
    config: ProviderConfig,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service.save_provider_config(config).await
}

#[tauri::command]
pub async fn list_provider_configs(db: State<'_, DbState>) -> Result<Vec<ProviderConfig>, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service.list_provider_configs().await
}

#[tauri::command]
pub async fn delete_provider_config(
    provider_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service.delete_provider_config(&provider_id).await
}

#[tauri::command]
pub async fn set_chat_routing_policy(
    policy: ChatRoutingPolicy,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service.set_chat_routing_policy(policy).await
}

#[tauri::command]
pub async fn get_chat_routing_policy(
    db: State<'_, DbState>,
) -> Result<Option<ChatRoutingPolicy>, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service.get_chat_routing_policy().await
}

#[tauri::command]
pub async fn set_capability_routing_policy(
    policy: CapabilityRoutingPolicy,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service.set_capability_routing_policy(policy).await
}

#[tauri::command]
pub async fn get_capability_routing_policy(
    capability: String,
    db: State<'_, DbState>,
) -> Result<Option<CapabilityRoutingPolicy>, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service.get_capability_routing_policy(&capability).await
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
    let service = ModelsAppService::with_probe(
        PoolModelsRepository::new(&db.0),
        NullProviderCatalog,
        RuntimeProviderHealthProbe,
    );
    service.test_all_provider_health().await
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
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service
        .list_provider_models(&provider_id, capability.as_deref())
        .await
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
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service
        .apply_capability_route_template(&capability, &template_id)
        .await
}

#[tauri::command]
pub async fn list_recent_route_attempt_logs(
    session_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    db: State<'_, DbState>,
) -> Result<Vec<RouteAttemptLog>, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service
        .list_recent_route_attempt_logs(session_id.as_deref(), limit, offset)
        .await
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
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service
        .export_route_attempt_logs_csv(
            session_id.as_deref(),
            hours,
            capability.as_deref(),
            result_filter.as_deref(),
            error_kind.as_deref(),
        )
        .await
}

pub async fn list_route_attempt_stats_from_pool(
    db: &SqlitePool,
    hours: i64,
    capability: Option<&str>,
) -> Result<Vec<RouteAttemptStat>, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(db), NullProviderCatalog);
    service.list_route_attempt_stats(hours, capability).await
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
) -> Result<String, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service.save_model_config(config, api_key).await
}

pub async fn save_model_config_with_pool(
    db: &SqlitePool,
    config: ModelConfig,
    api_key: String,
) -> Result<String, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(db), NullProviderCatalog);
    service.save_model_config(config, api_key).await
}

pub async fn resolve_default_model_id_with_pool(db: &SqlitePool) -> Result<Option<String>, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(db), NullProviderCatalog);
    service.resolve_default_model_id().await
}

pub async fn resolve_default_usable_model_id_with_pool(
    db: &SqlitePool,
) -> Result<Option<String>, String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(db), NullProviderCatalog);
    service.resolve_default_usable_model_id().await
}

#[tauri::command]
pub async fn list_model_configs(db: State<'_, DbState>) -> Result<Vec<ModelConfig>, String> {
    resolve_default_model_id_with_pool(&db.0).await?;

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
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service.delete_model_config(&model_id).await
}

pub async fn delete_model_config_with_pool(db: &SqlitePool, model_id: &str) -> Result<(), String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(db), NullProviderCatalog);
    service.delete_model_config(model_id).await
}

#[tauri::command]
pub async fn test_connection_cmd(
    config: ModelConfig,
    api_key: String,
) -> Result<ModelConnectionTestResult, String> {
    let transport = resolve_model_transport(&config.api_format, &config.base_url, None);
    let connection_result = if transport.kind == ModelTransportKind::AnthropicMessages {
        crate::adapters::anthropic::test_connection(&config.base_url, &api_key, &config.model_name)
            .await
    } else {
        crate::adapters::openai::test_connection(
            &transport,
            &config.base_url,
            &api_key,
            &config.model_name,
        )
        .await
    };

    let result = match connection_result {
        Ok(true) => build_success_connection_test_result(),
        Ok(false) => build_failed_connection_test_result("模型平台拒绝了连接测试请求。"),
        Err(error) => build_failed_connection_test_result(&error.to_string()),
    };

    Ok(result)
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

#[tauri::command]
pub async fn set_default_model(model_id: String, db: State<'_, DbState>) -> Result<(), String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(&db.0), NullProviderCatalog);
    service.set_default_model(&model_id).await
}

pub async fn set_default_model_with_pool(db: &SqlitePool, model_id: &str) -> Result<(), String> {
    let service = ModelsAppService::new(PoolModelsRepository::new(db), NullProviderCatalog);
    service.set_default_model(model_id).await
}

/// 列出内置 Provider 插件能力（用于设置页初始化和能力路由配置）
#[tauri::command]
pub async fn list_builtin_provider_plugins() -> Result<Vec<ProviderPluginInfo>, String> {
    let service = ModelsAppService::new(
        NullModelsRepository,
        BuiltinProviderCatalog::china_first_p0(),
    );
    service.list_provider_plugins()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_errors::ModelErrorKind;

    #[tokio::test]
    async fn test_connection_cmd_returns_structured_success_for_mock_provider() {
        let result = test_connection_cmd(
            ModelConfig {
                id: String::new(),
                name: "Mock".to_string(),
                api_format: "openai".to_string(),
                base_url: "http://mock".to_string(),
                model_name: "gpt-4o-mini".to_string(),
                is_default: false,
            },
            "sk-test".to_string(),
        )
        .await
        .expect("command result");

        assert!(result.ok);
        assert_eq!(result.title, "连接成功");
    }

    #[tokio::test]
    async fn test_connection_cmd_normalizes_network_failures() {
        let result = test_connection_cmd(
            ModelConfig {
                id: String::new(),
                name: "Broken".to_string(),
                api_format: "openai".to_string(),
                base_url: "http://127.0.0.1:9/v1".to_string(),
                model_name: "gpt-4o-mini".to_string(),
                is_default: false,
            },
            "sk-test".to_string(),
        )
        .await
        .expect("command result");

        assert!(!result.ok);
        assert_eq!(result.kind, ModelErrorKind::Network);
        assert_eq!(result.title, "网络连接失败");
    }
}
