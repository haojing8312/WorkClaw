use super::types::AUTOSTART_NAME;
use crate::windows_process::hide_console_window;
use std::env;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::fmt::Write as FmtWrite;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::fs::{self, File};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::io::ErrorKind;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::path::PathBuf;
use std::process::Command;
use tauri::AppHandle;

#[cfg(target_os = "windows")]
fn format_windows_command_failure(action: &str, output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let details = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("退出码 {:?}", output.status.code())
    };
    format!("{action}失败: {details}")
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn resolve_home_dir() -> Result<PathBuf, String> {
    if let Some(home) = env::var_os("HOME") {
        return Ok(PathBuf::from(home));
    }

    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .ok_or_else(|| "无法解析用户主目录".to_string())
}

pub fn sync_launch_at_login(_app: &AppHandle, enabled: bool) -> Result<(), String> {
    let exe_path = env::current_exe().map_err(|e| format!("读取当前可执行文件路径失败: {e}"))?;

    if exe_path.to_string_lossy().is_empty() {
        return Err("可执行文件路径为空，无法设置开机启动".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        let quoted = format!("\"{}\"", exe_path.to_string_lossy());
        if enabled {
            let mut command = Command::new("reg");
            command.args([
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                AUTOSTART_NAME,
                "/t",
                "REG_SZ",
                "/d",
                quoted.as_str(),
                "/f",
            ]);
            hide_console_window(&mut command);
            let output = command
                .output()
                .map_err(|e| format!("设置 Windows 开机启动失败: {e}"))?;
            if !output.status.success() {
                return Err(format_windows_command_failure(
                    "设置 Windows 开机启动",
                    &output,
                ));
            }
        } else {
            let mut query_command = Command::new("reg");
            query_command.args([
                "query",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                AUTOSTART_NAME,
            ]);
            hide_console_window(&mut query_command);
            let query_output = query_command
                .output()
                .map_err(|e| format!("查询 Windows 开机启动状态失败: {e}"))?;

            if query_output.status.success() {
                let mut delete_command = Command::new("reg");
                delete_command.args([
                    "delete",
                    "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    "/v",
                    AUTOSTART_NAME,
                    "/f",
                ]);
                hide_console_window(&mut delete_command);
                let delete_output = delete_command
                    .output()
                    .map_err(|e| format!("移除 Windows 开机启动失败: {e}"))?;
                if !delete_output.status.success() {
                    return Err(format_windows_command_failure(
                        "移除 Windows 开机启动",
                        &delete_output,
                    ));
                }
            }
        }
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let home = resolve_home_dir()?;
        let launch_dir = home.join("Library").join("LaunchAgents");
        fs::create_dir_all(&launch_dir).map_err(|e| format!("创建 LaunchAgents 目录失败: {e}"))?;

        let plist_path = launch_dir.join(format!("{AUTOSTART_NAME}.plist"));
        if !enabled {
            match fs::remove_file(&plist_path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
                Err(e) => Err(format!("移除 LaunchAgent 文件失败: {e}")),
            }?;
            return Ok(());
        }

        let exe_path_s = exe_path.to_string_lossy();
        let mut plist = String::new();
        plist.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        plist.push_str("<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n");
        plist.push_str("<plist version=\"1.0\">\n<dict>\n");
        plist.push_str("  <key>Label</key>\n");
        let _ = writeln!(plist, "  <string>{}</string>", AUTOSTART_NAME);
        plist.push_str("  <key>ProgramArguments</key>\n  <array>\n");
        let _ = writeln!(plist, "    <string>{}</string>", exe_path_s);
        plist.push_str("  </array>\n");
        plist.push_str("  <key>RunAtLoad</key>\n  <true/>\n");
        plist.push_str("  <key>KeepAlive</key>\n  <false/>\n");
        plist.push_str("</dict>\n</plist>\n");

        let mut file =
            File::create(&plist_path).map_err(|e| format!("写入 LaunchAgent 文件失败: {e}"))?;
        use std::io::Write;
        file.write_all(plist.as_bytes())
            .map_err(|e| format!("写入 LaunchAgent 文件失败: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let home = resolve_home_dir()?;
        let base = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let autostart_dir = base.join("autostart");
        fs::create_dir_all(&autostart_dir).map_err(|e| format!("创建 autostart 目录失败: {e}"))?;

        let desktop_path = autostart_dir.join(format!("{AUTOSTART_NAME}.desktop"));
        if !enabled {
            match fs::remove_file(&desktop_path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
                Err(e) => Err(format!("移除自启动配置失败: {e}")),
            }?;
            return Ok(());
        }

        let exe_path_s = exe_path.to_string_lossy();
        let mut desktop = String::new();
        desktop.push_str("[Desktop Entry]\n");
        desktop.push_str("Type=Application\n");
        desktop.push_str("Name=WorkClaw\n");
        let _ = writeln!(desktop, "Exec={}", exe_path_s);
        desktop.push_str("X-GNOME-Autostart-enabled=true\n");
        fs::write(&desktop_path, desktop).map_err(|e| format!("写入 Desktop 文件失败: {e}"))?;
        return Ok(());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    Ok(())
}
