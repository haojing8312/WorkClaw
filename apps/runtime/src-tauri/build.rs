fn main() {
    #[cfg(target_os = "windows")]
    {
        let manifest_path = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("windows-common-controls-v6.manifest");
        let manifest_arg = format!("/MANIFESTINPUT:{}", manifest_path.display());

        println!("cargo:rerun-if-changed={}", manifest_path.display());

        // Headless examples and unit-test harnesses still link through Tauri/Wry on Windows.
        // Embed the common-controls v6 activation manifest so imports such as
        // TaskDialogIndirect resolve before the executable reaches main().
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg={manifest_arg}");

        // Keep headless evals on the main thread for tao compatibility, but give
        // the real-regression examples a larger primary-thread stack for heavy scenarios.
        println!("cargo:rustc-link-arg-examples=/STACK:134217728");
    }

    tauri_build::build()
}
