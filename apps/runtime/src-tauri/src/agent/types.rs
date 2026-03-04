use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

/// 工具执行上下文
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    /// 工作目录路径，如有值则所有文件操作限制在此目录下
    pub work_dir: Option<PathBuf>,
    /// 当前回合允许调用的工具集合（已规范化工具名）
    pub allowed_tools: Option<Vec<String>>,
}

impl ToolContext {
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
            // 规范化处理 .. 和符号链接
            let check_path = if canonical.exists() {
                canonical.canonicalize()?
            } else if let Some(parent) = canonical.parent() {
                if parent.exists() {
                    parent
                        .canonicalize()?
                        .join(canonical.file_name().unwrap_or_default())
                } else {
                    canonical.clone()
                }
            } else {
                canonical.clone()
            };

            let wd_canonical = wd.canonicalize().unwrap_or_else(|_| wd.clone());
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

#[derive(Debug)]
pub enum AgentState {
    Thinking,
    ToolCalling,
    Finished,
    Error(String),
}
