use crate::commands::chat::ApprovalManagerState;
use crate::commands::employee_agents::EmployeeInboundDispatchSession;
#[cfg(test)]
use crate::commands::employee_agents::AgentEmployee;
use crate::commands::im_gateway::FeishuCallbackResult;
use crate::commands::openclaw_plugins::{
    OpenClawPluginFeishuRuntimeState,
};
#[cfg(test)]
use crate::commands::openclaw_plugins::OpenClawPluginChannelSnapshotResult;
use crate::commands::skills::DbState;
use crate::im::runtime_bridge::{
    build_im_role_dispatch_request_for_channel, build_im_role_event_payload_for_channel,
};
use crate::im::types::ImEvent;
use tauri::Emitter;
use tauri::State;

#[path = "feishu_gateway/types.rs"]
mod types;
#[path = "feishu_gateway/payload_parser.rs"]
mod payload_parser;
#[path = "feishu_gateway/repo.rs"]
mod repo;
#[path = "feishu_gateway/gate_service.rs"]
mod gate_service;
#[path = "feishu_gateway/pairing_service.rs"]
mod pairing_service;
#[path = "feishu_gateway/approval_service.rs"]
mod approval_service;
#[path = "feishu_gateway/outbound_service.rs"]
mod outbound_service;
#[path = "feishu_gateway/relay_service.rs"]
mod relay_service;
#[path = "feishu_gateway/ingress_service.rs"]
mod ingress_service;
#[path = "feishu_gateway/settings_service.rs"]
mod settings_service;
#[path = "feishu_gateway/planning_service.rs"]
mod planning_service;
#[path = "feishu_gateway/tauri_commands.rs"]
mod tauri_commands;
#[path = "feishu_gateway/test_support.rs"]
#[doc(hidden)]
pub mod test_support;

pub use payload_parser::parse_feishu_payload;
pub use repo::{
    calculate_feishu_signature, get_app_setting, resolve_feishu_app_credentials,
    resolve_feishu_sidecar_base_url, set_app_setting,
};
pub use types::{
    FeishuGatewayResult, FeishuGatewaySettings, FeishuPairingRequestRecord,
    ImRouteDecisionEvent, ParsedFeishuPayload,
};
use gate_service::{
    evaluate_openclaw_feishu_gate_with_pool, is_direct_feishu_chat,
    select_feishu_channel_account_snapshot,
};
#[cfg(test)]
use gate_service::evaluate_openclaw_feishu_gate;
pub use approval_service::{
    maybe_handle_feishu_approval_command_with_pool, notify_feishu_approval_requested_with_pool,
};
pub use ingress_service::{validate_feishu_auth_with_pool, validate_feishu_signature_with_pool};
pub use outbound_service::{
    clear_feishu_runtime_state_for_outbound, remember_feishu_runtime_state_for_outbound,
    send_feishu_text_message_with_pool, set_feishu_official_runtime_outbound_send_hook_for_tests,
};
pub use relay_service::{
    call_sidecar_json, send_feishu_via_sidecar, FeishuChatInfo, FeishuChatListResult,
    FeishuEmployeeConnectionInput, FeishuEmployeeConnectionStatuses, FeishuEmployeeWsStatus,
    FeishuEventRelayState, FeishuEventRelayStatus, FeishuWsEventRecord, FeishuWsStatus,
    FeishuWsStatusSummary,
};
use approval_service::parse_feishu_approval_command;
pub(crate) use approval_service::notify_feishu_approval_resolved_with_pool;
pub(crate) use outbound_service::{
    lookup_feishu_thread_for_session_with_pool, send_feishu_text_message_via_official_runtime_with_pool,
};
#[cfg(test)]
pub(crate) use outbound_service::{
    build_feishu_outbound_route_target, lookup_feishu_chat_id_for_sender_with_pool,
    lookup_latest_feishu_inbox_message_id_for_thread_with_pool,
    remember_feishu_chat_id_for_sender_with_pool,
};
pub(crate) use pairing_service::{
    list_feishu_pairing_allow_from_with_pool, list_feishu_pairing_requests_with_pool,
    upsert_feishu_pairing_request_with_pool,
};
pub(crate) use ingress_service::{
    dispatch_feishu_inbound_to_workclaw_with_pool_and_app,
    list_enabled_employee_feishu_connections_with_pool,
    resolve_default_feishu_account_id_with_pool,
};
#[cfg(test)]
pub(crate) use ingress_service::resolve_fallback_default_feishu_account_id;
pub(crate) use relay_service::{
    reconcile_feishu_employee_connections_with_pool, start_feishu_event_relay_with_pool_and_app,
};
#[cfg(test)]
pub(crate) use relay_service::{resolve_ws_role_id, sanitize_ws_inbound_text};
use pairing_service::resolve_feishu_pairing_request_with_pool;
#[cfg(test)]
use pairing_service::{
    generate_feishu_pairing_code, resolve_feishu_pairing_account_id,
};
use relay_service::{
    get_feishu_employee_connection_statuses_with_pool, get_feishu_event_relay_status_in_state,
    get_feishu_long_connection_status_with_pool, list_feishu_chats_with_pool,
    push_role_summary_to_feishu_with_pool,
    start_feishu_long_connection_with_pool, stop_feishu_event_relay_in_state,
    stop_feishu_long_connection_with_pool, sync_feishu_ws_events_core,
};
use settings_service::{
    get_feishu_gateway_settings_with_state, set_feishu_gateway_settings_with_state,
};
pub use planning_service::{plan_role_dispatch_requests_for_feishu, plan_role_events_for_feishu};

fn emit_employee_inbound_dispatch_sessions(
    app: &tauri::AppHandle,
    channel: &str,
    dispatches: &[EmployeeInboundDispatchSession],
) {
    for dispatch in dispatches {
        let _ = app.emit(
            "im-route-decision",
            ImRouteDecisionEvent {
                session_id: dispatch.session_id.clone(),
                thread_id: dispatch.thread_id.clone(),
                agent_id: dispatch.route_agent_id.clone(),
                session_key: dispatch.route_session_key.clone(),
                matched_by: dispatch.matched_by.clone(),
            },
        );

        let _ = app.emit(
            "im-role-event",
            build_im_role_event_payload_for_channel(
                &dispatch.session_id,
                &dispatch.thread_id,
                &dispatch.role_id,
                &dispatch.employee_name,
                channel,
                "running",
                "飞书消息已同步到桌面会话，正在执行",
                None,
            ),
        );

        let _ = app.emit("im-role-dispatch-request", {
            let mut req = build_im_role_dispatch_request_for_channel(
                &dispatch.session_id,
                &dispatch.thread_id,
                &dispatch.role_id,
                &dispatch.employee_name,
                channel,
                &dispatch.prompt,
                "general-purpose",
            );
            req.message_id = dispatch.message_id.clone();
            req
        });
    }
}

#[tauri::command]
pub async fn list_feishu_pairing_requests(
    status: Option<String>,
    db: State<'_, DbState>,
) -> Result<Vec<FeishuPairingRequestRecord>, String> {
    tauri_commands::list_feishu_pairing_requests(status, db).await
}

#[tauri::command]
pub async fn approve_feishu_pairing_request(
    request_id: String,
    resolved_by_user: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuPairingRequestRecord, String> {
    tauri_commands::approve_feishu_pairing_request(request_id, resolved_by_user, db).await
}

#[tauri::command]
pub async fn deny_feishu_pairing_request(
    request_id: String,
    resolved_by_user: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuPairingRequestRecord, String> {
    tauri_commands::deny_feishu_pairing_request(request_id, resolved_by_user, db).await
}

#[tauri::command]
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
    tauri_commands::handle_feishu_event(
        payload,
        auth_token,
        signature,
        timestamp,
        nonce,
        app,
        db,
        approvals,
    )
    .await
}

#[tauri::command]
pub async fn send_feishu_text_message(
    app: tauri::AppHandle,
    chat_id: String,
    text: String,
    app_id: Option<String>,
    app_secret: Option<String>,
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
    runtime_state: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<String, String> {
    tauri_commands::send_feishu_text_message(
        app,
        chat_id,
        text,
        app_id,
        app_secret,
        sidecar_base_url,
        db,
        runtime_state,
    )
    .await
}

#[tauri::command]
pub async fn list_feishu_chats(
    page_size: Option<usize>,
    page_token: Option<String>,
    user_id_type: Option<String>,
    app_id: Option<String>,
    app_secret: Option<String>,
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuChatListResult, String> {
    tauri_commands::list_feishu_chats(
        page_size,
        page_token,
        user_id_type,
        app_id,
        app_secret,
        sidecar_base_url,
        db,
    )
    .await
}

#[tauri::command]
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
    tauri_commands::push_role_summary_to_feishu(
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
        db,
    )
    .await
}

#[tauri::command]
pub async fn set_feishu_gateway_settings(
    settings: FeishuGatewaySettings,
    db: State<'_, DbState>,
) -> Result<(), String> {
    tauri_commands::set_feishu_gateway_settings(settings, db).await
}

#[tauri::command]
pub async fn get_feishu_gateway_settings(
    db: State<'_, DbState>,
) -> Result<FeishuGatewaySettings, String> {
    tauri_commands::get_feishu_gateway_settings(db).await
}

#[tauri::command]
pub async fn start_feishu_long_connection(
    sidecar_base_url: Option<String>,
    app_id: Option<String>,
    app_secret: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuWsStatus, String> {
    tauri_commands::start_feishu_long_connection(sidecar_base_url, app_id, app_secret, db).await
}

#[tauri::command]
pub async fn stop_feishu_long_connection(
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuWsStatus, String> {
    tauri_commands::stop_feishu_long_connection(sidecar_base_url, db).await
}

#[tauri::command]
pub async fn get_feishu_long_connection_status(
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
) -> Result<FeishuWsStatus, String> {
    tauri_commands::get_feishu_long_connection_status(sidecar_base_url, db).await
}

#[tauri::command]
pub async fn get_feishu_employee_connection_statuses(
    sidecar_base_url: Option<String>,
    db: State<'_, DbState>,
    relay: State<'_, FeishuEventRelayState>,
) -> Result<FeishuEmployeeConnectionStatuses, String> {
    tauri_commands::get_feishu_employee_connection_statuses(sidecar_base_url, db, relay).await
}

#[tauri::command]
pub async fn sync_feishu_ws_events(
    sidecar_base_url: Option<String>,
    limit: Option<usize>,
    app: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<usize, String> {
    tauri_commands::sync_feishu_ws_events(sidecar_base_url, limit, app, db).await
}

#[tauri::command]
pub async fn start_feishu_event_relay(
    sidecar_base_url: Option<String>,
    interval_ms: Option<u64>,
    limit: Option<usize>,
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    relay: State<'_, FeishuEventRelayState>,
) -> Result<FeishuEventRelayStatus, String> {
    tauri_commands::start_feishu_event_relay(sidecar_base_url, interval_ms, limit, app, db, relay)
        .await
}

#[tauri::command]
pub async fn stop_feishu_event_relay(
    relay: State<'_, FeishuEventRelayState>,
) -> Result<FeishuEventRelayStatus, String> {
    tauri_commands::stop_feishu_event_relay(relay).await
}

#[tauri::command]
pub async fn get_feishu_event_relay_status(
    relay: State<'_, FeishuEventRelayState>,
) -> Result<FeishuEventRelayStatus, String> {
    tauri_commands::get_feishu_event_relay_status(relay).await
}

fn apply_default_feishu_account_id(event: &mut ImEvent, default_account_id: Option<&str>) {
    let already_has_account = event
        .account_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    if already_has_account {
        return;
    }

    if let Some(default_account_id) = default_account_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        event.account_id = Some(default_account_id.to_string());
    }
}

#[cfg(test)]
#[path = "feishu_gateway/tests.rs"]
mod tests;
