use crate::windows_process::hide_console_window;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarRuntimePaths {
    pub cwd: PathBuf,
    pub resource_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSidecarRuntime {
    pub command: String,
    pub script: PathBuf,
    pub working_dir: PathBuf,
}

pub struct SidecarManager {
    process: Arc<Mutex<Option<Child>>>,
    env_vars: Arc<Mutex<HashMap<String, String>>>,
    url: String,
    resource_dir: Option<PathBuf>,
}

fn child_has_exited(child: &mut Child) -> Result<Option<ExitStatus>> {
    child
        .try_wait()
        .map_err(anyhow::Error::from)
}

fn resolve_packaged_node_command(bundle_dir: &Path) -> String {
    let bundled_node = if cfg!(windows) {
        bundle_dir.join("node.exe")
    } else {
        bundle_dir.join("node")
    };

    if bundled_node.is_file() {
        bundled_node.to_string_lossy().to_string()
    } else {
        "node".to_string()
    }
}

fn resolve_packaged_sidecar_runtime(bundle_dir: &Path) -> Option<ResolvedSidecarRuntime> {
    let script_candidates = [
        bundle_dir.join("dist").join("index.js"),
        bundle_dir.join("index.js"),
    ];
    let script = script_candidates
        .into_iter()
        .find(|candidate| candidate.is_file())?;

    Some(ResolvedSidecarRuntime {
        command: resolve_packaged_node_command(bundle_dir),
        script: script.clone(),
        working_dir: script
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| bundle_dir.to_path_buf()),
    })
}

pub fn resolve_sidecar_runtime(paths: SidecarRuntimePaths) -> Result<ResolvedSidecarRuntime> {
    let mut searched = Vec::new();

    let dev_candidates = [
        paths.cwd.join("sidecar").join("dist").join("index.js"),
        paths
            .cwd
            .join("..")
            .join("sidecar")
            .join("dist")
            .join("index.js"),
    ];
    for script in dev_candidates {
        searched.push(script.display().to_string());
        if script.is_file() {
            return Ok(ResolvedSidecarRuntime {
                command: "node".to_string(),
                working_dir: script
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| paths.cwd.clone()),
                script,
            });
        }
    }

    if let Some(resource_dir) = paths.resource_dir.as_ref() {
        for bundle_dir in [
            resource_dir.join("sidecar-runtime"),
            resource_dir.join("resources").join("sidecar-runtime"),
        ] {
            searched.push(bundle_dir.display().to_string());
            if let Some(runtime) = resolve_packaged_sidecar_runtime(&bundle_dir) {
                return Ok(runtime);
            }
        }
    }

    for bundle_dir in [paths.cwd.join("resources").join("sidecar-runtime")] {
        searched.push(bundle_dir.display().to_string());
        if let Some(runtime) = resolve_packaged_sidecar_runtime(&bundle_dir) {
            return Ok(runtime);
        }
    }

    Err(anyhow::anyhow!(
        "Sidecar runtime not found. Expected packaged resources or a dev script such as sidecar/dist/index.js. Searched: {}",
        searched.join(", ")
    ))
}

impl SidecarManager {
    pub fn new() -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
            env_vars: Arc::new(Mutex::new(HashMap::new())),
            url: "http://localhost:8765".to_string(),
            resource_dir: None,
        }
    }

    pub fn with_resource_dir(resource_dir: Option<PathBuf>) -> Self {
        Self {
            resource_dir,
            process: Arc::new(Mutex::new(None)),
            env_vars: Arc::new(Mutex::new(HashMap::new())),
            url: "http://localhost:8765".to_string(),
        }
    }

    pub async fn start(&self) -> Result<()> {
        {
            let mut proc = self.process.lock().unwrap();
            if let Some(child) = proc.as_mut() {
                match child_has_exited(child)? {
                    None => {
                        // The sidecar process is still alive. Wait for health below instead of
                        // returning immediately, so we can recover if the server is still warming up.
                    }
                    Some(_) => {
                        *proc = None;
                    }
                }
            }
        }

        let cwd = std::env::current_dir().unwrap_or_default();
        let runtime = resolve_sidecar_runtime(SidecarRuntimePaths {
            cwd,
            resource_dir: self.resource_dir.clone(),
        })?;

        {
            let mut proc = self.process.lock().unwrap();
            if proc.is_none() {
                let mut command = Command::new(&runtime.command);
                command
                    .arg(&runtime.script)
                    .current_dir(&runtime.working_dir)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                for (key, value) in self.env_vars.lock().unwrap().iter() {
                    command.env(key, value);
                }

                hide_console_window(&mut command);

                let child = command.spawn()?;
                *proc = Some(child);
            }
        }

        // Wait for server to be ready (max 5 seconds)
        for _ in 0..50 {
            if self.health_check().await.is_ok() {
                return Ok(());
            }

            {
                let mut proc = self.process.lock().unwrap();
                if let Some(child) = proc.as_mut() {
                    if child_has_exited(child)?.is_some() {
                        *proc = None;
                        break;
                    }
                } else {
                    break;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        self.stop();

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
