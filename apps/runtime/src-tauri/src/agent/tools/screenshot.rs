use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::process::Command;

/// 截取屏幕截图并保存到指定路径
pub struct ScreenshotTool;

impl Tool for ScreenshotTool {
    fn name(&self) -> &str {
        "screenshot"
    }

    fn description(&self) -> &str {
        "截取屏幕截图并保存到指定路径。支持 PNG 格式输出。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "截图保存路径（相对或绝对），建议使用 .png 后缀"
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;

        let checked = ctx.check_path(path)?;
        let path_str = checked.to_string_lossy().to_string();

        // 确保父目录存在
        if let Some(parent) = checked.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| anyhow!("创建父目录失败: {}", e))?;
            }
        }

        // 根据平台执行截图命令
        let output = capture_screenshot(&path_str)?;

        // 验证截图文件是否生成
        if checked.exists() {
            let size = std::fs::metadata(&checked).map(|m| m.len()).unwrap_or(0);
            Ok(format!(
                "截图已保存到 {}（{} 字节）\n{}",
                path_str, size, output
            ))
        } else {
            Err(anyhow!("截图失败：文件未生成。命令输出:\n{}", output))
        }
    }
}

/// 根据操作系统执行截图命令
#[cfg(target_os = "windows")]
fn capture_screenshot(path: &str) -> Result<String> {
    // 使用 PowerShell 进行屏幕捕获
    let escaped_path = path.replace('\'', "''");
    let ps_script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; \
         $bmp = New-Object System.Drawing.Bitmap(\
             [System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Width, \
             [System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Height\
         ); \
         $g = [System.Drawing.Graphics]::FromImage($bmp); \
         $g.CopyFromScreen(0, 0, 0, 0, $bmp.Size); \
         $bmp.Save('{}'); \
         $g.Dispose(); \
         $bmp.Dispose()",
        escaped_path
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .output()
        .map_err(|e| anyhow!("启动 PowerShell 失败: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(anyhow!(
            "PowerShell 截图命令失败（退出码 {}）:\n{}",
            output.status.code().unwrap_or(-1),
            stderr
        ));
    }

    Ok(format!("{}{}", stdout, stderr).trim().to_string())
}

#[cfg(target_os = "macos")]
fn capture_screenshot(path: &str) -> Result<String> {
    // macOS 使用 screencapture 命令
    let output = Command::new("screencapture")
        .args(["-x", path]) // -x 禁用截图音效
        .output()
        .map_err(|e| anyhow!("启动 screencapture 失败: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(anyhow!(
            "screencapture 命令失败（退出码 {}）:\n{}",
            output.status.code().unwrap_or(-1),
            stderr
        ));
    }

    Ok(format!("{}{}", stdout, stderr).trim().to_string())
}

#[cfg(target_os = "linux")]
fn capture_screenshot(path: &str) -> Result<String> {
    // Linux：优先尝试 gnome-screenshot，回退到 import (ImageMagick)
    let gnome_result = Command::new("gnome-screenshot").args(["-f", path]).output();

    match gnome_result {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Ok(format!("{}{}", stdout, stderr).trim().to_string())
        }
        _ => {
            // 回退到 ImageMagick 的 import 命令
            let output = Command::new("import")
                .args(["-window", "root", path])
                .output()
                .map_err(|e| anyhow!("截图失败：gnome-screenshot 和 import 均不可用: {}", e))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if !output.status.success() {
                return Err(anyhow!(
                    "import 命令失败（退出码 {}）:\n{}",
                    output.status.code().unwrap_or(-1),
                    stderr
                ));
            }

            Ok(format!("{}{}", stdout, stderr).trim().to_string())
        }
    }
}
