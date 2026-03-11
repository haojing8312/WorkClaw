use std::collections::HashMap;
use std::sync::Arc;

use crate::commands::feishu_gateway::set_app_setting;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeishuBrowserSetupSession {
    pub session_id: String,
    pub provider: String,
    pub step: String,
    pub app_id: Option<String>,
    pub app_secret_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SetupEvent {
    LoginRequired,
    CredentialsReported { app_id: String, app_secret: String },
    BindSucceeded,
    BindFailed { reason: String },
}

#[derive(Clone, Default)]
pub struct FeishuBrowserSetupStore {
    sessions: Arc<Mutex<HashMap<String, FeishuBrowserSetupSession>>>,
}

impl FeishuBrowserSetupStore {
    pub async fn start_session(
        &self,
        provider: String,
    ) -> Result<FeishuBrowserSetupSession, String> {
        let session = FeishuBrowserSetupSession {
            session_id: Uuid::new_v4().to_string(),
            provider,
            step: "INIT".to_string(),
            app_id: None,
            app_secret_present: false,
        };

        self.sessions
            .lock()
            .await
            .insert(session.session_id.clone(), session.clone());

        Ok(session)
    }

    pub async fn get_session(
        &self,
        session_id: String,
    ) -> Result<FeishuBrowserSetupSession, String> {
        self.sessions
            .lock()
            .await
            .get(&session_id)
            .cloned()
            .ok_or_else(|| format!("feishu browser setup session not found: {}", session_id))
    }

    pub async fn apply_event(
        &self,
        session_id: String,
        event: SetupEvent,
    ) -> Result<FeishuBrowserSetupSession, String> {
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| format!("feishu browser setup session not found: {}", session_id))?;

        match event {
            SetupEvent::LoginRequired => {
                session.step = "LOGIN_REQUIRED".to_string();
            }
            SetupEvent::CredentialsReported { app_id, app_secret } => {
                session.step = "BIND_LOCAL".to_string();
                session.app_id = Some(app_id);
                session.app_secret_present = !app_secret.trim().is_empty();
            }
            SetupEvent::BindSucceeded => {
                session.step = "ENABLE_LONG_CONNECTION".to_string();
            }
            SetupEvent::BindFailed { .. } => {
                session.step = "FAILED".to_string();
            }
        }

        Ok(session.clone())
    }

    pub async fn report_credentials_and_bind(
        &self,
        pool: &SqlitePool,
        session_id: String,
        app_id: String,
        app_secret: String,
    ) -> Result<FeishuBrowserSetupSession, String> {
        set_app_setting(pool, "feishu_app_id", app_id.as_str()).await?;
        set_app_setting(pool, "feishu_app_secret", app_secret.as_str()).await?;

        let updated = self
            .apply_event(
                session_id.clone(),
                SetupEvent::CredentialsReported {
                    app_id,
                    app_secret,
                },
            )
            .await?;

        self.apply_event(session_id, SetupEvent::BindSucceeded).await?;
        self.get_session(updated.session_id).await
    }
}

#[derive(Clone, Default)]
pub struct FeishuBrowserSetupState(pub FeishuBrowserSetupStore);

#[tauri::command]
pub async fn start_feishu_browser_setup(
    provider: String,
    state: State<'_, FeishuBrowserSetupState>,
) -> Result<FeishuBrowserSetupSession, String> {
    state.0.start_session(provider).await
}

#[tauri::command]
pub async fn get_feishu_browser_setup_session(
    session_id: String,
    state: State<'_, FeishuBrowserSetupState>,
) -> Result<FeishuBrowserSetupSession, String> {
    state.0.get_session(session_id).await
}

#[tauri::command]
pub async fn apply_feishu_browser_setup_event(
    session_id: String,
    event: SetupEvent,
    state: State<'_, FeishuBrowserSetupState>,
) -> Result<FeishuBrowserSetupSession, String> {
    state.0.apply_event(session_id, event).await
}
