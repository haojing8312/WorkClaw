use serde::Deserialize;

/// SKILL.md 中声明的 MCP 服务器依赖
#[derive(Deserialize, Debug, Clone, serde::Serialize)]
pub struct McpServerDep {
    pub name: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    /// 需要的环境变量名称列表
    #[serde(default)]
    pub env: Option<Vec<String>>,
}

/// allowed_tools 支持两种 YAML 格式：逗号分隔字符串或数组
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
enum AllowedToolsValue {
    /// YAML 数组格式: ["Bash", "Read"]
    Array(Vec<String>),
    /// 逗号分隔字符串格式: "Bash, Read, Glob"
    CommaSeparated(String),
}

impl AllowedToolsValue {
    /// 转换为统一的 Vec<String>
    fn into_vec(self) -> Vec<String> {
        match self {
            AllowedToolsValue::Array(v) => v,
            AllowedToolsValue::CommaSeparated(s) => s
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect(),
        }
    }
}

/// 从 SKILL.md 解析出的 Skill 配置
#[derive(Debug, Clone, Default)]
pub struct SkillConfig {
    pub name: Option<String>,
    pub description: Option<String>,
    pub allowed_tools: Option<Vec<String>>,
    pub model: Option<String>,
    pub max_iterations: Option<usize>,
    /// Claude Code 兼容: 参数提示文本
    pub argument_hint: Option<String>,
    /// Claude Code 兼容: 禁止 Skill 自行调用 LLM（默认 false）
    pub disable_model_invocation: bool,
    /// Claude Code 兼容: 用户可主动触发（默认 true）
    pub user_invocable: bool,
    /// Claude Code 兼容: 上下文模式，如 "fork" 表示独立上下文
    pub context: Option<String>,
    /// Claude Code 兼容: 指定运行的 Agent，如 "Explore", "Plan"
    pub agent: Option<String>,
    /// MCP 服务器依赖列表
    pub mcp_servers: Vec<McpServerDep>,
    pub system_prompt: String,
}

/// YAML frontmatter 的反序列化结构
#[derive(Deserialize, Default)]
struct FrontMatter {
    name: Option<String>,
    description: Option<String>,
    allowed_tools: Option<AllowedToolsValue>,
    model: Option<String>,
    max_iterations: Option<usize>,
    /// Claude Code 兼容字段（YAML alias: argument-hint）
    #[serde(alias = "argument-hint")]
    argument_hint: Option<String>,
    /// Claude Code 兼容字段（YAML alias: disable-model-invocation）
    #[serde(alias = "disable-model-invocation", default)]
    disable_model_invocation: bool,
    /// Claude Code 兼容字段（YAML alias: user-invocable）
    #[serde(alias = "user-invocable", default = "default_true")]
    user_invocable: bool,
    /// Claude Code 兼容: 上下文模式
    context: Option<String>,
    /// Claude Code 兼容: 指定 Agent
    agent: Option<String>,
    /// MCP 服务器依赖（YAML alias: mcp-servers）
    #[serde(alias = "mcp-servers", default)]
    mcp_servers: Vec<McpServerDep>,
}

/// serde 默认值函数：返回 true
fn default_true() -> bool {
    true
}

impl SkillConfig {
    /// 解析 SKILL.md 内容，提取 YAML frontmatter 和 system prompt
    pub fn parse(content: &str) -> Self {
        // 没有 frontmatter 标记
        if !content.starts_with("---") {
            return Self {
                system_prompt: content.to_string(),
                ..Default::default()
            };
        }

        // 跳过开头的 "---\n"，查找第二个 "---"
        let rest = &content[3..];
        let end_pos = match rest.find("\n---") {
            Some(pos) => pos,
            None => {
                // 没找到结束标记，整个内容作为 prompt
                return Self {
                    system_prompt: content.to_string(),
                    ..Default::default()
                };
            }
        };

        let yaml_str = &rest[..end_pos];
        // "---" (3) + yaml + "\n---" (4) = prompt 开始位置
        let prompt_start = 3 + end_pos + 4;
        let system_prompt = if prompt_start < content.len() {
            content[prompt_start..].trim_start_matches('\n').to_string()
        } else {
            String::new()
        };

        let fm: FrontMatter = serde_yaml::from_str(yaml_str).unwrap_or_default();

        Self {
            name: fm.name,
            description: fm.description,
            allowed_tools: fm.allowed_tools.map(|v| v.into_vec()),
            model: fm.model,
            max_iterations: fm.max_iterations,
            argument_hint: fm.argument_hint,
            disable_model_invocation: fm.disable_model_invocation,
            user_invocable: fm.user_invocable,
            context: fm.context,
            agent: fm.agent,
            mcp_servers: fm.mcp_servers,
            system_prompt,
        }
    }

    /// 在 system_prompt 中替换参数占位符
    ///
    /// 支持的占位符：
    /// - `$ARGUMENTS` → 全部参数空格拼接
    /// - `$ARGUMENTS[N]` → 第 N 个参数（0-indexed）
    /// - `$N` → `$ARGUMENTS[N]` 的简写
    /// - `${CLAUDE_SESSION_ID}` → 当前会话 ID
    pub fn substitute_arguments(&mut self, args: &[&str], session_id: &str) {
        let all_args = args.join(" ");
        let mut result = self.system_prompt.clone();

        // 先替换 $ARGUMENTS[N] 格式（避免被 $ARGUMENTS 先匹配）
        // 使用简单的循环替换，支持 $ARGUMENTS[0] ~ $ARGUMENTS[99]
        for i in (0..args.len()).rev() {
            let placeholder = format!("$ARGUMENTS[{}]", i);
            result = result.replace(&placeholder, args[i]);
        }

        // 替换 $N 简写格式（从大到小避免 $1 匹配到 $10 的前缀）
        for i in (0..args.len()).rev() {
            let placeholder = format!("${}", i);
            result = result.replace(&placeholder, args[i]);
        }

        // 替换 $ARGUMENTS（全部参数）
        result = result.replace("$ARGUMENTS", &all_args);

        // 替换 ${CLAUDE_SESSION_ID}
        result = result.replace("${CLAUDE_SESSION_ID}", session_id);

        self.system_prompt = result;
    }
}
