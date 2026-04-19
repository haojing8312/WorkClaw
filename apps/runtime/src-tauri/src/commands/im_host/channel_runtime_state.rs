use super::startup_restore::ImChannelRestoreReport;
use serde_json::Value;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const MAX_IM_CHANNEL_HOST_ACTIONS: usize = 16;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ImChannelHostActionRecord {
    pub channel: String,
    pub action: String,
    pub desired_running: bool,
    pub ok: bool,
    pub detail: String,
    pub error: Option<String>,
    pub source: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ImChannelHostRuntimeSnapshot {
    pub last_restore_report: Option<ImChannelRestoreReport>,
    pub recent_actions: Vec<ImChannelHostActionRecord>,
    #[serde(default)]
    pub runtime_status_by_channel: HashMap<String, Value>,
}

#[derive(Debug, Default)]
pub(crate) struct ImChannelHostRuntimeStore {
    last_restore_report: Option<ImChannelRestoreReport>,
    recent_actions: VecDeque<ImChannelHostActionRecord>,
    runtime_status_by_channel: HashMap<String, Value>,
}

#[derive(Clone, Default)]
pub struct ImChannelHostRuntimeState(pub(crate) Arc<Mutex<ImChannelHostRuntimeStore>>);

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn record_im_channel_restore_report(
    state: &ImChannelHostRuntimeState,
    report: ImChannelRestoreReport,
) -> Result<(), String> {
    let mut guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock im channel host runtime state".to_string())?;
    guard.last_restore_report = Some(report);
    Ok(())
}

pub fn record_im_channel_host_action(
    state: &ImChannelHostRuntimeState,
    channel: &str,
    action: &str,
    desired_running: bool,
    ok: bool,
    detail: String,
    error: Option<String>,
    source: &str,
) -> Result<(), String> {
    let mut guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock im channel host runtime state".to_string())?;
    guard.recent_actions.push_front(ImChannelHostActionRecord {
        channel: channel.trim().to_ascii_lowercase(),
        action: action.trim().to_string(),
        desired_running,
        ok,
        detail,
        error,
        source: source.trim().to_string(),
        occurred_at: now_rfc3339(),
    });
    while guard.recent_actions.len() > MAX_IM_CHANNEL_HOST_ACTIONS {
        let _ = guard.recent_actions.pop_back();
    }
    Ok(())
}

pub fn record_im_channel_runtime_status(
    state: &ImChannelHostRuntimeState,
    channel: &str,
    runtime_status: Value,
) -> Result<(), String> {
    let normalized_channel = channel.trim().to_ascii_lowercase();
    if normalized_channel.is_empty() {
        return Err("channel is required".to_string());
    }
    let mut guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock im channel host runtime state".to_string())?;
    guard
        .runtime_status_by_channel
        .insert(normalized_channel, runtime_status);
    Ok(())
}

pub fn get_im_channel_runtime_status_in_state(
    state: &ImChannelHostRuntimeState,
    channel: &str,
) -> Result<Option<Value>, String> {
    let normalized_channel = channel.trim().to_ascii_lowercase();
    if normalized_channel.is_empty() {
        return Ok(None);
    }
    let guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock im channel host runtime state".to_string())?;
    Ok(guard
        .runtime_status_by_channel
        .get(&normalized_channel)
        .cloned())
}

pub fn get_im_channel_host_runtime_snapshot_in_state(
    state: &ImChannelHostRuntimeState,
) -> Result<ImChannelHostRuntimeSnapshot, String> {
    let guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock im channel host runtime state".to_string())?;
    Ok(ImChannelHostRuntimeSnapshot {
        last_restore_report: guard.last_restore_report.clone(),
        recent_actions: guard.recent_actions.iter().cloned().collect(),
        runtime_status_by_channel: guard.runtime_status_by_channel.clone(),
    })
}
