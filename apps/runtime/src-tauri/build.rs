fn main() {
    #[cfg(target_os = "windows")]
    {
        // Keep headless evals on the main thread for tao compatibility, but give
        // the agent_eval binary a larger primary-thread stack for heavy real scenarios.
        println!("cargo:rustc-link-arg-bin=agent_eval=/STACK:134217728");
    }

    tauri_build::build()
}
