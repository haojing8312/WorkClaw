use super::{
    apply_default_feishu_account_id, emit_employee_inbound_dispatch_sessions,
    get_app_setting,
    maybe_handle_feishu_approval_command_with_pool, parse_feishu_approval_command,
    remember_feishu_runtime_state_for_outbound,
    FeishuCallbackResult, FeishuEmployeeConnectionInput, FeishuGatewayResult,
    ImEvent, OpenClawPluginFeishuRuntimeState,
};
use crate::approval_bus::ApprovalManager;
use crate::commands::approvals::load_approval_record_with_pool;
use crate::commands::chat::ApprovalManagerState;
use crate::commands::employee_agents::bridge_inbound_event_to_employee_sessions_with_pool;
use crate::commands::feishu_gateway::pairing_service::maybe_create_feishu_pairing_request_with_pool;
use crate::commands::feishu_gateway::types::FeishuInboundGateDecision;
use crate::commands::im_gateway::process_im_event;
use crate::commands::openclaw_gateway::resolve_openclaw_route_with_pool;
use crate::commands::openclaw_plugins::get_openclaw_plugin_feishu_channel_snapshot_with_pool;
use crate::commands::skills::DbState;
use sqlx::SqlitePool;
use tauri::Emitter;
use tauri::Manager;
use tauri::State;

fn normalize_optional_non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
}

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
    if let Ok(snapshot) =
        get_openclaw_plugin_feishu_channel_snapshot_with_pool(pool, "openclaw-lark").await
    {
        if let Some(default_account_id) =
            normalize_optional_non_empty(snapshot.snapshot.default_account_id)
        {
            return Ok(Some(default_account_id));
        }
        let fallback = snapshot
            .snapshot
            .account_ids
            .into_iter()
            .map(|value: String| value.trim().to_string())
            .find(|value: &String| !value.is_empty());
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
    approval_manager: Option<&ApprovalManager>,
) -> Result<FeishuCallbackResult, String> {
    if let Some(runtime_state) = app.try_state::<OpenClawPluginFeishuRuntimeState>() {
        remember_feishu_runtime_state_for_outbound(runtime_state.inner());
    }
    let result = process_im_event(pool, event.clone()).await?;
    if result.deduped {
        return Ok(result);
    }

    if let Some(approval_manager) = approval_manager {
        let approval_command = parse_feishu_approval_command(event.text.as_deref());
        if let Some(command) = approval_command {
            if maybe_handle_feishu_approval_command_with_pool(pool, approval_manager, event, None)
                .await?
                .is_some()
            {
                if let Some(record) =
                    load_approval_record_with_pool(pool, &command.approval_id).await?
                {
                    let _ = app.emit("approval-resolved", &record);
                }
                return Ok(result);
            }
        }
    }

    let route_decision = resolve_openclaw_route_with_pool(pool, event).await.ok();
    let dispatches =
        bridge_inbound_event_to_employee_sessions_with_pool(pool, event, route_decision.as_ref())
            .await?;
    emit_employee_inbound_dispatch_sessions(app, "feishu", &dispatches);

    if dispatches.is_empty() {
        let planned = super::plan_role_events_for_feishu(pool, event).await?;
        for evt in planned {
            let _ = app.emit("im-role-event", evt);
        }
        let dispatches = super::plan_role_dispatch_requests_for_feishu(pool, event).await?;
        for req in dispatches {
            let _ = app.emit("im-role-dispatch-request", req);
        }
    }

    Ok(result)
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
            match super::evaluate_openclaw_feishu_gate_with_pool(&db.0, &event).await? {
                FeishuInboundGateDecision::Allow => {}
                FeishuInboundGateDecision::Reject { reason } => {
                    if reason == "pairing_pending" {
                        let _ =
                            maybe_create_feishu_pairing_request_with_pool(&db.0, &event).await?;
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
