use anyhow::Result;
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct SidecarManager {
    process: Arc<Mutex<Option<Child>>>,
    env_vars: Arc<Mutex<HashMap<String, String>>>,
    url: String,
}

impl SidecarManager {
    pub fn new() -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
            env_vars: Arc::new(Mutex::new(HashMap::new())),
            url: "http://localhost:8765".to_string(),
        }
    }

    pub async fn start(&self) -> Result<()> {
        let mut proc = self.process.lock().unwrap();
        if proc.is_some() {
            return Ok(()); // Already started
        }

        // Resolve sidecar path: try relative to CWD, then parent (for Tauri runtime)
        let cwd = std::env::current_dir().unwrap_or_default();
        let sidecar_script = {
            let candidate = cwd.join("sidecar").join("dist").join("index.js");
            if candidate.exists() {
                candidate
            } else {
                // When running from src-tauri/, look in ../sidecar/
                cwd.join("..").join("sidecar").join("dist").join("index.js")
            }
        };

        let mut command = Command::new("node");
        command
            .arg(&sidecar_script)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in self.env_vars.lock().unwrap().iter() {
            command.env(key, value);
        }

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            // Prevent a separate console window when launching the sidecar.
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let child = command.spawn()?;

        *proc = Some(child);
        drop(proc); // Release lock before polling

        // Wait for server to be ready (max 5 seconds)
        for _ in 0..50 {
            if self.health_check().await.is_ok() {
                eprintln!("[sidecar] Service started: {}", self.url);
                return Ok(());
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        Err(anyhow::anyhow!("Sidecar startup timeout"))
    }

    pub async fn health_check(&self) -> Result<()> {
        let resp = reqwest::get(&format!("{}/health", self.url)).await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Health check failed"))
        }
    }

    pub fn stop(&self) {
        let mut proc = self.process.lock().unwrap();
        if let Some(mut child) = proc.take() {
            let _ = child.kill();
            let _ = child.wait();
            eprintln!("[sidecar] Service stopped");
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn set_env_var(&self, key: impl Into<String>, value: impl Into<String>) {
        self.env_vars
            .lock()
            .unwrap()
            .insert(key.into(), value.into());
    }
}

impl Drop for SidecarManager {
    fn drop(&mut self) {
        self.stop();
    }
}
