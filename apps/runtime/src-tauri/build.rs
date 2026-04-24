fn main() {
    #[cfg(target_os = "windows")]
    {
        // Keep headless evals on the main thread for tao compatibility, but give
        // the real-regression examples a larger primary-thread stack for heavy scenarios.
        println!("cargo:rustc-link-arg-examples=/STACK:134217728");
    }

    tauri_build::build()
}
