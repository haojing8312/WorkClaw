use super::{get_app_setting, set_app_setting, FeishuGatewaySettings};
use crate::commands::skills::DbState;
use tauri::State;

pub async fn set_feishu_gateway_settings_with_state(
    settings: FeishuGatewaySettings,
    db: State<'_, DbState>,
) -> Result<(), String> {
    set_app_setting(&db.0, "feishu_app_id", settings.app_id.as_str()).await?;
    set_app_setting(&db.0, "feishu_app_secret", settings.app_secret.as_str()).await?;
    set_app_setting(
        &db.0,
        "feishu_ingress_token",
        settings.ingress_token.as_str(),
    )
    .await?;
    set_app_setting(&db.0, "feishu_encrypt_key", settings.encrypt_key.as_str()).await?;
    set_app_setting(
        &db.0,
        "feishu_sidecar_base_url",
        settings.sidecar_base_url.as_str(),
    )
    .await?;
    Ok(())
}

pub async fn get_feishu_gateway_settings_with_state(
    db: State<'_, DbState>,
) -> Result<FeishuGatewaySettings, String> {
    Ok(FeishuGatewaySettings {
        app_id: get_app_setting(&db.0, "feishu_app_id")
            .await?
            .unwrap_or_default(),
        app_secret: get_app_setting(&db.0, "feishu_app_secret")
            .await?
            .unwrap_or_default(),
        ingress_token: get_app_setting(&db.0, "feishu_ingress_token")
            .await?
            .unwrap_or_default(),
        encrypt_key: get_app_setting(&db.0, "feishu_encrypt_key")
            .await?
            .unwrap_or_default(),
        sidecar_base_url: get_app_setting(&db.0, "feishu_sidecar_base_url")
            .await?
            .unwrap_or_else(|| "http://localhost:8765".to_string()),
    })
}
