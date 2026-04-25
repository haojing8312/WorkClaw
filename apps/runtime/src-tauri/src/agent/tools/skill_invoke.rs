use crate::agent::permissions::narrow_allowed_tools;
use crate::agent::skill_config::SkillConfig;
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use runtime_skill_core::SkillCommandDispatchSpec;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const WORKSPACE_SKILL_ID_MARKER_FILE: &str = ".workclaw-skill-id";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillResolutionMode {
    PromptFollowing,
    CommandDispatch,
}

impl SkillResolutionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            SkillResolutionMode::PromptFollowing => "prompt_following",
            SkillResolutionMode::CommandDispatch => "command_dispatch",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SkillResolution {
    mode: SkillResolutionMode,
    narrowed_tools: Vec<String>,
    unrestricted_tools: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillResolvedInvocation {
    pub skill_name: String,
    pub skill_path: PathBuf,
    pub description: Option<String>,
    pub declared_tools: Vec<String>,
    pub narrowed_tools: Vec<String>,
    pub unrestricted_tools: bool,
    pub user_invocable: bool,
    pub disable_model_invocation: bool,
    pub max_iterations: Option<usize>,
    pub mode: SkillResolutionMode,
    pub command_dispatch: Option<SkillCommandDispatchSpec>,
    pub system_prompt: String,
}

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

    fn search_roots_text(&self) -> String {
        self.search_roots
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn path_ends_with_skill_md(path: &str) -> bool {
        let normalized = path.replace('\\', "/");
        normalized.ends_with("/SKILL.md") || normalized.ends_with("/skill.md")
    }

    fn skill_name_from_path(raw: &str) -> Result<String> {
        let p = Path::new(raw.trim());
        p.parent()
            .and_then(|x| x.file_name())
            .and_then(|x| x.to_str())
            .ok_or_else(|| anyhow!("INVALID_SKILL_NAME: 无效 skill_name: {}", raw))
            .map(|value| value.to_string())
    }

    fn path_is_within_search_roots(&self, path: &Path) -> bool {
        let Ok(canonical_path) = path.canonicalize() else {
            return false;
        };

        self.search_roots.iter().any(|root| {
            root.canonicalize()
                .map(|canonical_root| canonical_path.starts_with(&canonical_root))
                .unwrap_or(false)
        })
    }

    fn resolve_explicit_skill_path(&self, raw: &str) -> Result<Option<(String, PathBuf)>> {
        if !Self::path_ends_with_skill_md(raw) {
            return Ok(None);
        }

        let path = PathBuf::from(raw.trim());
        if !path.is_file() {
            return Ok(None);
        }

        if !self.path_is_within_search_roots(&path) {
            return Err(anyhow!(
                "PERMISSION_DENIED: skill path 不在允许范围内: {}",
                raw
            ));
        }

        Ok(Some((Self::skill_name_from_path(raw)?, path)))
    }

    fn find_skill_md_in_dir(dir: &Path) -> Option<PathBuf> {
        ["SKILL.md", "skill.md"]
            .iter()
            .map(|name| dir.join(name))
            .find(|path| path.exists())
    }

    fn find_skill_md(&self, skill_name: &str) -> Option<PathBuf> {
        self.search_roots
            .iter()
            .find_map(|root| Self::find_skill_md_in_dir(&root.join(skill_name)))
    }

    fn find_skill_by_display_name(&self, raw: &str) -> Option<(String, PathBuf)> {
        let target = raw.trim();
        if target.is_empty() {
            return None;
        }

        self.search_roots.iter().find_map(|root| {
            let entries = std::fs::read_dir(root).ok()?;
            for entry in entries.flatten() {
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if !file_type.is_dir() {
                    continue;
                }

                let skill_dir = entry.path();
                let Some(skill_md) = Self::find_skill_md_in_dir(&skill_dir) else {
                    continue;
                };
                let Ok(content) = std::fs::read_to_string(&skill_md) else {
                    continue;
                };
                let config = SkillConfig::parse(&content);
                let Some(display_name) = config.name else {
                    continue;
                };
                let display_name = display_name.trim().to_string();
                let matches = display_name == target
                    || (display_name.is_ascii()
                        && target.is_ascii()
                        && display_name.eq_ignore_ascii_case(target));
                if matches {
                    let dir_name = skill_dir.file_name()?.to_str()?.to_string();
                    return Some((dir_name, skill_md));
                }
            }
            None
        })
    }

    fn find_skill_by_workspace_skill_id(&self, raw: &str) -> Option<(String, PathBuf)> {
        let target = raw.trim();
        if target.is_empty() {
            return None;
        }

        self.search_roots.iter().find_map(|root| {
            let entries = std::fs::read_dir(root).ok()?;
            for entry in entries.flatten() {
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if !file_type.is_dir() {
                    continue;
                }

                let skill_dir = entry.path();
                let marker_path = skill_dir.join(WORKSPACE_SKILL_ID_MARKER_FILE);
                let Ok(marker_value) = std::fs::read_to_string(marker_path) else {
                    continue;
                };
                if marker_value.trim() != target {
                    continue;
                }

                let skill_md = Self::find_skill_md_in_dir(&skill_dir)?;
                return Some((target.to_string(), skill_md));
            }
            None
        })
    }

    fn resolve_skill_target(&self, raw: &str) -> Result<(String, PathBuf)> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(anyhow!("INVALID_SKILL_NAME: skill_name 不能为空"));
        }

        if let Some(explicit_path) = self.resolve_explicit_skill_path(trimmed)? {
            return Ok(explicit_path);
        }

        match Self::normalize_skill_name(trimmed) {
            Ok(skill_name) => {
                if let Some(mapped) = self.find_skill_by_workspace_skill_id(&skill_name) {
                    return Ok(mapped);
                }
                if let Some(skill_path) = self.find_skill_md(&skill_name) {
                    return Ok((skill_name, skill_path));
                }
                if let Some(mapped) = self.find_skill_by_display_name(trimmed) {
                    return Ok(mapped);
                }
                Err(anyhow!(
                    "SKILL_NOT_FOUND: 未找到 Skill: {}。搜索路径: {}",
                    skill_name,
                    self.search_roots_text()
                ))
            }
            Err(normalize_err) => {
                if let Some(mapped) = self.find_skill_by_display_name(trimmed) {
                    Ok(mapped)
                } else {
                    Err(normalize_err)
                }
            }
        }
    }

    fn resolve_allowed_tools(
        parent_allowed: Option<&[String]>,
        child_declared: &[String],
    ) -> SkillResolution {
        let narrowed_tools = narrow_allowed_tools(
            parent_allowed,
            if child_declared.is_empty() {
                None
            } else {
                Some(child_declared)
            },
        );
        let unrestricted_tools = parent_allowed.is_none() && child_declared.is_empty();

        SkillResolution {
            mode: SkillResolutionMode::PromptFollowing,
            narrowed_tools,
            unrestricted_tools,
        }
    }

    fn ensure_dispatch_is_allowed(
        dispatch: &SkillCommandDispatchSpec,
        parent_allowed: Option<&[String]>,
    ) -> Result<()> {
        if let Some(parent_allowed) = parent_allowed {
            let dispatch_allowed = narrow_allowed_tools(
                Some(parent_allowed),
                Some(std::slice::from_ref(&dispatch.tool_name)),
            );
            if dispatch_allowed.is_empty() {
                return Err(anyhow!(
                    "PERMISSION_DENIED: Skill command dispatch 目标工具 '{}' 不在父会话允许范围内",
                    dispatch.tool_name
                ));
            }
        }
        Ok(())
    }

    fn render_skill_result(invocation: &SkillResolvedInvocation) -> String {
        let dispatch_summary = match &invocation.command_dispatch {
            Some(dispatch) => format!(
                "kind={}, tool_name={}, arg_mode={:?}",
                match dispatch.kind {
                    runtime_skill_core::SkillCommandDispatchKind::Tool => "tool",
                },
                dispatch.tool_name,
                dispatch.arg_mode
            ),
            None => "(none)".to_string(),
        };
        let declared_tools = if invocation.declared_tools.is_empty() {
            "(未声明)".to_string()
        } else {
            invocation.declared_tools.join(", ")
        };
        let narrowed_tools_text = if invocation.unrestricted_tools {
            "(继承父会话全部工具 / unrestricted)".to_string()
        } else if invocation.narrowed_tools.is_empty() {
            "(无显式收紧结果)".to_string()
        } else {
            invocation.narrowed_tools.join(", ")
        };

        format!(
            "## Skill: {}\n\
解析模式: {}\n\
来源: {}\n\
描述: {}\n\
用户可直接调用: {}\n\
禁用模型自动调用: {}\n\
命令分派: {}\n\
声明工具: {}\n\
收紧后工具: {}\n\
最大迭代: {}\n\n\
请严格执行以下 Skill 指令（原文）:\n\n{}",
            invocation.skill_name,
            invocation.mode.as_str(),
            invocation.skill_path.display(),
            invocation.description.clone().unwrap_or_default(),
            invocation.user_invocable,
            invocation.disable_model_invocation,
            dispatch_summary,
            declared_tools,
            narrowed_tools_text,
            invocation
                .max_iterations
                .map(|v| v.to_string())
                .unwrap_or_else(|| "(未声明)".to_string()),
            invocation.system_prompt
        )
    }

    pub fn resolve_invocation(
        &self,
        input: Value,
        ctx: &ToolContext,
    ) -> Result<SkillResolvedInvocation> {
        let raw_name = input["skill_name"]
            .as_str()
            .ok_or_else(|| anyhow!("BAD_REQUEST: 缺少 skill_name 参数"))?;
        let (skill_name, skill_path) = self.resolve_skill_target(raw_name)?;

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

        let result = (|| -> Result<SkillResolvedInvocation> {
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
            let mut resolution =
                Self::resolve_allowed_tools(ctx.allowed_tools.as_deref(), &child_declared);
            if let Some(dispatch) = &config.command_dispatch {
                Self::ensure_dispatch_is_allowed(dispatch, ctx.allowed_tools.as_deref())?;
                resolution.mode = SkillResolutionMode::CommandDispatch;
            }

            Ok(SkillResolvedInvocation {
                skill_name: skill_name.clone(),
                skill_path: skill_path.clone(),
                description: config.description.clone(),
                declared_tools: child_declared,
                narrowed_tools: resolution.narrowed_tools,
                unrestricted_tools: resolution.unrestricted_tools,
                user_invocable: config.user_invocable,
                disable_model_invocation: config.disable_model_invocation,
                max_iterations: config.max_iterations,
                mode: resolution.mode,
                command_dispatch: config.command_dispatch.clone(),
                system_prompt: config.system_prompt,
            })
        })();

        if let Ok(mut stack_guard) = self.call_stack.lock() {
            let _ = stack_guard.pop();
        }

        result
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
                    "description": "目标 Skill 的 invoke_name 或 SKILL.md 路径，如 using-superpowers、executing-plans"
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

    fn structured_output(&self, input: &Value, ctx: &ToolContext) -> Result<Option<Value>> {
        let resolved = self.resolve_invocation(input.clone(), ctx)?;
        serde_json::to_value(resolved)
            .map(Some)
            .map_err(|err| anyhow!("SKILL_RESOLUTION_SERIALIZE_FAILED: {}", err))
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let resolved = self.resolve_invocation(input, ctx)?;
        Ok(Self::render_skill_result(&resolved))
    }
}

#[cfg(test)]
mod tests {
    use super::{SkillInvokeTool, SkillResolutionMode};
    use crate::agent::types::{Tool, ToolContext};
    use serde_json::json;
    use tempfile::TempDir;

    fn create_skill(root: &TempDir, name: &str, skill_md: &str) {
        let skill_dir = root.path().join(name);
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        std::fs::write(skill_dir.join("SKILL.md"), skill_md).expect("write SKILL.md");
    }

    #[test]
    fn skill_tool_returns_prompt_following_mode_when_child_declares_no_tools() {
        let tmp = TempDir::new().expect("temp dir");
        create_skill(
            &tmp,
            "instruction-only-skill",
            "---\nname: instruction-only-skill\ndescription: helper\n---\n\nChild prompt",
        );

        let tool = SkillInvokeTool::new("sess-1".to_string(), vec![tmp.path().to_path_buf()]);
        let ctx = ToolContext {
            work_dir: None,
            allowed_tools: None,
            session_id: None,
            task_temp_dir: None,
            execution_caps: None,
            file_task_caps: None,
        };
        let out = tool
            .execute(json!({"skill_name": "instruction-only-skill"}), &ctx)
            .expect("instruction-only skill should succeed");

        assert!(out.contains("解析模式: prompt_following"));
        assert!(out.contains("收紧后工具: (继承父会话全部工具 / unrestricted)"));
    }

    #[test]
    fn skill_tool_keeps_narrowed_tools_when_declared_tools_overlap() {
        let tmp = TempDir::new().expect("temp dir");
        create_skill(
            &tmp,
            "executable-skill",
            "---\nname: executable-skill\nallowed_tools: \"read_file, web_search\"\n---\n\nChild prompt",
        );

        let tool = SkillInvokeTool::new("sess-1".to_string(), vec![tmp.path().to_path_buf()]);
        let ctx = ToolContext {
            work_dir: None,
            allowed_tools: Some(vec!["read_file".to_string()]),
            session_id: None,
            task_temp_dir: None,
            execution_caps: None,
            file_task_caps: None,
        };
        let out = tool
            .execute(json!({"skill_name": "executable-skill"}), &ctx)
            .expect("executable skill should succeed");

        assert!(out.contains("解析模式: prompt_following"));
        assert!(out.contains("收紧后工具: read_file"));
    }

    #[test]
    fn skill_tool_allows_empty_overlap_when_no_dispatch_is_requested() {
        let tmp = TempDir::new().expect("temp dir");
        create_skill(
            &tmp,
            "blocked-skill",
            "---\nname: blocked-skill\nallowed_tools: \"bash\"\n---\n\nChild prompt",
        );

        let tool = SkillInvokeTool::new("sess-1".to_string(), vec![tmp.path().to_path_buf()]);
        let ctx = ToolContext {
            work_dir: None,
            allowed_tools: Some(vec!["read_file".to_string()]),
            session_id: None,
            task_temp_dir: None,
            execution_caps: None,
            file_task_caps: None,
        };
        let out = tool
            .execute(json!({"skill_name": "blocked-skill"}), &ctx)
            .expect("prompt-following skill should still load");

        assert!(out.contains("解析模式: prompt_following"));
        assert!(out.contains("收紧后工具: (无显式收紧结果)"));
    }

    #[test]
    fn skill_tool_exposes_command_dispatch_metadata() {
        let tmp = TempDir::new().expect("temp dir");
        create_skill(
            &tmp,
            "dispatch-skill",
            "---\nname: dispatch-skill\ndisable-model-invocation: true\ncommand-dispatch: tool\ncommand-tool: exec\ncommand-arg-mode: raw\n---\n\nChild prompt",
        );

        let tool = SkillInvokeTool::new("sess-1".to_string(), vec![tmp.path().to_path_buf()]);
        let ctx = ToolContext {
            work_dir: None,
            allowed_tools: Some(vec!["exec".to_string(), "read_file".to_string()]),
            session_id: None,
            task_temp_dir: None,
            execution_caps: None,
            file_task_caps: None,
        };
        let out = tool
            .execute(json!({"skill_name": "dispatch-skill"}), &ctx)
            .expect("dispatch skill should resolve");

        assert!(out.contains("解析模式: command_dispatch"));
        assert!(out.contains("禁用模型自动调用: true"));
        assert!(out.contains("命令分派: kind=tool, tool_name=exec"));
    }

    #[test]
    fn skill_tool_resolve_invocation_returns_prompt_following_state() {
        let tmp = TempDir::new().expect("temp dir");
        create_skill(
            &tmp,
            "instruction-only-skill",
            "---\nname: instruction-only-skill\ndescription: helper\nallowed_tools: \"read_file\"\n---\n\nChild prompt",
        );

        let tool = SkillInvokeTool::new("sess-1".to_string(), vec![tmp.path().to_path_buf()]);
        let ctx = ToolContext {
            work_dir: None,
            allowed_tools: Some(vec!["read_file".to_string(), "glob".to_string()]),
            session_id: None,
            task_temp_dir: None,
            execution_caps: None,
            file_task_caps: None,
        };
        let resolved = tool
            .resolve_invocation(json!({"skill_name": "instruction-only-skill"}), &ctx)
            .expect("resolution should succeed");

        assert_eq!(resolved.mode, SkillResolutionMode::PromptFollowing);
        assert_eq!(resolved.skill_name, "instruction-only-skill");
        assert_eq!(resolved.declared_tools, vec!["read_file".to_string()]);
        assert_eq!(resolved.narrowed_tools, vec!["read_file".to_string()]);
        assert!(!resolved.disable_model_invocation);
        assert!(resolved.command_dispatch.is_none());
        assert!(resolved.system_prompt.contains("Child prompt"));
    }

    #[test]
    fn skill_tool_resolve_invocation_returns_dispatch_state() {
        let tmp = TempDir::new().expect("temp dir");
        create_skill(
            &tmp,
            "dispatch-skill",
            "---\nname: dispatch-skill\ndisable-model-invocation: true\ncommand-dispatch: tool\ncommand-tool: exec\ncommand-arg-mode: raw\n---\n\nChild prompt",
        );

        let tool = SkillInvokeTool::new("sess-1".to_string(), vec![tmp.path().to_path_buf()]);
        let ctx = ToolContext {
            work_dir: None,
            allowed_tools: Some(vec!["exec".to_string(), "read_file".to_string()]),
            session_id: None,
            task_temp_dir: None,
            execution_caps: None,
            file_task_caps: None,
        };
        let resolved = tool
            .resolve_invocation(json!({"skill_name": "dispatch-skill"}), &ctx)
            .expect("dispatch resolution should succeed");

        assert_eq!(resolved.mode, SkillResolutionMode::CommandDispatch);
        assert!(resolved.disable_model_invocation);
        assert_eq!(
            resolved
                .command_dispatch
                .as_ref()
                .map(|dispatch| dispatch.tool_name.as_str()),
            Some("exec")
        );
    }

    #[test]
    fn skill_tool_resolves_projected_skill_directory_by_workspace_skill_id_marker() {
        let tmp = TempDir::new().expect("temp dir");
        create_skill(
            &tmp,
            "feishu-pm-runtime",
            "---\nname: feishu-pm-runtime\n---\n\nChild prompt",
        );
        std::fs::write(
            tmp.path()
                .join("feishu-pm-runtime")
                .join(".workclaw-skill-id"),
            "local-feishu-pm-runtime",
        )
        .expect("write skill id marker");

        let tool = SkillInvokeTool::new("sess-1".to_string(), vec![tmp.path().to_path_buf()]);
        let ctx = ToolContext {
            work_dir: None,
            allowed_tools: None,
            session_id: None,
            task_temp_dir: None,
            execution_caps: None,
            file_task_caps: None,
        };
        let out = tool
            .execute(json!({"skill_name": "local-feishu-pm-runtime"}), &ctx)
            .expect("workspace skill id alias should resolve");

        assert!(out.contains("## Skill: local-feishu-pm-runtime"));
        assert!(out.contains("Child prompt"));
    }

    #[test]
    fn skill_tool_prefers_workspace_skill_id_marker_over_same_named_directory() {
        let projected_root = TempDir::new().expect("projected root");
        create_skill(
            &projected_root,
            "feishu-pm-runtime",
            "---\nname: feishu-pm-runtime\n---\n\nProjected prompt",
        );
        std::fs::write(
            projected_root
                .path()
                .join("feishu-pm-runtime")
                .join(".workclaw-skill-id"),
            "local-feishu-pm-runtime",
        )
        .expect("write skill id marker");

        let conflicting_root = TempDir::new().expect("conflicting root");
        create_skill(
            &conflicting_root,
            "local-feishu-pm-runtime",
            "---\nname: conflicting\n---\n\nConflicting prompt",
        );

        let tool = SkillInvokeTool::new(
            "sess-1".to_string(),
            vec![
                projected_root.path().to_path_buf(),
                conflicting_root.path().to_path_buf(),
            ],
        );
        let ctx = ToolContext {
            work_dir: None,
            allowed_tools: None,
            session_id: None,
            task_temp_dir: None,
            execution_caps: None,
            file_task_caps: None,
        };
        let out = tool
            .execute(json!({"skill_name": "local-feishu-pm-runtime"}), &ctx)
            .expect("workspace skill id marker should win");

        assert!(out.contains("Projected prompt"));
        assert!(!out.contains("Conflicting prompt"));
    }

    #[test]
    fn skill_tool_blocks_when_dispatch_target_is_outside_parent_scope() {
        let tmp = TempDir::new().expect("temp dir");
        create_skill(
            &tmp,
            "dispatch-skill",
            "---\nname: dispatch-skill\ncommand-dispatch: tool\ncommand-tool: exec\n---\n\nChild prompt",
        );

        let tool = SkillInvokeTool::new("sess-1".to_string(), vec![tmp.path().to_path_buf()]);
        let ctx = ToolContext {
            work_dir: None,
            allowed_tools: Some(vec!["read_file".to_string()]),
            session_id: None,
            task_temp_dir: None,
            execution_caps: None,
            file_task_caps: None,
        };
        let err = tool
            .execute(json!({"skill_name": "dispatch-skill"}), &ctx)
            .expect_err("dispatch outside parent scope should be rejected");

        assert!(err.to_string().contains("PERMISSION_DENIED"));
        assert!(err.to_string().contains("exec"));
    }
}
