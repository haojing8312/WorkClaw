use crate::agent::tool_manifest::{ToolCategory, ToolMetadata, ToolSource};
use crate::agent::tools::process_manager::ProcessManager;
use crate::agent::tools::tool_result;
use crate::agent::types::{Tool, ToolContext};
use crate::windows_process::hide_console_window;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use wait_timeout::ChildExt;

use super::bash::BashTool;

pub struct ExecTool {
    process_manager: Option<Arc<ProcessManager>>,
}

impl ExecTool {
    pub fn new() -> Self {
        Self {
            process_manager: None,
        }
    }

    pub fn with_process_manager(pm: Arc<ProcessManager>) -> Self {
        Self {
            process_manager: Some(pm),
        }
    }

    #[cfg(target_os = "windows")]
    fn get_shell() -> (&'static str, &'static [&'static str], &'static str) {
        (
            "powershell",
            &["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"],
            "powershell",
        )
    }

    #[cfg(not(target_os = "windows"))]
    fn get_shell() -> (&'static str, &'static [&'static str], &'static str) {
        ("bash", &["-c"], "bash")
    }
}

impl Tool for ExecTool {
    fn name(&self) -> &str {
        "exec"
    }

    fn description(&self) -> &str {
        "执行稳定命令入口。Windows 使用 PowerShell，Unix 使用 bash。返回结构化结果，其中 details 包含 stdout/stderr/exit_code 等字段。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "要执行的命令"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "超时时间（毫秒，可选，默认 120000）"
                },
                "background": {
                    "type": "boolean",
                    "description": "是否在后台运行（可选，默认 false）"
                }
            },
            "required": ["command"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            display_name: Some("Exec".to_string()),
            category: ToolCategory::Shell,
            destructive: true,
            requires_approval: true,
            source: ToolSource::Runtime,
            ..ToolMetadata::default()
        }
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 command 参数"))?;

        if BashTool::is_dangerous(command) {
            return tool_result::failure(
                self.name(),
                "危险命令已被拦截",
                "DANGEROUS_COMMAND_BLOCKED",
                "危险命令已被拦截。此命令可能造成不可逆损害。",
                BashTool::enrich_execution_details_with_shell(
                    json!({
                        "command": command,
                        "background": false,
                    }),
                    ctx,
                    Self::get_shell().2,
                ),
            );
        }

        let background = input["background"].as_bool().unwrap_or(false);
        if background {
            if let Some(ref pm) = self.process_manager {
                let work_dir = ctx.work_dir.as_deref();
                let (shell, shell_args, shell_label) = Self::get_shell();
                let handle = pm.spawn_with_shell_handle(command, work_dir, shell, shell_args)?;
                return tool_result::success(
                    self.name(),
                    format!("后台进程已启动，process_id: {}", handle.id),
                    BashTool::enrich_execution_details_with_shell(
                        json!({
                            "command": command,
                            "background": true,
                            "process_id": handle.id,
                            "output_file_path": handle.output_file_path.to_string_lossy().to_string(),
                        }),
                        ctx,
                        shell_label,
                    ),
                );
            }
            return Err(anyhow!("后台模式不可用：未配置 ProcessManager"));
        }

        let timeout_ms = input["timeout_ms"].as_u64().unwrap_or(120_000);
        let timeout = Duration::from_millis(timeout_ms);
        let (shell, shell_args, shell_label) = Self::get_shell();

        let mut cmd = Command::new(shell);
        cmd.args(shell_args)
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(ref wd) = ctx.work_dir {
            cmd.current_dir(wd);
        }

        hide_console_window(&mut cmd);
        let mut child = cmd.spawn()?;

        match child.wait_timeout(timeout)? {
            Some(status) => {
                let mut stdout_str = String::new();
                let mut stderr_str = String::new();
                if let Some(mut out) = child.stdout.take() {
                    out.read_to_string(&mut stdout_str).ok();
                }
                if let Some(mut err) = child.stderr.take() {
                    err.read_to_string(&mut stderr_str).ok();
                }

                if !status.success() {
                    tool_result::failure(
                        self.name(),
                        format!("命令执行失败（退出码 {}）", status.code().unwrap_or(-1)),
                        "COMMAND_EXIT_NONZERO",
                        format!("命令执行失败（退出码 {}）", status.code().unwrap_or(-1)),
                        BashTool::enrich_execution_details_with_shell(
                            json!({
                                "command": command,
                                "exit_code": status.code().unwrap_or(-1),
                                "timed_out": false,
                                "background": false,
                                "stdout": stdout_str,
                                "stderr": stderr_str,
                            }),
                            ctx,
                            shell_label,
                        ),
                    )
                } else {
                    tool_result::success(
                        self.name(),
                        format!("命令执行完成（退出码 {}）", status.code().unwrap_or(0)),
                        BashTool::enrich_execution_details_with_shell(
                            json!({
                                "command": command,
                                "exit_code": status.code().unwrap_or(0),
                                "timed_out": false,
                                "background": false,
                                "stdout": stdout_str,
                                "stderr": stderr_str,
                            }),
                            ctx,
                            shell_label,
                        ),
                    )
                }
            }
            None => {
                let _ = child.kill();
                let _ = child.wait();
                tool_result::failure(
                    self.name(),
                    format!("命令执行超时（{}ms），已终止", timeout_ms),
                    "COMMAND_TIMEOUT",
                    format!("命令执行超时（{}ms），已终止", timeout_ms),
                    BashTool::enrich_execution_details_with_shell(
                        json!({
                            "command": command,
                            "exit_code": Value::Null,
                            "timed_out": true,
                            "background": false,
                            "stdout": "",
                            "stderr": "",
                        }),
                        ctx,
                        shell_label,
                    ),
                )
            }
        }
    }
}
