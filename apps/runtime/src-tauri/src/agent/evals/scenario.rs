use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalScenario {
    pub id: String,
    pub title: String,
    pub capability_id: String,
    pub kind: String,
    pub mode: String,
    pub side_effect: String,
    pub enabled: bool,
    pub input: EvalScenarioInput,
    pub expect: EvalScenarioExpect,
    pub thresholds: EvalThresholds,
    #[serde(default)]
    pub record_metrics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalScenarioInput {
    pub user_text: String,
    #[serde(default)]
    pub workspace_files: Vec<EvalWorkspaceFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalWorkspaceFile {
    pub path: String,
    pub data_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalScenarioExpect {
    #[serde(default)]
    pub route: Option<EvalRouteExpect>,
    #[serde(default)]
    pub execution: Option<EvalExecutionExpect>,
    #[serde(default)]
    pub structured: Option<EvalStructuredExpect>,
    #[serde(default)]
    pub tools: EvalToolExpect,
    #[serde(default)]
    pub output: EvalOutputExpect,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalRouteExpect {
    pub family: String,
    #[serde(default)]
    pub runner_not: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalExecutionExpect {
    pub leaf_exit_code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalStructuredExpect {
    pub equals: EvalStructuredEquals,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalStructuredEquals {
    pub employee: String,
    pub start_date: String,
    pub end_date: String,
    #[serde(default)]
    pub daily_count: Option<u32>,
    #[serde(default)]
    pub plan_count: Option<u32>,
    #[serde(default)]
    pub report_count: Option<u32>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalToolExpect {
    #[serde(default)]
    pub called_all: Vec<String>,
    #[serde(default)]
    pub called_any: Vec<String>,
    #[serde(default)]
    pub not_called: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalOutputExpect {
    #[serde(default)]
    pub contains_all: Vec<String>,
    #[serde(default)]
    pub contains_any: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalThresholds {
    pub pass_total_ms: u64,
    pub warn_total_ms: u64,
    pub max_turn_count: u32,
    pub max_tool_count: u32,
}

#[cfg(test)]
mod tests {
    use super::EvalScenario;
    use std::fs;
    use std::path::Path;

    fn scenario_fixture_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("agent-evals")
            .join("scenarios")
            .join("pm_weekly_summary_xietao_2026_03_30_2026_04_04.yaml")
    }

    #[test]
    fn scenario_yaml_parses_expected_thresholds() {
        let raw = fs::read_to_string(scenario_fixture_path()).expect("read scenario");
        let scenario: EvalScenario = serde_yaml::from_str(&raw).expect("parse scenario");

        assert_eq!(scenario.capability_id, "pm_weekly_summary");
        assert_eq!(scenario.thresholds.pass_total_ms, 150_000);
        assert_eq!(scenario.thresholds.warn_total_ms, 180_000);
        let structured = scenario.expect.structured.expect("structured assertions");
        assert_eq!(structured.equals.daily_count, Some(6));
        assert_eq!(structured.equals.plan_count, Some(6));
        assert_eq!(structured.equals.report_count, Some(5));
    }

    #[test]
    fn scenario_yaml_parses_expected_route_and_output_contract() {
        let raw = fs::read_to_string(scenario_fixture_path()).expect("read scenario");
        let scenario: EvalScenario = serde_yaml::from_str(&raw).expect("parse scenario");

        let route = scenario.expect.route.expect("route assertions");
        assert_eq!(route.family, "feishu-pm");
        assert_eq!(route.runner_not.as_deref(), Some("OpenTaskRunner"));
        assert_eq!(scenario.expect.output.contains_all.len(), 2);
        assert_eq!(
            scenario.record_metrics.last().map(String::as_str),
            Some("fallback_reason")
        );
    }

    #[test]
    fn generic_runtime_scenario_can_omit_pm_specific_assertions() {
        let raw = r#"
id: workspace_image_set_vision_2026_04_25
title: 工作区图片视觉分析
capability_id: workspace_image_set_vision
kind: real-agent
mode: runtime-tool-routing
side_effect: none
enabled: true
input:
  user_text: 读取这些图片，并告诉我每个图片的内容
  workspace_files:
    - path: red-dot.png
      data_base64: aGVsbG8=
expect:
  tools:
    called_all:
      - vision_analyze
thresholds:
  pass_total_ms: 150000
  warn_total_ms: 180000
  max_turn_count: 4
  max_tool_count: 6
"#;

        let scenario: EvalScenario = serde_yaml::from_str(raw).expect("parse generic scenario");

        assert!(scenario.expect.route.is_none());
        assert!(scenario.expect.execution.is_none());
        assert!(scenario.expect.structured.is_none());
        assert_eq!(scenario.expect.tools.called_all, vec!["vision_analyze"]);
        assert_eq!(scenario.input.workspace_files[0].path, "red-dot.png");
    }
}
