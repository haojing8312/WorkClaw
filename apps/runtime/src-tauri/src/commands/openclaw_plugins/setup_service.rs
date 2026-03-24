use crate::commands::feishu_gateway::{
    get_app_setting, list_feishu_pairing_requests_with_pool, set_app_setting,
};
use reqwest::Client;
use sqlx::SqlitePool;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

use super::{
    current_feishu_runtime_status, get_feishu_plugin_environment_status_internal,
    get_openclaw_plugin_install_by_id_with_pool, FeishuPluginEnvironmentStatus,
    FeishuSetupProgress, OpenClawPluginFeishuCredentialProbeResult,
    OpenClawPluginFeishuRuntimeState,
};

fn resolve_employee_agent_identity(
    employee_id: &str,
    role_id: &str,
    openclaw_agent_id: &str,
) -> String {
    let openclaw_agent_id = openclaw_agent_id.trim();
    if !openclaw_agent_id.is_empty() {
        return openclaw_agent_id.to_string();
    }

    let employee_id = employee_id.trim();
    if !employee_id.is_empty() {
        return employee_id.to_string();
    }

    role_id.trim().to_string()
}

async fn default_feishu_routing_employee_name_with_pool(
    pool: &SqlitePool,
) -> Result<Option<String>, String> {
    let binding = sqlx::query_as::<_, (String,)>(
        "SELECT agent_id
         FROM im_routing_bindings
         WHERE channel = 'feishu'
           AND enabled = 1
           AND trim(peer_id) = ''
         ORDER BY priority ASC, updated_at DESC
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some((binding_agent_id,)) = binding else {
        return Ok(None);
    };

    let employees = sqlx::query_as::<_, (String, String, String, String, i64)>(
        "SELECT employee_id, role_id, COALESCE(openclaw_agent_id, ''), name, enabled
         FROM agent_employees",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(employees.into_iter().find_map(
        |(employee_id, role_id, openclaw_agent_id, name, enabled)| {
            if enabled == 0 {
                return None;
            }
            let resolved =
                resolve_employee_agent_identity(&employee_id, &role_id, &openclaw_agent_id);
            if resolved.eq_ignore_ascii_case(binding_agent_id.trim()) {
                Some(name)
            } else {
                None
            }
        },
    ))
}

async fn count_scoped_feishu_routing_bindings_with_pool(
    pool: &SqlitePool,
) -> Result<usize, String> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM im_routing_bindings
         WHERE channel = 'feishu'
           AND enabled = 1
           AND trim(peer_id) != ''",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(count.max(0) as usize)
}

pub(crate) fn derive_feishu_setup_summary_state(
    environment: &FeishuPluginEnvironmentStatus,
    credentials_configured: bool,
    plugin_installed: bool,
    runtime_running: bool,
    runtime_last_error: Option<&str>,
    auth_status: &str,
    pending_pairings: usize,
    default_routing_employee_name: Option<&str>,
    scoped_routing_count: usize,
) -> String {
    if !environment.can_install_plugin || !environment.can_start_runtime {
        return "env_missing".to_string();
    }
    if runtime_last_error
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
    {
        return "runtime_error".to_string();
    }
    if !plugin_installed {
        return "plugin_not_installed".to_string();
    }
    if !credentials_configured {
        return "ready_to_bind".to_string();
    }
    if !runtime_running {
        return "plugin_starting".to_string();
    }
    if pending_pairings > 0 {
        return "awaiting_pairing_approval".to_string();
    }
    if auth_status != "approved" {
        return "awaiting_auth".to_string();
    }
    if default_routing_employee_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
        && scoped_routing_count == 0
    {
        return "ready_for_routing".to_string();
    }
    "ready".to_string()
}

pub(crate) async fn get_feishu_setup_progress_with_pool(
    pool: &SqlitePool,
    runtime_state: &OpenClawPluginFeishuRuntimeState,
) -> Result<FeishuSetupProgress, String> {
    let environment = get_feishu_plugin_environment_status_internal();
    let app_id = get_app_setting(pool, "feishu_app_id")
        .await?
        .unwrap_or_default();
    let app_secret = get_app_setting(pool, "feishu_app_secret")
        .await?
        .unwrap_or_default();
    let credentials_configured = !app_id.trim().is_empty() && !app_secret.trim().is_empty();

    let install = get_openclaw_plugin_install_by_id_with_pool(pool, "openclaw-lark")
        .await
        .ok();
    let runtime_status = current_feishu_runtime_status(runtime_state);
    let pairing_requests = list_feishu_pairing_requests_with_pool(pool, None).await?;
    let pending_pairings = pairing_requests
        .iter()
        .filter(|record| record.status == "pending")
        .count();
    let auth_status = if pairing_requests
        .iter()
        .any(|record| record.status == "approved")
    {
        "approved".to_string()
    } else if credentials_configured && runtime_status.running {
        "pending".to_string()
    } else {
        "unknown".to_string()
    };
    let default_routing_employee_name =
        default_feishu_routing_employee_name_with_pool(pool).await?;
    let scoped_routing_count = count_scoped_feishu_routing_bindings_with_pool(pool).await?;
    let summary_state = derive_feishu_setup_summary_state(
        &environment,
        credentials_configured,
        install.is_some(),
        runtime_status.running,
        runtime_status.last_error.as_deref(),
        &auth_status,
        pending_pairings,
        default_routing_employee_name.as_deref(),
        scoped_routing_count,
    );

    Ok(FeishuSetupProgress {
        environment,
        credentials_configured,
        plugin_installed: install.is_some(),
        plugin_version: install.as_ref().map(|record| record.version.clone()),
        runtime_running: runtime_status.running,
        runtime_last_error: runtime_status.last_error,
        auth_status,
        pending_pairings,
        default_routing_employee_name,
        scoped_routing_count,
        summary_state,
    })
}

pub(crate) fn should_auto_restore_feishu_runtime(progress: &FeishuSetupProgress) -> bool {
    progress.plugin_installed
        && progress.credentials_configured
        && !progress.runtime_running
        && progress.auth_status == "approved"
}

pub(crate) fn resolve_openclaw_shim_root(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app_data_dir: {e}"))?;
    Ok(app_data_dir.join("openclaw-cli-shim"))
}

pub(crate) fn build_openclaw_shim_state_file_path(shim_root: &Path) -> PathBuf {
    shim_root.join("state.json")
}

pub(crate) fn resolve_controlled_openclaw_state_root(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app_data_dir: {e}"))?;
    Ok(app_data_dir.join("openclaw-state"))
}

#[derive(Debug, Default, serde::Deserialize)]
pub(crate) struct OpenClawShimRecordedCommand {
    #[serde(default)]
    pub(crate) args: Vec<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
pub(crate) struct OpenClawShimStateSnapshot {
    #[serde(default)]
    pub(crate) config: serde_json::Value,
    #[serde(default)]
    pub(crate) commands: Vec<OpenClawShimRecordedCommand>,
}

fn read_openclaw_shim_state_snapshot(
    shim_root: &Path,
) -> Result<OpenClawShimStateSnapshot, String> {
    let state_path = build_openclaw_shim_state_file_path(shim_root);
    if !state_path.exists() {
        return Ok(OpenClawShimStateSnapshot::default());
    }

    let raw = fs::read_to_string(&state_path)
        .map_err(|error| format!("failed to read openclaw shim state: {error}"))?;
    if raw.trim().is_empty() {
        return Ok(OpenClawShimStateSnapshot::default());
    }

    serde_json::from_str::<OpenClawShimStateSnapshot>(&raw)
        .map_err(|error| format!("failed to parse openclaw shim state: {error}"))
}

fn get_json_path_string(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }

    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn derive_feishu_credentials_from_shim_snapshot(
    snapshot: &OpenClawShimStateSnapshot,
) -> Option<(String, String)> {
    let credential_paths: [(&[&str], &[&str]); 3] = [
        (
            &["channels", "feishu", "appId"],
            &["channels", "feishu", "appSecret"],
        ),
        (
            &["channels", "feishu", "accounts", "default", "appId"],
            &["channels", "feishu", "accounts", "default", "appSecret"],
        ),
        (&["feishu", "appId"], &["feishu", "appSecret"]),
    ];

    for (app_id_path, app_secret_path) in credential_paths {
        let app_id = get_json_path_string(&snapshot.config, app_id_path);
        let app_secret = get_json_path_string(&snapshot.config, app_secret_path);
        if let (Some(app_id), Some(app_secret)) = (app_id, app_secret) {
            return Some((app_id, app_secret));
        }
    }

    let mut app_id = None;
    let mut app_secret = None;
    for command in &snapshot.commands {
        let args = &command.args;
        if args.len() < 4 || args[0] != "config" || args[1] != "set" {
            continue;
        }

        let key = args[2].trim();
        let value = args[3].trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }

        let normalized_key = key.to_ascii_lowercase();
        if !normalized_key.contains("feishu") {
            continue;
        }
        if normalized_key.ends_with(".appid") {
            app_id = Some(value.to_string());
        } else if normalized_key.ends_with(".appsecret") {
            app_secret = Some(value.to_string());
        }
    }

    match (app_id, app_secret) {
        (Some(app_id), Some(app_secret)) => Some((app_id, app_secret)),
        _ => None,
    }
}

pub(crate) async fn sync_feishu_gateway_credentials_from_shim_with_pool(
    pool: &SqlitePool,
    shim_root: &Path,
) -> Result<bool, String> {
    let snapshot = read_openclaw_shim_state_snapshot(shim_root)?;
    let Some((app_id, app_secret)) = derive_feishu_credentials_from_shim_snapshot(&snapshot) else {
        return Ok(false);
    };

    let current_app_id = get_app_setting(pool, "feishu_app_id")
        .await?
        .unwrap_or_default();
    let current_app_secret = get_app_setting(pool, "feishu_app_secret")
        .await?
        .unwrap_or_default();

    if current_app_id.trim() == app_id.trim() && current_app_secret.trim() == app_secret.trim() {
        return Ok(false);
    }

    set_app_setting(pool, "feishu_app_id", app_id.trim()).await?;
    set_app_setting(pool, "feishu_app_secret", app_secret.trim()).await?;
    Ok(true)
}

fn get_json_path<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn resolve_openclaw_state_secret_ref(
    config: &serde_json::Value,
    state_root: &Path,
    secret_ref: &serde_json::Value,
) -> Option<String> {
    let source = secret_ref.get("source")?.as_str()?.trim();
    let id = secret_ref.get("id")?.as_str()?.trim();
    if source.eq_ignore_ascii_case("env") {
        if let Some(value) = std::env::var_os(id).and_then(|value| value.into_string().ok()) {
            let trimmed = value.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }

        let env_path = state_root.join(".env");
        let raw = fs::read_to_string(env_path).ok()?;
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some((key, value)) = trimmed.split_once('=') else {
                continue;
            };
            if key.trim() == id {
                let secret = value.trim().to_string();
                if !secret.is_empty() {
                    return Some(secret);
                }
            }
        }
        return None;
    }

    if !source.eq_ignore_ascii_case("file") {
        return None;
    }

    let provider_name = secret_ref.get("provider")?.as_str()?.trim();
    let provider = get_json_path(config, &["secrets", "providers", provider_name])?;
    let provider_path = provider.get("path")?.as_str()?.trim();
    if provider_path.is_empty() {
        return None;
    }
    let home_dir = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let file_path = if provider_path.starts_with("~/") {
        home_dir?.join(provider_path.trim_start_matches("~/"))
    } else {
        PathBuf::from(provider_path)
    };
    let raw = fs::read_to_string(file_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;

    if provider
        .get("mode")
        .and_then(|entry| entry.as_str())
        .map(|mode| mode == "singleValue")
        .unwrap_or(false)
    {
        return json
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
    }

    let mut current = &json;
    for segment in id.trim_start_matches('/').split('/') {
        if segment.is_empty() {
            continue;
        }
        let unescaped = segment.replace("~1", "/").replace("~0", "~");
        current = current.get(unescaped)?;
    }
    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn derive_feishu_credentials_from_openclaw_state_config(
    config: &serde_json::Value,
    state_root: &Path,
) -> Option<(String, String)> {
    let feishu = get_json_path(config, &["channels", "feishu"])?;
    let app_id = feishu
        .get("appId")
        .and_then(|entry| entry.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)?;

    let app_secret_value = feishu.get("appSecret")?;
    let app_secret = match app_secret_value {
        serde_json::Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return None;
            }
            trimmed.to_string()
        }
        serde_json::Value::Object(_) => {
            resolve_openclaw_state_secret_ref(config, state_root, app_secret_value)?
        }
        _ => return None,
    };

    Some((app_id, app_secret))
}

pub(crate) async fn sync_feishu_gateway_credentials_from_openclaw_state_with_pool(
    pool: &SqlitePool,
    state_root: &Path,
) -> Result<bool, String> {
    let config_path = state_root.join("openclaw.json");
    if !config_path.exists() {
        return Ok(false);
    }

    let raw = fs::read_to_string(&config_path).map_err(|error| {
        format!(
            "failed to read controlled OpenClaw config {}: {error}",
            config_path.display()
        )
    })?;
    if raw.trim().is_empty() {
        return Ok(false);
    }
    let config: serde_json::Value = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "failed to parse controlled OpenClaw config {}: {error}",
            config_path.display()
        )
    })?;

    let Some((app_id, app_secret)) =
        derive_feishu_credentials_from_openclaw_state_config(&config, state_root)
    else {
        return Ok(false);
    };

    let current_app_id = get_app_setting(pool, "feishu_app_id")
        .await?
        .unwrap_or_default();
    let current_app_secret = get_app_setting(pool, "feishu_app_secret")
        .await?
        .unwrap_or_default();

    if current_app_id.trim() == app_id.trim() && current_app_secret.trim() == app_secret.trim() {
        return Ok(false);
    }

    set_app_setting(pool, "feishu_app_id", app_id.trim()).await?;
    set_app_setting(pool, "feishu_app_secret", app_secret.trim()).await?;
    Ok(true)
}

fn feishu_open_api_base_url() -> String {
    std::env::var("WORKCLAW_FEISHU_OPEN_API_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "https://open.feishu.cn".to_string())
}

pub(crate) fn parse_feishu_app_access_token_response(
    value: serde_json::Value,
) -> Result<String, String> {
    let code = value
        .get("code")
        .and_then(|entry| entry.as_i64())
        .unwrap_or(-1);
    if code != 0 {
        let msg = value
            .get("msg")
            .and_then(|entry| entry.as_str())
            .unwrap_or("unknown error");
        return Err(format!("API error: {msg}"));
    }

    value
        .get("app_access_token")
        .and_then(|entry| entry.as_str())
        .filter(|entry| !entry.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| "missing app_access_token".to_string())
}

pub(crate) fn parse_feishu_bot_info_response(
    app_id: &str,
    value: serde_json::Value,
) -> OpenClawPluginFeishuCredentialProbeResult {
    let code = value
        .get("code")
        .and_then(|entry| entry.as_i64())
        .unwrap_or(-1);
    if code != 0 {
        let msg = value
            .get("msg")
            .and_then(|entry| entry.as_str())
            .unwrap_or("unknown error");
        return OpenClawPluginFeishuCredentialProbeResult {
            ok: false,
            app_id: app_id.to_string(),
            bot_name: None,
            bot_open_id: None,
            error: Some(format!("API error: {msg}")),
        };
    }

    let bot = value
        .get("bot")
        .or_else(|| value.get("data").and_then(|entry| entry.get("bot")));

    OpenClawPluginFeishuCredentialProbeResult {
        ok: true,
        app_id: app_id.to_string(),
        bot_name: bot
            .and_then(|entry| entry.get("bot_name"))
            .and_then(|entry| entry.as_str())
            .map(str::to_string),
        bot_open_id: bot
            .and_then(|entry| entry.get("open_id"))
            .and_then(|entry| entry.as_str())
            .map(str::to_string),
        error: None,
    }
}

async fn probe_openclaw_plugin_feishu_credentials_with_client(
    client: &Client,
    app_id: &str,
    app_secret: &str,
) -> Result<OpenClawPluginFeishuCredentialProbeResult, String> {
    if app_id.trim().is_empty() || app_secret.trim().is_empty() {
        return Ok(OpenClawPluginFeishuCredentialProbeResult {
            ok: false,
            app_id: app_id.trim().to_string(),
            bot_name: None,
            bot_open_id: None,
            error: Some("missing credentials (appId, appSecret)".to_string()),
        });
    }

    let base_url = feishu_open_api_base_url().trim_end_matches('/').to_string();
    let token_response = client
        .post(format!(
            "{base_url}/open-apis/auth/v3/app_access_token/internal"
        ))
        .json(&serde_json::json!({
            "app_id": app_id,
            "app_secret": app_secret,
        }))
        .send()
        .await
        .map_err(|error| format!("failed to request app_access_token: {error}"))?;
    let token_json: serde_json::Value = token_response
        .json()
        .await
        .map_err(|error| format!("failed to decode app_access_token response: {error}"))?;
    let access_token = match parse_feishu_app_access_token_response(token_json) {
        Ok(token) => token,
        Err(error) => {
            return Ok(OpenClawPluginFeishuCredentialProbeResult {
                ok: false,
                app_id: app_id.to_string(),
                bot_name: None,
                bot_open_id: None,
                error: Some(error),
            })
        }
    };

    let bot_response = client
        .get(format!("{base_url}/open-apis/bot/v3/info"))
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|error| format!("failed to request bot info: {error}"))?;
    let bot_json: serde_json::Value = bot_response
        .json()
        .await
        .map_err(|error| format!("failed to decode bot info response: {error}"))?;

    Ok(parse_feishu_bot_info_response(app_id, bot_json))
}

pub(crate) async fn probe_openclaw_plugin_feishu_credentials_with_app_secret(
    app_id: &str,
    app_secret: &str,
) -> Result<OpenClawPluginFeishuCredentialProbeResult, String> {
    let client = Client::builder()
        .build()
        .map_err(|error| format!("failed to build feishu probe client: {error}"))?;
    probe_openclaw_plugin_feishu_credentials_with_client(&client, app_id, app_secret).await
}
