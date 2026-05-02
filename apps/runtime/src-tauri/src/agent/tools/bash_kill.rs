use crate::agent::tools::process_manager::ProcessManager;
use crate::agent::tools::tool_result;
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::Arc;

/// 终止后台进程的工具
pub struct BashKillTool {
    process_manager: Arc<ProcessManager>,
    tool_name: &'static str,
}

impl BashKillTool {
    pub fn new(process_manager: Arc<ProcessManager>) -> Self {
        Self::with_name(process_manager, "bash_kill")
    }

    fn with_name(process_manager: Arc<ProcessManager>, tool_name: &'static str) -> Self {
        Self {
            process_manager,
            tool_name,
        }
    }
}

impl Tool for BashKillTool {
    fn name(&self) -> &str {
        self.tool_name
    }

    fn description(&self) -> &str {
        "终止指定的后台进程。返回结构化结果，其中 details 包含 process_id。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "process_id": {
                    "type": "string",
                    "description": "要终止的后台进程 ID"
                }
            },
            "required": ["process_id"]
        })
    }

    fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let process_id = input["process_id"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 process_id 参数"))?;

        self.process_manager.kill(process_id)?;

        tool_result::success(
            self.name(),
            format!("已终止进程 {}", process_id),
            json!({
                "process_id": process_id,
            }),
        )
    }
}

/// 终止 exec 后台进程的工具。
pub struct ExecKillTool {
    inner: BashKillTool,
}

impl ExecKillTool {
    pub fn new(process_manager: Arc<ProcessManager>) -> Self {
        Self {
            inner: BashKillTool::with_name(process_manager, "exec_kill"),
        }
    }
}

impl Tool for ExecKillTool {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        "终止指定的 exec 后台进程。返回结构化结果，其中 details 包含 process_id。"
    }

    fn input_schema(&self) -> Value {
        self.inner.input_schema()
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        self.inner.execute(input, ctx)
    }
}
