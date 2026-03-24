use sqlx::SqlitePool;
use std::fs;
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::AppHandle;

use super::{
    append_disable_dep0190_node_option, build_openclaw_lark_tools_npx_args,
    build_openclaw_shim_state_file_path, ensure_controlled_openclaw_state_projection,
    get_openclaw_plugin_install_by_id_with_pool, hide_console_window, now_rfc3339,
    resolve_controlled_openclaw_state_root, resolve_npx_command, resolve_openclaw_shim_root,
    resolve_windows_node_command_path, OpenClawLarkInstallerAutoInputState,
    OpenClawLarkInstallerMode, OpenClawLarkInstallerSessionState,
    OpenClawLarkInstallerSessionStatus, OPENCLAW_SHIM_VERSION,
};

pub(crate) fn build_openclaw_shim_script(state_file: &Path) -> String {
    let state_file_str = state_file.to_string_lossy().replace('\\', "\\\\");
    format!(
        r#"#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const stateFile = process.env.WORKCLAW_OPENCLAW_SHIM_STATE_FILE || "{state_file_str}";
const version = process.env.WORKCLAW_OPENCLAW_SHIM_VERSION || "{OPENCLAW_SHIM_VERSION}";

function loadState() {{
  if (!fs.existsSync(stateFile)) {{
    return {{ config: {{}}, commands: [] }};
  }}

  try {{
    const raw = fs.readFileSync(stateFile, "utf8").trim();
    if (!raw) {{
      return {{ config: {{}}, commands: [] }};
    }}
    const parsed = JSON.parse(raw);
    return {{
      config: parsed && typeof parsed.config === "object" && parsed.config ? parsed.config : {{}},
      commands: Array.isArray(parsed?.commands) ? parsed.commands : [],
    }};
  }} catch (_error) {{
    return {{ config: {{}}, commands: [] }};
  }}
}}

function saveState(state) {{
  fs.mkdirSync(path.dirname(stateFile), {{ recursive: true }});
  fs.writeFileSync(stateFile, JSON.stringify(state, null, 2));
}}

function getPathValue(root, pathParts) {{
  let current = root;
  for (const part of pathParts) {{
    if (!current || typeof current !== "object" || !(part in current)) {{
      return undefined;
    }}
    current = current[part];
  }}
  return current;
}}

function setPathValue(root, pathParts, value) {{
  let current = root;
  for (let index = 0; index < pathParts.length - 1; index += 1) {{
    const part = pathParts[index];
    if (!current[part] || typeof current[part] !== "object") {{
      current[part] = {{}};
    }}
    current = current[part];
  }}
  current[pathParts[pathParts.length - 1]] = value;
}}

function parseValue(raw, useJson) {{
  if (!useJson) {{
    return raw;
  }}
  return JSON.parse(raw);
}}

function recordCommand(state, args) {{
  state.commands.push({{ at: new Date().toISOString(), args }});
  if (state.commands.length > 50) {{
    state.commands = state.commands.slice(-50);
  }}
}}

const args = process.argv.slice(2);
const state = loadState();

if (args.length === 0 || args[0] === "-v" || args[0] === "--version" || args[0] === "version") {{
  console.log(version);
  process.exit(0);
}}

if (args[0] === "config" && args[1] === "get" && typeof args[2] === "string") {{
  const value = getPathValue(state.config, args[2].split("."));
  if (value === undefined) {{
    process.exit(0);
  }}
  if (typeof value === "string") {{
    console.log(value);
  }} else {{
    console.log(JSON.stringify(value));
  }}
  process.exit(0);
}}

if (args[0] === "config" && args[1] === "set" && typeof args[2] === "string" && typeof args[3] === "string") {{
  const useJson = args.includes("--json");
  try {{
    setPathValue(state.config, args[2].split("."), parseValue(args[3], useJson));
    recordCommand(state, args);
    saveState(state);
    console.log(`updated ${{args[2]}}`);
    process.exit(0);
  }} catch (error) {{
    console.error(`[workclaw-openclaw-shim] failed to parse value: ${{error instanceof Error ? error.message : String(error)}}`);
    process.exit(1);
  }}
}}

if (args[0] === "gateway" && (args[1] === "restart" || args[1] === "start" || args[1] === "stop")) {{
  recordCommand(state, args);
  saveState(state);
  console.log(`gateway ${{args[1]}} requested via WorkClaw shim`);
  process.exit(0);
}}

if ((args[0] === "plugins" || args[0] === "plugin") && (args[1] === "install" || args[1] === "uninstall") && typeof args[2] === "string") {{
  recordCommand(state, args);
  saveState(state);
  console.log(`plugin ${{args[1]}} satisfied via WorkClaw shim: ${{args[2]}}`);
  process.exit(0);
}}

if (args[0] === "pairing" && args[1] === "approve" && typeof args[2] === "string" && typeof args[3] === "string") {{
  recordCommand(state, args);
  saveState(state);
  console.log(`pairing approved for ${{args[2]}} ${{args[3]}}`);
  process.exit(0);
}}

console.error(`[workclaw-openclaw-shim] unsupported command: ${{args.join(" ")}}`);
process.exit(2);
"#
    )
}

pub(crate) fn ensure_openclaw_cli_shim(shim_root: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(shim_root)
        .map_err(|e| format!("failed to create openclaw shim dir: {e}"))?;

    let state_file = build_openclaw_shim_state_file_path(shim_root);
    if !state_file.exists() {
        fs::write(&state_file, "{\n  \"config\": {},\n  \"commands\": []\n}")
            .map_err(|e| format!("failed to initialize openclaw shim state: {e}"))?;
    }

    let script_path = shim_root.join("openclaw-shim.mjs");
    fs::write(&script_path, build_openclaw_shim_script(&state_file))
        .map_err(|e| format!("failed to write openclaw shim script: {e}"))?;

    #[cfg(windows)]
    {
        let cmd_path = shim_root.join("openclaw.cmd");
        let cmd_contents = format!(
            "@echo off\r\n\"{}\" \"{}\" %*\r\n",
            "node",
            script_path.display()
        );
        fs::write(&cmd_path, cmd_contents)
            .map_err(|e| format!("failed to write openclaw shim cmd wrapper: {e}"))?;
    }

    #[cfg(not(windows))]
    {
        let shell_path = shim_root.join("openclaw");
        let shell_contents = format!(
            "#!/usr/bin/env sh\nnode \"{}\" \"$@\"\n",
            script_path.display()
        );
        fs::write(&shell_path, shell_contents)
            .map_err(|e| format!("failed to write openclaw shim wrapper: {e}"))?;
        let mut permissions = fs::metadata(&shell_path)
            .map_err(|e| format!("failed to read openclaw shim wrapper metadata: {e}"))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&shell_path, permissions)
            .map_err(|e| format!("failed to mark openclaw shim wrapper executable: {e}"))?;
    }

    Ok(shim_root.to_path_buf())
}

pub(crate) fn prepend_env_path(command: &mut Command, shim_dir: &Path) {
    super::apply_command_search_path(command, &[shim_dir.to_path_buf()]);
}

fn push_installer_output(status: &mut OpenClawLarkInstallerSessionStatus, line: &str) {
    status.recent_output.push(line.to_string());
    if status.recent_output.len() > 200 {
        let overflow = status.recent_output.len() - 200;
        status.recent_output.drain(0..overflow);
    }
    status.last_output_at = Some(now_rfc3339());
}

pub(crate) fn infer_installer_prompt_hint(line: &str) -> Option<String> {
    let normalized = line.to_lowercase();
    if normalized.contains("what would you like to do") || line.contains("请选择操作") {
        return Some("请选择“新建机器人”或“关联已有机器人”".to_string());
    }
    if normalized.contains("enter your app id") || line.contains("请输入 App ID") {
        return Some("请输入机器人 App ID".to_string());
    }
    if normalized.contains("enter your app secret") || line.contains("请输入 App Secret") {
        return Some("请输入机器人 App Secret".to_string());
    }
    if normalized.contains("scan with feishu to create your bot") || line.contains("扫码") {
        return Some("请使用飞书扫码完成机器人创建".to_string());
    }
    if normalized.contains("fetching configuration results")
        || line.contains("正在获取你的机器人配置结果")
    {
        return Some("正在等待飞书官方接口返回机器人 App ID / App Secret，请稍候。".to_string());
    }
    if normalized.contains("[debug] poll result:") && normalized.contains("authorization_pending") {
        return Some(
            "飞书官方接口仍在等待这次扫码配置完成回传结果（authorization_pending）。".to_string(),
        );
    }
    if normalized.contains("[debug] poll result:") && normalized.contains("slow_down") {
        return Some("飞书官方接口要求放慢轮询频率，仍在继续等待配置结果。".to_string());
    }
    if normalized.contains("[debug] poll result:") && normalized.contains("expired_token") {
        return Some("这次扫码会话已过期，请重新启动新建机器人向导。".to_string());
    }
    if normalized.contains("[debug] poll result:") && normalized.contains("access_denied") {
        return Some("飞书端已拒绝本次授权，请重新发起新建机器人向导。".to_string());
    }
    None
}

pub(crate) fn derive_installer_auto_input(
    mode: &OpenClawLarkInstallerMode,
    app_id: Option<&str>,
    app_secret: Option<&str>,
    line: &str,
    auto: &mut OpenClawLarkInstallerAutoInputState,
) -> Option<String> {
    let normalized = line.to_lowercase();
    let has_choice_prompt =
        normalized.contains("what would you like to do") || line.contains("请选择操作");
    if has_choice_prompt && !auto.selection_sent {
        auto.selection_sent = true;
        return Some(match mode {
            OpenClawLarkInstallerMode::Create => "\r".to_string(),
            OpenClawLarkInstallerMode::Link => "\u{1b}[B\r".to_string(),
        });
    }

    let has_app_id_prompt =
        normalized.contains("enter your app id") || line.contains("请输入 App ID");
    if has_app_id_prompt {
        if let Some(value) = app_id.filter(|value| !value.trim().is_empty()) {
            auto.app_id_sent = true;
            return Some(format!("{}\r", value.trim()));
        }
    }

    let has_app_secret_prompt =
        normalized.contains("enter your app secret") || line.contains("请输入 App Secret");
    if has_app_secret_prompt {
        if let Some(value) = app_secret.filter(|value| !value.trim().is_empty()) {
            auto.app_secret_sent = true;
            return Some(format!("{}\r", value.trim()));
        }
    }

    None
}

pub(crate) fn current_openclaw_lark_installer_session_status(
    state: &OpenClawLarkInstallerSessionState,
) -> OpenClawLarkInstallerSessionStatus {
    state
        .0
        .lock()
        .map(|guard| guard.status.clone())
        .unwrap_or_default()
}

pub(crate) fn stop_openclaw_lark_installer_session_in_state(
    state: &OpenClawLarkInstallerSessionState,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    let (process, stdin) = {
        let mut guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock installer session state".to_string())?;
        (guard.process.take(), guard.stdin.take())
    };

    drop(stdin);

    if let Some(slot) = process {
        if let Ok(mut child_guard) = slot.lock() {
            if let Some(mut child) = child_guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    let mut guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock installer session state".to_string())?;
    guard.status.running = false;
    if guard.status.started_at.is_some() {
        guard.status.last_output_at = Some(now_rfc3339());
    }
    guard.status.prompt_hint = None;
    Ok(guard.status.clone())
}

pub(crate) async fn start_openclaw_lark_installer_session_with_pool(
    pool: &SqlitePool,
    state: &OpenClawLarkInstallerSessionState,
    mode: OpenClawLarkInstallerMode,
    app_id: Option<&str>,
    app_secret: Option<&str>,
    app: &AppHandle,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    let _ = stop_openclaw_lark_installer_session_in_state(state);

    let install = get_openclaw_plugin_install_by_id_with_pool(pool, "openclaw-lark").await?;
    let plugin_install_path = Path::new(&install.install_path);

    let shim_root = resolve_openclaw_shim_root(app)?;
    let shim_dir = ensure_openclaw_cli_shim(&shim_root)?;
    let controlled_openclaw_state_root = resolve_controlled_openclaw_state_root(app)?;
    ensure_controlled_openclaw_state_projection(
        &controlled_openclaw_state_root,
        plugin_install_path,
    )?;

    let installer_args = build_openclaw_lark_tools_npx_args(None);
    #[cfg(target_os = "windows")]
    let mut command = {
        let node_program = resolve_windows_node_command_path()?;
        let npx_cli = node_program
            .parent()
            .map(|parent: &Path| {
                parent
                    .join("node_modules")
                    .join("npm")
                    .join("bin")
                    .join("npx-cli.js")
            })
            .filter(|candidate: &PathBuf| candidate.exists());
        if let Some(npx_cli) = npx_cli {
            let mut command = Command::new(&node_program);
            command.arg(npx_cli).args(&installer_args);
            append_disable_dep0190_node_option(&mut command);
            command
        } else {
            let mut command = Command::new(resolve_npx_command());
            command.args(&installer_args);
            command
        }
    };
    #[cfg(not(target_os = "windows"))]
    let mut command = {
        let mut command = Command::new(resolve_npx_command());
        command.args(&installer_args);
        command
    };
    command
        .current_dir(plugin_install_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console_window(&mut command);
    prepend_env_path(&mut command, &shim_dir);
    command
        .env(
            "WORKCLAW_OPENCLAW_SHIM_STATE_FILE",
            build_openclaw_shim_state_file_path(&shim_dir),
        )
        .env("WORKCLAW_OPENCLAW_SHIM_VERSION", OPENCLAW_SHIM_VERSION)
        .env("OPENCLAW_STATE_DIR", &controlled_openclaw_state_root);

    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to launch official installer: {e}"))?;
    let pid = child.id();
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "failed to capture official installer stdin".to_string())?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let child_slot = Arc::new(Mutex::new(Some(child)));
    let stdin_slot = Arc::new(Mutex::new(stdin));

    {
        let mut guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock installer session state".to_string())?;
        guard.process = Some(child_slot.clone());
        guard.stdin = Some(stdin_slot.clone());
        guard.auto = OpenClawLarkInstallerAutoInputState::default();
        guard.app_id = app_id.map(str::to_string);
        guard.app_secret = app_secret.map(str::to_string);
        guard.status = OpenClawLarkInstallerSessionStatus {
            running: true,
            mode: Some(mode.clone()),
            started_at: Some(now_rfc3339()),
            last_output_at: None,
            last_error: None,
            prompt_hint: Some("正在启动飞书官方安装向导".to_string()),
            recent_output: vec![format!(
                "[system] official installer started (pid={pid}, mode={})",
                match mode {
                    OpenClawLarkInstallerMode::Create => "create",
                    OpenClawLarkInstallerMode::Link => "link",
                }
            )],
        };
    }

    if let Some(stdout) = stdout {
        let state_clone = state.clone();
        let stdin_clone = stdin_slot.clone();
        let mode_clone = mode.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim_end();
                if trimmed.trim().is_empty() {
                    continue;
                }
                let auto_input = {
                    let mut maybe_auto_input = None;
                    if let Ok(mut guard) = state_clone.0.lock() {
                        push_installer_output(&mut guard.status, trimmed);
                        guard.status.prompt_hint = infer_installer_prompt_hint(trimmed);
                        let app_id = guard.app_id.clone();
                        let app_secret = guard.app_secret.clone();
                        maybe_auto_input = derive_installer_auto_input(
                            &mode_clone,
                            app_id.as_deref(),
                            app_secret.as_deref(),
                            trimmed,
                            &mut guard.auto,
                        );
                        if let Some(ref payload) = maybe_auto_input {
                            let display = payload
                                .replace('\r', "\\r")
                                .replace('\n', "\\n")
                                .replace('\u{1b}', "\\u001b");
                            push_installer_output(
                                &mut guard.status,
                                &format!("[auto-input] {display}"),
                            );
                        }
                    }
                    maybe_auto_input
                };

                if let Some(payload) = auto_input {
                    if let Ok(mut stdin_guard) = stdin_clone.lock() {
                        let _ = stdin_guard.write_all(payload.as_bytes());
                        let _ = stdin_guard.flush();
                    }
                }
            }
        });
    }

    if let Some(stderr) = stderr {
        let state_clone = state.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim_end();
                if trimmed.trim().is_empty() {
                    continue;
                }
                eprintln!("[openclaw-lark-installer] {}", trimmed);
                if let Ok(mut guard) = state_clone.0.lock() {
                    guard.status.last_error = Some(trimmed.to_string());
                    push_installer_output(&mut guard.status, &format!("[stderr] {trimmed}"));
                }
            }
        });
    }

    {
        let state_clone = state.clone();
        let child_slot_clone = child_slot.clone();
        thread::spawn(move || loop {
            let exit_status = {
                let mut child_guard = match child_slot_clone.lock() {
                    Ok(guard) => guard,
                    Err(_) => break,
                };
                if let Some(child) = child_guard.as_mut() {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            let success = status.success();
                            let code = status.code();
                            *child_guard = None;
                            Some((success, code, None::<String>))
                        }
                        Ok(None) => None,
                        Err(error) => {
                            *child_guard = None;
                            Some((false, Some(-1), Some(error.to_string())))
                        }
                    }
                } else {
                    break;
                }
            };

            match exit_status {
                Some((success, code, command_error)) => {
                    if let Ok(mut guard) = state_clone.0.lock() {
                        guard.process = None;
                        guard.stdin = None;
                        guard.status.running = false;
                        guard.status.prompt_hint = None;
                        let final_line = if success {
                            "[system] official installer finished".to_string()
                        } else if let Some(error) = command_error {
                            guard.status.last_error =
                                Some(format!("official installer wait failed: {error}"));
                            format!("[system] official installer failed: {error}")
                        } else {
                            let message = match code {
                                Some(value) if value >= 0 => {
                                    format!("official installer exited with code {value}")
                                }
                                _ => "official installer exited unexpectedly".to_string(),
                            };
                            if guard
                                .status
                                .last_error
                                .as_deref()
                                .unwrap_or("")
                                .trim()
                                .is_empty()
                            {
                                guard.status.last_error = Some(message.clone());
                            }
                            format!("[system] {message}")
                        };
                        push_installer_output(&mut guard.status, &final_line);
                    }
                    break;
                }
                None => thread::sleep(Duration::from_millis(250)),
            }
        });
    }

    Ok(current_openclaw_lark_installer_session_status(state))
}

pub(crate) fn send_openclaw_lark_installer_input_in_state(
    state: &OpenClawLarkInstallerSessionState,
    input: &str,
) -> Result<OpenClawLarkInstallerSessionStatus, String> {
    let payload = if input.ends_with('\n') || input.ends_with('\r') {
        input.to_string()
    } else {
        format!("{input}\r")
    };

    let stdin = {
        let guard = state
            .0
            .lock()
            .map_err(|_| "failed to lock installer session state".to_string())?;
        guard
            .stdin
            .clone()
            .ok_or_else(|| "official installer is not accepting input".to_string())?
    };

    {
        let mut stdin_guard = stdin
            .lock()
            .map_err(|_| "failed to lock installer stdin".to_string())?;
        stdin_guard
            .write_all(payload.as_bytes())
            .map_err(|e| format!("failed to send installer input: {e}"))?;
        stdin_guard
            .flush()
            .map_err(|e| format!("failed to flush installer input: {e}"))?;
    }

    let mut guard = state
        .0
        .lock()
        .map_err(|_| "failed to lock installer session state".to_string())?;
    push_installer_output(
        &mut guard.status,
        &format!("[manual-input] {}", input.trim_end()),
    );
    Ok(guard.status.clone())
}
