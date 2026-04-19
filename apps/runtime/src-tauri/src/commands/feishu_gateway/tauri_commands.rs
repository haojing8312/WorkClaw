use super::ingress_service::handle_feishu_event_with_pool_and_app;
use super::{
    current_feishu_runtime_status, get_feishu_employee_connection_statuses_with_pool,
    get_feishu_event_relay_status_in_state, get_feishu_gateway_settings_with_state,
    get_feishu_long_connection_status_with_pool, list_feishu_chats_with_pool,
    list_feishu_pairing_requests_with_pool, push_role_summary_to_feishu_with_pool,
    resolve_feishu_pairing_request_with_pool, set_feishu_gateway_settings_with_state,
    start_feishu_event_relay_with_pool_and_app, start_feishu_long_connection_with_pool,
    start_openclaw_plugin_feishu_runtime_with_pool, stop_feishu_event_relay_in_state,
    stop_feishu_long_connection_with_pool, stop_openclaw_plugin_feishu_runtime_in_state,
    sync_feishu_ws_events_core, ApprovalManagerState, DbState, FeishuEmployeeConnectionStatuses,
    FeishuEventRelayState, FeishuEventRelayStatus, FeishuGatewayResult, FeishuGatewaySettings,
    FeishuPairingRequestRecord, FeishuWsStatus, OpenClawPluginFeishuRuntimeState,
};
use crate::commands::openclaw_plugins::OpenClawPluginFeishuRuntimeStatus;
use tauri::{AppHandle, State};

pub(crate) fn should_restart_official_feishu_runtime_after_pairing_approval(
    runtime_status: &OpenClawPluginFeishuRuntimeStatus,
    account_id: &str,
) -> bool {
    runtime_status.running
        && runtime_status
            .account_id
            .eq_ignore_ascii_case(account_id.trim())
}

async fn maybe_restart_official_feishu_runtime_after_pairing_approval(
    app: &AppHandle,
    db: &DbState,
    runtime: &OpenClawPluginFeishuRuntimeState,
    account_id: &str,
) -> Result<(), String> {
    let runtime_status = current_feishu_runtime_status(runtime);
    if !should_restart_official_feishu_runtime_after_pairing_approval(&runtime_status, account_id) {
        return Ok(());
    }

    let plugin_id = if runtime_status.plugin_id.trim().is_empty() {
        "openclaw-lark".to_string()
    } else {
        runtime_status.plugin_id.trim().to_string()
    };
    let restart_account_id = if account_id.trim().is_empty() {
        runtime_status.account_id
    } else {
        account_id.trim().to_string()
    };

    stop_openclaw_plugin_feishu_runtime_in_state(runtime)?;
    start_openclaw_plugin_feishu_runtime_with_pool(
        &db.0,
        runtime,
        &plugin_id,
        Some(&restart_account_id),
        Some(app.clone()),
    )
    .await
    .map(|_| ())
}

pub async fn list_feishu_pairing_requests(
    status: Option<String>,
    db: State<'_, DbState>,
) -> Result<Vec<FeishuPairingRequestRecord>, String> {
    list_feishu_pairing_requests_with_pool(&db.0, status).await
}

pub async fn approve_feishu_pairing_request(
    request_id: String,
    resolved_by_user: Option<String>,
    app: AppHandle,
    db: State<'_, DbState>,
    runtime: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<FeishuPairingRequestRecord, String> {
    let record =
        resolve_feishu_pairing_request_with_pool(&db.0, &request_id, "approved", resolved_by_user)
            .await?;
    maybe_restart_official_feishu_runtime_after_pairing_approval(
        &app,
        &db,
        runtime.inner(),
        &record.account_id,
    )
    .await?;
    Ok(record)
}

pub async fn deny_feishu_pairing_request(
    request_id: String,
    resolved_by_user: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuPairingRequestRecord, String> {
    resolve_feishu_pairing_request_with_pool(&db.0, &request_id, "denied", resolved_by_user).await
}

pub async fn handle_feishu_event(
    payload: String,
    auth_token: Option<String>,
    signature: Option<String>,
    timestamp: Option<String>,
    nonce: Option<String>,
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    approvals: State<'_, ApprovalManagerState>,
) -> Result<FeishuGatewayResult, String> {
    handle_feishu_event_with_pool_and_app(
        payload, auth_token, signature, timestamp, nonce, app, db, approvals,
    )
    .await
}

pub async fn send_feishu_text_message(
    app: tauri::AppHandle,
    chat_id: String,
    text: String,
    _app_id: Option<String>,
    _app_secret: Option<String>,
    _sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
    runtime_state: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<String, String> {
    super::relay_service::send_feishu_text_message(app, chat_id, text, db, runtime_state.inner())
        .await
}

pub async fn list_feishu_chats(
    page_size: Option<usize>,
    page_token: Option<String>,
    user_id_type: Option<String>,
    app_id: Option<String>,
    app_secret: Option<String>,
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<super::FeishuChatListResult, String> {
    list_feishu_chats_with_pool(
        &db.0,
        page_size,
        page_token,
        user_id_type,
        app_id,
        app_secret,
        sidecar_base_url,
    )
    .await
}

pub async fn push_role_summary_to_feishu(
    chat_id: String,
    role_id: String,
    role_name: String,
    conclusion: String,
    evidence: String,
    uncertainty: String,
    next_step: String,
    app_id: Option<String>,
    app_secret: Option<String>,
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<String, String> {
    push_role_summary_to_feishu_with_pool(
        &db.0,
        chat_id,
        role_id,
        role_name,
        conclusion,
        evidence,
        uncertainty,
        next_step,
        app_id,
        app_secret,
        sidecar_base_url,
    )
    .await
}

pub async fn set_feishu_gateway_settings(
    settings: FeishuGatewaySettings,
    db: State<'_, DbState>,
) -> Result<(), String> {
    set_feishu_gateway_settings_with_state(settings, db).await
}

pub async fn get_feishu_gateway_settings(
    db: State<'_, DbState>,
) -> Result<FeishuGatewaySettings, String> {
    get_feishu_gateway_settings_with_state(db).await
}

pub async fn start_feishu_long_connection(
    sidecar_base_url: Option<String>,
    app_id: Option<String>,
    app_secret: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuWsStatus, String> {
    start_feishu_long_connection_with_pool(&db.0, sidecar_base_url, app_id, app_secret).await
}

pub async fn stop_feishu_long_connection(
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuWsStatus, String> {
    stop_feishu_long_connection_with_pool(&db.0, sidecar_base_url).await
}

pub async fn get_feishu_long_connection_status(
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuWsStatus, String> {
    get_feishu_long_connection_status_with_pool(&db.0, sidecar_base_url).await
}

pub async fn get_feishu_employee_connection_statuses(
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
    relay: State<'_, FeishuEventRelayState>,
) -> Result<FeishuEmployeeConnectionStatuses, String> {
    get_feishu_employee_connection_statuses_with_pool(&db.0, sidecar_base_url, relay.inner()).await
}

pub async fn sync_feishu_ws_events(
    sidecar_base_url: Option<String>,
    limit: Option<usize>,
    app: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<usize, String> {
    sync_feishu_ws_events_core(&db.0, sidecar_base_url, limit, Some(&app)).await
}

pub async fn start_feishu_event_relay(
    sidecar_base_url: Option<String>,
    interval_ms: Option<u64>,
    limit: Option<usize>,
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    relay: State<'_, FeishuEventRelayState>,
) -> Result<FeishuEventRelayStatus, String> {
    start_feishu_event_relay_with_pool_and_app(
        &db.0,
        relay.inner().clone(),
        Some(app),
        sidecar_base_url,
        interval_ms,
        limit,
    )
    .await
}

pub async fn stop_feishu_event_relay(
    relay: State<'_, FeishuEventRelayState>,
) -> Result<FeishuEventRelayStatus, String> {
    Ok(stop_feishu_event_relay_in_state(relay.inner().clone()))
}

pub async fn get_feishu_event_relay_status(
    relay: State<'_, FeishuEventRelayState>,
) -> Result<FeishuEventRelayStatus, String> {
    Ok(get_feishu_event_relay_status_in_state(relay.inner()))
}
