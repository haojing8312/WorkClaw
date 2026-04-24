#![recursion_limit = "256"]

use anyhow::Context;
use runtime_lib::agent::evals::{
    evaluate_and_write_report, EvalReportStatus, EvalScenario, LocalEvalConfig, RealAgentEvalRunner,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

struct CliArgs {
    scenario_id: String,
    config_path: PathBuf,
}

fn main() {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("[agent-eval] {error}");
            std::process::exit(1);
        }
    };

    match runtime.block_on(run()) {
        Ok(status) => {
            let code = match status {
                EvalReportStatus::Pass | EvalReportStatus::Warn => 0,
                EvalReportStatus::Fail => 1,
            };
            std::process::exit(code);
        }
        Err(error) => {
            eprintln!("[agent-eval] {error}");
            std::process::exit(1);
        }
    }
}

async fn run() -> Result<EvalReportStatus, String> {
    let args = parse_args(env::args().skip(1).collect())?;
    let config = load_yaml::<LocalEvalConfig>(&args.config_path)?;
    let scenario_path = scenario_path_for(&args.scenario_id);
    let scenario = load_yaml::<EvalScenario>(&scenario_path)?;
    if !scenario.enabled {
        return Err(format!("场景已禁用: {}", scenario.id));
    }

    let runner = RealAgentEvalRunner::new(&config).await?;
    let run = runner.run_scenario(&config, &scenario).await?;
    let outcome = evaluate_and_write_report(&config, &scenario, &run)?;

    let report = &outcome.report;
    println!(
        "[agent-eval] scenario={} status={:?} total_ms={:?} skill={:?} runner={:?}",
        report.scenario_id,
        report.status,
        report.timing.total_duration_ms,
        report.decision.selected_skill,
        report.decision.selected_runner
    );
    if let Some(path) = &report.artifacts.report_yaml_path {
        println!("[agent-eval] report={path}");
    }

    Ok(report.status.clone())
}

fn parse_args(args: Vec<String>) -> Result<CliArgs, String> {
    let mut scenario_id: Option<String> = None;
    let mut config_path = default_config_path();

    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--scenario" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--scenario 缺少值".to_string())?;
                scenario_id = Some(value.clone());
                index += 2;
            }
            "--config" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--config 缺少值".to_string())?;
                config_path = PathBuf::from(value);
                index += 2;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => {
                return Err(format!("未知参数: {other}"));
            }
        }
    }

    let scenario_id = scenario_id.ok_or_else(|| "缺少 --scenario".to_string())?;
    Ok(CliArgs {
        scenario_id,
        config_path,
    })
}

fn print_usage() {
    eprintln!(
        "Usage: cargo run --manifest-path apps/runtime/src-tauri/Cargo.toml --example agent_eval -- --scenario <id> [--config <path>]"
    );
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn default_config_path() -> PathBuf {
    repo_root()
        .join("agent-evals")
        .join("config")
        .join("config.local.yaml")
}

fn scenario_path_for(scenario_id: &str) -> PathBuf {
    repo_root()
        .join("agent-evals")
        .join("scenarios")
        .join(format!("{scenario_id}.yaml"))
}

fn load_yaml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let raw =
        fs::read_to_string(path).map_err(|e| format!("读取文件失败 {}: {e}", path.display()))?;
    serde_yaml::from_str(&raw).map_err(|e| format!("解析 YAML 失败 {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{default_config_path, parse_args, scenario_path_for};

    #[test]
    fn parse_args_accepts_scenario_and_optional_config() {
        let parsed = parse_args(vec![
            "--scenario".to_string(),
            "pm_weekly_summary".to_string(),
            "--config".to_string(),
            "D:/tmp/config.local.yaml".to_string(),
        ])
        .expect("parse args");

        assert_eq!(parsed.scenario_id, "pm_weekly_summary");
        assert_eq!(
            parsed.config_path.to_string_lossy(),
            "D:/tmp/config.local.yaml"
        );
    }

    #[test]
    fn parse_args_uses_default_local_config_path() {
        let parsed = parse_args(vec![
            "--scenario".to_string(),
            "pm_weekly_summary".to_string(),
        ])
        .expect("parse args");

        assert_eq!(parsed.scenario_id, "pm_weekly_summary");
        assert_eq!(parsed.config_path, default_config_path());
        assert_eq!(
            scenario_path_for("pm_weekly_summary")
                .file_name()
                .and_then(|value| value.to_str()),
            Some("pm_weekly_summary.yaml")
        );
    }
}
