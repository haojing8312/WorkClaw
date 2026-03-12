use runtime_lib::commands::feishu_browser_setup::{FeishuBrowserSetupStore, SetupEvent};

#[tokio::test]
async fn start_session_returns_init_state() {
    let state = FeishuBrowserSetupStore::default();

    let session = state.start_session("feishu".to_string()).await.unwrap();

    assert_eq!(session.provider, "feishu");
    assert_eq!(session.step, "INIT");
    assert_eq!(session.app_id, None);
    assert!(!session.app_secret_present);
}

#[tokio::test]
async fn start_session_can_move_to_login_required() {
    let state = FeishuBrowserSetupStore::default();
    let session = state.start_session("feishu".to_string()).await.unwrap();

    let updated = state
        .apply_event(session.session_id.clone(), SetupEvent::LoginRequired)
        .await
        .unwrap();

    assert_eq!(updated.step, "LOGIN_REQUIRED");
}

#[tokio::test]
async fn credentials_report_transitions_to_bind_local() {
    let state = FeishuBrowserSetupStore::default();
    let session = state.start_session("feishu".to_string()).await.unwrap();

    let updated = state
        .apply_event(
            session.session_id.clone(),
            SetupEvent::CredentialsReported {
                app_id: "cli_x".into(),
                app_secret: "sec_x".into(),
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.step, "BIND_LOCAL");
    assert_eq!(updated.app_id.as_deref(), Some("cli_x"));
    assert!(updated.app_secret_present);
}
