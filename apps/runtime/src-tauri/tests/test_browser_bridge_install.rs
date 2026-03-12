use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use runtime_lib::commands::browser_bridge_install::{
    get_browser_bridge_install_status_with_env, install_browser_bridge_with_env,
    BrowserBridgeInstallEnv,
};

#[test]
fn browser_bridge_status_defaults_to_not_installed() {
    let env = BrowserBridgeInstallEnv {
        local_app_data: Some(PathBuf::from("C:/Users/test/AppData/Local")),
        user_profile: Some(PathBuf::from("C:/Users/test")),
        repo_root: tempfile::tempdir().unwrap().into_path(),
    };

    let status = get_browser_bridge_install_status_with_env(&env, None);

    assert_eq!(status.state, "not_installed");
    assert!(status.chrome_found);
    assert!(!status.native_host_installed);
    assert!(!status.extension_dir_ready);
    assert!(!status.bridge_connected);
    assert_eq!(status.last_error, None);
}

#[test]
fn browser_bridge_install_errors_without_chrome_path() {
    let env = BrowserBridgeInstallEnv {
        local_app_data: None,
        user_profile: None,
        repo_root: tempfile::tempdir().unwrap().into_path(),
    };

    let error = install_browser_bridge_with_env(&env).unwrap_err();

    assert!(error.contains("Chrome 用户目录"));
}

#[test]
fn browser_bridge_status_becomes_connected_after_recent_heartbeat() {
    let env = BrowserBridgeInstallEnv {
        local_app_data: Some(PathBuf::from("C:/Users/test/AppData/Local")),
        user_profile: Some(PathBuf::from("C:/Users/test")),
        repo_root: tempfile::tempdir().unwrap().into_path(),
    };

    let status = get_browser_bridge_install_status_with_env(
        &env,
        Some(SystemTime::now() - Duration::from_secs(5)),
    );

    assert_eq!(status.state, "connected");
    assert!(status.chrome_found);
    assert!(status.bridge_connected);
}
