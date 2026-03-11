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
    assert_eq!(prefs.default_language, "zh-CN");
    assert!(prefs.immersive_translation_enabled);
    assert_eq!(prefs.immersive_translation_display, "translated_only");
    assert_eq!(prefs.immersive_translation_trigger, "auto");
    assert_eq!(prefs.translation_engine, "model_then_free");
    assert_eq!(prefs.translation_model_id, "");
    assert_eq!(prefs.operation_permission_mode, "standard");
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
            default_work_dir: Some(target.clone()),
            default_language: Some("en-US".to_string()),
            immersive_translation_enabled: Some(false),
            immersive_translation_display: Some("bilingual_inline".to_string()),
            immersive_translation_trigger: Some("manual".to_string()),
            translation_engine: Some("model_only".to_string()),
            translation_model_id: Some("model-1".to_string()),
            auto_update_enabled: None,
            update_channel: None,
            dismissed_update_version: None,
            last_update_check_at: None,
            launch_at_login: Some(false),
            launch_minimized: Some(false),
            close_to_tray: Some(true),
            operation_permission_mode: Some("full_access".to_string()),
        },
    )
    .await
    .expect("set runtime preferences");
    assert_eq!(saved.default_work_dir, target);
    assert_eq!(saved.default_language, "en-US");
    assert!(!saved.immersive_translation_enabled);
    assert_eq!(saved.immersive_translation_display, "bilingual_inline");
    assert_eq!(saved.immersive_translation_trigger, "manual");
    assert_eq!(saved.translation_engine, "model_only");
    assert_eq!(saved.translation_model_id, "model-1");
    assert_eq!(saved.operation_permission_mode, "full_access");

    let resolved = resolve_default_work_dir_with_pool(&pool)
        .await
        .expect("resolve default work dir");
    assert_eq!(resolved, target);
    assert!(std::path::Path::new(&resolved).exists());
}

#[tokio::test]
async fn runtime_preferences_partial_update_keeps_existing_translation_settings() {
    let (pool, tmp) = helpers::setup_test_db().await;
    let dir_a = tmp.path().join("a").to_string_lossy().to_string();
    let dir_b = tmp.path().join("b").to_string_lossy().to_string();

    set_runtime_preferences_with_pool(
        &pool,
        RuntimePreferencesInput {
            default_work_dir: Some(dir_a),
            default_language: Some("en-US".to_string()),
            immersive_translation_enabled: Some(false),
            immersive_translation_display: Some("bilingual_inline".to_string()),
            immersive_translation_trigger: Some("manual".to_string()),
            translation_engine: Some("model_only".to_string()),
            translation_model_id: Some("model-a".to_string()),
            auto_update_enabled: None,
            update_channel: None,
            dismissed_update_version: None,
            last_update_check_at: None,
            launch_at_login: Some(true),
            launch_minimized: Some(false),
            close_to_tray: Some(false),
            operation_permission_mode: Some("full_access".to_string()),
        },
    )
    .await
    .expect("seed runtime preferences");

    let updated = set_runtime_preferences_with_pool(
        &pool,
        RuntimePreferencesInput {
            default_work_dir: Some(dir_b.clone()),
            default_language: None,
            immersive_translation_enabled: None,
            immersive_translation_display: None,
            immersive_translation_trigger: None,
            translation_engine: None,
            translation_model_id: None,
            auto_update_enabled: None,
            update_channel: None,
            dismissed_update_version: None,
            last_update_check_at: None,
            launch_at_login: None,
            launch_minimized: None,
            close_to_tray: None,
            operation_permission_mode: None,
        },
    )
    .await
    .expect("partial update runtime preferences");

    assert_eq!(updated.default_work_dir, dir_b);
    assert_eq!(updated.default_language, "en-US");
    assert!(!updated.immersive_translation_enabled);
    assert_eq!(updated.immersive_translation_display, "bilingual_inline");
    assert_eq!(updated.immersive_translation_trigger, "manual");
    assert_eq!(updated.translation_engine, "model_only");
    assert_eq!(updated.translation_model_id, "model-a");
    assert_eq!(updated.operation_permission_mode, "full_access");
}

#[tokio::test]
async fn runtime_preferences_invalid_permission_mode_falls_back_to_standard() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let saved = set_runtime_preferences_with_pool(
        &pool,
        RuntimePreferencesInput {
            default_work_dir: None,
            default_language: None,
            immersive_translation_enabled: None,
            immersive_translation_display: None,
            immersive_translation_trigger: None,
            translation_engine: None,
            translation_model_id: None,
            auto_update_enabled: None,
            update_channel: None,
            dismissed_update_version: None,
            last_update_check_at: None,
            launch_at_login: None,
            launch_minimized: None,
            close_to_tray: None,
            operation_permission_mode: Some("weird".to_string()),
        },
    )
    .await
    .expect("save runtime preferences");

    assert_eq!(saved.operation_permission_mode, "standard");
}
