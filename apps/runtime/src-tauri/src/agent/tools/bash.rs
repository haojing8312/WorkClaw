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

/// Shell 命令执行工具，支持同步执行和后台模式
pub struct BashTool {
    /// 后台进程管理器（可选）。设置后支持 background 模式
    process_manager: Option<Arc<ProcessManager>>,
}

impl BashTool {
    pub fn new() -> Self {
        Self {
            process_manager: None,
        }
    }

    /// 创建带有 ProcessManager 的 BashTool，支持后台模式
    pub fn with_process_manager(pm: Arc<ProcessManager>) -> Self {
        Self {
            process_manager: Some(pm),
        }
    }

    #[cfg(target_os = "windows")]
    fn get_shell() -> (&'static str, &'static str) {
        ("cmd", "/C")
    }

    #[cfg(not(target_os = "windows"))]
    fn get_shell() -> (&'static str, &'static str) {
        ("bash", "-c")
    }

    /// 检查命令是否包含危险操作模式
    pub(crate) fn is_dangerous(command: &str) -> bool {
        let lower = command.to_lowercase();
        let patterns = [
            "rm -rf /",
            "rm -rf /*",
            "rm -rf ~",
            "format c:",
            "format d:",
            "shutdown",
            "reboot",
            "> /dev/sda",
            "dd if=/dev/zero",
            ":(){ :|:& };:",
            "mkfs.",
            "wipefs",
        ];
        patterns.iter().any(|p| lower.contains(p))
    }

    pub(crate) fn enrich_execution_details_with_shell(
        details: Value,
        ctx: &ToolContext,
        platform_shell: &str,
    ) -> Value {
        let mut details = details;
        if let Some(details_obj) = details.as_object_mut() {
            details_obj.insert(
                "platform_shell".to_string(),
                Value::String(platform_shell.to_string()),
            );
            details_obj.insert(
                "work_dir".to_string(),
                ctx.work_dir
                    .as_ref()
                    .map(|path| Value::String(path.to_string_lossy().to_string()))
                    .unwrap_or(Value::Null),
            );
            details_obj.insert(
                "task_temp_dir".to_string(),
                ctx.task_temp_dir
                    .as_ref()
                    .map(|path| Value::String(path.to_string_lossy().to_string()))
                    .unwrap_or(Value::Null),
            );
        }
        details
    }

    fn enrich_execution_details(details: Value, ctx: &ToolContext) -> Value {
        let platform_shell = ctx
            .execution_caps
            .as_ref()
            .and_then(|caps| caps.preferred_shell.clone())
            .unwrap_or_else(|| {
                let (shell, _) = Self::get_shell();
                shell.to_string()
            });
        Self::enrich_execution_details_with_shell(details, ctx, &platform_shell)
    }
}

impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "执行 shell 命令。Windows 使用 cmd，Unix 使用 bash。返回结构化结果，其中 details 包含 stdout/stderr/exit_code 等字段。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "要执行的 shell 命令"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "超时时间（毫秒，可选，默认 120000）"
                },
                "background": {
                    "type": "boolean",
                    "description": "是否在后台运行（可选，默认 false）。后台模式下返回 process_id，可用 bash_output 获取输出"
                }
            },
            "required": ["command"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            display_name: Some("Shell".to_string()),
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

        // 危险命令检查
        if Self::is_dangerous(command) {
            return tool_result::failure(
                self.name(),
                "危险命令已被拦截",
                "DANGEROUS_COMMAND_BLOCKED",
                "危险命令已被拦截。此命令可能造成不可逆损害。",
                Self::enrich_execution_details(
                    json!({
                        "command": command,
                        "background": false,
                    }),
                    ctx,
                ),
            );
        }

        // 后台模式：通过 ProcessManager 启动进程
        let background = input["background"].as_bool().unwrap_or(false);
        if background {
            if let Some(ref pm) = self.process_manager {
                let work_dir = ctx.work_dir.as_deref();
                let handle = pm.spawn_handle(command, work_dir)?;
                return tool_result::success(
                    self.name(),
                    format!("后台进程已启动，process_id: {}", handle.id),
                    Self::enrich_execution_details(
                        json!({
                            "command": command,
                            "background": true,
                            "process_id": handle.id,
                            "output_file_path": handle.output_file_path.to_string_lossy().to_string(),
                        }),
                        ctx,
                    ),
                );
            } else {
                return Err(anyhow!("后台模式不可用：未配置 ProcessManager"));
            }
        }

        // 同步模式（原有逻辑）
        // 提取超时参数，默认 120 秒
        let timeout_ms = input["timeout_ms"].as_u64().unwrap_or(120_000);
        let timeout = Duration::from_millis(timeout_ms);

        let (shell, flag) = Self::get_shell();

        // 使用 spawn 启动子进程，以便后续进行超时控制
        let mut cmd = Command::new(shell);
        cmd.arg(flag)
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // 如果 ToolContext 指定了工作目录，设置子进程的 cwd
        if let Some(ref wd) = ctx.work_dir {
            cmd.current_dir(wd);
        }

        hide_console_window(&mut cmd);
        let mut child = cmd.spawn()?;

        // 等待子进程完成或超时
        match child.wait_timeout(timeout)? {
            Some(status) => {
                // 子进程已正常退出，读取输出
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
                        Self::enrich_execution_details(
                            json!({
                                "command": command,
                                "exit_code": status.code().unwrap_or(-1),
                                "timed_out": false,
                                "background": false,
                                "stdout": stdout_str,
                                "stderr": stderr_str,
                            }),
                            ctx,
                        ),
                    )
                } else {
                    tool_result::success(
                        self.name(),
                        format!("命令执行完成（退出码 {}）", status.code().unwrap_or(0)),
                        Self::enrich_execution_details(
                            json!({
                                "command": command,
                                "exit_code": status.code().unwrap_or(0),
                                "timed_out": false,
                                "background": false,
                                "stdout": stdout_str,
                                "stderr": stderr_str,
                            }),
                            ctx,
                        ),
                    )
                }
            }
            None => {
                // 超时：终止子进程
                let _ = child.kill();
                let _ = child.wait();
                tool_result::failure(
                    self.name(),
                    format!("命令执行超时（{}ms），已终止", timeout_ms),
                    "COMMAND_TIMEOUT",
                    format!("命令执行超时（{}ms），已终止", timeout_ms),
                    Self::enrich_execution_details(
                        json!({
                            "command": command,
                            "exit_code": Value::Null,
                            "timed_out": true,
                            "background": false,
                            "stdout": "",
                            "stderr": "",
                        }),
                        ctx,
                    ),
                )
            }
        }
    }
}
