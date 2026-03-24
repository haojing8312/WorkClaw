use crate::commands::skills::DbState;
use tauri::{AppHandle, State};

use super::{
    current_feishu_runtime_status, current_openclaw_lark_installer_session_status,
    delete_openclaw_plugin_install_with_pool, get_feishu_plugin_environment_status_internal,
    get_feishu_setup_progress_with_pool, get_openclaw_plugin_feishu_advanced_settings_with_pool,
    get_openclaw_plugin_feishu_channel_snapshot_with_pool_and_app,
    inspect_openclaw_plugin_with_pool_and_app,
    list_openclaw_plugin_channel_hosts_with_pool_and_app, list_openclaw_plugin_installs_with_pool,
    normalize_required,
    probe_openclaw_plugin_feishu_credentials_with_app_secret, resolve_controlled_openclaw_state_root,
    resolve_openclaw_shim_root, set_openclaw_plugin_feishu_advanced_settings_with_pool,
    install_service::install_openclaw_plugin_from_npm_with_pool_and_app,
    start_openclaw_lark_installer_session_with_pool, start_openclaw_plugin_feishu_runtime_with_pool,
    stop_openclaw_lark_installer_session_in_state, stop_openclaw_plugin_feishu_runtime_in_state,
    sync_feishu_gateway_credentials_from_openclaw_state_with_pool,
    sync_feishu_gateway_credentials_from_shim_with_pool, upsert_openclaw_plugin_install_with_pool,
    FeishuPluginEnvironmentStatus, FeishuSetupProgress, OpenClawLarkInstallerMode,
    OpenClawLarkInstallerSessionState, OpenClawLarkInstallerSessionStatus,
    OpenClawPluginChannelHost, OpenClawPluginChannelSnapshotResult,
    OpenClawPluginFeishuAdvancedSettings, OpenClawPluginFeishuCredentialProbeResult,
    OpenClawPluginFeishuRuntimeState, OpenClawPluginFeishuRuntimeStatus,
    OpenClawPluginInspectionResult, OpenClawPluginInstallInput, OpenClawPluginInstallRecord,
};

pub(crate) async fn start_openclaw_plugin_feishu_runtime_command(
    plugin_id: String,
    account_id: Option<String>,
    app: AppHandle,
    db: State<'_, DbState>,
    runtime: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<OpenClawPluginFeishuRuntimeStatus, String> {
    start_openclaw_plugin_feishu_runtime_with_pool(
        &db.0,
        runtime.inner(),
        &plugin_id,
        account_id.as_deref(),
        Some(app),
    )
    .await
}

pub(crate) async fn stop_openclaw_plugin_feishu_runtime_command(
    runtime: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<OpenClawPluginFeishuRuntimeStatus, String> {
    stop_openclaw_plugin_feishu_runtime_in_state(runtime.inner())
}

pub(crate) async fn get_openclaw_plugin_feishu_runtime_status_command(
    runtime: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<OpenClawPluginFeishuRuntimeStatus, String> {
    Ok(current_feishu_runtime_status(runtime.inner()))
}

pub(crate) async fn get_feishu_plugin_environment_status_command(
) -> Result<FeishuPluginEnvironmentStatus, String> {
    Ok(get_feishu_plugin_environment_status_internal())
}

pub(crate) async fn get_feishu_setup_progress_command(
    app: AppHandle,
    db: State<'_, DbState>,
    runtime: State<'_, OpenClawPluginFeishuRuntimeState>,
) -> Result<FeishuSetupProgress, String> {
    if let Ok(shim_root) = resolve_openclaw_shim_root(&app) {
        let _ = sync_feishu_gateway_credentials_from_shim_with_pool(&db.0, &shim_root).await;
    }
    if let Ok(state_root) = resolve_controlled_openclaw_state_root(&app) {
        let _ = sync_feishu_gateway_credentials_from_openclaw_state_with_pool(&db.0, &state_root).await;
    }
    get_feishu_setup_progress_with_pool(&db.0, runtime.inner()).await
}

pub(crate) async fn get_openclaw_plugin_feishu_advanced_settings_command(
    db: State<'_, DbState>,
) -> Result<OpenClawPluginFeishuAdvancedSettings, String> {
    get_openclaw_plugin_feishu_advanced_settings_with_pool(&db.0).await
}

pub(crate) async fn set_openclaw_plugin_feishu_advanced_settings_command(
    settings: OpenClawPluginFeishuAdvancedSettings,
    db: State<'_, DbState>,
) -> Result<OpenClawPluginFeishuAdvancedSettings, String> {
    set_openclaw_plugin_feishu_advanced_settings_with_pool(&db.0, &settings).await
}

pub(crate) async fn start_openclaw_lark_installer_session_command(
    mode: OpenClawLarkInstallerMode,
    app_id: Option<String>,
    app_secret: Option<String>,
    app: AppHandle,
    db: State<'_, DbState>,
    installer: State<'_, OpenClawLarkInstallerSessionState>,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    start_openclaw_lark_installer_session_with_pool(
        &db.0,
        installer.inner(),
        mode,
        app_id.as_deref(),
        app_secret.as_deref(),
        &app,
    )
    .await
}

pub(crate) async fn get_openclaw_lark_installer_session_status_command(
    app: AppHandle,
    db: State<'_, DbState>,
    installer: State<'_, OpenClawLarkInstallerSessionState>,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    if let Ok(shim_root) = resolve_openclaw_shim_root(&app) {
        let _ = sync_feishu_gateway_credentials_from_shim_with_pool(&db.0, &shim_root).await;
    }
    if let Ok(state_root) = resolve_controlled_openclaw_state_root(&app) {
        let _ = sync_feishu_gateway_credentials_from_openclaw_state_with_pool(&db.0, &state_root).await;
    }
    Ok(current_openclaw_lark_installer_session_status(installer.inner()))
}

pub(crate) async fn send_openclaw_lark_installer_input_command(
    input: String,
    installer: State<'_, OpenClawLarkInstallerSessionState>,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    super::send_openclaw_lark_installer_input_in_state(installer.inner(), &input)
}

pub(crate) async fn stop_openclaw_lark_installer_session_command(
    installer: State<'_, OpenClawLarkInstallerSessionState>,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    stop_openclaw_lark_installer_session_in_state(installer.inner())
}

pub(crate) async fn probe_openclaw_plugin_feishu_credentials_command(
    app_id: String,
    app_secret: String,
) -> Result<OpenClawPluginFeishuCredentialProbeResult, String> {
    probe_openclaw_plugin_feishu_credentials_with_app_secret(&app_id, &app_secret).await
}

pub(crate) async fn upsert_openclaw_plugin_install_command(
    input: OpenClawPluginInstallInput,
    db: State<'_, DbState>,
) -> Result<OpenClawPluginInstallRecord, String> {
    upsert_openclaw_plugin_install_with_pool(&db.0, input).await
}

pub(crate) async fn list_openclaw_plugin_installs_command(
    db: State<'_, DbState>,
) -> Result<Vec<OpenClawPluginInstallRecord>, String> {
    list_openclaw_plugin_installs_with_pool(&db.0).await
}

pub(crate) async fn delete_openclaw_plugin_install_command(
    plugin_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    delete_openclaw_plugin_install_with_pool(&db.0, &plugin_id).await
}

pub(crate) async fn inspect_openclaw_plugin_command(
    plugin_id: String,
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<OpenClawPluginInspectionResult, String> {
    inspect_openclaw_plugin_with_pool_and_app(&db.0, &plugin_id, Some(&app)).await
}

pub(crate) async fn list_openclaw_plugin_channel_hosts_command(
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<Vec<OpenClawPluginChannelHost>, String> {
    list_openclaw_plugin_channel_hosts_with_pool_and_app(&db.0, Some(&app)).await
}

pub(crate) async fn get_openclaw_plugin_feishu_channel_snapshot_command(
    plugin_id: String,
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<OpenClawPluginChannelSnapshotResult, String> {
    get_openclaw_plugin_feishu_channel_snapshot_with_pool_and_app(&db.0, &plugin_id, Some(&app))
        .await
}

pub(crate) async fn install_openclaw_plugin_from_npm_command(
    plugin_id: String,
    npm_spec: String,
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<OpenClawPluginInstallRecord, String> {
    let normalized_plugin_id = normalize_required(&plugin_id, "plugin_id")?;
    let normalized_npm_spec = normalize_required(&npm_spec, "npm_spec")?;
    install_openclaw_plugin_from_npm_with_pool_and_app(
        &db.0,
        &normalized_plugin_id,
        &normalized_npm_spec,
        &app,
    )
    .await
}
