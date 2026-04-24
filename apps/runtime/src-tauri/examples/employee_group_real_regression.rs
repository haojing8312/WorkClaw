#![recursion_limit = "256"]

use anyhow::{anyhow, bail, Context, Result};
#[path = "../../tests/helpers/mod.rs"]
mod test_helpers;

use runtime_lib::commands::employee_agents::test_support::{
    create_employee_group_with_pool, create_employee_team_with_pool,
    start_employee_group_run_with_pool,
};
use runtime_lib::commands::employee_agents::{
    upsert_agent_employee_with_pool, CreateEmployeeGroupInput, CreateEmployeeTeamInput,
    EmployeeGroupRunStep, StartEmployeeGroupRunInput, UpsertAgentEmployeeInput,
};
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
struct RealModelConfig {
    api_format: String,
    base_url: String,
    model_name: String,
    api_key: String,
}

#[derive(Debug, serde::Serialize)]
struct ScenarioSummary {
    scenario: &'static str,
    run_id: String,
    state: String,
    execute_edges: Vec<String>,
}

fn main() -> Result<()> {
    let handle = std::thread::Builder::new()
        .name("employee-group-real-regression".to_string())
        .stack_size(128 * 1024 * 1024)
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .context("build tokio runtime")?;
            runtime.block_on(async_main())
        })
        .context("spawn regression runner thread")?;

    handle
        .join()
        .map_err(|_| anyhow!("regression runner thread panicked"))?
}

async fn async_main() -> Result<()> {
    let config = load_real_model_config()?;
    let (pool, runtime_tmp) = test_helpers::setup_test_db().await;

    seed_model_config(&pool, &config).await?;

    let single_member =
        run_single_member_self_execute_regression(&pool, runtime_tmp.path()).await?;
    let multi_member = run_multi_member_no_rules_regression(&pool, runtime_tmp.path()).await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "provider": {
                "api_format": config.api_format,
                "base_url": mask_base_url(&config.base_url),
                "model_name": config.model_name,
                "api_key": mask_api_key(&config.api_key),
            },
            "runtime_root": runtime_tmp.path().to_string_lossy(),
            "results": [single_member, multi_member],
        }))?
    );

    Ok(())
}

fn load_real_model_config() -> Result<RealModelConfig> {
    let api_key = read_required_env("WORKCLAW_REAL_MODEL_API_KEY")?;
    let raw_base_url = read_required_env("WORKCLAW_REAL_MODEL_BASE_URL")?;
    let model_name = read_required_env("WORKCLAW_REAL_MODEL_NAME")?;
    let api_format =
        std::env::var("WORKCLAW_REAL_MODEL_API_FORMAT").unwrap_or_else(|_| "openai".to_string());

    Ok(RealModelConfig {
        api_format,
        base_url: normalize_openai_compatible_base_url(&raw_base_url),
        model_name,
        api_key,
    })
}

fn read_required_env(name: &str) -> Result<String> {
    let value = std::env::var(name)
        .with_context(|| format!("missing required environment variable {name}"))?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("environment variable {name} cannot be empty");
    }
    Ok(trimmed.to_string())
}

fn normalize_openai_compatible_base_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        return trimmed.to_string();
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return format!("{trimmed}/v1");
    }
    trimmed.to_string()
}

async fn seed_model_config(pool: &SqlitePool, config: &RealModelConfig) -> Result<()> {
    sqlx::query("DELETE FROM model_configs")
        .execute(pool)
        .await
        .context("clear existing model configs")?;

    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('real-model', 'real-model', ?, ?, ?, 1, ?)",
    )
    .bind(&config.api_format)
    .bind(&config.base_url)
    .bind(&config.model_name)
    .bind(&config.api_key)
    .execute(pool)
    .await
    .context("insert real model config")?;

    Ok(())
}

async fn run_single_member_self_execute_regression(
    pool: &SqlitePool,
    tmp_root: &std::path::Path,
) -> Result<ScenarioSummary> {
    let employee_id = "dahuangzi";
    ensure_employee(
        pool,
        tmp_root,
        employee_id,
        "大皇子",
        "负责单人执行，只输出简洁中文结论，不调用工具，不重复委派自己。",
        true,
    )
    .await?;

    let group_id = create_employee_team_with_pool(
        pool,
        CreateEmployeeTeamInput {
            name: "单人收口回归".to_string(),
            coordinator_employee_id: employee_id.to_string(),
            member_employee_ids: vec![employee_id.to_string()],
            entry_employee_id: employee_id.to_string(),
            planner_employee_id: employee_id.to_string(),
            reviewer_employee_id: String::new(),
            review_mode: "none".to_string(),
            execution_mode: "sequential".to_string(),
            visibility_mode: "internal".to_string(),
            rules: vec![],
        },
    )
    .await
    .map_err(|error| anyhow!("create single-member team: {error}"))?;

    let outcome = start_employee_group_run_with_pool(
        pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "请直接给出一句中文执行结论，不要调用工具。".to_string(),
            execution_window: 1,
            timeout_employee_ids: vec![],
            max_retry_per_step: 1,
        },
    )
    .await
    .map_err(|error| anyhow!("start single-member group run: {error}"))?;

    assert_run_done("single_member_self_execute", &outcome.state)?;
    assert_no_self_dispatch_edges("single_member_self_execute", &outcome.steps)?;

    Ok(ScenarioSummary {
        scenario: "single_member_self_execute",
        run_id: outcome.run_id,
        state: outcome.state,
        execute_edges: summarize_execute_edges(&outcome.steps),
    })
}

async fn run_multi_member_no_rules_regression(
    pool: &SqlitePool,
    tmp_root: &std::path::Path,
) -> Result<ScenarioSummary> {
    for (employee_id, name, is_default) in [
        ("taizi", "太子", false),
        ("zhongshu", "中书", false),
        ("dahuangzi2", "大皇子二号", false),
        ("shangshu", "尚书", true),
    ] {
        ensure_employee(
            pool,
            tmp_root,
            employee_id,
            name,
            "负责多人协作执行，只输出简洁中文结论，不调用工具，不重复委派自己。",
            is_default,
        )
        .await?;
    }

    let group_id = create_employee_group_with_pool(
        pool,
        CreateEmployeeGroupInput {
            name: "多人无规则回归".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec![
                "taizi".to_string(),
                "zhongshu".to_string(),
                "dahuangzi2".to_string(),
                "shangshu".to_string(),
            ],
        },
    )
    .await
    .map_err(|error| anyhow!("create multi-member group: {error}"))?;

    let outcome = start_employee_group_run_with_pool(
        pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "请每位执行者都给出一句中文执行结论，不要调用工具。".to_string(),
            execution_window: 2,
            timeout_employee_ids: vec![],
            max_retry_per_step: 1,
        },
    )
    .await
    .map_err(|error| anyhow!("start multi-member group run: {error}"))?;

    assert_run_done("multi_member_no_rules", &outcome.state)?;
    assert_no_self_dispatch_edges("multi_member_no_rules", &outcome.steps)?;

    let execute_edges = summarize_execute_edges(&outcome.steps);
    if execute_edges.is_empty() {
        bail!("multi_member_no_rules produced no execute steps");
    }

    Ok(ScenarioSummary {
        scenario: "multi_member_no_rules",
        run_id: outcome.run_id,
        state: outcome.state,
        execute_edges,
    })
}

async fn ensure_employee(
    pool: &SqlitePool,
    tmp_root: &std::path::Path,
    employee_id: &str,
    name: &str,
    persona: &str,
    is_default: bool,
) -> Result<()> {
    let employee_workdir = tmp_root.join("employees").join(employee_id);
    std::fs::create_dir_all(&employee_workdir)
        .with_context(|| format!("create workdir for {employee_id}"))?;

    upsert_agent_employee_with_pool(
        pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: employee_id.to_string(),
            name: name.to_string(),
            role_id: employee_id.to_string(),
            persona: persona.to_string(),
            feishu_open_id: String::new(),
            feishu_app_id: String::new(),
            feishu_app_secret: String::new(),
            primary_skill_id: "real-regression".to_string(),
            default_work_dir: employee_workdir.to_string_lossy().to_string(),
            openclaw_agent_id: employee_id.to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["app".to_string()],
            enabled: true,
            is_default,
            skill_ids: vec![],
        },
    )
    .await
    .map_err(|error| anyhow!("upsert employee {employee_id}: {error}"))?;

    Ok(())
}

fn assert_run_done(scenario: &str, state: &str) -> Result<()> {
    if state == "done" {
        return Ok(());
    }
    bail!("{scenario} expected state=done, got state={state}");
}

fn assert_no_self_dispatch_edges(scenario: &str, steps: &[EmployeeGroupRunStep]) -> Result<()> {
    let offenders = steps
        .iter()
        .filter(|step| step.step_type == "execute")
        .filter_map(|step| {
            let source = step.dispatch_source_employee_id.trim();
            let assignee = step.assignee_employee_id.trim();
            (!source.is_empty() && source.eq_ignore_ascii_case(assignee)).then(|| {
                format!(
                    "{} -> {} (step_id={})",
                    step.dispatch_source_employee_id, step.assignee_employee_id, step.id
                )
            })
        })
        .collect::<Vec<_>>();

    if offenders.is_empty() {
        return Ok(());
    }

    bail!(
        "{scenario} emitted self-dispatch edges: {}",
        offenders.join(", ")
    );
}

fn summarize_execute_edges(steps: &[EmployeeGroupRunStep]) -> Vec<String> {
    steps
        .iter()
        .filter(|step| step.step_type == "execute")
        .map(|step| {
            let source = step.dispatch_source_employee_id.trim();
            if source.is_empty() {
                format!("SELF_EXECUTE -> {}", step.assignee_employee_id)
            } else {
                format!("{source} -> {}", step.assignee_employee_id)
            }
        })
        .collect()
}

fn mask_api_key(api_key: &str) -> String {
    if api_key.len() <= 8 {
        return "***".to_string();
    }
    format!("{}***{}", &api_key[..4], &api_key[api_key.len() - 4..])
}

fn mask_base_url(base_url: &str) -> String {
    base_url.trim().to_string()
}
