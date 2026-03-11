use async_trait::async_trait;
use runtime_models_app::{
    ModelsAppService, ModelsConfigRepository, ModelsReadRepository, ProviderCatalog,
    ProviderPluginInfo, RouteAttemptLog, RouteAttemptStat,
};
use std::sync::Mutex;

#[derive(Default)]
struct FakeRepo {
    recent_args: Mutex<Vec<(Option<String>, i64, i64)>>,
    stats_args: Mutex<Vec<(i64, Option<String>)>>,
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

    async fn list_recent_route_attempt_logs(
        &self,
        session_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<RouteAttemptLog>, String> {
        self.recent_args.lock().expect("recent args").push((
            session_id.map(str::to_string),
            limit,
            offset,
        ));
        Ok(vec![RouteAttemptLog {
            session_id: "s1".to_string(),
            capability: "chat".to_string(),
            api_format: "openai".to_string(),
            model_name: "deepseek-chat".to_string(),
            attempt_index: 1,
            retry_index: 0,
            error_kind: "ok".to_string(),
            success: true,
            error_message: String::new(),
            created_at: "2026-03-11T00:00:00Z".to_string(),
        }])
    }

    async fn list_route_attempt_logs_since(
        &self,
        _session_id: Option<&str>,
        _cutoff_rfc3339: &str,
    ) -> Result<Vec<RouteAttemptLog>, String> {
        Ok(vec![
            RouteAttemptLog {
                session_id: "s1".to_string(),
                capability: "chat".to_string(),
                api_format: "openai".to_string(),
                model_name: "deepseek-chat".to_string(),
                attempt_index: 1,
                retry_index: 0,
                error_kind: "rate_limit".to_string(),
                success: false,
                error_message: "boom \"quoted\"".to_string(),
                created_at: "2026-03-11T00:00:00Z".to_string(),
            },
            RouteAttemptLog {
                session_id: "s2".to_string(),
                capability: "vision".to_string(),
                api_format: "openai".to_string(),
                model_name: "qwen-vl-max".to_string(),
                attempt_index: 1,
                retry_index: 0,
                error_kind: "ok".to_string(),
                success: true,
                error_message: String::new(),
                created_at: "2026-03-11T00:01:00Z".to_string(),
            },
        ])
    }

    async fn list_route_attempt_stats(
        &self,
        hours: i64,
        capability: Option<&str>,
    ) -> Result<Vec<RouteAttemptStat>, String> {
        self.stats_args
            .lock()
            .expect("stats args")
            .push((hours, capability.map(str::to_string)));
        Ok(vec![RouteAttemptStat {
            capability: capability.unwrap_or("chat").to_string(),
            error_kind: "ok".to_string(),
            success: true,
            count: 2,
        }])
    }
}

struct EmptyCatalog;

impl ProviderCatalog for EmptyCatalog {
    fn list_provider_plugins(&self) -> Result<Vec<ProviderPluginInfo>, String> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn recent_logs_normalize_limit_and_offset() {
    let repo = FakeRepo::default();
    let service = ModelsAppService::new(repo, EmptyCatalog);
    let logs = service
        .list_recent_route_attempt_logs(Some("s1"), Some(999), Some(-5))
        .await
        .expect("recent logs");
    assert_eq!(logs.len(), 1);
}

#[tokio::test]
async fn export_csv_filters_rows_and_escapes_quotes() {
    let service = ModelsAppService::new(FakeRepo::default(), EmptyCatalog);
    let csv = service
        .export_route_attempt_logs_csv(None, Some(24), Some("chat"), Some("failed"), None)
        .await
        .expect("csv export");
    assert!(csv.contains("rate_limit"));
    assert!(!csv.contains("qwen-vl-max"));
    assert!(csv.contains("\"boom \"\"quoted\"\"\""));
}

#[tokio::test]
async fn route_attempt_stats_clamp_hours_before_query() {
    let repo = FakeRepo::default();
    let service = ModelsAppService::new(repo, EmptyCatalog);
    let stats = service
        .list_route_attempt_stats(24 * 90, Some("chat"))
        .await
        .expect("stats");
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].capability, "chat");
}
