mod helpers;

use runtime_lib::commands::feishu_browser_setup::FeishuBrowserSetupStore;
use runtime_lib::commands::feishu_gateway::get_app_setting;

#[tokio::test]
async fn credentials_report_runs_local_binding_and_marks_secret_present() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let store = FeishuBrowserSetupStore::default();
    let session = store.start_session("feishu".to_string()).await.unwrap();

    let updated = store
        .report_credentials_and_bind(
            &pool,
            session.session_id,
            "cli_test".to_string(),
            "sec_test".to_string(),
        )
        .await
        .unwrap();

    assert_eq!(updated.step, "ENABLE_LONG_CONNECTION");
    assert_eq!(updated.app_id.as_deref(), Some("cli_test"));
    assert!(updated.app_secret_present);
    assert_eq!(
        get_app_setting(&pool, "feishu_app_id").await.unwrap().as_deref(),
        Some("cli_test")
    );
    assert_eq!(
        get_app_setting(&pool, "feishu_app_secret")
            .await
            .unwrap()
            .as_deref(),
        Some("sec_test")
    );
}
