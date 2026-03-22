use runtime_lib::sidecar::{resolve_sidecar_runtime, SidecarManager, SidecarRuntimePaths};
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn resolve_sidecar_runtime_prefers_packaged_resource_layout() {
    let tmp = tempdir().expect("tempdir");
    let resource_dir = tmp.path().join("resources");
    let sidecar_dir = resource_dir.join("sidecar-runtime");
    std::fs::create_dir_all(&sidecar_dir).expect("create sidecar resource dir");
    std::fs::write(sidecar_dir.join("index.js"), "console.log('sidecar');")
        .expect("write sidecar script");
    std::fs::write(sidecar_dir.join("node.exe"), "node").expect("write bundled node placeholder");

    let resolved = resolve_sidecar_runtime(SidecarRuntimePaths {
        cwd: PathBuf::from("C:/Program Files/WorkClaw"),
        resource_dir: Some(resource_dir),
    })
    .expect("packaged sidecar runtime should resolve");

    assert!(resolved.script.ends_with("sidecar-runtime/index.js"));
    assert!(resolved
        .command
        .replace('\\', "/")
        .ends_with("sidecar-runtime/node.exe"));
}

#[test]
fn resolve_sidecar_runtime_prefers_dev_layout_when_available() {
    let tmp = tempdir().expect("tempdir");
    let resource_dir = tmp.path().join("resources");
    let packaged_sidecar_dir = resource_dir.join("sidecar-runtime");
    std::fs::create_dir_all(&packaged_sidecar_dir).expect("create packaged sidecar dir");
    std::fs::write(packaged_sidecar_dir.join("index.js"), "console.log('packaged sidecar');")
        .expect("write packaged sidecar script");
    std::fs::write(packaged_sidecar_dir.join("node.exe"), "node")
        .expect("write packaged node placeholder");

    let dev_sidecar_dir = tmp.path().join("sidecar").join("dist");
    std::fs::create_dir_all(&dev_sidecar_dir).expect("create dev sidecar dir");
    std::fs::write(dev_sidecar_dir.join("index.js"), "console.log('dev sidecar');")
        .expect("write dev sidecar script");

    let resolved = resolve_sidecar_runtime(SidecarRuntimePaths {
        cwd: tmp.path().to_path_buf(),
        resource_dir: Some(resource_dir),
    })
    .expect("dev sidecar runtime should resolve ahead of packaged resources");

    assert_eq!(resolved.command, "node");
    assert!(resolved.script.ends_with("sidecar/dist/index.js"));
}

#[test]
fn resolve_sidecar_runtime_falls_back_to_dev_layout() {
    let tmp = tempdir().expect("tempdir");
    let dev_sidecar_dir = tmp.path().join("sidecar").join("dist");
    std::fs::create_dir_all(&dev_sidecar_dir).expect("create dev sidecar dir");
    std::fs::write(
        dev_sidecar_dir.join("index.js"),
        "console.log('dev sidecar');",
    )
    .expect("write dev sidecar script");

    let resolved = resolve_sidecar_runtime(SidecarRuntimePaths {
        cwd: tmp.path().to_path_buf(),
        resource_dir: None,
    })
    .expect("dev sidecar runtime should resolve");

    assert_eq!(resolved.command, "node");
    assert!(resolved.script.ends_with("sidecar/dist/index.js"));
}

#[test]
fn resolve_sidecar_runtime_returns_specific_error_when_missing() {
    let tmp = tempdir().expect("tempdir");
    let error = resolve_sidecar_runtime(SidecarRuntimePaths {
        cwd: tmp.path().to_path_buf(),
        resource_dir: None,
    })
    .expect_err("missing sidecar runtime should be an error");

    let message = error.to_string();
    assert!(message.contains("Sidecar runtime not found"));
    assert!(message.contains("sidecar/dist/index.js"));
}

#[tokio::test]
async fn test_sidecar_start_and_health_check() {
    let runtime = resolve_sidecar_runtime(SidecarRuntimePaths {
        cwd: std::env::current_dir().expect("current dir"),
        resource_dir: None,
    });
    if runtime.is_err() {
        eprintln!(
            "skipping live sidecar startup test because no runnable sidecar runtime is available"
        );
        return;
    }

    let manager = SidecarManager::new();

    // Start sidecar
    let result = manager.start().await;
    assert!(
        result.is_ok(),
        "Sidecar should start successfully: {:?}",
        result.err()
    );

    // Health check should succeed
    let health = manager.health_check().await;
    assert!(
        health.is_ok(),
        "Health check should pass: {:?}",
        health.err()
    );

    // Stop sidecar
    manager.stop();
}
