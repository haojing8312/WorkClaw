mod helpers;

use runtime_lib::browser_bridge_callback::BrowserBridgeCallbackServer;
use runtime_lib::commands::feishu_browser_setup::FeishuBrowserSetupStore;
use runtime_lib::commands::feishu_gateway::get_app_setting;

#[tokio::test]
async fn browser_bridge_callback_binds_credentials_and_advances_session() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let store = FeishuBrowserSetupStore::default();
    let session = store.start_session("feishu".to_string()).await.unwrap();

    let server = BrowserBridgeCallbackServer::new(pool.clone(), store.clone());
    let base_url = server.start().await.expect("start callback server");

    let response = reqwest::Client::new()
        .post(format!("{}/browser-bridge/callback", base_url))
        .json(&serde_json::json!({
            "version": 1,
            "sessionId": session.session_id,
            "kind": "request",
            "payload": {
                "type": "credentials.report",
                "appId": "cli_callback_123",
                "appSecret": "sec_callback_456"
            }
        }))
        .send()
        .await
        .expect("post callback");

    assert!(response.status().is_success());
    let body: serde_json::Value = response.json().await.expect("decode callback response");
    assert_eq!(body["kind"], "response");
    assert_eq!(body["payload"]["type"], "action.pause");

    let updated = store
        .get_session(session.session_id)
        .await
        .expect("load updated session");
    assert_eq!(updated.step, "ENABLE_LONG_CONNECTION");
    assert_eq!(updated.app_id.as_deref(), Some("cli_callback_123"));
    assert!(updated.app_secret_present);
    assert_eq!(
        get_app_setting(&pool, "feishu_app_id").await.unwrap().as_deref(),
        Some("cli_callback_123")
    );
    assert_eq!(
        get_app_setting(&pool, "feishu_app_secret")
            .await
            .unwrap()
            .as_deref(),
        Some("sec_callback_456")
    );

    server.stop();
}
