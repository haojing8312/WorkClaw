use super::tool_manifest::ToolMetadata;
use crate::agent::run_guard::RunStopReason;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

/// 工具执行上下文
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    /// 工作目录路径，如有值则所有文件操作限制在此目录下
    pub work_dir: Option<PathBuf>,
    /// 当前回合允许调用的工具集合（已规范化工具名）
    pub allowed_tools: Option<Vec<String>>,
    /// 当前会话标识，便于工具层记录和诊断
    pub session_id: Option<String>,
    /// 任务级临时目录，用于中间产物和受控退路
    pub task_temp_dir: Option<PathBuf>,
    /// 运行时探测到的执行能力
    pub execution_caps: Option<ExecutionCaps>,
    /// 文件任务预检结果
    pub file_task_caps: Option<FileTaskCaps>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionCaps {
    pub platform: Option<String>,
    pub preferred_shell: Option<String>,
    pub python_candidates: Vec<String>,
    pub node_candidates: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileTaskCaps {
    pub requested_path: Option<PathBuf>,
    pub resolved_path: Option<PathBuf>,
    pub exists: bool,
    pub extension: Option<String>,
    pub read_mode: Option<String>,
    pub reason: Option<String>,
}

impl ToolContext {
    fn normalize_for_scope_check(path: &Path) -> anyhow::Result<PathBuf> {
        if path.exists() {
            return Ok(path.canonicalize()?);
        }

        let existing_ancestor = path.ancestors().find(|ancestor| ancestor.exists());
        let Some(existing_ancestor) = existing_ancestor else {
            return Ok(path.to_path_buf());
        };

        let mut normalized = existing_ancestor.canonicalize()?;
        let remainder = path
            .strip_prefix(existing_ancestor)
            .unwrap_or_else(|_| Path::new(""));

        for component in remainder.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    normalized.pop();
                }
                Component::Normal(part) => normalized.push(part),
                Component::Prefix(_) | Component::RootDir => {}
            }
        }

        Ok(normalized)
    }

    /// 检查路径是否在工作目录范围内，返回规范化后的绝对路径
    pub fn check_path(&self, path: &str) -> anyhow::Result<PathBuf> {
        let target = std::path::Path::new(path);
        let canonical = if target.is_absolute() {
            target.to_path_buf()
        } else if let Some(ref wd) = self.work_dir {
            wd.join(target)
        } else {
            std::env::current_dir()?.join(target)
        };

        if let Some(ref wd) = self.work_dir {
            let check_path = Self::normalize_for_scope_check(&canonical)?;
            let wd_canonical = Self::normalize_for_scope_check(wd)?;
            if !check_path.starts_with(&wd_canonical) {
                anyhow::bail!("路径 {} 不在工作目录 {} 范围内", path, wd.display());
            }
        }
        Ok(canonical)
    }
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String>;
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::default()
    }
    fn structured_output(&self, _input: &Value, _ctx: &ToolContext) -> Result<Option<Value>> {
        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
}

#[derive(Debug)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResultError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResultEnvelope {
    pub ok: bool,
    pub tool: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ToolResultError>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug)]
pub enum LLMResponse {
    Text(String),
    ToolCalls(Vec<ToolCall>),
    /// LLM 返回工具调用时附带的伴随文本（如"让我搜索一下…"）
    TextWithToolCalls(String, Vec<ToolCall>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamDelta {
    Text(String),
    Reasoning(String),
}

#[derive(Debug)]
pub enum AgentState {
    Thinking,
    ToolCalling,
    Finished,
    Error(String),
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct ToolCallEvent {
    pub session_id: String,
    pub tool_name: String,
    pub tool_input: Value,
    pub tool_output: Option<String>,
    pub status: String, // "started" | "completed" | "error"
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct BackgroundProcessEvent {
    pub session_id: String,
    pub process_id: String,
    pub command: String,
    pub status: String, // "completed" | "failed"
    pub exit_code: Option<i32>,
    pub output_file_path: String,
    pub output_file_size: u64,
}

/// Agent 状态事件，用于前端展示当前执行阶段
#[derive(serde::Serialize, Clone, Debug)]
pub struct AgentStateEvent {
    pub session_id: String,
    /// 状态类型: "thinking" | "tool_calling" | "finished" | "error"
    pub state: String,
    /// 工具名列表（tool_calling 时）或错误信息（error 时）
    pub detail: Option<String>,
    pub iteration: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason_last_completed_step: Option<String>,
}

impl AgentStateEvent {
    pub fn basic(
        session_id: impl Into<String>,
        state: impl Into<String>,
        detail: Option<String>,
        iteration: usize,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            state: state.into(),
            detail,
            iteration,
            stop_reason_kind: None,
            stop_reason_title: None,
            stop_reason_message: None,
            stop_reason_last_completed_step: None,
        }
    }

    pub fn stopped(
        session_id: impl Into<String>,
        iteration: usize,
        reason: &RunStopReason,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            state: "stopped".to_string(),
            detail: reason
                .detail
                .clone()
                .or_else(|| Some(reason.message.clone())),
            iteration,
            stop_reason_kind: Some(reason.kind.as_key().to_string()),
            stop_reason_title: Some(reason.title.clone()),
            stop_reason_message: Some(reason.message.clone()),
            stop_reason_last_completed_step: reason.last_completed_step.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentStateEvent, BackgroundProcessEvent, ToolCallEvent};
    use serde_json::{Value, json};

    #[test]
    fn tool_call_event_serializes_expected_shape() {
        let event = ToolCallEvent {
            session_id: "sess-1".to_string(),
            tool_name: "read_file".to_string(),
            tool_input: json!({"path":"README.md"}),
            tool_output: Some("ok".to_string()),
            status: "completed".to_string(),
        };

        let value = serde_json::to_value(event).expect("serialize event");
        assert_eq!(
            value,
            json!({
                "session_id": "sess-1",
                "tool_name": "read_file",
                "tool_input": {"path":"README.md"},
                "tool_output": "ok",
                "status": "completed",
            })
        );
    }

    #[test]
    fn background_process_event_serializes_expected_shape() {
        let event = BackgroundProcessEvent {
            session_id: "sess-1".to_string(),
            process_id: "proc-1".to_string(),
            command: "echo ok".to_string(),
            status: "completed".to_string(),
            exit_code: Some(0),
            output_file_path: "C:\\Temp\\proc-1.log".to_string(),
            output_file_size: 42,
        };

        let value = serde_json::to_value(event).expect("serialize event");
        assert_eq!(
            value,
            json!({
                "session_id": "sess-1",
                "process_id": "proc-1",
                "command": "echo ok",
                "status": "completed",
                "exit_code": 0,
                "output_file_path": "C:\\Temp\\proc-1.log",
                "output_file_size": 42,
            })
        );
    }

    #[test]
    fn agent_state_event_serializes_stop_reason_fields() {
        let event = AgentStateEvent::basic("sess-2", "thinking", None, 3);
        let value = serde_json::to_value(event).expect("serialize event");
        assert_eq!(
            value,
            json!({
                "session_id": "sess-2",
                "state": "thinking",
                "detail": null,
                "iteration": 3,
            })
        );
    }

    #[test]
    fn agent_state_event_omits_empty_stop_reason_fields() {
        let event = AgentStateEvent::basic("sess-3", "finished", Some("done".to_string()), 7);
        let value = serde_json::to_value(event).expect("serialize event");
        let object = value.as_object().expect("object");
        assert_eq!(
            object.get("session_id"),
            Some(&Value::String("sess-3".to_string()))
        );
        assert_eq!(
            object.get("state"),
            Some(&Value::String("finished".to_string()))
        );
        assert_eq!(
            object.get("detail"),
            Some(&Value::String("done".to_string()))
        );
        assert_eq!(object.get("iteration"), Some(&Value::from(7)));
        assert!(!object.contains_key("stop_reason_kind"));
        assert!(!object.contains_key("stop_reason_title"));
        assert!(!object.contains_key("stop_reason_message"));
        assert!(!object.contains_key("stop_reason_last_completed_step"));
    }
}
