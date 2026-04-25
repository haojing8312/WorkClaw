use super::{
    EvalAssertionResults, EvalReport, EvalReportArtifacts, EvalReportDecision, EvalReportStatus,
    EvalReportTiming, EvalReportUsage, EvalScenario, HeadlessEvalRun, LocalEvalConfig,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalOutcome {
    pub report: EvalReport,
    pub artifact_dir: PathBuf,
}

#[derive(Debug, Clone, Default)]
struct ToolOutputObservation {
    parsed_output: Option<Value>,
    stdout_raw: Option<String>,
    stdout_json: Option<Value>,
    stderr_raw: Option<String>,
    exit_code: Option<i64>,
}

pub fn evaluate_and_write_report(
    config: &LocalEvalConfig,
    scenario: &EvalScenario,
    run: &HeadlessEvalRun,
) -> Result<EvalOutcome, String> {
    let observations = collect_tool_output_observations(&run.messages);
    let total_duration_ms = compute_total_duration_ms(run);
    let route_latency_ms = compute_route_latency_ms(run);
    let leaf_exec_duration_ms = compute_leaf_exec_duration_ms(run);
    let turn_count = run.session_runs.len() as u32;
    let tool_count = run
        .trace
        .as_ref()
        .map(|trace| trace.tools.len() as u32)
        .unwrap_or_default();
    let selected_skill = (!run.skill_id.trim().is_empty()).then(|| run.skill_id.clone());
    let selected_runner = infer_selected_runner(scenario, &selected_skill);
    let fallback_reason = infer_fallback_reason(run);

    let assertions = EvalAssertionResults {
        route: evaluate_route_assertions(
            scenario,
            run,
            selected_skill.as_deref(),
            selected_runner.as_deref(),
        ),
        execution: evaluate_execution_assertions(scenario, run, &observations),
        structured: evaluate_structured_assertions(scenario, &observations),
        tools: evaluate_tool_assertions(scenario, run),
        output: evaluate_output_assertions(scenario, run),
        thresholds: evaluate_thresholds(scenario, total_duration_ms, turn_count, tool_count),
    };

    let status = if [
        assertions.route.as_str(),
        assertions.execution.as_str(),
        assertions.structured.as_str(),
        assertions.tools.as_str(),
        assertions.output.as_str(),
        assertions.thresholds.as_str(),
    ]
    .contains(&"fail")
    {
        EvalReportStatus::Fail
    } else if assertions.thresholds == "warn" {
        EvalReportStatus::Warn
    } else {
        EvalReportStatus::Pass
    };

    let artifact_dir = PathBuf::from(&config.artifacts.output_dir)
        .join("runs")
        .join(&scenario.id)
        .join(sanitize_id_component(&run.session_id));
    fs::create_dir_all(&artifact_dir).map_err(|e| format!("创建评测报告目录失败: {e}"))?;

    let mut report = EvalReport {
        run_id: run
            .session_runs
            .last()
            .map(|item| item.id.clone())
            .unwrap_or_else(|| run.session_id.clone()),
        scenario_id: scenario.id.clone(),
        status,
        decision: EvalReportDecision {
            capability_id: scenario.capability_id.clone(),
            selected_skill,
            selected_runner,
            fallback_reason,
        },
        timing: EvalReportTiming {
            total_duration_ms,
            route_latency_ms,
            leaf_exec_duration_ms,
        },
        usage: EvalReportUsage {
            turn_count,
            tool_count,
        },
        assertions,
        metrics: build_requested_metrics(
            scenario,
            run,
            total_duration_ms,
            route_latency_ms,
            leaf_exec_duration_ms,
            turn_count,
            tool_count,
        ),
        artifacts: EvalReportArtifacts::default(),
        final_output_excerpt: truncate_text(&run.final_output, 600),
    };

    let mut artifacts = EvalReportArtifacts::default();
    persist_debug_artifacts(config, run, &observations, &artifact_dir, &mut artifacts)?;
    report.artifacts = artifacts;

    let report_json_path = artifact_dir.join("report.json");
    write_json_file(&report_json_path, &report)?;
    let report_yaml_path = artifact_dir.join("report.yaml");
    write_yaml_file(&report_yaml_path, &report)?;
    report.artifacts.report_json_path = Some(report_json_path.display().to_string());
    report.artifacts.report_yaml_path = Some(report_yaml_path.display().to_string());
    write_json_file(&report_json_path, &report)?;
    write_yaml_file(&report_yaml_path, &report)?;

    Ok(EvalOutcome {
        report,
        artifact_dir,
    })
}

fn persist_debug_artifacts(
    config: &LocalEvalConfig,
    run: &HeadlessEvalRun,
    observations: &[ToolOutputObservation],
    artifact_dir: &Path,
    artifacts: &mut EvalReportArtifacts,
) -> Result<(), String> {
    if config.diagnostics.export_trace {
        if let Some(trace) = &run.trace {
            let trace_path = artifact_dir.join("trace.json");
            write_json_file(&trace_path, trace)?;
            artifacts.trace_path = Some(trace_path.display().to_string());
        }
    }

    if config.diagnostics.export_journal {
        let journal_path = artifact_dir.join("journal_state.json");
        write_json_file(&journal_path, &run.journal_state)?;
        artifacts.journal_path = Some(journal_path.display().to_string());

        let messages_path = artifact_dir.join("messages.json");
        write_json_file(&messages_path, &run.messages)?;
        artifacts.messages_path = Some(messages_path.display().to_string());

        let route_attempt_logs_path = artifact_dir.join("route_attempt_logs.json");
        write_json_file(&route_attempt_logs_path, &run.route_attempt_logs)?;
        artifacts.route_attempt_logs_path = Some(route_attempt_logs_path.display().to_string());

        let session_markdown_path = artifact_dir.join("session.md");
        fs::write(&session_markdown_path, &run.session_markdown)
            .map_err(|e| format!("写入 session markdown 失败: {e}"))?;
        artifacts.session_markdown_path = Some(session_markdown_path.display().to_string());
    }

    if config.diagnostics.export_stdout_stderr {
        if let Some(observation) = observations.iter().find(|item| item.exit_code.is_some()) {
            if let Some(stdout) = observation.stdout_raw.as_deref() {
                let stdout_path = artifact_dir.join("stdout.txt");
                fs::write(&stdout_path, stdout).map_err(|e| format!("写入 stdout 失败: {e}"))?;
                artifacts.stdout_path = Some(stdout_path.display().to_string());
            }
            if let Some(stderr) = observation.stderr_raw.as_deref() {
                let stderr_path = artifact_dir.join("stderr.txt");
                fs::write(&stderr_path, stderr).map_err(|e| format!("写入 stderr 失败: {e}"))?;
                artifacts.stderr_path = Some(stderr_path.display().to_string());
            }
        }
    }

    Ok(())
}

fn evaluate_route_assertions(
    scenario: &EvalScenario,
    run: &HeadlessEvalRun,
    selected_skill: Option<&str>,
    selected_runner: Option<&str>,
) -> String {
    let Some(route) = scenario.expect.route.as_ref() else {
        return "pass".to_string();
    };
    let family = route.family.trim();
    let family_hit = (!family.is_empty())
        && (selected_skill
            .map(|skill| skill.contains(family))
            .unwrap_or(false)
            || run
                .route_attempt_logs
                .iter()
                .any(|log| log.capability.contains(family)));
    let runner_allowed = scenario
        .expect
        .route
        .as_ref()
        .and_then(|route| route.runner_not.as_deref())
        .map(|blocked| selected_runner != Some(blocked))
        .unwrap_or(true);

    if family_hit && runner_allowed {
        "pass".to_string()
    } else {
        "fail".to_string()
    }
}

fn evaluate_execution_assertions(
    scenario: &EvalScenario,
    run: &HeadlessEvalRun,
    observations: &[ToolOutputObservation],
) -> String {
    let run_completed = run_completed(run);
    let Some(execution) = scenario.expect.execution.as_ref() else {
        return if run.execution_error.is_none() && run_completed {
            "pass".to_string()
        } else {
            "fail".to_string()
        };
    };
    let expected_exit_code = execution.leaf_exit_code as i64;
    let actual_exit_code = observations.iter().find_map(|item| item.exit_code);

    if run.execution_error.is_none()
        && run_completed
        && actual_exit_code == Some(expected_exit_code)
    {
        "pass".to_string()
    } else {
        "fail".to_string()
    }
}

fn run_completed(run: &HeadlessEvalRun) -> bool {
    run.trace
        .as_ref()
        .map(|trace| trace.final_status == "completed")
        .unwrap_or_else(|| {
            run.session_runs
                .last()
                .map(|item| item.status == "completed")
                .unwrap_or(false)
        })
}

fn evaluate_structured_assertions(
    scenario: &EvalScenario,
    observations: &[ToolOutputObservation],
) -> String {
    let Some(structured) = scenario.expect.structured.as_ref() else {
        return "pass".to_string();
    };
    let candidates = collect_structured_candidates(observations);
    let expected = &structured.equals;

    let employee_ok =
        find_string_value(&candidates, "employee").as_deref() == Some(expected.employee.as_str());
    let start_date_ok = find_string_value(&candidates, "start_date").as_deref()
        == Some(expected.start_date.as_str());
    let end_date_ok =
        find_string_value(&candidates, "end_date").as_deref() == Some(expected.end_date.as_str());
    let daily_count_ok = match expected.daily_count {
        Some(count) => {
            resolve_expected_count(&candidates, "daily_count", "daily_facts")
                == Some(u64::from(count))
        }
        None => true,
    };
    let plan_count_ok = match expected.plan_count {
        Some(count) => {
            resolve_expected_count(&candidates, "plan_count", "plan_facts")
                == Some(u64::from(count))
        }
        None => true,
    };
    let report_count_ok = match expected.report_count {
        Some(count) => {
            resolve_expected_count(&candidates, "report_count", "report_facts")
                == Some(u64::from(count))
        }
        None => true,
    };

    if employee_ok
        && start_date_ok
        && end_date_ok
        && daily_count_ok
        && plan_count_ok
        && report_count_ok
    {
        "pass".to_string()
    } else {
        "fail".to_string()
    }
}

fn evaluate_tool_assertions(scenario: &EvalScenario, run: &HeadlessEvalRun) -> String {
    let called_tools = collect_called_tool_names(run);
    let expected = &scenario.expect.tools;

    let called_all_ok = expected
        .called_all
        .iter()
        .all(|name| called_tools.iter().any(|called| called == name));
    let called_any_ok = if expected.called_any.is_empty() {
        true
    } else {
        expected
            .called_any
            .iter()
            .any(|name| called_tools.iter().any(|called| called == name))
    };
    let not_called_ok = expected
        .not_called
        .iter()
        .all(|name| !called_tools.iter().any(|called| called == name));

    if called_all_ok && called_any_ok && not_called_ok {
        "pass".to_string()
    } else {
        "fail".to_string()
    }
}

fn collect_called_tool_names(run: &HeadlessEvalRun) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(trace) = run.trace.as_ref() {
        names.extend(trace.tools.iter().map(|tool| tool.tool_name.clone()));
    }
    for message in &run.messages {
        let Some(items) = message.get("streamItems").and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let Some(name) = item
                .get("toolCall")
                .and_then(|tool_call| tool_call.get("name"))
                .and_then(Value::as_str)
            else {
                continue;
            };
            if !names.iter().any(|existing| existing == name) {
                names.push(name.to_string());
            }
        }
    }
    names
}

fn evaluate_output_assertions(scenario: &EvalScenario, run: &HeadlessEvalRun) -> String {
    let haystack = if run.final_output.trim().is_empty() {
        run.session_markdown.as_str()
    } else {
        run.final_output.as_str()
    };

    let contains_all_ok = scenario
        .expect
        .output
        .contains_all
        .iter()
        .all(|needle| haystack.contains(needle));
    let contains_any_ok = if scenario.expect.output.contains_any.is_empty() {
        true
    } else {
        scenario
            .expect
            .output
            .contains_any
            .iter()
            .any(|needle| haystack.contains(needle))
    };

    if contains_all_ok && contains_any_ok {
        "pass".to_string()
    } else {
        "fail".to_string()
    }
}

fn evaluate_thresholds(
    scenario: &EvalScenario,
    total_duration_ms: Option<u64>,
    turn_count: u32,
    tool_count: u32,
) -> String {
    if turn_count > scenario.thresholds.max_turn_count
        || tool_count > scenario.thresholds.max_tool_count
    {
        return "fail".to_string();
    }

    match total_duration_ms {
        Some(value) if value <= scenario.thresholds.pass_total_ms => "pass".to_string(),
        Some(value) if value <= scenario.thresholds.warn_total_ms => "warn".to_string(),
        Some(_) | None => "fail".to_string(),
    }
}

fn build_requested_metrics(
    scenario: &EvalScenario,
    run: &HeadlessEvalRun,
    total_duration_ms: Option<u64>,
    route_latency_ms: Option<u64>,
    leaf_exec_duration_ms: Option<u64>,
    turn_count: u32,
    tool_count: u32,
) -> BTreeMap<String, Value> {
    let selected_skill = (!run.skill_id.trim().is_empty()).then(|| run.skill_id.clone());
    let selected_runner = infer_selected_runner(scenario, &selected_skill);
    let fallback_reason = infer_fallback_reason(run);

    let mut metrics = BTreeMap::new();
    for key in &scenario.record_metrics {
        let value = match key.as_str() {
            "selected_skill" => selected_skill
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
            "selected_runner" => selected_runner
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
            "route_latency_ms" => route_latency_ms.map(Value::from).unwrap_or(Value::Null),
            "total_duration_ms" => total_duration_ms.map(Value::from).unwrap_or(Value::Null),
            "leaf_exec_duration_ms" => leaf_exec_duration_ms
                .map(Value::from)
                .unwrap_or(Value::Null),
            "turn_count" => Value::from(turn_count),
            "tool_count" => Value::from(tool_count),
            "called_tools" => Value::Array(
                collect_called_tool_names(run)
                    .into_iter()
                    .map(Value::String)
                    .collect(),
            ),
            "fallback_reason" => fallback_reason
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
            _ => Value::Null,
        };
        metrics.insert(key.clone(), value);
    }
    metrics
}

fn infer_selected_runner(
    scenario: &EvalScenario,
    selected_skill: &Option<String>,
) -> Option<String> {
    if selected_skill.is_none() {
        return Some("OpenTaskRunner".to_string());
    }

    match scenario.mode.as_str() {
        "direct-dispatch" => Some("DirectDispatchRunner".to_string()),
        _ => Some("SkillSessionRunner".to_string()),
    }
}

fn infer_fallback_reason(run: &HeadlessEvalRun) -> Option<String> {
    if let Some(message) = run.execution_error.as_ref() {
        return Some(message.clone());
    }

    let latest_failed_attempt = run
        .route_attempt_logs
        .iter()
        .rev()
        .find(|item| !item.success)?;
    if latest_failed_attempt.error_kind.trim().is_empty() {
        Some(latest_failed_attempt.error_message.clone())
    } else {
        Some(latest_failed_attempt.error_kind.clone())
    }
}

fn collect_tool_output_observations(messages: &[Value]) -> Vec<ToolOutputObservation> {
    let mut observations = Vec::new();
    for message in messages {
        let Some(items) = message.get("streamItems").and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let Some(tool_call) = item.get("toolCall") else {
                continue;
            };
            let raw_output = tool_call
                .get("output")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let parsed_output = serde_json::from_str::<Value>(raw_output).ok();
            let stdout_raw = parsed_output
                .as_ref()
                .and_then(|parsed| parsed.pointer("/details/stdout"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let stdout_json = stdout_raw
                .as_deref()
                .and_then(|stdout| serde_json::from_str::<Value>(stdout).ok());
            let stderr_raw = parsed_output
                .as_ref()
                .and_then(|parsed| parsed.pointer("/details/stderr"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let exit_code = parsed_output
                .as_ref()
                .and_then(|parsed| parsed.pointer("/details/exit_code"))
                .and_then(Value::as_i64);

            observations.push(ToolOutputObservation {
                parsed_output,
                stdout_raw,
                stdout_json,
                stderr_raw,
                exit_code,
            });
        }
    }
    observations
}

fn collect_structured_candidates(observations: &[ToolOutputObservation]) -> Vec<Value> {
    let mut candidates = Vec::new();
    for observation in observations {
        if let Some(stdout_json) = observation.stdout_json.as_ref() {
            candidates.push(stdout_json.clone());
        }
        if let Some(parsed_output) = observation.parsed_output.as_ref() {
            candidates.push(parsed_output.clone());
        }
    }
    candidates
}

fn find_string_value(candidates: &[Value], key: &str) -> Option<String> {
    candidates
        .iter()
        .find_map(|candidate| find_key_value(candidate, key))
        .and_then(|value| value.as_str().map(str::to_string))
}

fn find_u64_value(candidates: &[Value], key: &str) -> Option<u64> {
    candidates
        .iter()
        .find_map(|candidate| find_key_value(candidate, key))
        .and_then(Value::as_u64)
}

fn find_array_len_value(candidates: &[Value], key: &str) -> Option<u64> {
    candidates
        .iter()
        .find_map(|candidate| find_key_value(candidate, key))
        .and_then(Value::as_array)
        .map(|items| items.len() as u64)
}

fn resolve_expected_count(candidates: &[Value], count_key: &str, facts_key: &str) -> Option<u64> {
    find_u64_value(candidates, count_key).or_else(|| find_array_len_value(candidates, facts_key))
}

fn find_key_value<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => {
            if let Some(found) = map.get(key) {
                return Some(found);
            }
            map.values().find_map(|nested| find_key_value(nested, key))
        }
        Value::Array(items) => items.iter().find_map(|nested| find_key_value(nested, key)),
        _ => None,
    }
}

fn compute_total_duration_ms(run: &HeadlessEvalRun) -> Option<u64> {
    let start = run
        .session_runs
        .first()
        .and_then(|item| parse_rfc3339(&item.created_at))
        .or_else(|| {
            run.trace
                .as_ref()
                .and_then(|trace| trace.first_event_at.as_deref())
                .and_then(parse_rfc3339)
        })?;
    let end = run
        .session_runs
        .last()
        .and_then(|item| parse_rfc3339(&item.updated_at))
        .or_else(|| {
            run.trace
                .as_ref()
                .and_then(|trace| trace.last_event_at.as_deref())
                .and_then(parse_rfc3339)
        })?;
    Some((end - start).num_milliseconds().max(0) as u64)
}

fn compute_route_latency_ms(run: &HeadlessEvalRun) -> Option<u64> {
    let route_at = run
        .route_attempt_logs
        .iter()
        .map(|item| item.created_at.as_str())
        .filter_map(parse_rfc3339)
        .min()?;
    let first_event = run
        .trace
        .as_ref()
        .and_then(|trace| trace.first_event_at.as_deref())
        .and_then(parse_rfc3339)
        .or_else(|| {
            run.session_runs
                .first()
                .and_then(|item| parse_rfc3339(&item.created_at))
        })?;
    Some((first_event - route_at).num_milliseconds().max(0) as u64)
}

fn compute_leaf_exec_duration_ms(run: &HeadlessEvalRun) -> Option<u64> {
    let trace = run.trace.as_ref()?;
    let started_at = trace
        .events
        .iter()
        .find(|event| event.event_type == "tool_started")
        .and_then(|event| parse_rfc3339(&event.created_at))?;
    let completed_at = trace
        .events
        .iter()
        .rev()
        .find(|event| event.event_type == "tool_completed")
        .and_then(|event| parse_rfc3339(&event.created_at))?;
    Some((completed_at - started_at).num_milliseconds().max(0) as u64)
}

fn parse_rfc3339(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    let mut chars = trimmed.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn sanitize_id_component(raw: &str) -> String {
    let lowered = raw.trim().to_ascii_lowercase();
    let mut out = String::new();
    let mut previous_dash = false;
    for ch in lowered.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            out.push('-');
            previous_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn write_json_file(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(value).map_err(|e| format!("序列化 JSON 失败: {e}"))?;
    fs::write(path, raw).map_err(|e| format!("写入 JSON 文件失败: {e}"))
}

fn write_yaml_file(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let raw = serde_yaml::to_string(value).map_err(|e| format!("序列化 YAML 失败: {e}"))?;
    fs::write(path, raw).map_err(|e| format!("写入 YAML 文件失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{evaluate_and_write_report, EvalOutcome};
    use crate::agent::evals::{EvalScenario, HeadlessEvalRun, LocalEvalConfig};
    use crate::agent::runtime::trace_builder::{
        RunTraceToolSummary, SessionRunEventSummary, SessionRunTrace, SessionRunTraceLifecycle,
    };
    use crate::commands::models::RouteAttemptLog;
    use crate::commands::session_runs::SessionRunProjection;
    use crate::session_journal::SessionJournalState;
    use serde_json::json;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    fn scenario_fixture_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("agent-evals")
            .join("scenarios")
            .join("pm_weekly_summary_xietao_2026_03_30_2026_04_04.yaml")
    }

    fn load_scenario() -> EvalScenario {
        let raw = fs::read_to_string(scenario_fixture_path()).expect("read scenario fixture");
        serde_yaml::from_str(&raw).expect("parse scenario")
    }

    fn test_config(output_dir: &Path) -> LocalEvalConfig {
        serde_yaml::from_str(&format!(
            r#"
runtime:
  workspace_root: D:\\code\\WorkClaw
models:
  default_profile: local_eval
providers:
  local_eval:
    provider: openai
    model: gpt-5.4
    api_key_env: OPENAI_API_KEY
artifacts:
  output_dir: {}
capabilities:
  pm_weekly_summary:
    workspace_root: E:\\code\\work\\飞书多维表格自动化skill
    entry_kind: workspace_skill
    entry_name: feishu-pm-hub
diagnostics:
  export_journal: true
  export_trace: true
  export_stdout_stderr: true
"#,
            output_dir.display()
        ))
        .expect("parse test config")
    }

    fn build_run(total_duration_ms: u64, daily_count: u64) -> HeadlessEvalRun {
        let total_seconds = total_duration_ms / 1000;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        let completed = format!("2026-04-04T09:{minutes:02}:{seconds:02}Z");
        let stdout_payload = json!({
            "employee": "谢涛",
            "start_date": "2026-03-30",
            "end_date": "2026-04-04",
            "daily_count": daily_count,
            "plan_count": 6,
            "report_count": 5
        })
        .to_string();
        let tool_output = json!({
            "ok": true,
            "tool": "exec",
            "summary": "命令执行完成（退出码 0）",
            "details": {
                "exit_code": 0,
                "stdout": stdout_payload,
                "stderr": ""
            }
        })
        .to_string();

        HeadlessEvalRun {
            scenario_id: "pm_weekly_summary_xietao_2026_03_30_2026_04_04".to_string(),
            capability_id: "pm_weekly_summary".to_string(),
            session_id: "session-1".to_string(),
            skill_id: "feishu-pm-hub".to_string(),
            model_id: "eval-local".to_string(),
            work_dir: PathBuf::from("D:/code/WorkClaw/temp/agent-evals/workspaces/pm"),
            imported_skill_count: 1,
            missing_mcp: Vec::new(),
            execution_error: None,
            session_runs: vec![SessionRunProjection {
                id: "run-1".to_string(),
                session_id: "session-1".to_string(),
                user_message_id: "user-1".to_string(),
                assistant_message_id: Some("assistant-1".to_string()),
                status: "completed".to_string(),
                buffered_text: String::new(),
                error_kind: None,
                error_message: None,
                created_at: "2026-04-04T09:00:00Z".to_string(),
                updated_at: completed,
                task_identity: None,
                turn_state: None,
                task_path: None,
                task_status: None,
                task_record: None,
                task_continuation_mode: None,
                task_continuation_source: None,
                task_continuation_reason: None,
            }],
            route_attempt_logs: vec![RouteAttemptLog {
                session_id: "session-1".to_string(),
                capability: "feishu-pm".to_string(),
                api_format: "openai".to_string(),
                model_name: "gpt-5.4".to_string(),
                attempt_index: 0,
                retry_index: 0,
                error_kind: String::new(),
                success: true,
                error_message: String::new(),
                created_at: "2026-04-04T08:59:59Z".to_string(),
            }],
            trace: Some(SessionRunTrace {
                session_id: "session-1".to_string(),
                run_id: "run-1".to_string(),
                final_status: "completed".to_string(),
                event_count: 4,
                first_event_at: Some("2026-04-04T09:00:00Z".to_string()),
                last_event_at: Some("2026-04-04T09:00:12Z".to_string()),
                lifecycle: SessionRunTraceLifecycle {
                    started: true,
                    completed: true,
                    failed: false,
                    cancelled: false,
                    stopped: false,
                    waiting_approval: false,
                },
                stop_reason_kind: None,
                tools: vec![RunTraceToolSummary {
                    call_id: "call-1".to_string(),
                    tool_name: "skill".to_string(),
                    status: "completed".to_string(),
                    input_preview: None,
                    output_preview: None,
                    child_session_id: None,
                    is_error: false,
                }],
                approvals: Vec::new(),
                guard_warnings: Vec::new(),
                parse_warnings: Vec::new(),
                child_session_link: None,
                task_graph: Vec::new(),
                events: vec![
                    SessionRunEventSummary {
                        session_id: "session-1".to_string(),
                        run_id: "run-1".to_string(),
                        event_type: "run_started".to_string(),
                        created_at: "2026-04-04T09:00:00Z".to_string(),
                        status: Some("thinking".to_string()),
                        tool_name: None,
                        call_id: None,
                        approval_id: None,
                        warning_kind: None,
                        error_kind: None,
                        message: None,
                        detail: None,
                        irreversible: None,
                        last_completed_step: None,
                        child_session_id: None,
                        is_error: None,
                        parse_warning: None,
                    },
                    SessionRunEventSummary {
                        session_id: "session-1".to_string(),
                        run_id: "run-1".to_string(),
                        event_type: "tool_started".to_string(),
                        created_at: "2026-04-04T09:00:01Z".to_string(),
                        status: Some("tool_calling".to_string()),
                        tool_name: Some("skill".to_string()),
                        call_id: Some("call-1".to_string()),
                        approval_id: None,
                        warning_kind: None,
                        error_kind: None,
                        message: None,
                        detail: None,
                        irreversible: None,
                        last_completed_step: None,
                        child_session_id: None,
                        is_error: Some(false),
                        parse_warning: None,
                    },
                    SessionRunEventSummary {
                        session_id: "session-1".to_string(),
                        run_id: "run-1".to_string(),
                        event_type: "tool_completed".to_string(),
                        created_at: "2026-04-04T09:00:08Z".to_string(),
                        status: Some("thinking".to_string()),
                        tool_name: Some("skill".to_string()),
                        call_id: Some("call-1".to_string()),
                        approval_id: None,
                        warning_kind: None,
                        error_kind: None,
                        message: None,
                        detail: None,
                        irreversible: None,
                        last_completed_step: None,
                        child_session_id: None,
                        is_error: Some(false),
                        parse_warning: None,
                    },
                    SessionRunEventSummary {
                        session_id: "session-1".to_string(),
                        run_id: "run-1".to_string(),
                        event_type: "run_completed".to_string(),
                        created_at: "2026-04-04T09:00:12Z".to_string(),
                        status: Some("completed".to_string()),
                        tool_name: None,
                        call_id: None,
                        approval_id: None,
                        warning_kind: None,
                        error_kind: None,
                        message: None,
                        detail: None,
                        irreversible: None,
                        last_completed_step: None,
                        child_session_id: None,
                        is_error: Some(false),
                        parse_warning: None,
                    },
                ],
            }),
            messages: vec![json!({
                "id": "assistant-1",
                "role": "assistant",
                "content": "谢涛在该时间窗内主要推进金川区域排水管网改造工程（一期）和土左2025老旧小区改造，并继续跟进排污通道图纸跟进。",
                "streamItems": [{
                    "type": "tool_call",
                    "toolCall": {
                        "id": "call-1",
                        "name": "skill",
                        "output": tool_output,
                        "status": "completed"
                    }
                }]
            })],
            session_markdown: "session markdown".to_string(),
            journal_state: SessionJournalState::default(),
            final_output: "谢涛在该时间窗内主要推进金川区域排水管网改造工程（一期）和土左2025老旧小区改造，并继续跟进排污通道图纸跟进。".to_string(),
        }
    }

    fn build_run_with_nested_summary_facts(
        total_duration_ms: u64,
        daily_count: u64,
    ) -> HeadlessEvalRun {
        let total_seconds = total_duration_ms / 1000;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        let completed = format!("2026-04-04T09:{minutes:02}:{seconds:02}Z");
        let stdout_payload = json!({
            "app_token": "app-token",
            "start_date": "2026-03-30",
            "end_date": "2026-04-04",
            "summaries": [{
                "employee": "谢涛",
                "start_date": "2026-03-30",
                "end_date": "2026-04-04",
                "daily_facts": vec![json!({"title": "日报"}); daily_count as usize],
                "plan_facts": vec![json!({"title": "计划"}); 6],
                "report_facts": vec![json!({"title": "汇报"}); 5],
                "summary": "谢涛在该时间窗内主要推进金川区域排水管网改造工程（一期）和土左2025老旧小区改造，并继续跟进排污通道图纸跟进。"
            }]
        })
        .to_string();
        let tool_output = json!({
            "ok": true,
            "tool": "exec",
            "summary": "命令执行完成（退出码 0）",
            "details": {
                "exit_code": 0,
                "stdout": stdout_payload,
                "stderr": ""
            }
        })
        .to_string();

        HeadlessEvalRun {
            scenario_id: "pm_weekly_summary_xietao_2026_03_30_2026_04_04".to_string(),
            capability_id: "pm_weekly_summary".to_string(),
            session_id: "session-1".to_string(),
            skill_id: "feishu-pm-hub".to_string(),
            model_id: "eval-local".to_string(),
            work_dir: PathBuf::from("D:/code/WorkClaw/temp/agent-evals/workspaces/pm"),
            imported_skill_count: 1,
            missing_mcp: Vec::new(),
            execution_error: None,
            session_runs: vec![SessionRunProjection {
                id: "run-1".to_string(),
                session_id: "session-1".to_string(),
                user_message_id: "user-1".to_string(),
                assistant_message_id: Some("assistant-1".to_string()),
                status: "completed".to_string(),
                buffered_text: String::new(),
                error_kind: None,
                error_message: None,
                created_at: "2026-04-04T09:00:00Z".to_string(),
                updated_at: completed,
                task_identity: None,
                turn_state: None,
                task_path: None,
                task_status: None,
                task_record: None,
                task_continuation_mode: None,
                task_continuation_source: None,
                task_continuation_reason: None,
            }],
            route_attempt_logs: vec![RouteAttemptLog {
                session_id: "session-1".to_string(),
                capability: "feishu-pm".to_string(),
                api_format: "openai".to_string(),
                model_name: "gpt-5.4".to_string(),
                attempt_index: 0,
                retry_index: 0,
                error_kind: String::new(),
                success: true,
                error_message: String::new(),
                created_at: "2026-04-04T08:59:59Z".to_string(),
            }],
            trace: Some(SessionRunTrace {
                session_id: "session-1".to_string(),
                run_id: "run-1".to_string(),
                final_status: "completed".to_string(),
                event_count: 4,
                first_event_at: Some("2026-04-04T09:00:00Z".to_string()),
                last_event_at: Some("2026-04-04T09:00:12Z".to_string()),
                lifecycle: SessionRunTraceLifecycle {
                    started: true,
                    completed: true,
                    failed: false,
                    cancelled: false,
                    stopped: false,
                    waiting_approval: false,
                },
                stop_reason_kind: None,
                tools: vec![RunTraceToolSummary {
                    call_id: "call-1".to_string(),
                    tool_name: "skill".to_string(),
                    status: "completed".to_string(),
                    input_preview: None,
                    output_preview: None,
                    child_session_id: None,
                    is_error: false,
                }],
                approvals: Vec::new(),
                guard_warnings: Vec::new(),
                parse_warnings: Vec::new(),
                child_session_link: None,
                task_graph: Vec::new(),
                events: vec![
                    SessionRunEventSummary {
                        session_id: "session-1".to_string(),
                        run_id: "run-1".to_string(),
                        event_type: "run_started".to_string(),
                        created_at: "2026-04-04T09:00:00Z".to_string(),
                        status: Some("thinking".to_string()),
                        tool_name: None,
                        call_id: None,
                        approval_id: None,
                        warning_kind: None,
                        error_kind: None,
                        message: None,
                        detail: None,
                        irreversible: None,
                        last_completed_step: None,
                        child_session_id: None,
                        is_error: None,
                        parse_warning: None,
                    },
                    SessionRunEventSummary {
                        session_id: "session-1".to_string(),
                        run_id: "run-1".to_string(),
                        event_type: "tool_started".to_string(),
                        created_at: "2026-04-04T09:00:01Z".to_string(),
                        status: Some("tool_calling".to_string()),
                        tool_name: Some("skill".to_string()),
                        call_id: Some("call-1".to_string()),
                        approval_id: None,
                        warning_kind: None,
                        error_kind: None,
                        message: None,
                        detail: None,
                        irreversible: None,
                        last_completed_step: None,
                        child_session_id: None,
                        is_error: Some(false),
                        parse_warning: None,
                    },
                    SessionRunEventSummary {
                        session_id: "session-1".to_string(),
                        run_id: "run-1".to_string(),
                        event_type: "tool_completed".to_string(),
                        created_at: "2026-04-04T09:00:08Z".to_string(),
                        status: Some("thinking".to_string()),
                        tool_name: Some("skill".to_string()),
                        call_id: Some("call-1".to_string()),
                        approval_id: None,
                        warning_kind: None,
                        error_kind: None,
                        message: None,
                        detail: None,
                        irreversible: None,
                        last_completed_step: None,
                        child_session_id: None,
                        is_error: Some(false),
                        parse_warning: None,
                    },
                    SessionRunEventSummary {
                        session_id: "session-1".to_string(),
                        run_id: "run-1".to_string(),
                        event_type: "run_completed".to_string(),
                        created_at: "2026-04-04T09:00:12Z".to_string(),
                        status: Some("completed".to_string()),
                        tool_name: None,
                        call_id: None,
                        approval_id: None,
                        warning_kind: None,
                        error_kind: None,
                        message: None,
                        detail: None,
                        irreversible: None,
                        last_completed_step: None,
                        child_session_id: None,
                        is_error: Some(false),
                        parse_warning: None,
                    },
                ],
            }),
            messages: vec![json!({
                "id": "assistant-1",
                "role": "assistant",
                "content": "谢涛在该时间窗内主要推进金川区域排水管网改造工程（一期）和土左2025老旧小区改造，并继续跟进排污通道图纸跟进。",
                "streamItems": [{
                    "type": "tool_call",
                    "toolCall": {
                        "id": "call-1",
                        "name": "skill",
                        "output": tool_output,
                        "status": "completed"
                    }
                }]
            })],
            session_markdown: "session markdown".to_string(),
            journal_state: SessionJournalState::default(),
            final_output: "谢涛在该时间窗内主要推进金川区域排水管网改造工程（一期）和土左2025老旧小区改造，并继续跟进排污通道图纸跟进。".to_string(),
        }
    }

    #[test]
    fn evaluate_and_write_report_returns_pass_for_matching_golden_case() {
        let temp = tempdir().expect("tempdir");
        let config = test_config(temp.path());
        let scenario = load_scenario();
        let run = build_run(90_000, 6);

        let outcome = evaluate_and_write_report(&config, &scenario, &run).expect("evaluate report");

        assert_eq!(
            outcome.report.status,
            crate::agent::evals::EvalReportStatus::Pass
        );
        assert_eq!(outcome.report.assertions.route, "pass");
        assert_eq!(outcome.report.assertions.execution, "pass");
        assert_eq!(outcome.report.assertions.structured, "pass");
        assert_eq!(outcome.report.assertions.output, "pass");
        assert_eq!(outcome.report.assertions.thresholds, "pass");
        assert_eq!(outcome.report.metrics["selected_skill"], "feishu-pm-hub");
        assert!(outcome.report.artifacts.report_json_path.is_some());
        assert!(outcome.artifact_dir.join("report.yaml").exists());
    }

    #[test]
    fn evaluate_and_write_report_returns_warn_when_only_duration_exceeds_pass_threshold() {
        let temp = tempdir().expect("tempdir");
        let config = test_config(temp.path());
        let scenario = load_scenario();
        let run = build_run(160_000, 6);

        let outcome = evaluate_and_write_report(&config, &scenario, &run).expect("evaluate report");

        assert_eq!(
            outcome.report.status,
            crate::agent::evals::EvalReportStatus::Warn
        );
        assert_eq!(outcome.report.assertions.thresholds, "warn");
    }

    #[test]
    fn evaluate_and_write_report_returns_fail_for_structured_mismatch() {
        let temp = tempdir().expect("tempdir");
        let config = test_config(temp.path());
        let scenario = load_scenario();
        let run = build_run(90_000, 5);

        let outcome = evaluate_and_write_report(&config, &scenario, &run).expect("evaluate report");

        assert_eq!(
            outcome.report.status,
            crate::agent::evals::EvalReportStatus::Fail
        );
        assert_eq!(outcome.report.assertions.structured, "fail");
    }

    #[test]
    fn evaluate_and_write_report_accepts_nested_summary_fact_lengths() {
        let temp = tempdir().expect("tempdir");
        let config = test_config(temp.path());
        let scenario = load_scenario();
        let run = build_run_with_nested_summary_facts(90_000, 6);

        let outcome = evaluate_and_write_report(&config, &scenario, &run).expect("evaluate report");

        assert_eq!(
            outcome.report.status,
            crate::agent::evals::EvalReportStatus::Pass
        );
        assert_eq!(outcome.report.assertions.structured, "pass");
    }

    #[test]
    fn evaluate_and_write_report_accepts_tool_only_runtime_scenario() {
        let temp = tempdir().expect("tempdir");
        let config = test_config(temp.path());
        let mut scenario = load_scenario();
        scenario.capability_id = "workspace_image_set_vision".to_string();
        scenario.expect.route = None;
        scenario.expect.execution = None;
        scenario.expect.structured = None;
        scenario.expect.output.contains_all.clear();
        scenario.expect.output.contains_any.clear();
        scenario.expect.tools.called_all = vec!["vision_analyze".to_string()];
        scenario.record_metrics = vec!["called_tools".to_string()];
        let mut run = build_run(90_000, 6);
        run.skill_id = "builtin-general".to_string();
        run.route_attempt_logs.clear();
        if let Some(trace) = run.trace.as_mut() {
            trace.tools[0].tool_name = "vision_analyze".to_string();
        }
        if let Some(name) = run.messages[0].pointer_mut("/streamItems/0/toolCall/name") {
            *name = serde_json::json!("vision_analyze");
        }

        let outcome = evaluate_and_write_report(&config, &scenario, &run).expect("evaluate report");

        assert_eq!(
            outcome.report.status,
            crate::agent::evals::EvalReportStatus::Pass
        );
        assert_eq!(outcome.report.assertions.route, "pass");
        assert_eq!(outcome.report.assertions.execution, "pass");
        assert_eq!(outcome.report.assertions.structured, "pass");
        assert_eq!(outcome.report.assertions.tools, "pass");
        assert_eq!(
            outcome.report.metrics["called_tools"],
            serde_json::json!(["vision_analyze"])
        );
    }

    #[test]
    fn evaluate_and_write_report_exports_debug_artifacts() {
        let temp = tempdir().expect("tempdir");
        let config = test_config(temp.path());
        let scenario = load_scenario();
        let run = build_run(90_000, 6);

        let EvalOutcome {
            report,
            artifact_dir,
        } = evaluate_and_write_report(&config, &scenario, &run).expect("evaluate report");

        assert!(artifact_dir.join("trace.json").exists());
        assert!(artifact_dir.join("stdout.txt").exists());
        assert!(artifact_dir.join("messages.json").exists());
        assert!(report.artifacts.trace_path.is_some());
        assert!(report.artifacts.stdout_path.is_some());
    }
}
