#![recursion_limit = "256"]

use anyhow::{anyhow, Context, Result};
#[path = "../../tests/helpers/mod.rs"]
mod test_helpers;

use runtime_lib::commands::employee_agents::test_support::{
    create_employee_group_with_pool, start_employee_group_run_with_pool,
};
use runtime_lib::commands::employee_agents::{
    reassign_group_run_step_with_pool, upsert_agent_employee_with_pool, CreateEmployeeGroupInput,
    StartEmployeeGroupRunInput, UpsertAgentEmployeeInput,
};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, serde::Serialize)]
struct RegressionCaseSummary {
    scenario: &'static str,
    status: &'static str,
}

fn main() -> Result<()> {
    let handle = std::thread::Builder::new()
        .name("employee-group-run-regression".to_string())
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
    let (pool, _tmp) = test_helpers::setup_test_db().await;

    let results = vec![
        run_execute_rules_regression(&pool).await?,
        run_reassign_rejects_disallowed_target_regression(&pool).await?,
        run_reassign_uses_step_dispatch_source_regression(&pool).await?,
    ];

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "status": "pass",
            "results": results,
        }))?
    );

    Ok(())
}

async fn reset_group_run_tables(pool: &SqlitePool) -> Result<()> {
    for statement in [
        "DELETE FROM employee_group_rules",
        "DELETE FROM group_run_events",
        "DELETE FROM group_run_steps",
        "DELETE FROM group_runs",
        "DELETE FROM employee_groups",
        "DELETE FROM agent_employees",
        "DELETE FROM model_configs",
    ] {
        sqlx::query(statement)
            .execute(pool)
            .await
            .with_context(|| format!("execute reset statement: {statement}"))?;
    }
    Ok(())
}

async fn seed_default_model(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(pool)
    .await
    .context("seed default model")?;
    Ok(())
}

async fn seed_employee(
    pool: &SqlitePool,
    employee_id: &str,
    persona: &str,
    is_default: bool,
) -> Result<()> {
    upsert_agent_employee_with_pool(
        pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: employee_id.to_string(),
            name: employee_id.to_string(),
            role_id: employee_id.to_string(),
            persona: persona.to_string(),
            feishu_open_id: String::new(),
            feishu_app_id: String::new(),
            feishu_app_secret: String::new(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: format!("E:/workspace/{employee_id}"),
            openclaw_agent_id: employee_id.to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["app".to_string()],
            enabled: true,
            is_default,
            skill_ids: vec!["builtin-general".to_string()],
        },
    )
    .await
    .map_err(|error| anyhow!("seed employee {employee_id}: {error}"))?;
    Ok(())
}

async fn run_execute_rules_regression(pool: &SqlitePool) -> Result<RegressionCaseSummary> {
    reset_group_run_tables(pool).await?;
    seed_default_model(pool).await?;

    for employee_id in ["shangshu", "bingbu", "gongbu", "hubu"] {
        seed_employee(pool, employee_id, "负责团队协作执行", employee_id == "shangshu").await?;
    }

    let group_id = create_employee_group_with_pool(
        pool,
        CreateEmployeeGroupInput {
            name: "规则调度团队".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec![
                "shangshu".to_string(),
                "bingbu".to_string(),
                "gongbu".to_string(),
                "hubu".to_string(),
            ],
        },
    )
    .await
    .map_err(|error| anyhow!("create execute-rules group: {error}"))?;

    for (from_employee_id, to_employee_id, priority) in
        [("shangshu", "gongbu", 10_i64), ("shangshu", "hubu", 20_i64)]
    {
        sqlx::query(
            "INSERT INTO employee_group_rules (
                id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
             ) VALUES (?, ?, ?, ?, 'delegate', 'execute', 0, ?, datetime('now'))",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&group_id)
        .bind(from_employee_id)
        .bind(to_employee_id)
        .bind(priority)
        .execute(pool)
        .await
        .context("insert execute rule")?;
    }

    let outcome = start_employee_group_run_with_pool(
        pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "推进复杂执行".to_string(),
            execution_window: 3,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .map_err(|error| anyhow!("start execute-rules run: {error}"))?;

    let execute_steps = sqlx::query_as::<_, (String, String)>(
        "SELECT assignee_employee_id, COALESCE(dispatch_source_employee_id, '')
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute'
         ORDER BY assignee_employee_id ASC",
    )
    .bind(&outcome.run_id)
    .fetch_all(pool)
    .await
    .context("load execute steps")?;

    if execute_steps
        != vec![
            ("gongbu".to_string(), "shangshu".to_string()),
            ("hubu".to_string(), "shangshu".to_string()),
        ]
    {
        return Err(anyhow!(
            "unexpected execute steps for rules regression: {:?}",
            execute_steps
        ));
    }

    Ok(RegressionCaseSummary {
        scenario: "start_group_run_uses_execute_rules_instead_of_all_members",
        status: "pass",
    })
}

async fn run_reassign_rejects_disallowed_target_regression(
    pool: &SqlitePool,
) -> Result<RegressionCaseSummary> {
    reset_group_run_tables(pool).await?;
    seed_default_model(pool).await?;

    for employee_id in ["shangshu", "bingbu", "gongbu", "hubu"] {
        seed_employee(pool, employee_id, "负责规则改派", employee_id == "shangshu").await?;
    }

    let group_id = create_employee_group_with_pool(
        pool,
        CreateEmployeeGroupInput {
            name: "规则改派团队".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec![
                "shangshu".to_string(),
                "bingbu".to_string(),
                "gongbu".to_string(),
                "hubu".to_string(),
            ],
        },
    )
    .await
    .map_err(|error| anyhow!("create reassign-rules group: {error}"))?;

    sqlx::query(
        "INSERT INTO employee_group_rules (
            id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
         ) VALUES (?, ?, 'shangshu', 'gongbu', 'delegate', 'execute', 0, 10, datetime('now'))",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&group_id)
    .execute(pool)
    .await
    .context("insert coordinator execute rule")?;
    sqlx::query(
        "INSERT INTO employee_group_rules (
            id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
         ) VALUES (?, ?, 'menxia', 'hubu', 'delegate', 'execute', 0, 20, datetime('now'))",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&group_id)
    .execute(pool)
    .await
    .context("insert unrelated execute rule")?;

    let outcome = start_employee_group_run_with_pool(
        pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "推进执行".to_string(),
            execution_window: 3,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .map_err(|error| anyhow!("start reassign-rules run: {error}"))?;

    let step_id = sqlx::query_as::<_, (String,)>(
        "SELECT id
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute' AND assignee_employee_id = 'gongbu'
         LIMIT 1",
    )
    .bind(&outcome.run_id)
    .fetch_one(pool)
    .await
    .context("load gongbu step")?
    .0;

    sqlx::query(
        "UPDATE group_run_steps
         SET status = 'failed', output = '兵部失败'
         WHERE id = ?",
    )
    .bind(&step_id)
    .execute(pool)
    .await
    .context("mark failed step")?;

    sqlx::query(
        "UPDATE group_runs
         SET state = 'failed', current_phase = 'execute', waiting_for_employee_id = 'gongbu'
         WHERE id = ?",
    )
    .bind(&outcome.run_id)
    .execute(pool)
    .await
    .context("mark run failed")?;

    let err = reassign_group_run_step_with_pool(pool, &step_id, "hubu")
        .await
        .expect_err("reject disallowed execute target");
    if !err.contains("not eligible for execute reassignment") {
        return Err(anyhow!("unexpected reassign validation error: {err}"));
    }

    Ok(RegressionCaseSummary {
        scenario: "reassign_group_step_rejects_targets_not_allowed_by_execute_rules",
        status: "pass",
    })
}

async fn run_reassign_uses_step_dispatch_source_regression(
    pool: &SqlitePool,
) -> Result<RegressionCaseSummary> {
    reset_group_run_tables(pool).await?;
    seed_default_model(pool).await?;

    for employee_id in ["shangshu", "bingbu", "gongbu", "hubu", "libu"] {
        seed_employee(pool, employee_id, "负责来源改派", employee_id == "shangshu").await?;
    }

    let group_id = create_employee_group_with_pool(
        pool,
        CreateEmployeeGroupInput {
            name: "来源改派团队".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec![
                "shangshu".to_string(),
                "bingbu".to_string(),
                "gongbu".to_string(),
                "hubu".to_string(),
                "libu".to_string(),
            ],
        },
    )
    .await
    .map_err(|error| anyhow!("create dispatch-source group: {error}"))?;

    for (from_employee_id, to_employee_id, priority) in [
        ("shangshu", "gongbu", 10_i64),
        ("shangshu", "hubu", 20_i64),
        ("menxia", "libu", 30_i64),
    ] {
        sqlx::query(
            "INSERT INTO employee_group_rules (
                id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
             ) VALUES (?, ?, ?, ?, 'delegate', 'execute', 0, ?, datetime('now'))",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&group_id)
        .bind(from_employee_id)
        .bind(to_employee_id)
        .bind(priority)
        .execute(pool)
        .await
        .context("insert execute rule with alternate source")?;
    }

    let outcome = start_employee_group_run_with_pool(
        pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "推进执行".to_string(),
            execution_window: 3,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .map_err(|error| anyhow!("start dispatch-source run: {error}"))?;

    let step_id = sqlx::query_as::<_, (String,)>(
        "SELECT id
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute' AND assignee_employee_id = 'gongbu'
         LIMIT 1",
    )
    .bind(&outcome.run_id)
    .fetch_one(pool)
    .await
    .context("load execute step")?
    .0;

    sqlx::query(
        "UPDATE group_run_steps
         SET status = 'failed',
             output = '兵部失败',
             output_summary = '兵部失败',
             dispatch_source_employee_id = 'menxia'
         WHERE id = ?",
    )
    .bind(&step_id)
    .execute(pool)
    .await
    .context("mark failed step with dispatch source")?;

    sqlx::query(
        "UPDATE group_runs
         SET state = 'failed', current_phase = 'execute', waiting_for_employee_id = 'gongbu'
         WHERE id = ?",
    )
    .bind(&outcome.run_id)
    .execute(pool)
    .await
    .context("mark run failed")?;

    reassign_group_run_step_with_pool(pool, &step_id, "libu")
        .await
        .map_err(|error| anyhow!("reassign using step dispatch source: {error}"))?;

    let (assignee_employee_id, status): (String, String) = sqlx::query_as(
        "SELECT assignee_employee_id, status
         FROM group_run_steps
         WHERE id = ?",
    )
    .bind(&step_id)
    .fetch_one(pool)
    .await
    .context("reload step after reassign")?;

    if assignee_employee_id != "libu" || status != "pending" {
        return Err(anyhow!(
            "unexpected reassigned step state: assignee={assignee_employee_id}, status={status}"
        ));
    }

    Ok(RegressionCaseSummary {
        scenario: "reassign_group_step_uses_step_dispatch_source_when_present",
        status: "pass",
    })
}
