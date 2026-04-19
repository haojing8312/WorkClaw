use super::{
    apply_default_feishu_account_id, get_app_setting,
    remember_feishu_runtime_state_for_outbound, FeishuCallbackResult,
    FeishuEmployeeConnectionInput, FeishuGatewayResult, ImEvent, OpenClawPluginFeishuRuntimeState,
};
use crate::approval_bus::ApprovalManager;
use crate::commands::chat::ApprovalManagerState;
use crate::commands::feishu_gateway::metadata_service::resolve_feishu_host_metadata_with_pool;
use crate::commands::feishu_gateway::types::FeishuInboundGateDecision;
use crate::commands::im_host::dispatch_im_inbound_to_workclaw_with_pool_and_app;
use crate::commands::skills::DbState;
use sqlx::SqlitePool;
use tauri::Manager;
use tauri::State;

pub(crate) fn resolve_fallback_default_feishu_account_id(
    has_default_credentials: bool,
    employee_account_ids: &[String],
) -> Option<String> {
    if has_default_credentials {
        return Some("default".to_string());
    }

    employee_account_ids
        .iter()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) async fn resolve_default_feishu_account_id_with_pool(
    pool: &SqlitePool,
) -> Result<Option<String>, String> {
    if let Ok(metadata) = resolve_feishu_host_metadata_with_pool(pool).await {
        if let Some(default_account_id) = metadata.default_account_id().map(str::to_string) {
            return Ok(Some(default_account_id));
        }
        let fallback = metadata.account_ids().map(str::to_string).next();
        if fallback.is_some() {
            return Ok(fallback);
        }
    }

    let app_id = get_app_setting(pool, "feishu_app_id")
        .await?
        .unwrap_or_default();
    let app_secret = get_app_setting(pool, "feishu_app_secret")
        .await?
        .unwrap_or_default();
    let employee_account_ids = list_enabled_employee_feishu_connections_with_pool(pool)
        .await?
        .into_iter()
        .map(|item| item.employee_id)
        .collect::<Vec<_>>();

    Ok(resolve_fallback_default_feishu_account_id(
        !app_id.trim().is_empty() && !app_secret.trim().is_empty(),
        &employee_account_ids,
    ))
}

pub async fn validate_feishu_auth_with_pool(
    pool: &SqlitePool,
    auth_token: Option<String>,
) -> Result<(), String> {
    let configured: Option<(String,)> =
        sqlx::query_as("SELECT value FROM app_settings WHERE key = 'feishu_ingress_token' LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    let expected = configured.map(|(v,)| v).unwrap_or_default();
    if expected.trim().is_empty() {
        return Ok(());
    }
    if auth_token.unwrap_or_default() == expected {
        Ok(())
    } else {
        Err("feishu auth token invalid".to_string())
    }
}

pub(crate) async fn list_enabled_employee_feishu_connections_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<FeishuEmployeeConnectionInput>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT employee_id, role_id, name, feishu_app_id, feishu_app_secret
         FROM agent_employees
         WHERE enabled = 1
           AND TRIM(feishu_app_id) <> ''
           AND TRIM(feishu_app_secret) <> ''
         ORDER BY is_default DESC, updated_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut result = Vec::with_capacity(rows.len());
    for (employee_id_raw, role_id, name, app_id, app_secret) in rows {
        let employee_id = if employee_id_raw.trim().is_empty() {
            role_id.trim().to_string()
        } else {
            employee_id_raw.trim().to_string()
        };
        if employee_id.is_empty() {
            continue;
        }
        result.push(FeishuEmployeeConnectionInput {
            employee_id,
            name: name.trim().to_string(),
            app_id: app_id.trim().to_string(),
            app_secret: app_secret.trim().to_string(),
        });
    }
    Ok(result)
}

pub async fn validate_feishu_signature_with_pool(
    pool: &SqlitePool,
    payload: &str,
    timestamp: Option<String>,
    nonce: Option<String>,
    signature: Option<String>,
) -> Result<(), String> {
    let encrypt_key = get_app_setting(pool, "feishu_encrypt_key")
        .await?
        .unwrap_or_default();
    if encrypt_key.trim().is_empty() {
        return Ok(());
    }

    let ts = timestamp.ok_or_else(|| "missing feishu timestamp".to_string())?;
    let nn = nonce.ok_or_else(|| "missing feishu nonce".to_string())?;
    let sig = signature.ok_or_else(|| "missing feishu signature".to_string())?;
    let expected = super::calculate_feishu_signature(&ts, &nn, &encrypt_key, payload);
    if expected == sig.to_ascii_lowercase() {
        Ok(())
    } else {
        Err("feishu signature invalid".to_string())
    }
}

pub(crate) async fn dispatch_feishu_inbound_to_workclaw_with_pool_and_app(
    pool: &SqlitePool,
    app: &tauri::AppHandle,
    event: &ImEvent,
    _approval_manager: Option<&ApprovalManager>,
) -> Result<FeishuCallbackResult, String> {
    if let Some(runtime_state) = app.try_state::<OpenClawPluginFeishuRuntimeState>() {
        remember_feishu_runtime_state_for_outbound(runtime_state.inner());
    }
    dispatch_im_inbound_to_workclaw_with_pool_and_app(pool, app, event).await
}

pub async fn handle_feishu_event_with_pool_and_app(
    payload: String,
    auth_token: Option<String>,
    signature: Option<String>,
    timestamp: Option<String>,
    nonce: Option<String>,
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    approvals: State<'_, ApprovalManagerState>,
) -> Result<FeishuGatewayResult, String> {
    validate_feishu_auth_with_pool(&db.0, auth_token).await?;
    validate_feishu_signature_with_pool(&db.0, &payload, timestamp, nonce, signature).await?;
    if let Some(runtime_state) = app.try_state::<OpenClawPluginFeishuRuntimeState>() {
        remember_feishu_runtime_state_for_outbound(runtime_state.inner());
    }
    match super::parse_feishu_payload(&payload)? {
        super::ParsedFeishuPayload::Challenge(challenge) => Ok(FeishuGatewayResult {
            accepted: true,
            deduped: false,
            challenge: Some(challenge),
        }),
        super::ParsedFeishuPayload::Event(mut event) => {
            let default_account_id = resolve_default_feishu_account_id_with_pool(&db.0).await?;
            apply_default_feishu_account_id(&mut event, default_account_id.as_deref());
            match super::evaluate_openclaw_feishu_gate_from_registry_with_pool(
                &db.0,
                app.state::<OpenClawPluginFeishuRuntimeState>().inner(),
                app.state::<crate::commands::channel_connectors::ChannelConnectorMonitorState>()
                    .inner(),
                app.state::<crate::commands::im_host::ImChannelHostRuntimeState>()
                    .inner(),
                &app,
                &event,
            )
            .await?
            {
                FeishuInboundGateDecision::Allow => {}
                FeishuInboundGateDecision::Reject { reason } => {
                    if reason == "pairing_pending" {
                        let _ = super::pairing_service::maybe_create_feishu_pairing_request_from_registry_with_pool(
                            &db.0,
                            app.state::<OpenClawPluginFeishuRuntimeState>().inner(),
                            app.state::<crate::commands::channel_connectors::ChannelConnectorMonitorState>()
                                .inner(),
                            app.state::<crate::commands::im_host::ImChannelHostRuntimeState>()
                                .inner(),
                            &app,
                            &event,
                        )
                        .await?;
                    }
                    return Ok(FeishuGatewayResult {
                        accepted: false,
                        deduped: false,
                        challenge: None,
                    });
                }
            }
            let r = dispatch_feishu_inbound_to_workclaw_with_pool_and_app(
                &db.0,
                &app,
                &event,
                Some(approvals.inner().0.as_ref()),
            )
            .await?;
            Ok(FeishuGatewayResult {
                accepted: r.accepted,
                deduped: r.deduped,
                challenge: None,
            })
        }
    }
}
