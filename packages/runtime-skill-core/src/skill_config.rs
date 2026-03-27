use serde::Deserialize;
use serde_json::Value as JsonValue;

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

#[derive(Debug, Clone)]
pub struct SkillConfig {
    pub name: Option<String>,
    pub description: Option<String>,
    pub allowed_tools: Option<Vec<String>>,
    pub model: Option<String>,
    pub max_iterations: Option<usize>,
    pub argument_hint: Option<String>,
    pub disable_model_invocation: bool,
    pub user_invocable: bool,
    pub invocation: SkillInvocationPolicy,
    pub metadata: Option<OpenClawSkillMetadata>,
    pub command_dispatch: Option<SkillCommandDispatchSpec>,
    pub context: Option<String>,
    pub agent: Option<String>,
    pub mcp_servers: Vec<McpServerDep>,
    pub system_prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillInvocationPolicy {
    pub user_invocable: bool,
    pub disable_model_invocation: bool,
}

impl Default for SkillInvocationPolicy {
    fn default() -> Self {
        Self {
            user_invocable: true,
            disable_model_invocation: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct OpenClawSkillMetadata {
    pub always: Option<bool>,
    pub emoji: Option<String>,
    pub homepage: Option<String>,
    pub skill_key: Option<String>,
    pub primary_env: Option<String>,
    pub os: Vec<String>,
    pub requires: Option<OpenClawSkillMetadataRequires>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct OpenClawSkillMetadataRequires {
    pub bins: Vec<String>,
    pub any_bins: Vec<String>,
    pub env: Vec<String>,
    pub config: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SkillCommandDispatchKind {
    Tool,
}

impl SkillCommandDispatchKind {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "tool" => Some(Self::Tool),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SkillCommandArgMode {
    Raw,
}

impl SkillCommandArgMode {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "" | "raw" => Some(Self::Raw),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillCommandDispatchSpec {
    pub kind: SkillCommandDispatchKind,
    pub tool_name: String,
    pub arg_mode: SkillCommandArgMode,
}

impl Default for SkillConfig {
    fn default() -> Self {
        Self {
            name: None,
            description: None,
            allowed_tools: None,
            model: None,
            max_iterations: None,
            argument_hint: None,
            disable_model_invocation: false,
            user_invocable: true,
            invocation: SkillInvocationPolicy::default(),
            metadata: None,
            command_dispatch: None,
            context: None,
            agent: None,
            mcp_servers: Vec::new(),
            system_prompt: String::new(),
        }
    }
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
    metadata: Option<serde_yaml::Value>,
    #[serde(alias = "command-dispatch")]
    command_dispatch: Option<String>,
    #[serde(alias = "command-tool")]
    command_tool: Option<String>,
    #[serde(alias = "command-arg-mode")]
    command_arg_mode: Option<String>,
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
        let invocation = SkillInvocationPolicy {
            user_invocable: fm.user_invocable,
            disable_model_invocation: fm.disable_model_invocation,
        };
        let metadata = fm
            .metadata
            .as_ref()
            .and_then(parse_openclaw_metadata_block);
        let command_dispatch = parse_command_dispatch(
            fm.command_dispatch.as_deref(),
            fm.command_tool.as_deref(),
            fm.command_arg_mode.as_deref(),
        );

        Self {
            name: fm.name,
            description: fm.description,
            allowed_tools: fm.allowed_tools.map(|v| v.into_vec()),
            model: fm.model,
            max_iterations: fm.max_iterations,
            argument_hint: fm.argument_hint,
            disable_model_invocation: fm.disable_model_invocation,
            user_invocable: fm.user_invocable,
            invocation,
            metadata,
            command_dispatch,
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

fn yaml_string_list(value: Option<&JsonValue>) -> Vec<String> {
    match value {
        Some(JsonValue::Array(values)) => values
            .iter()
            .filter_map(JsonValue::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect(),
        Some(JsonValue::String(raw)) => raw
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_metadata_json_value(value: &serde_yaml::Value) -> Option<JsonValue> {
    match value {
        serde_yaml::Value::String(raw) => {
            json5::from_str::<JsonValue>(raw).ok().and_then(|parsed| {
                let manifest = parsed.get("openclaw").cloned()?;
                manifest.is_object().then_some(manifest)
            })
        }
        _ => serde_json::to_value(value)
            .ok()
            .and_then(|parsed| parsed.get("openclaw").cloned().filter(JsonValue::is_object)),
    }
}

fn parse_openclaw_metadata_block(value: &serde_yaml::Value) -> Option<OpenClawSkillMetadata> {
    let JsonValue::Object(metadata) = parse_metadata_json_value(value)? else {
        return None;
    };

    let requires = metadata
        .get("requires")
        .and_then(JsonValue::as_object)
        .map(|requires| OpenClawSkillMetadataRequires {
            bins: yaml_string_list(requires.get("bins")),
            any_bins: yaml_string_list(requires.get("anyBins")),
            env: yaml_string_list(requires.get("env")),
            config: yaml_string_list(requires.get("config")),
        });

    Some(OpenClawSkillMetadata {
        always: metadata.get("always").and_then(JsonValue::as_bool),
        emoji: metadata
            .get("emoji")
            .and_then(JsonValue::as_str)
            .map(ToString::to_string),
        homepage: metadata
            .get("homepage")
            .and_then(JsonValue::as_str)
            .map(ToString::to_string),
        skill_key: metadata
            .get("skillKey")
            .and_then(JsonValue::as_str)
            .map(ToString::to_string),
        primary_env: metadata
            .get("primaryEnv")
            .and_then(JsonValue::as_str)
            .map(ToString::to_string),
        os: yaml_string_list(metadata.get("os")),
        requires,
    })
}

fn parse_command_dispatch(
    kind: Option<&str>,
    tool_name: Option<&str>,
    arg_mode: Option<&str>,
) -> Option<SkillCommandDispatchSpec> {
    let kind = SkillCommandDispatchKind::parse(kind?)?;
    let tool_name = tool_name?.trim();
    if tool_name.is_empty() {
        return None;
    }

    Some(SkillCommandDispatchSpec {
        kind,
        tool_name: tool_name.to_string(),
        arg_mode: SkillCommandArgMode::parse(arg_mode.unwrap_or_default())?,
    })
}
