use crate::agent::permissions::narrow_allowed_tools;
use crate::agent::skill_config::SkillConfig;
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Skill 调用工具：按名称加载本地 SKILL.md，并返回可执行指令文本。
///
/// 设计目标：
/// 1) 让编排型 Skill（如 using-superpowers）在单会话内按需调用子 Skill
/// 2) 避免一次性注入所有 Skill 到 system prompt
/// 3) 通过调用栈和深度限制避免递归循环
pub struct SkillInvokeTool {
    session_id: String,
    search_roots: Vec<PathBuf>,
    max_depth: usize,
    call_stack: Mutex<Vec<String>>,
}

impl SkillInvokeTool {
    pub fn new(session_id: String, search_roots: Vec<PathBuf>) -> Self {
        Self {
            session_id,
            search_roots,
            max_depth: 4,
            call_stack: Mutex::new(Vec::new()),
        }
    }

    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth.max(1);
        self
    }

    fn normalize_skill_name(raw: &str) -> Result<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(anyhow!("INVALID_SKILL_NAME: skill_name 不能为空"));
        }

        let normalized = trimmed.replace('\\', "/");
        let name = if normalized.ends_with("/SKILL.md") {
            let p = Path::new(&normalized);
            p.parent()
                .and_then(|x| x.file_name())
                .and_then(|x| x.to_str())
                .ok_or_else(|| anyhow!("INVALID_SKILL_NAME: 无效 skill_name: {}", raw))?
                .to_string()
        } else {
            normalized
                .split('/')
                .filter(|s| !s.is_empty())
                .next_back()
                .ok_or_else(|| anyhow!("INVALID_SKILL_NAME: 无效 skill_name: {}", raw))?
                .to_string()
        };

        let valid = name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');
        if !valid {
            return Err(anyhow!(
                "INVALID_SKILL_NAME: skill_name 含非法字符，仅允许字母数字以及 - _ ."
            ));
        }

        Ok(name)
    }

    fn find_skill_md(&self, skill_name: &str) -> Option<PathBuf> {
        self.search_roots
            .iter()
            .map(|root| root.join(skill_name).join("SKILL.md"))
            .find(|p| p.exists())
    }
}

impl Tool for SkillInvokeTool {
    fn name(&self) -> &str {
        "skill"
    }

    fn description(&self) -> &str {
        "调用另一个 Skill。输入 skill_name 和 arguments，系统会加载该 Skill 的 SKILL.md 并返回指令内容。适用于技能编排场景。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "skill_name": {
                    "type": "string",
                    "description": "目标 Skill 名称，如 using-superpowers、executing-plans"
                },
                "arguments": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "传递给子 Skill 的参数列表（可选）"
                }
            },
            "required": ["skill_name"]
        })
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let raw_name = input["skill_name"]
            .as_str()
            .ok_or_else(|| anyhow!("BAD_REQUEST: 缺少 skill_name 参数"))?;
        let skill_name = Self::normalize_skill_name(raw_name)?;

        let mut stack_guard = self
            .call_stack
            .lock()
            .map_err(|e| anyhow!("调用栈锁失败: {}", e))?;
        if stack_guard.len() >= self.max_depth {
            return Err(anyhow!(
                "CALL_DEPTH_EXCEEDED: Skill 调用深度超过限制({})，当前调用栈: {}",
                self.max_depth,
                stack_guard.join(" -> ")
            ));
        }
        if stack_guard.iter().any(|s| s == &skill_name) {
            return Err(anyhow!(
                "CALL_CYCLE_DETECTED: 检测到循环调用: {} -> {}",
                stack_guard.join(" -> "),
                skill_name
            ));
        }
        stack_guard.push(skill_name.clone());
        drop(stack_guard);

        let result = (|| -> Result<String> {
            let skill_path = self.find_skill_md(&skill_name).ok_or_else(|| {
                anyhow!(
                    "SKILL_NOT_FOUND: 未找到 Skill: {}。搜索路径: {}",
                    skill_name,
                    self.search_roots
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join("; ")
                )
            })?;

            let content = std::fs::read_to_string(&skill_path)
                .map_err(|e| anyhow!("SKILL_READ_FAILED: 读取 SKILL.md 失败: {}", e))?;
            let mut config = SkillConfig::parse(&content);

            let args: Vec<String> = input["arguments"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            config.substitute_arguments(&arg_refs, &self.session_id);

            let child_declared = config.allowed_tools.clone().unwrap_or_default();
            let narrowed_tools = narrow_allowed_tools(
                ctx.allowed_tools.as_deref(),
                if child_declared.is_empty() {
                    None
                } else {
                    Some(child_declared.as_slice())
                },
            );
            if ctx.allowed_tools.is_some()
                && !child_declared.is_empty()
                && narrowed_tools.is_empty()
            {
                return Err(anyhow!(
                    "PERMISSION_DENIED: 子 Skill '{}' 声明的工具不在父会话允许范围内",
                    skill_name
                ));
            }
            let declared_tools = if child_declared.is_empty() {
                "(未声明)".to_string()
            } else {
                child_declared.join(", ")
            };
            let narrowed_tools_text = if narrowed_tools.is_empty() {
                "(无可用工具)".to_string()
            } else {
                narrowed_tools.join(", ")
            };

            Ok(format!(
                "## Skill: {}\n\
来源: {}\n\
描述: {}\n\
声明工具: {}\n\
收紧后工具: {}\n\
最大迭代: {}\n\n\
请严格执行以下 Skill 指令（原文）:\n\n{}",
                skill_name,
                skill_path.display(),
                config.description.unwrap_or_default(),
                declared_tools,
                narrowed_tools_text,
                config
                    .max_iterations
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "(未声明)".to_string()),
                config.system_prompt
            ))
        })();

        if let Ok(mut stack_guard) = self.call_stack.lock() {
            let _ = stack_guard.pop();
        }

        result
    }
}
