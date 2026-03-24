use crate::windows_process::hide_console_window;
use sqlx::SqlitePool;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::AppHandle;

use super::{
    apply_command_search_path, normalize_required, resolve_npm_command,
    resolve_openclaw_plugin_workspace_root, upsert_openclaw_plugin_install_with_pool,
    OpenClawPluginInstallInput, OpenClawPluginInstallRecord,
};

fn resolve_installed_package_dir(workspace: &Path, npm_spec: &str) -> Result<PathBuf, String> {
    let normalized = normalize_required(npm_spec, "npm_spec")?;
    let package_name = normalized
        .split('@')
        .next_back()
        .ok_or_else(|| format!("invalid npm spec: {normalized}"))?;
    let package_path = if normalized.starts_with('@') {
        let parts: Vec<&str> = normalized.split('/').collect();
        if parts.len() < 2 {
            return Err(format!("invalid scoped npm spec: {normalized}"));
        }
        workspace.join("node_modules").join(parts[0]).join(parts[1])
    } else {
        workspace.join("node_modules").join(package_name)
    };
    Ok(package_path)
}

fn load_plugin_manifest_json(package_dir: &Path, package_json: &serde_json::Value) -> String {
    let manifest_path = package_dir.join("openclaw.plugin.json");
    if let Ok(contents) = fs::read_to_string(&manifest_path) {
        if serde_json::from_str::<serde_json::Value>(&contents).is_ok() {
            return contents;
        }
    }

    package_json
        .get("openclaw")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}))
        .to_string()
}

fn write_workspace_package_json(workspace_dir: &Path, plugin_id: &str) -> Result<(), String> {
    let workspace_package_json = serde_json::json!({
        "name": format!("workclaw-openclaw-plugin-{plugin_id}"),
        "private": true,
    })
    .to_string();
    fs::write(workspace_dir.join("package.json"), workspace_package_json)
        .map_err(|e| format!("failed to write plugin workspace package.json: {e}"))
}

pub async fn install_openclaw_plugin_from_npm_with_pool_and_app(
    pool: &SqlitePool,
    plugin_id: &str,
    npm_spec: &str,
    app: &AppHandle,
) -> Result<OpenClawPluginInstallRecord, String> {
    let normalized_plugin_id = normalize_required(plugin_id, "plugin_id")?;
    let normalized_npm_spec = normalize_required(npm_spec, "npm_spec")?;
    let plugin_root = resolve_openclaw_plugin_workspace_root(app, &normalized_plugin_id)?;
    let workspace_dir = plugin_root.join("workspace");

    if workspace_dir.exists() {
        fs::remove_dir_all(&workspace_dir).map_err(|e| {
            format!(
                "failed to clean previous plugin workspace {}: {e}",
                workspace_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&workspace_dir).map_err(|e| {
        format!(
            "failed to create plugin workspace {}: {e}",
            workspace_dir.display()
        )
    })?;
    write_workspace_package_json(&workspace_dir, &normalized_plugin_id)?;

    let mut command = Command::new(resolve_npm_command());
    command
        .current_dir(&workspace_dir)
        .arg("install")
        .arg("--no-save")
        .arg("--no-package-lock")
        .arg(&normalized_npm_spec);
    apply_command_search_path(&mut command, &[]);
    hide_console_window(&mut command);
    let output = command
        .output()
        .map_err(|e| format!("failed to launch npm install for official plugin: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(format!("安装飞书官方插件失败: {detail}"));
    }

    let package_dir = resolve_installed_package_dir(&workspace_dir, &normalized_npm_spec)?;
    let package_json_path = package_dir.join("package.json");
    let package_json_text = fs::read_to_string(&package_json_path)
        .map_err(|e| format!("failed to read installed plugin package.json: {e}"))?;
    let package_json: serde_json::Value = serde_json::from_str(&package_json_text)
        .map_err(|e| format!("failed to parse installed plugin package.json: {e}"))?;
    let version = package_json
        .get("version")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "installed plugin package.json is missing version".to_string())?
        .to_string();
    let manifest_json = load_plugin_manifest_json(&package_dir, &package_json);

    upsert_openclaw_plugin_install_with_pool(
        pool,
        OpenClawPluginInstallInput {
            plugin_id: normalized_plugin_id,
            npm_spec: normalized_npm_spec,
            version,
            install_path: package_dir.to_string_lossy().to_string(),
            source_type: "npm".to_string(),
            manifest_json,
        },
    )
    .await
}
