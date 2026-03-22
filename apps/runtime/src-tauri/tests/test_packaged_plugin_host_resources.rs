use std::fs;
use std::path::PathBuf;

#[test]
fn tauri_bundle_includes_plugin_host_resources() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_path = manifest_dir.join("tauri.conf.json");
    let raw = fs::read_to_string(&config_path).expect("read tauri.conf.json");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse tauri.conf.json");

    let resources = parsed["bundle"]["resources"]
        .as_array()
        .expect("bundle.resources should be an array");
    let has_plugin_host = resources.iter().any(|entry| {
        entry
            .as_str()
            .map(|value| value.contains("plugin-host"))
            .unwrap_or(false)
    });

    assert!(
        has_plugin_host,
        "bundle.resources must include plugin-host assets for packaged Feishu support"
    );
}
