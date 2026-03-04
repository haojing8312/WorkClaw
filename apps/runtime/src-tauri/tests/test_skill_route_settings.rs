mod helpers;

use runtime_lib::commands::models::{
    apply_capability_route_template_from_pool, get_capability_routing_policy_from_pool,
    get_chat_routing_policy_from_pool, list_capability_route_templates_for,
    list_provider_configs_from_pool, list_provider_models_from_pool,
    list_route_attempt_stats_from_pool, load_routing_settings_from_pool,
    save_provider_config_to_pool, upsert_capability_routing_policy_to_pool,
    upsert_chat_routing_policy_to_pool, CapabilityRoutingPolicy, ChatRoutingPolicy, ProviderConfig,
};

#[tokio::test]
async fn route_settings_use_defaults_when_empty() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let settings = load_routing_settings_from_pool(&pool)
        .await
        .expect("load settings");
    assert_eq!(settings.max_call_depth, 4);
    assert_eq!(settings.node_timeout_seconds, 60);
    assert_eq!(settings.retry_count, 0);
}

#[tokio::test]
async fn route_settings_parse_from_app_settings_table() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('route_max_call_depth', '7')",
    )
    .execute(&pool)
    .await
    .expect("set depth");
    sqlx::query("INSERT OR REPLACE INTO app_settings (key, value) VALUES ('route_node_timeout_seconds', '120')")
        .execute(&pool)
        .await
        .expect("set timeout");
    sqlx::query(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('route_retry_count', '2')",
    )
    .execute(&pool)
    .await
    .expect("set retry");

    let settings = load_routing_settings_from_pool(&pool)
        .await
        .expect("load settings");
    assert_eq!(settings.max_call_depth, 7);
    assert_eq!(settings.node_timeout_seconds, 120);
    assert_eq!(settings.retry_count, 2);
}

#[tokio::test]
async fn provider_tables_exist_after_init() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let required_tables = [
        "provider_configs",
        "provider_capabilities",
        "model_catalog_cache",
        "routing_policies",
        "route_attempt_logs",
    ];

    for table in required_tables {
        let row = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .expect("query sqlite_master");
        assert_eq!(row, 1, "table {table} should exist");
    }
}

#[tokio::test]
async fn provider_config_can_be_saved_and_listed() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let config = ProviderConfig {
        id: String::new(),
        provider_key: "deepseek".to_string(),
        display_name: "DeepSeek CN".to_string(),
        protocol_type: "openai".to_string(),
        base_url: "https://api.deepseek.com/v1".to_string(),
        auth_type: "api_key".to_string(),
        api_key_encrypted: "sk-test".to_string(),
        org_id: String::new(),
        extra_json: "{}".to_string(),
        enabled: true,
    };
    let id = save_provider_config_to_pool(&pool, config)
        .await
        .expect("save provider");
    assert!(!id.is_empty());

    let listed = list_provider_configs_from_pool(&pool)
        .await
        .expect("list providers");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].provider_key, "deepseek");
    assert_eq!(listed[0].protocol_type, "openai");
}

#[tokio::test]
async fn chat_routing_policy_can_be_saved_and_loaded() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let policy = ChatRoutingPolicy {
        primary_provider_id: "provider-1".to_string(),
        primary_model: "deepseek-chat".to_string(),
        fallback_chain_json: "[{\"provider_id\":\"provider-2\",\"model\":\"qwen-max\"}]"
            .to_string(),
        timeout_ms: 45000,
        retry_count: 1,
        enabled: true,
    };
    upsert_chat_routing_policy_to_pool(&pool, policy)
        .await
        .expect("save policy");

    let loaded = get_chat_routing_policy_from_pool(&pool)
        .await
        .expect("load policy")
        .expect("policy should exist");
    assert_eq!(loaded.primary_provider_id, "provider-1");
    assert_eq!(loaded.primary_model, "deepseek-chat");
    assert!(loaded.fallback_chain_json.contains("qwen-max"));
}

#[tokio::test]
async fn list_provider_models_reads_and_writes_cache() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let provider = ProviderConfig {
        id: String::new(),
        provider_key: "qwen".to_string(),
        display_name: "Qwen".to_string(),
        protocol_type: "openai".to_string(),
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
        auth_type: "api_key".to_string(),
        api_key_encrypted: "sk-qwen".to_string(),
        org_id: String::new(),
        extra_json: "{}".to_string(),
        enabled: true,
    };
    let provider_id = save_provider_config_to_pool(&pool, provider)
        .await
        .expect("save provider");

    let first = list_provider_models_from_pool(&pool, &provider_id, Some("chat"))
        .await
        .expect("first list");
    assert!(first.iter().any(|m| m == "qwen-max"));

    let cache_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM model_catalog_cache WHERE provider_id = ?",
    )
    .bind(&provider_id)
    .fetch_one(&pool)
    .await
    .expect("cache count");
    assert!(cache_count > 0);

    let second = list_provider_models_from_pool(&pool, &provider_id, Some("chat"))
        .await
        .expect("second list");
    assert_eq!(first, second);
}

#[tokio::test]
async fn capability_routing_policy_supports_vision() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let policy = CapabilityRoutingPolicy {
        capability: "vision".to_string(),
        primary_provider_id: "provider-vision".to_string(),
        primary_model: "qwen-vl-max".to_string(),
        fallback_chain_json: "[]".to_string(),
        timeout_ms: 30000,
        retry_count: 1,
        enabled: true,
    };
    upsert_capability_routing_policy_to_pool(&pool, policy)
        .await
        .expect("save vision policy");

    let loaded = get_capability_routing_policy_from_pool(&pool, "vision")
        .await
        .expect("load vision policy")
        .expect("vision policy exists");
    assert_eq!(loaded.capability, "vision");
    assert_eq!(loaded.primary_model, "qwen-vl-max");
}

#[tokio::test]
async fn route_attempt_stats_are_aggregated() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO route_attempt_logs (id, session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, success, error_message, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("log-1")
    .bind("s1")
    .bind("chat")
    .bind("openai")
    .bind("deepseek-chat")
    .bind(1_i64)
    .bind(0_i64)
    .bind("rate_limit")
    .bind(false)
    .bind("429")
    .bind(&now)
    .execute(&pool)
    .await
    .expect("insert log 1");

    sqlx::query(
        "INSERT INTO route_attempt_logs (id, session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, success, error_message, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("log-2")
    .bind("s1")
    .bind("chat")
    .bind("openai")
    .bind("deepseek-chat")
    .bind(2_i64)
    .bind(1_i64)
    .bind("ok")
    .bind(true)
    .bind("")
    .bind(&now)
    .execute(&pool)
    .await
    .expect("insert log 2");

    let stats = list_route_attempt_stats_from_pool(&pool, 24, Some("chat"))
        .await
        .expect("stats");
    assert!(stats
        .iter()
        .any(|s| !s.success && s.error_kind == "rate_limit" && s.count >= 1));
    assert!(stats
        .iter()
        .any(|s| s.success && s.error_kind == "ok" && s.count >= 1));
}

#[tokio::test]
async fn capability_route_templates_can_be_listed_for_chat() {
    let templates = list_capability_route_templates_for(Some("chat"));
    assert!(!templates.is_empty());
    assert!(templates
        .iter()
        .any(|t| t.template_id == "china-first-p0" && t.capability == "chat"));
}

#[tokio::test]
async fn apply_template_requires_enabled_primary_provider() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let err = apply_capability_route_template_from_pool(&pool, "chat", "china-first-p0")
        .await
        .expect_err("should fail when no enabled providers");
    assert!(err.contains("deepseek") || err.contains("qwen") || err.contains("moonshot"));
}

#[tokio::test]
async fn apply_template_builds_policy_and_fallback_chain() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let deepseek_id = save_provider_config_to_pool(
        &pool,
        ProviderConfig {
            id: String::new(),
            provider_key: "deepseek".to_string(),
            display_name: "DeepSeek".to_string(),
            protocol_type: "openai".to_string(),
            base_url: "https://api.deepseek.com/v1".to_string(),
            auth_type: "api_key".to_string(),
            api_key_encrypted: "sk-ds".to_string(),
            org_id: String::new(),
            extra_json: "{}".to_string(),
            enabled: true,
        },
    )
    .await
    .expect("save deepseek");
    let qwen_id = save_provider_config_to_pool(
        &pool,
        ProviderConfig {
            id: String::new(),
            provider_key: "qwen".to_string(),
            display_name: "Qwen".to_string(),
            protocol_type: "openai".to_string(),
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            auth_type: "api_key".to_string(),
            api_key_encrypted: "sk-qwen".to_string(),
            org_id: String::new(),
            extra_json: "{}".to_string(),
            enabled: true,
        },
    )
    .await
    .expect("save qwen");
    let _moonshot_disabled = save_provider_config_to_pool(
        &pool,
        ProviderConfig {
            id: String::new(),
            provider_key: "moonshot".to_string(),
            display_name: "Moonshot".to_string(),
            protocol_type: "openai".to_string(),
            base_url: "https://api.moonshot.ai/v1".to_string(),
            auth_type: "api_key".to_string(),
            api_key_encrypted: "sk-ms".to_string(),
            org_id: String::new(),
            extra_json: "{}".to_string(),
            enabled: false,
        },
    )
    .await
    .expect("save moonshot disabled");

    let policy = apply_capability_route_template_from_pool(&pool, "chat", "china-first-p0")
        .await
        .expect("apply template");
    assert_eq!(policy.capability, "chat");
    assert_eq!(policy.primary_provider_id, deepseek_id);
    assert_eq!(policy.primary_model, "deepseek-chat");
    assert!(policy.fallback_chain_json.contains(&qwen_id));
}
