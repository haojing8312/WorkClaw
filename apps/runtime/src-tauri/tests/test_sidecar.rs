use runtime_lib::sidecar::SidecarManager;

#[tokio::test]
async fn test_sidecar_start_and_health_check() {
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
