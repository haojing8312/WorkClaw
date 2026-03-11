use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, serde::Serialize)]
pub struct McpServerDep {
    pub name: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub env: Option<Vec<String>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
enum AllowedToolsValue {
    Array(Vec<String>),
    CommaSeparated(String),
}

impl AllowedToolsValue {
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

#[derive(Debug, Clone, Default)]
pub struct SkillConfig {
    pub name: Option<String>,
    pub description: Option<String>,
    pub allowed_tools: Option<Vec<String>>,
    pub model: Option<String>,
    pub max_iterations: Option<usize>,
    pub argument_hint: Option<String>,
    pub disable_model_invocation: bool,
    pub user_invocable: bool,
    pub context: Option<String>,
    pub agent: Option<String>,
    pub mcp_servers: Vec<McpServerDep>,
    pub system_prompt: String,
}

#[derive(Deserialize, Default)]
struct FrontMatter {
    name: Option<String>,
    description: Option<String>,
    allowed_tools: Option<AllowedToolsValue>,
    model: Option<String>,
    max_iterations: Option<usize>,
    #[serde(alias = "argument-hint")]
    argument_hint: Option<String>,
    #[serde(alias = "disable-model-invocation", default)]
    disable_model_invocation: bool,
    #[serde(alias = "user-invocable", default = "default_true")]
    user_invocable: bool,
    context: Option<String>,
    agent: Option<String>,
    #[serde(alias = "mcp-servers", default)]
    mcp_servers: Vec<McpServerDep>,
}

fn default_true() -> bool {
    true
}

impl SkillConfig {
    pub fn parse(content: &str) -> Self {
        if !content.starts_with("---") {
            return Self {
                system_prompt: content.to_string(),
                ..Default::default()
            };
        }

        let rest = &content[3..];
        let end_pos = match rest.find("\n---") {
            Some(pos) => pos,
            None => {
                return Self {
                    system_prompt: content.to_string(),
                    ..Default::default()
                };
            }
        };

        let yaml_str = &rest[..end_pos];
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

    pub fn substitute_arguments(&mut self, args: &[&str], session_id: &str) {
        let all_args = args.join(" ");
        let mut result = self.system_prompt.clone();

        for i in (0..args.len()).rev() {
            let placeholder = format!("$ARGUMENTS[{}]", i);
            result = result.replace(&placeholder, args[i]);
        }

        for i in (0..args.len()).rev() {
            let placeholder = format!("${}", i);
            result = result.replace(&placeholder, args[i]);
        }

        result = result.replace("$ARGUMENTS", &all_args);
        result = result.replace("${CLAUDE_SESSION_ID}", session_id);

        self.system_prompt = result;
    }
}
