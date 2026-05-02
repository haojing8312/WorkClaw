use crate::agent::tools::process_manager::ProcessManager;
use crate::agent::tools::tool_result;
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::Arc;

/// 获取后台进程输出的工具
pub struct BashOutputTool {
    process_manager: Arc<ProcessManager>,
    tool_name: &'static str,
}

impl BashOutputTool {
    pub fn new(process_manager: Arc<ProcessManager>) -> Self {
        Self::with_name(process_manager, "bash_output")
    }

    fn with_name(process_manager: Arc<ProcessManager>, tool_name: &'static str) -> Self {
        Self {
            process_manager,
            tool_name,
        }
    }
}

impl Tool for BashOutputTool {
    fn name(&self) -> &str {
        self.tool_name
    }

    fn description(&self) -> &str {
        "获取后台进程的输出。可以选择阻塞等待进程完成或立即返回当前输出。返回结构化结果，其中 details 包含 stdout/stderr/exit_code。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "process_id": {
                    "type": "string",
                    "description": "后台进程的 ID"
                },
                "block": {
                    "type": "boolean",
                    "description": "是否阻塞等待进程结束（默认 false）"
                }
            },
            "required": ["process_id"]
        })
    }

    fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let process_id = input["process_id"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 process_id 参数"))?;

        let block = input["block"].as_bool().unwrap_or(false);

        let output = self.process_manager.get_output(process_id, block)?;

        let exit_code = output.exit_code.unwrap_or(-1);
        let status = if output.exited {
            format!("已退出 (退出码: {})", exit_code)
        } else {
            "运行中".to_string()
        };

        tool_result::success(
            self.name(),
            format!("进程 {} 状态: {}", process_id, status),
            json!({
                "process_id": process_id,
                "block": block,
                "stdout": output.stdout,
                "stderr": output.stderr,
                "exited": output.exited,
                "exit_code": if output.exited { Value::from(exit_code) } else { Value::Null },
                "output_file_path": output.output_file_path.to_string_lossy().to_string(),
                "output_file_size": output.output_file_size,
                "status_text": status,
            }),
        )
    }
}

/// 获取 exec 后台进程输出的工具。
pub struct ExecOutputTool {
    inner: BashOutputTool,
}

impl ExecOutputTool {
    pub fn new(process_manager: Arc<ProcessManager>) -> Self {
        Self {
            inner: BashOutputTool::with_name(process_manager, "exec_output"),
        }
    }
}

impl Tool for ExecOutputTool {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        "获取 exec 后台进程的输出。可以选择阻塞等待进程完成或立即返回当前输出。返回结构化结果，其中 details 包含 stdout/stderr/exit_code/output_file_path。"
    }

    fn input_schema(&self) -> Value {
        self.inner.input_schema()
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        self.inner.execute(input, ctx)
    }
}
