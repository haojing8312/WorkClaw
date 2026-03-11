mod helpers;

use runtime_lib::commands::models::{
    delete_model_config_with_pool, resolve_default_model_id_with_pool,
    save_model_config_with_pool, set_default_model_with_pool, ModelConfig,
};

async fn insert_model(
    pool: &sqlx::SqlitePool,
    id: &str,
    name: &str,
    api_format: &str,
    is_default: bool,
) {
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(name)
    .bind(api_format)
    .bind(format!("https://{id}.example.com/v1"))
    .bind("gpt-test")
    .bind(is_default)
    .bind(format!("sk-{id}"))
    .execute(pool)
    .await
    .expect("insert model");
}

#[tokio::test]
async fn save_model_config_returns_generated_id() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let saved_id = save_model_config_with_pool(
        &pool,
        ModelConfig {
            id: String::new(),
            name: "New Model".to_string(),
            api_format: "openai".to_string(),
            base_url: "https://new.example.com/v1".to_string(),
            model_name: "gpt-4.1-mini".to_string(),
            is_default: false,
        },
        "sk-new".to_string(),
    )
    .await
    .expect("save model config");

    assert!(!saved_id.trim().is_empty());

    let rows: Vec<(String, String)> = sqlx::query_as("SELECT id, name FROM model_configs")
        .fetch_all(&pool)
        .await
        .expect("query saved models");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, saved_id);
    assert_eq!(rows[0].1, "New Model");
}

#[tokio::test]
async fn set_default_model_switches_only_non_search_models() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    insert_model(&pool, "model-1", "Primary", "openai", true).await;
    insert_model(&pool, "model-2", "Secondary", "openai", false).await;
    insert_model(&pool, "search-1", "Search", "search_brave", true).await;

    set_default_model_with_pool(&pool, "model-2")
        .await
        .expect("set default model");

    let models: Vec<(String, bool)> = sqlx::query_as(
        "SELECT id, CAST(is_default AS BOOLEAN) FROM model_configs WHERE api_format NOT LIKE 'search_%' ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .expect("query models");
    assert_eq!(
        models,
        vec![
            ("model-1".to_string(), false),
            ("model-2".to_string(), true),
        ]
    );

    let search_default: (bool,) = sqlx::query_as(
        "SELECT CAST(is_default AS BOOLEAN) FROM model_configs WHERE id = 'search-1'",
    )
    .fetch_one(&pool)
    .await
    .expect("query search default");
    assert!(search_default.0);
}

#[tokio::test]
async fn delete_default_model_promotes_first_remaining_model() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    insert_model(&pool, "model-1", "Primary", "openai", true).await;
    insert_model(&pool, "model-2", "Secondary", "openai", false).await;
    insert_model(&pool, "model-3", "Third", "openai", false).await;

    delete_model_config_with_pool(&pool, "model-1")
        .await
        .expect("delete default model");

    let models: Vec<(String, bool)> = sqlx::query_as(
        "SELECT id, CAST(is_default AS BOOLEAN) FROM model_configs WHERE api_format NOT LIKE 'search_%' ORDER BY rowid ASC",
    )
    .fetch_all(&pool)
    .await
    .expect("query remaining models");

    assert_eq!(
        models,
        vec![
            ("model-2".to_string(), true),
            ("model-3".to_string(), false),
        ]
    );
}

#[tokio::test]
async fn delete_non_default_model_keeps_existing_default() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    insert_model(&pool, "model-1", "Primary", "openai", true).await;
    insert_model(&pool, "model-2", "Secondary", "openai", false).await;

    delete_model_config_with_pool(&pool, "model-2")
        .await
        .expect("delete non-default model");

    let models: Vec<(String, bool)> = sqlx::query_as(
        "SELECT id, CAST(is_default AS BOOLEAN) FROM model_configs WHERE api_format NOT LIKE 'search_%' ORDER BY rowid ASC",
    )
    .fetch_all(&pool)
    .await
    .expect("query remaining models");

    assert_eq!(models, vec![("model-1".to_string(), true)]);
}

#[tokio::test]
async fn resolve_default_model_id_self_heals_when_default_flag_is_missing() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    insert_model(&pool, "model-1", "Primary", "openai", false).await;
    insert_model(&pool, "model-2", "Secondary", "openai", false).await;

    let resolved = resolve_default_model_id_with_pool(&pool)
        .await
        .expect("resolve default model");

    assert_eq!(resolved.as_deref(), Some("model-1"));

    let models: Vec<(String, bool)> = sqlx::query_as(
        "SELECT id, CAST(is_default AS BOOLEAN) FROM model_configs WHERE api_format NOT LIKE 'search_%' ORDER BY rowid ASC",
    )
    .fetch_all(&pool)
    .await
    .expect("query healed models");

    assert_eq!(
        models,
        vec![
            ("model-1".to_string(), true),
            ("model-2".to_string(), false),
        ]
    );
}

#[tokio::test]
async fn resolve_default_model_id_ignores_search_configs_when_healing() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    insert_model(&pool, "search-1", "Search", "search_brave", false).await;
    insert_model(&pool, "model-1", "Primary", "openai", false).await;

    let resolved = resolve_default_model_id_with_pool(&pool)
        .await
        .expect("resolve default model");

    assert_eq!(resolved.as_deref(), Some("model-1"));

    let search_default: (bool,) = sqlx::query_as(
        "SELECT CAST(is_default AS BOOLEAN) FROM model_configs WHERE id = 'search-1'",
    )
    .fetch_one(&pool)
    .await
    .expect("query search default");
    assert!(!search_default.0);
}
