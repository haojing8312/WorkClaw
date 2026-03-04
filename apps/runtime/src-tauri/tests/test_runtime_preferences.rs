mod helpers;

use runtime_lib::commands::runtime_preferences::{
    get_runtime_preferences_with_pool, resolve_default_work_dir_with_pool,
    set_runtime_preferences_with_pool, RuntimePreferencesInput,
};

#[tokio::test]
async fn runtime_preferences_returns_default_when_not_configured() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let prefs = get_runtime_preferences_with_pool(&pool)
        .await
        .expect("get runtime preferences");
    assert!(!prefs.default_work_dir.trim().is_empty());
    assert!(prefs.default_work_dir.contains("WorkClaw"));
}

#[tokio::test]
async fn runtime_preferences_can_be_saved_and_resolved_with_auto_create() {
    let (pool, tmp) = helpers::setup_test_db().await;
    let target = tmp
        .path()
        .join("workspace_target")
        .to_string_lossy()
        .to_string();

    let saved = set_runtime_preferences_with_pool(
        &pool,
        RuntimePreferencesInput {
            default_work_dir: target.clone(),
        },
    )
    .await
    .expect("set runtime preferences");
    assert_eq!(saved.default_work_dir, target);

    let resolved = resolve_default_work_dir_with_pool(&pool)
        .await
        .expect("resolve default work dir");
    assert_eq!(resolved, target);
    assert!(std::path::Path::new(&resolved).exists());
}
