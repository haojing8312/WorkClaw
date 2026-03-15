#[cfg(target_os = "windows")]
#[test]
fn windows_command_execution_paths_hide_console_windows() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let targets = [
        "agent/tools/bash.rs",
        "agent/tools/process_manager.rs",
        "agent/tools/screenshot.rs",
        "commands/dialog.rs",
        "commands/runtime_preferences.rs",
    ];

    for target in targets {
        let source_path = root.join(target);
        let source = std::fs::read_to_string(&source_path)
            .unwrap_or_else(|_| panic!("read {}", source_path.display()));
        assert!(
            source.contains("hide_console_window"),
            "expected {} to hide Windows console windows",
            source_path.display()
        );
    }
}
