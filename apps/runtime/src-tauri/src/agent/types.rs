use anyhow::Result;
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
