use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvalReportStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct EvalReportDecision {
    pub capability_id: String,
    #[serde(default)]
    pub selected_skill: Option<String>,
    #[serde(default)]
    pub selected_runner: Option<String>,
    #[serde(default)]
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct EvalAssertionResults {
    pub route: String,
    pub execution: String,
    pub structured: String,
    pub output: String,
    pub thresholds: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct EvalReportTiming {
    #[serde(default)]
    pub total_duration_ms: Option<u64>,
    #[serde(default)]
    pub route_latency_ms: Option<u64>,
    #[serde(default)]
    pub leaf_exec_duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct EvalReportUsage {
    pub turn_count: u32,
    pub tool_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct EvalReportArtifacts {
    #[serde(default)]
    pub report_json_path: Option<String>,
    #[serde(default)]
    pub report_yaml_path: Option<String>,
    #[serde(default)]
    pub journal_path: Option<String>,
    #[serde(default)]
    pub trace_path: Option<String>,
    #[serde(default)]
    pub messages_path: Option<String>,
    #[serde(default)]
    pub route_attempt_logs_path: Option<String>,
    #[serde(default)]
    pub stdout_path: Option<String>,
    #[serde(default)]
    pub stderr_path: Option<String>,
    #[serde(default)]
    pub session_markdown_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalReport {
    pub run_id: String,
    pub scenario_id: String,
    pub status: EvalReportStatus,
    pub decision: EvalReportDecision,
    pub timing: EvalReportTiming,
    pub usage: EvalReportUsage,
    pub assertions: EvalAssertionResults,
    #[serde(default)]
    pub metrics: BTreeMap<String, Value>,
    #[serde(default)]
    pub artifacts: EvalReportArtifacts,
    pub final_output_excerpt: String,
}

impl EvalReport {
    pub fn passing(scenario_id: impl Into<String>, capability_id: impl Into<String>) -> Self {
        Self {
            run_id: String::new(),
            scenario_id: scenario_id.into(),
            status: EvalReportStatus::Pass,
            decision: EvalReportDecision {
                capability_id: capability_id.into(),
                ..EvalReportDecision::default()
            },
            timing: EvalReportTiming::default(),
            usage: EvalReportUsage::default(),
            assertions: EvalAssertionResults {
                route: "pass".to_string(),
                execution: "pass".to_string(),
                structured: "pass".to_string(),
                output: "pass".to_string(),
                thresholds: "pass".to_string(),
            },
            metrics: BTreeMap::new(),
            artifacts: EvalReportArtifacts::default(),
            final_output_excerpt: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EvalReport, EvalReportStatus};

    #[test]
    fn passing_helper_sets_pass_defaults() {
        let report = EvalReport::passing("scenario-1", "pm_weekly_summary");

        assert_eq!(report.scenario_id, "scenario-1");
        assert_eq!(report.status, EvalReportStatus::Pass);
        assert_eq!(report.decision.capability_id, "pm_weekly_summary");
        assert_eq!(report.assertions.route, "pass");
        assert_eq!(report.assertions.thresholds, "pass");
        assert_eq!(report.usage.turn_count, 0);
        assert!(report.metrics.is_empty());
    }

    #[test]
    fn report_serializes_stable_pass_status() {
        let report = EvalReport::passing("scenario-1", "pm_weekly_summary");
        let value = serde_json::to_value(&report).expect("serialize report");

        assert_eq!(value["status"], "pass");
        assert_eq!(value["scenario_id"], "scenario-1");
        assert_eq!(value["decision"]["capability_id"], "pm_weekly_summary");
        assert_eq!(value["usage"]["turn_count"], 0);
        assert_eq!(report.status, EvalReportStatus::Pass);
    }
}
