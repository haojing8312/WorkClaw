use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tauri::State;

const BROWSER_BRIDGE_CONNECTED_TTL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserBridgeInstallStatus {
    pub state: String,
    pub chrome_found: bool,
    pub native_host_installed: bool,
    pub extension_dir_ready: bool,
    pub bridge_connected: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BrowserBridgeInstallEnv {
    pub local_app_data: Option<PathBuf>,
    pub user_profile: Option<PathBuf>,
    pub repo_root: PathBuf,
}

#[derive(Clone, Default)]
pub struct BrowserBridgeInstallStore {
    last_heartbeat_at: Arc<Mutex<Option<SystemTime>>>,
}

impl BrowserBridgeInstallStore {
    pub fn mark_connected_now(&self) {
        *self.last_heartbeat_at.lock().unwrap() = Some(SystemTime::now());
    }

    pub fn last_heartbeat_at(&self) -> Option<SystemTime> {
        *self.last_heartbeat_at.lock().unwrap()
    }
}

#[derive(Clone, Default)]
pub struct BrowserBridgeInstallState(pub BrowserBridgeInstallStore);

impl BrowserBridgeInstallEnv {
    pub fn from_process() -> Self {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| manifest_dir.clone());

        Self {
            local_app_data: std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
            user_profile: std::env::var_os("USERPROFILE").map(PathBuf::from),
            repo_root,
        }
    }
}

pub fn resolve_chrome_user_data_dir(env: &BrowserBridgeInstallEnv) -> Result<PathBuf, String> {
    if let Some(local_app_data) = env.local_app_data.as_ref() {
        return Ok(local_app_data
            .join("Google")
            .join("Chrome")
            .join("User Data"));
    }

    if let Some(user_profile) = env.user_profile.as_ref() {
        return Ok(user_profile
            .join("AppData")
            .join("Local")
            .join("Google")
            .join("Chrome")
            .join("User Data"));
    }

    Err("无法解析 Chrome 用户目录，请确认 Chrome 已安装并设置本机用户目录。".to_string())
}

pub fn browser_bridge_extension_dir(env: &BrowserBridgeInstallEnv) -> PathBuf {
    env.repo_root
        .join(".workclaw")
        .join("browser-bridge")
        .join("chrome-extension")
}

fn native_host_manifest_path(chrome_user_data_dir: &Path) -> PathBuf {
    chrome_user_data_dir
        .join("NativeMessagingHosts")
        .join("workclaw.chrome_bridge.json")
}

pub fn get_browser_bridge_install_status_with_env(
    env: &BrowserBridgeInstallEnv,
    last_heartbeat_at: Option<SystemTime>,
) -> BrowserBridgeInstallStatus {
    match resolve_chrome_user_data_dir(env) {
        Ok(chrome_user_data_dir) => {
            let native_host_installed = native_host_manifest_path(&chrome_user_data_dir).exists();
            let extension_dir_ready = browser_bridge_extension_dir(env).exists();
            let bridge_connected = last_heartbeat_at
                .and_then(|timestamp| SystemTime::now().duration_since(timestamp).ok())
                .map(|elapsed| elapsed <= BROWSER_BRIDGE_CONNECTED_TTL)
                .unwrap_or(false);

            BrowserBridgeInstallStatus {
                state: if bridge_connected {
                    "connected".to_string()
                } else if native_host_installed && extension_dir_ready {
                    "waiting_for_enable".to_string()
                } else {
                    "not_installed".to_string()
                },
                chrome_found: true,
                native_host_installed,
                extension_dir_ready,
                bridge_connected,
                last_error: None,
            }
        }
        Err(error) => BrowserBridgeInstallStatus {
            state: "not_installed".to_string(),
            chrome_found: false,
            native_host_installed: false,
            extension_dir_ready: false,
            bridge_connected: false,
            last_error: Some(error),
        },
    }
}

pub fn install_browser_bridge_with_env(
    env: &BrowserBridgeInstallEnv,
) -> Result<BrowserBridgeInstallStatus, String> {
    let _chrome_user_data_dir = resolve_chrome_user_data_dir(env)?;

    Ok(BrowserBridgeInstallStatus {
        state: "waiting_for_enable".to_string(),
        chrome_found: true,
        native_host_installed: false,
        extension_dir_ready: false,
        bridge_connected: false,
        last_error: None,
    })
}

fn open_path(path: &Path) -> Result<(), String> {
    let target = path.to_string_lossy().to_string();
    if target.trim().is_empty() {
        return Err("路径不能为空".to_string());
    }

    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("cmd")
        .args(["/C", "start", "", &target])
        .status();

    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg(path).status();

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let status = std::process::Command::new("xdg-open").arg(path).status();

    let status = status.map_err(|e| format!("打开路径失败: {}", e))?;
    if !status.success() {
        return Err(format!("打开路径失败: {}", target));
    }
    Ok(())
}

#[tauri::command]
pub async fn get_browser_bridge_install_status(
    state: State<'_, BrowserBridgeInstallState>,
) -> Result<BrowserBridgeInstallStatus, String> {
    Ok(get_browser_bridge_install_status_with_env(
        &BrowserBridgeInstallEnv::from_process(),
        state.0.last_heartbeat_at(),
    ))
}

#[tauri::command]
pub async fn install_browser_bridge() -> Result<BrowserBridgeInstallStatus, String> {
    install_browser_bridge_with_env(&BrowserBridgeInstallEnv::from_process())
}

#[tauri::command]
pub async fn open_browser_bridge_extension_page() -> Result<(), String> {
    crate::commands::dialog::open_external_url("chrome://extensions".to_string()).await
}

#[tauri::command]
pub async fn open_browser_bridge_extension_dir() -> Result<(), String> {
    let env = BrowserBridgeInstallEnv::from_process();
    open_path(browser_bridge_extension_dir(&env).as_path())
}
