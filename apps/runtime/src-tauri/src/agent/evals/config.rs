use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalEvalConfig {
    pub runtime: EvalRuntimeConfig,
    pub models: EvalModelConfig,
    pub providers: BTreeMap<String, ModelProviderProfile>,
    pub artifacts: EvalArtifactConfig,
    #[serde(deserialize_with = "deserialize_capabilities")]
    pub capabilities: BTreeMap<String, CapabilityMapping>,
    pub diagnostics: EvalDiagnosticsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalRuntimeConfig {
    pub workspace_root: String,
    #[serde(default)]
    pub cargo_manifest_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalModelConfig {
    pub default_profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelProviderProfile {
    pub provider: String,
    pub model: String,
    pub api_key_env: String,
    #[serde(default)]
    pub api_format: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalArtifactConfig {
    pub output_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityMapping {
    pub workspace_root: String,
    pub entry_kind: String,
    pub entry_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalDiagnosticsConfig {
    pub export_journal: bool,
    pub export_trace: bool,
    pub export_stdout_stderr: bool,
}

fn deserialize_capabilities<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, CapabilityMapping>, D::Error>
where
    D: Deserializer<'de>,
{
    let mappings = BTreeMap::<String, CapabilityMapping>::deserialize(deserializer)?;
    if mappings.is_empty() {
        return Err(de::Error::custom(
            "capabilities must contain at least one local capability mapping",
        ));
    }
    Ok(mappings)
}

#[cfg(test)]
mod tests {
    use super::LocalEvalConfig;
    use std::fs;
    use std::path::Path;

    fn config_example_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("agent-evals")
            .join("config")
            .join("config.example.yaml")
    }

    #[test]
    fn config_yaml_requires_local_capability_mapping() {
        let raw = r#"
runtime:
  workspace_root: D:\\code\\WorkClaw
models:
  default_profile: minimax_anthropic
providers:
  minimax_anthropic:
    provider: minimax
    model: MiniMax-M2.5
    api_key_env: MINIMAX_API_KEY
artifacts:
  output_dir: D:\\code\\WorkClaw\\temp\\agent-evals
capabilities: {}
diagnostics:
  export_journal: true
  export_trace: true
  export_stdout_stderr: true
"#;

        let err = serde_yaml::from_str::<LocalEvalConfig>(raw).expect_err("config should fail");
        assert!(err.to_string().contains("capabilities"));
    }

    #[test]
    fn config_example_yaml_parses_expected_defaults() {
        let raw = fs::read_to_string(config_example_path()).expect("read config example");
        let config: LocalEvalConfig = serde_yaml::from_str(&raw).expect("parse config example");

        assert_eq!(config.runtime.workspace_root, r"D:\\code\\WorkClaw");
        assert_eq!(config.models.default_profile, "minimax_anthropic");
        assert!(config.providers.contains_key("minimax_anthropic"));
        assert!(config.capabilities.contains_key("pm_weekly_summary"));
    }
}
