use crate::runtime_environment::runtime_paths_from_app;
use crate::windows_process::hide_console_window;
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::AppHandle;

use super::{
    build_feishu_openclaw_config_with_pool, get_openclaw_plugin_install_by_id_with_pool,
    list_openclaw_plugin_installs_with_pool, normalize_required, FeishuPluginEnvironmentStatus,
    OpenClawPluginChannelHost, OpenClawPluginChannelInspection,
    OpenClawPluginChannelSnapshotResult, OpenClawPluginInspectionResult,
    OpenClawPluginInstallRecord, FEISHU_PLUGIN_MIN_NODE_MAJOR,
};

fn normalize_command_version_output(output: &[u8]) -> Option<String> {
    String::from_utf8_lossy(output)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

fn parse_node_major_version(version: &str) -> Option<u64> {
    let normalized = version.trim().trim_start_matches(['v', 'V']);
    let major = normalized
        .split('.')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    major.parse::<u64>().ok()
}

fn is_supported_feishu_host_node_version(version: &str) -> bool {
    parse_node_major_version(version)
        .map(|major| major >= FEISHU_PLUGIN_MIN_NODE_MAJOR)
        .unwrap_or(false)
}

pub(crate) fn ensure_supported_feishu_host_node_version() -> Result<String, String> {
    match probe_windows_node_version(&["--version"]) {
        Ok(Some(version)) if is_supported_feishu_host_node_version(&version) => Ok(version),
        Ok(Some(version)) => Err(format!(
            "已检测到 Node.js {version}，但飞书官方插件当前要求 Node.js >= v{}",
            FEISHU_PLUGIN_MIN_NODE_MAJOR
        )),
        Ok(None) => Err("未检测到 Node.js".to_string()),
        Err(error) => Err(format!("检测 Node.js 失败: {error}")),
    }
}

#[cfg(target_os = "windows")]
fn expand_windows_env_tokens(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            result.push(ch);
            continue;
        }
        let mut name = String::new();
        while let Some(next) = chars.peek().copied() {
            chars.next();
            if next == '%' {
                break;
            }
            name.push(next);
        }
        if name.is_empty() {
            result.push('%');
            continue;
        }
        if let Some(expanded) = std::env::var_os(&name) {
            result.push_str(&expanded.to_string_lossy());
        } else {
            result.push('%');
            result.push_str(&name);
            result.push('%');
        }
    }
    result
}

#[cfg(target_os = "windows")]
fn parse_windows_path_entries(raw: &str) -> Vec<PathBuf> {
    raw.split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(expand_windows_env_tokens)
        .filter(|entry| !entry.trim().is_empty())
        .map(PathBuf::from)
        .collect()
}

#[cfg(target_os = "windows")]
pub(crate) fn parse_windows_registry_path_output(output: &str) -> Vec<PathBuf> {
    for line in output.lines() {
        let trimmed = line.trim();
        if !trimmed.to_ascii_lowercase().starts_with("path") {
            continue;
        }
        let Some(type_start) = trimmed.find("REG_") else {
            continue;
        };
        let after_name = trimmed[type_start..].trim();
        let mut parts = after_name.splitn(2, char::is_whitespace);
        let _value_type = parts.next();
        let value = parts.next().unwrap_or("").trim();
        if value.is_empty() {
            return Vec::new();
        }
        return parse_windows_path_entries(value);
    }
    Vec::new()
}

#[cfg(target_os = "windows")]
fn read_windows_registry_path_entries(scope: &str) -> Vec<PathBuf> {
    let mut command = Command::new("reg");
    command.args(["query", scope, "/v", "Path"]);
    hide_console_window(&mut command);
    match command.output() {
        Ok(output) if output.status.success() => {
            parse_windows_registry_path_output(&String::from_utf8_lossy(&output.stdout))
        }
        _ => Vec::new(),
    }
}

fn dedupe_path_entries(entries: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries {
        let key = if cfg!(target_os = "windows") {
            entry.to_string_lossy().to_lowercase()
        } else {
            entry.to_string_lossy().to_string()
        };
        if seen.insert(key) {
            deduped.push(entry);
        }
    }
    deduped
}

pub(crate) fn build_effective_path_entries(
    current_path: Option<&OsStr>,
    prepend: &[PathBuf],
    extra_entries: &[PathBuf],
) -> Vec<PathBuf> {
    let current_entries = current_path
        .map(std::env::split_paths)
        .into_iter()
        .flatten()
        .filter(|entry| !entry.as_os_str().is_empty());
    dedupe_path_entries(
        prepend
            .iter()
            .cloned()
            .chain(current_entries)
            .chain(extra_entries.iter().cloned()),
    )
}

#[cfg(target_os = "windows")]
fn collect_windows_registry_path_entries() -> Vec<PathBuf> {
    dedupe_path_entries(
        read_windows_registry_path_entries(r"HKCU\Environment")
            .into_iter()
            .chain(read_windows_registry_path_entries(
                r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
            )),
    )
}

#[cfg(not(target_os = "windows"))]
fn collect_windows_registry_path_entries() -> Vec<PathBuf> {
    Vec::new()
}

fn effective_command_path_entries(prepend: &[PathBuf]) -> Vec<PathBuf> {
    build_effective_path_entries(
        std::env::var_os("PATH").as_deref(),
        prepend,
        &collect_windows_registry_path_entries(),
    )
}

pub(crate) fn apply_command_search_path(command: &mut Command, prepend: &[PathBuf]) {
    let entries = effective_command_path_entries(prepend);
    if entries.is_empty() {
        return;
    }
    if let Ok(joined) = std::env::join_paths(entries) {
        command.env("PATH", joined);
    }
}

fn probe_command_version_with_program(
    command: &Path,
    args: &[&str],
) -> Result<Option<String>, String> {
    let mut process = Command::new(command);
    process.args(args);
    apply_command_search_path(&mut process, &[]);
    hide_console_window(&mut process);
    match process.output() {
        Ok(output) => {
            if output.status.success() {
                Ok(normalize_command_version_output(&output.stdout)
                    .or_else(|| normalize_command_version_output(&output.stderr)))
            } else {
                let detail = normalize_command_version_output(&output.stderr)
                    .or_else(|| normalize_command_version_output(&output.stdout))
                    .unwrap_or_else(|| {
                        format!("{} exited with status {}", command.display(), output.status)
                    });
                Err(detail)
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn probe_command_version(command: &str, args: &[&str]) -> Result<Option<String>, String> {
    probe_command_version_with_program(Path::new(command), args)
}

#[cfg(target_os = "windows")]
pub(crate) fn collect_windows_node_command_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(PathBuf::from("node"));
    candidates.push(PathBuf::from("node.exe"));

    for key in ["NVM_SYMLINK", "NVM_HOME"] {
        if let Some(value) = std::env::var_os(key) {
            let base = PathBuf::from(value);
            if !base.as_os_str().is_empty() {
                candidates.push(base.join("node.exe"));
            }
        }
    }

    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        candidates.push(PathBuf::from(program_files).join("nodejs").join("node.exe"));
    }
    if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
        candidates.push(
            PathBuf::from(program_files_x86)
                .join("nodejs")
                .join("node.exe"),
        );
    }
    if let Some(local_app_data) = std::env::var_os("LocalAppData") {
        candidates.push(
            PathBuf::from(local_app_data)
                .join("Programs")
                .join("nodejs")
                .join("node.exe"),
        );
    }

    for entry in effective_command_path_entries(&[]) {
        if entry.as_os_str().is_empty() {
            continue;
        }
        candidates.push(entry.join("node.exe"));
        candidates.push(entry.join("node"));
    }

    dedupe_path_entries(candidates)
}

#[cfg(target_os = "windows")]
fn probe_windows_node_version(args: &[&str]) -> Result<Option<String>, String> {
    let mut last_error = None;
    for candidate in collect_windows_node_command_candidates() {
        match probe_command_version_with_program(&candidate, args) {
            Ok(Some(version)) => return Ok(Some(version)),
            Ok(None) => continue,
            Err(error) => {
                last_error = Some(format!("{}: {error}", candidate.display()));
            }
        }
    }
    if let Some(error) = last_error {
        Err(error)
    } else {
        Ok(None)
    }
}

#[cfg(not(target_os = "windows"))]
fn probe_windows_node_version(args: &[&str]) -> Result<Option<String>, String> {
    probe_command_version("node", args)
}

pub(crate) fn derive_feishu_plugin_environment_status(
    node_probe: Result<Option<String>, String>,
    npm_probe: Result<Option<String>, String>,
    runtime_script_exists: bool,
) -> FeishuPluginEnvironmentStatus {
    let mut status = FeishuPluginEnvironmentStatus::default();
    let mut errors = Vec::new();

    match node_probe {
        Ok(version) => {
            status.node_available = version.is_some();
            status.node_version_supported = version
                .as_deref()
                .map(is_supported_feishu_host_node_version)
                .unwrap_or(false);
            status.node_version = version;
            if !status.node_available {
                errors.push("未检测到 Node.js".to_string());
            } else if !status.node_version_supported {
                let version_label = status.node_version.as_deref().unwrap_or("unknown");
                errors.push(format!(
                    "已检测到 Node.js {version_label}，但飞书官方插件当前要求 Node.js >= v{}",
                    FEISHU_PLUGIN_MIN_NODE_MAJOR
                ));
            }
        }
        Err(error) => {
            errors.push(format!("检测 Node.js 失败: {error}"));
        }
    }

    match npm_probe {
        Ok(version) => {
            status.npm_available = version.is_some();
            status.npm_version = version;
            if !status.npm_available {
                errors.push("未检测到 npm".to_string());
            }
        }
        Err(error) => {
            errors.push(format!("检测 npm 失败: {error}"));
        }
    }

    if !runtime_script_exists {
        errors.push("飞书插件运行脚本缺失".to_string());
    }

    status.can_install_plugin =
        status.node_available && status.node_version_supported && status.npm_available;
    status.can_start_runtime =
        status.node_available && status.node_version_supported && runtime_script_exists;
    status.error = if errors.is_empty() {
        None
    } else {
        Some(errors.join("；"))
    };
    status
}

pub(crate) fn get_feishu_plugin_environment_status_internal() -> FeishuPluginEnvironmentStatus {
    derive_feishu_plugin_environment_status(
        probe_windows_node_version(&["--version"]),
        probe_command_version(resolve_npm_command(), &["--version"]),
        resolve_plugin_host_run_feishu_script().exists(),
    )
}

#[cfg(target_os = "windows")]
pub(crate) fn resolve_npm_command() -> &'static str {
    "npm.cmd"
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn resolve_npm_command() -> &'static str {
    "npm"
}

#[cfg(target_os = "windows")]
pub(crate) fn resolve_npx_command() -> &'static str {
    "npx.cmd"
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn resolve_npx_command() -> &'static str {
    "npx"
}

pub(crate) fn build_openclaw_lark_tools_npx_args(version: Option<&str>) -> Vec<String> {
    let package_spec = version
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("@larksuite/openclaw-lark@{value}"))
        .unwrap_or_else(|| "@larksuite/openclaw-lark".to_string());

    vec![
        "-y".to_string(),
        package_spec,
        "install".to_string(),
        "--debug".to_string(),
    ]
}

#[cfg(target_os = "windows")]
pub(crate) fn resolve_windows_node_command_path() -> Result<PathBuf, String> {
    let mut last_error = None;
    for candidate in collect_windows_node_command_candidates() {
        match probe_command_version_with_program(&candidate, &["--version"]) {
            Ok(Some(_)) => return Ok(candidate),
            Ok(None) => continue,
            Err(error) => {
                last_error = Some(format!("{}: {error}", candidate.display()));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "failed to resolve Node.js command path".to_string()))
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn resolve_windows_node_command_path() -> Result<PathBuf, String> {
    Ok(PathBuf::from("node"))
}

pub(crate) fn append_disable_dep0190_node_option(command: &mut Command) {
    let option = "--disable-warning=DEP0190";
    let merged = match command.get_envs().find_map(|(key, value)| {
        if key == OsStr::new("NODE_OPTIONS") {
            value.and_then(|entry| entry.to_str()).map(str::to_string)
        } else {
            None
        }
    }) {
        Some(existing) if existing.split_whitespace().any(|part| part == option) => existing,
        Some(existing) if existing.trim().is_empty() => option.to_string(),
        Some(existing) => format!("{existing} {option}"),
        None => option.to_string(),
    };
    command.env("NODE_OPTIONS", merged);
}

pub(crate) fn resolve_openclaw_plugin_workspace_root(
    app: &AppHandle,
    plugin_id: &str,
) -> Result<PathBuf, String> {
    let runtime_paths = runtime_paths_from_app(app)?;
    let normalized = normalize_required(plugin_id, "plugin_id")?;
    Ok(runtime_paths.plugins.root.join(normalized))
}

pub(crate) fn resolve_plugin_host_dir() -> PathBuf {
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .to_path_buf();

    fn packaged_plugin_host_candidates(base_dir: &Path) -> Vec<PathBuf> {
        vec![
            base_dir.join("resources").join("plugin-host"),
            base_dir.join("_up_").join("plugin-host"),
            base_dir.join("plugin-host"),
        ]
    }

    let dev_dir = manifest_root.join("plugin-host");
    if dev_dir.exists() {
        return dev_dir;
    }

    let mut candidates = Vec::new();
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.extend(packaged_plugin_host_candidates(exe_dir));
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.extend(packaged_plugin_host_candidates(&cwd));
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .unwrap_or_else(|| manifest_root.join("plugin-host"))
}

pub(crate) fn resolve_plugin_host_inspect_script() -> PathBuf {
    resolve_plugin_host_dir()
        .join("scripts")
        .join("inspect-plugin.mjs")
}

pub(crate) fn resolve_plugin_host_run_feishu_script() -> PathBuf {
    resolve_plugin_host_dir()
        .join("scripts")
        .join("run-feishu-host.mjs")
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn build_plugin_host_fixture_root_from_runtime_root(runtime_root: &Path) -> PathBuf {
    runtime_root.join("plugin-host-fixtures")
}

pub(crate) fn resolve_plugin_host_fixture_root(app: &AppHandle) -> Result<PathBuf, String> {
    let runtime_paths = runtime_paths_from_app(app)?;
    Ok(runtime_paths.plugins.fixture_dir)
}

pub(crate) fn derive_channel_capabilities(
    channel: &OpenClawPluginChannelInspection,
) -> Vec<String> {
    let mut capabilities = Vec::new();
    let record = channel
        .capabilities
        .as_ref()
        .and_then(|value| value.as_object());

    if let Some(chat_types) = record
        .and_then(|capabilities| capabilities.get("chatTypes"))
        .and_then(|value| value.as_array())
    {
        for chat_type in chat_types.iter().filter_map(|value| value.as_str()) {
            capabilities.push(format!("chat_type:{chat_type}"));
        }
    }

    for (key, tag) in [
        ("media", "media"),
        ("reactions", "reactions"),
        ("threads", "threads"),
        ("polls", "polls"),
        ("nativeCommands", "native_commands"),
        ("blockStreaming", "block_streaming"),
    ] {
        if record
            .and_then(|capabilities| capabilities.get(key))
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            capabilities.push(tag.to_string());
        }
    }

    if channel.has_pairing {
        capabilities.push("pairing".to_string());
    }
    if channel.has_setup {
        capabilities.push("setup".to_string());
    }
    if channel.has_onboarding {
        capabilities.push("onboarding".to_string());
    }
    if channel.has_directory {
        capabilities.push("directory".to_string());
    }
    if channel.has_outbound {
        capabilities.push("outbound".to_string());
    }
    if channel.has_threading {
        capabilities.push("threading".to_string());
    }
    if channel.has_actions {
        capabilities.push("actions".to_string());
    }
    if channel.has_status {
        capabilities.push("status".to_string());
    }

    capabilities.sort();
    capabilities.dedup();
    capabilities
}

fn inspection_to_channel_hosts(
    install: &OpenClawPluginInstallRecord,
    inspection: &OpenClawPluginInspectionResult,
) -> Vec<OpenClawPluginChannelHost> {
    inspection
        .summary
        .channels
        .iter()
        .map(|channel| OpenClawPluginChannelHost {
            plugin_id: install.plugin_id.clone(),
            npm_spec: install.npm_spec.clone(),
            version: install.version.clone(),
            channel: channel
                .id
                .clone()
                .or_else(|| channel.meta.as_ref().and_then(|meta| meta.id.clone()))
                .unwrap_or_else(|| "unknown".to_string()),
            display_name: channel
                .meta
                .as_ref()
                .and_then(|meta| meta.label.clone())
                .or_else(|| channel.id.clone())
                .unwrap_or_else(|| install.plugin_id.clone()),
            capabilities: derive_channel_capabilities(channel),
            reload_config_prefixes: channel.reload_config_prefixes.clone(),
            target_hint: channel.target_hint.clone(),
            docs_path: channel
                .meta
                .as_ref()
                .and_then(|meta| meta.docs_path.clone()),
            status: "ready".to_string(),
            error: None,
        })
        .collect()
}

async fn inspect_openclaw_plugin_with_pool_and_app(
    pool: &SqlitePool,
    plugin_id: &str,
    app: Option<&AppHandle>,
) -> Result<OpenClawPluginInspectionResult, String> {
    let install = get_openclaw_plugin_install_by_id_with_pool(pool, plugin_id).await?;
    let script_path = resolve_plugin_host_inspect_script();
    if !script_path.exists() {
        return Err(format!(
            "plugin host inspect script is missing: {}",
            script_path.display()
        ));
    }

    let plugin_host_dir = resolve_plugin_host_dir();
    let mut command = Command::new("node");
    command
        .current_dir(&plugin_host_dir)
        .arg(script_path)
        .arg("--plugin-root")
        .arg(&install.install_path)
        .arg("--fixture-name")
        .arg(&install.plugin_id);
    if let Some(app) = app {
        command
            .arg("--fixture-root")
            .arg(resolve_plugin_host_fixture_root(app)?);
    }
    apply_command_search_path(&mut command, &[]);
    hide_console_window(&mut command);
    let output = command
        .output()
        .map_err(|e| format!("failed to launch plugin host inspect script: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(format!("plugin host inspect failed: {detail}"));
    }

    serde_json::from_slice::<OpenClawPluginInspectionResult>(&output.stdout)
        .map_err(|e| format!("failed to parse plugin host inspect json: {e}"))
}

#[cfg_attr(not(test), allow(dead_code))]
pub async fn inspect_openclaw_plugin_with_pool(
    pool: &SqlitePool,
    plugin_id: &str,
) -> Result<OpenClawPluginInspectionResult, String> {
    inspect_openclaw_plugin_with_pool_and_app(pool, plugin_id, None).await
}

pub(crate) async fn inspect_openclaw_plugin_with_pool_and_app_public(
    pool: &SqlitePool,
    plugin_id: &str,
    app: Option<&AppHandle>,
) -> Result<OpenClawPluginInspectionResult, String> {
    inspect_openclaw_plugin_with_pool_and_app(pool, plugin_id, app).await
}

async fn get_openclaw_plugin_feishu_channel_snapshot_with_pool_and_app(
    pool: &SqlitePool,
    plugin_id: &str,
    app: Option<&AppHandle>,
) -> Result<OpenClawPluginChannelSnapshotResult, String> {
    let install = get_openclaw_plugin_install_by_id_with_pool(pool, plugin_id).await?;
    let config_json = build_feishu_openclaw_config_with_pool(pool).await?;
    let script_path = resolve_plugin_host_inspect_script();
    if !script_path.exists() {
        return Err(format!(
            "plugin host inspect script is missing: {}",
            script_path.display()
        ));
    }

    let plugin_host_dir = resolve_plugin_host_dir();
    let mut command = Command::new("node");
    command
        .current_dir(&plugin_host_dir)
        .arg(script_path)
        .arg("--plugin-root")
        .arg(&install.install_path)
        .arg("--fixture-name")
        .arg(format!("{}-feishu-snapshot", install.plugin_id))
        .arg("--channel-snapshot")
        .arg("feishu")
        .arg("--config-json")
        .arg(config_json.to_string());
    if let Some(app) = app {
        command
            .arg("--fixture-root")
            .arg(resolve_plugin_host_fixture_root(app)?);
    }
    apply_command_search_path(&mut command, &[]);
    hide_console_window(&mut command);
    let output = command
        .output()
        .map_err(|e| format!("failed to launch plugin host snapshot script: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(format!("plugin host channel snapshot failed: {detail}"));
    }

    serde_json::from_slice::<OpenClawPluginChannelSnapshotResult>(&output.stdout)
        .map_err(|e| format!("failed to parse plugin host channel snapshot json: {e}"))
}

pub async fn get_openclaw_plugin_feishu_channel_snapshot_with_pool(
    pool: &SqlitePool,
    plugin_id: &str,
) -> Result<OpenClawPluginChannelSnapshotResult, String> {
    get_openclaw_plugin_feishu_channel_snapshot_with_pool_and_app(pool, plugin_id, None).await
}

pub(crate) async fn get_openclaw_plugin_feishu_channel_snapshot_with_pool_and_app_public(
    pool: &SqlitePool,
    plugin_id: &str,
    app: Option<&AppHandle>,
) -> Result<OpenClawPluginChannelSnapshotResult, String> {
    get_openclaw_plugin_feishu_channel_snapshot_with_pool_and_app(pool, plugin_id, app).await
}

async fn list_openclaw_plugin_channel_hosts_with_pool_and_app(
    pool: &SqlitePool,
    app: Option<&AppHandle>,
) -> Result<Vec<OpenClawPluginChannelHost>, String> {
    let installs = list_openclaw_plugin_installs_with_pool(pool).await?;
    let mut hosts = Vec::new();

    for install in installs {
        match inspect_openclaw_plugin_with_pool_and_app(pool, &install.plugin_id, app).await {
            Ok(inspection) => {
                hosts.extend(inspection_to_channel_hosts(&install, &inspection));
            }
            Err(error) => {
                hosts.push(OpenClawPluginChannelHost {
                    plugin_id: install.plugin_id.clone(),
                    npm_spec: install.npm_spec.clone(),
                    version: install.version.clone(),
                    channel: install.plugin_id.clone(),
                    display_name: install.plugin_id.clone(),
                    capabilities: Vec::new(),
                    reload_config_prefixes: Vec::new(),
                    target_hint: None,
                    docs_path: None,
                    status: "error".to_string(),
                    error: Some(error),
                });
            }
        }
    }

    Ok(hosts)
}

#[cfg_attr(not(test), allow(dead_code))]
pub async fn list_openclaw_plugin_channel_hosts_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<OpenClawPluginChannelHost>, String> {
    list_openclaw_plugin_channel_hosts_with_pool_and_app(pool, None).await
}

pub(crate) async fn list_openclaw_plugin_channel_hosts_with_pool_and_app_public(
    pool: &SqlitePool,
    app: Option<&AppHandle>,
) -> Result<Vec<OpenClawPluginChannelHost>, String> {
    list_openclaw_plugin_channel_hosts_with_pool_and_app(pool, app).await
}
