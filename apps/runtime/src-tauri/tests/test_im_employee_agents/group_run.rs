use crate::helpers;
use runtime_lib::commands::employee_agents::{
    cancel_employee_group_run_with_pool, get_employee_group_run_snapshot_with_pool,
    pause_employee_group_run_with_pool, reassign_group_run_step_with_pool,
    resume_employee_group_run_with_pool, review_group_run_step_with_pool,
    retry_employee_group_run_failed_steps_with_pool, upsert_agent_employee_with_pool,
    CreateEmployeeGroupInput, CreateEmployeeTeamInput, StartEmployeeGroupRunInput,
    UpsertAgentEmployeeInput,
};
use runtime_lib::commands::employee_agents::test_support::{
    continue_employee_group_run_with_pool, create_employee_group_with_pool,
    create_employee_team_with_pool, run_group_step_with_pool, start_employee_group_run_with_pool,
};
use uuid::Uuid;

#[tokio::test]
async fn start_employee_group_run_persists_plan_steps_and_events() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["project_manager", "dev_team", "qa_team"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: "".to_string(),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["feishu".to_string(), "app".to_string()],
                enabled: true,
                is_default: employee_id == "project_manager",
                skill_ids: vec![],
            },
        )
        .await
        .expect("seed team employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "交付战队".to_string(),
            coordinator_employee_id: "project_manager".to_string(),
            member_employee_ids: vec![
                "project_manager".to_string(),
                "dev_team".to_string(),
                "qa_team".to_string(),
            ],
        },
    )
    .await
    .expect("create group");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id: group_id.clone(),
            user_goal: "完成版本发布方案".to_string(),
            execution_window: 3,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start run");

    assert_eq!(outcome.group_id, group_id);
    assert_eq!(outcome.state, "done");
    assert!(!outcome.session_id.is_empty());
    assert!(!outcome.session_skill_id.is_empty());
    assert!(outcome.current_round >= 1);
    assert!(outcome.final_report.contains("计划"));
    assert!(outcome.final_report.contains("执行"));
    assert!(outcome.final_report.contains("汇报"));
    assert!(outcome.steps.iter().any(|step| step.step_type == "plan"));
    assert!(outcome.steps.iter().any(|step| step.step_type == "execute"));
    assert!(outcome.steps.iter().any(|step| {
        step.step_type == "execute" && step.dispatch_source_employee_id == "project_manager"
    }));

    let (run_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM group_runs WHERE id = ?")
        .bind(&outcome.run_id)
        .fetch_one(&pool)
        .await
        .expect("count group run");
    assert_eq!(run_count, 1);

    let (step_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM group_run_steps WHERE run_id = ?")
            .bind(&outcome.run_id)
            .fetch_one(&pool)
            .await
            .expect("count run steps");
    assert_eq!(step_count, outcome.steps.len() as i64);

    let (event_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM group_run_events WHERE run_id = ?")
            .bind(&outcome.run_id)
            .fetch_one(&pool)
            .await
            .expect("count run events");
    assert!(event_count > 0);

    let snapshot = get_employee_group_run_snapshot_with_pool(&pool, &outcome.session_id)
        .await
        .expect("get snapshot")
        .expect("snapshot should exist");
    assert_eq!(snapshot.run_id, outcome.run_id);
    assert_eq!(snapshot.group_id, outcome.group_id);
    assert_eq!(snapshot.steps.len(), outcome.steps.len());
    assert!(snapshot.steps.iter().any(|step| {
        step.step_type == "execute" && step.dispatch_source_employee_id == "project_manager"
    }));

    let (execute_step_id, dispatch_source_employee_id): (String, String) = sqlx::query_as(
        "SELECT id, dispatch_source_employee_id
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute'
         ORDER BY round_no ASC, id ASC
         LIMIT 1",
    )
    .bind(&outcome.run_id)
    .fetch_one(&pool)
    .await
    .expect("load execute step");
    assert_eq!(dispatch_source_employee_id, "project_manager");

    let (step_created_payload_json,): (String,) = sqlx::query_as(
        "SELECT payload_json
         FROM group_run_events
         WHERE run_id = ? AND step_id = ? AND event_type = 'step_created'
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
    )
    .bind(&outcome.run_id)
    .bind(&execute_step_id)
    .fetch_one(&pool)
    .await
    .expect("load step_created event");
    assert!(
        step_created_payload_json.contains("\"dispatch_source_employee_id\":\"project_manager\"")
    );
}

#[tokio::test]
async fn cancel_and_retry_failed_group_run_steps_work() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["project_manager", "dev_team"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: "".to_string(),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["feishu".to_string(), "app".to_string()],
                enabled: true,
                is_default: employee_id == "project_manager",
                skill_ids: vec![],
            },
        )
        .await
        .expect("seed team employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "交付战队".to_string(),
            coordinator_employee_id: "project_manager".to_string(),
            member_employee_ids: vec!["project_manager".to_string(), "dev_team".to_string()],
        },
    )
    .await
    .expect("create group");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "完成版本发布方案".to_string(),
            execution_window: 2,
            max_retry_per_step: 1,
            timeout_employee_ids: vec!["dev_team".to_string()],
        },
    )
    .await
    .expect("start run");

    sqlx::query("UPDATE group_runs SET state = 'executing' WHERE id = ?")
        .bind(&outcome.run_id)
        .execute(&pool)
        .await
        .expect("force run state for cancel test");

    sqlx::query(
        "UPDATE group_run_steps
         SET status = 'failed', output = '开发任务超时'
         WHERE id = (
             SELECT id FROM group_run_steps
             WHERE run_id = ? AND step_type = 'execute'
             ORDER BY round_no ASC, id ASC
             LIMIT 1
         )",
    )
    .bind(&outcome.run_id)
    .execute(&pool)
    .await
    .expect("seed failed execute step");

    cancel_employee_group_run_with_pool(&pool, &outcome.run_id)
        .await
        .expect("cancel run");
    let cancelled_state: (String,) = sqlx::query_as("SELECT state FROM group_runs WHERE id = ?")
        .bind(&outcome.run_id)
        .fetch_one(&pool)
        .await
        .expect("query run state after cancel");
    assert_eq!(cancelled_state.0, "cancelled");

    retry_employee_group_run_failed_steps_with_pool(&pool, &outcome.run_id)
        .await
        .expect("retry failed steps");
    let retried_state: (String,) = sqlx::query_as("SELECT state FROM group_runs WHERE id = ?")
        .bind(&outcome.run_id)
        .fetch_one(&pool)
        .await
        .expect("query run state after retry");
    assert_eq!(retried_state.0, "done");

    let failed_count_after_retry: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM group_run_steps WHERE run_id = ? AND status = 'failed'",
    )
    .bind(&outcome.run_id)
    .fetch_one(&pool)
    .await
    .expect("count failed after retry");
    assert_eq!(failed_count_after_retry.0, 0);
}

#[tokio::test]
async fn execute_group_step_uses_target_employee_context() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "shangshu".to_string(),
            name: "尚书省".to_string(),
            role_id: "shangshu".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "E:/workspace/shangshu".to_string(),
            openclaw_agent_id: "shangshu".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec!["builtin-general".to_string()],
        },
    )
    .await
    .expect("seed coordinator employee");

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "bingbu".to_string(),
            name: "兵部".to_string(),
            role_id: "bingbu".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "delivery-skill".to_string(),
            default_work_dir: "E:/workspace/bingbu".to_string(),
            openclaw_agent_id: "bingbu".to_string(),
            routing_priority: 90,
            enabled_scopes: vec!["app".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec!["delivery-skill".to_string()],
        },
    )
    .await
    .expect("seed execute employee");

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "默认复杂任务团队".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec!["shangshu".to_string(), "bingbu".to_string()],
        },
    )
    .await
    .expect("create group");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "执行交付方案".to_string(),
            execution_window: 2,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start group run");

    let (step_id,): (String,) = sqlx::query_as(
        "SELECT id FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute' AND assignee_employee_id = 'bingbu'
         LIMIT 1",
    )
    .bind(&outcome.run_id)
    .fetch_one(&pool)
    .await
    .expect("load execute step");

    let exec = run_group_step_with_pool(&pool, &step_id)
        .await
        .expect("run group step");

    assert_eq!(exec.assignee_employee_id, "bingbu");
    assert_eq!(exec.status, "completed");
    assert!(!exec.session_id.is_empty());

    let (skill_id, work_dir, employee_id): (String, String, String) =
        sqlx::query_as("SELECT skill_id, work_dir, employee_id FROM sessions WHERE id = ?")
            .bind(&exec.session_id)
            .fetch_one(&pool)
            .await
            .expect("load execution session");
    assert_eq!(skill_id, "delivery-skill");
    assert_eq!(work_dir, "E:/workspace/bingbu");
    assert_eq!(employee_id, "bingbu");

    let (step_status, step_session_id): (String, String) =
        sqlx::query_as("SELECT status, session_id FROM group_run_steps WHERE id = ?")
            .bind(&step_id)
            .fetch_one(&pool)
            .await
            .expect("reload executed step");
    assert_eq!(step_status, "completed");
    assert_eq!(step_session_id, exec.session_id);

    let messages: Vec<(String, String)> = sqlx::query_as(
        "SELECT role, content FROM messages WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(&exec.session_id)
    .fetch_all(&pool)
    .await
    .expect("load execution session messages");
    assert!(
        messages.len() >= 2,
        "expected user and assistant messages in execution session"
    );
    assert_eq!(messages[0].0, "user");
    assert!(messages[0].1.contains("执行交付方案"));
    assert_eq!(messages[messages.len() - 1].0, "assistant");
    assert!(messages[messages.len() - 1].1.contains("MOCK_RESPONSE"));
    assert!(exec.output.contains("MOCK_RESPONSE"));
    assert_ne!(exec.output, "bingbu 已基于员工上下文完成执行：执行交付方案");
}

#[tokio::test]
async fn execute_group_step_completes_group_run_and_appends_summary_when_last_step_finishes() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["shangshu", "bingbu"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: format!("{employee_id} 负责团队交付"),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string()],
                enabled: true,
                is_default: employee_id == "shangshu",
                skill_ids: vec!["builtin-general".to_string()],
            },
        )
        .await
        .expect("seed group employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "自动收口团队".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec!["shangshu".to_string(), "bingbu".to_string()],
        },
    )
    .await
    .expect("create group");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "完成复杂交付方案".to_string(),
            execution_window: 2,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start group run");

    let step_ids: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute'
         ORDER BY round_no ASC, id ASC",
    )
    .bind(&outcome.run_id)
    .fetch_all(&pool)
    .await
    .expect("load execute steps");
    assert_eq!(step_ids.len(), 2);
    for (step_id,) in step_ids {
        run_group_step_with_pool(&pool, &step_id)
            .await
            .expect("run execute step");
    }

    let (state, current_phase): (String, String) =
        sqlx::query_as("SELECT state, current_phase FROM group_runs WHERE id = ?")
            .bind(&outcome.run_id)
            .fetch_one(&pool)
            .await
            .expect("reload run state");
    assert_eq!(state, "done");
    assert_eq!(current_phase, "finalize");

    let summary_messages: Vec<(String, String)> = sqlx::query_as(
        "SELECT role, content FROM messages WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(&outcome.session_id)
    .fetch_all(&pool)
    .await
    .expect("load coordinator session messages");
    let last_assistant = summary_messages
        .iter()
        .rev()
        .find(|(role, _)| role == "assistant")
        .expect("assistant summary exists");
    assert!(last_assistant.1.contains("团队协作已完成"));
    assert!(last_assistant.1.contains("MOCK_RESPONSE"));

    let snapshot = get_employee_group_run_snapshot_with_pool(&pool, &outcome.session_id)
        .await
        .expect("load run snapshot")
        .expect("snapshot exists");
    let completed_execute_step = snapshot
        .steps
        .iter()
        .find(|step| step.step_type == "execute" && step.status == "completed")
        .expect("completed execute step");
    assert!(
        !completed_execute_step.session_id.trim().is_empty(),
        "completed execute step should expose session_id in snapshot"
    );
}

#[tokio::test]
async fn continue_group_run_executes_pending_steps_and_finishes_when_review_not_required() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["shangshu", "bingbu"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: format!("{employee_id} 负责团队交付"),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string()],
                enabled: true,
                is_default: employee_id == "shangshu",
                skill_ids: vec!["builtin-general".to_string()],
            },
        )
        .await
        .expect("seed employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "自动推进团队".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec!["shangshu".to_string(), "bingbu".to_string()],
        },
    )
    .await
    .expect("create group");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "输出并执行复杂交付方案".to_string(),
            execution_window: 2,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start run");

    let snapshot = continue_employee_group_run_with_pool(&pool, &outcome.run_id)
        .await
        .expect("continue run");

    assert_eq!(snapshot.state, "done");
    assert_eq!(snapshot.current_phase, "finalize");
    assert!(snapshot.final_report.contains("团队协作已完成"));
    assert!(
        snapshot
            .steps
            .iter()
            .filter(|step| step.step_type == "execute" && step.status == "completed")
            .count()
            >= 2
    );
}

#[tokio::test]
async fn hard_review_reject_moves_run_back_to_previous_phase() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'https://example.com', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["zhongshu", "menxia"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: "".to_string(),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string()],
                enabled: true,
                is_default: employee_id == "zhongshu",
                skill_ids: vec!["builtin-general".to_string()],
            },
        )
        .await
        .expect("seed reviewable employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "审议团队".to_string(),
            coordinator_employee_id: "zhongshu".to_string(),
            member_employee_ids: vec!["zhongshu".to_string(), "menxia".to_string()],
        },
    )
    .await
    .expect("create reviewable group");

    sqlx::query(
        "UPDATE employee_groups
         SET review_mode = 'hard', entry_employee_id = 'zhongshu'
         WHERE id = ?",
    )
    .bind(&group_id)
    .execute(&pool)
    .await
    .expect("enable hard review");
    sqlx::query(
        "INSERT INTO employee_group_rules (
            id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
         ) VALUES (?, ?, ?, ?, 'review', 'plan', 1, 100, ?)",
    )
    .bind("rule-review-1")
    .bind(&group_id)
    .bind("zhongshu")
    .bind("menxia")
    .bind("2026-03-07T00:00:00Z")
    .execute(&pool)
    .await
    .expect("seed review rule");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "输出复杂方案".to_string(),
            execution_window: 2,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start reviewable run");

    review_group_run_step_with_pool(&pool, &outcome.run_id, "reject", "缺少回滚方案")
        .await
        .expect("reject review");

    let (current_phase, review_round, state): (String, i64, String) =
        sqlx::query_as("SELECT current_phase, review_round, state FROM group_runs WHERE id = ?")
            .bind(&outcome.run_id)
            .fetch_one(&pool)
            .await
            .expect("reload run state");
    assert_eq!(current_phase, "plan");
    assert_eq!(review_round, 1);
    assert_eq!(state, "planning");
}

#[tokio::test]
async fn continue_group_run_after_reject_restarts_review_before_execute() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'https://example.com', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["zhongshu", "menxia", "bingbu"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: "".to_string(),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string()],
                enabled: true,
                is_default: employee_id == "zhongshu",
                skill_ids: vec!["builtin-general".to_string()],
            },
        )
        .await
        .expect("seed reviewable employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "退回复审团队".to_string(),
            coordinator_employee_id: "zhongshu".to_string(),
            member_employee_ids: vec![
                "zhongshu".to_string(),
                "menxia".to_string(),
                "bingbu".to_string(),
            ],
        },
    )
    .await
    .expect("create reviewable group");

    sqlx::query(
        "UPDATE employee_groups
         SET review_mode = 'hard', entry_employee_id = 'zhongshu'
         WHERE id = ?",
    )
    .bind(&group_id)
    .execute(&pool)
    .await
    .expect("enable hard review");
    sqlx::query(
        "INSERT INTO employee_group_rules (
            id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
         ) VALUES (?, ?, ?, ?, 'review', 'plan', 1, 100, ?)",
    )
    .bind("rule-review-reject-continue")
    .bind(&group_id)
    .bind("zhongshu")
    .bind("menxia")
    .bind("2026-03-07T00:00:00Z")
    .execute(&pool)
    .await
    .expect("seed review rule");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "输出复杂方案".to_string(),
            execution_window: 2,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start reviewable run");

    review_group_run_step_with_pool(&pool, &outcome.run_id, "reject", "缺少回滚方案")
        .await
        .expect("reject review");

    let snapshot = continue_employee_group_run_with_pool(&pool, &outcome.run_id)
        .await
        .expect("continue rejected run");

    assert_eq!(snapshot.state, "waiting_review");
    assert_eq!(snapshot.current_phase, "review");
    assert_eq!(snapshot.review_round, 1);
    assert_eq!(snapshot.waiting_for_employee_id, "menxia");
    assert!(
        snapshot
            .steps
            .iter()
            .filter(|step| step.step_type == "review" && step.status == "pending")
            .count()
            >= 1
    );
    assert_eq!(
        snapshot
            .steps
            .iter()
            .filter(|step| step.step_type == "execute" && step.status == "completed")
            .count(),
        0
    );
    assert!(snapshot.steps.iter().any(|step| step.step_type == "plan"
        && step.status == "completed"
        && step.output.contains("缺少回滚方案")));
}

#[tokio::test]
async fn hard_review_approve_advances_run_to_execute_phase() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'https://example.com', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["zhongshu", "menxia"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: "".to_string(),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string()],
                enabled: true,
                is_default: employee_id == "zhongshu",
                skill_ids: vec!["builtin-general".to_string()],
            },
        )
        .await
        .expect("seed reviewable employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "审议团队".to_string(),
            coordinator_employee_id: "zhongshu".to_string(),
            member_employee_ids: vec!["zhongshu".to_string(), "menxia".to_string()],
        },
    )
    .await
    .expect("create reviewable group");

    sqlx::query(
        "UPDATE employee_groups
         SET review_mode = 'hard', entry_employee_id = 'zhongshu'
         WHERE id = ?",
    )
    .bind(&group_id)
    .execute(&pool)
    .await
    .expect("enable hard review");
    sqlx::query(
        "INSERT INTO employee_group_rules (
            id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
         ) VALUES (?, ?, ?, ?, 'review', 'plan', 1, 100, ?)",
    )
    .bind("rule-review-approve-1")
    .bind(&group_id)
    .bind("zhongshu")
    .bind("menxia")
    .bind("2026-03-07T00:00:00Z")
    .execute(&pool)
    .await
    .expect("seed review rule");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "输出复杂方案".to_string(),
            execution_window: 2,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start reviewable run");

    review_group_run_step_with_pool(&pool, &outcome.run_id, "approve", "方案通过")
        .await
        .expect("approve review");

    let (current_phase, state): (String, String) =
        sqlx::query_as("SELECT current_phase, state FROM group_runs WHERE id = ?")
            .bind(&outcome.run_id)
            .fetch_one(&pool)
            .await
            .expect("reload run state");
    assert_eq!(current_phase, "execute");
    assert_eq!(state, "planning");
}

#[tokio::test]
async fn continue_group_run_waits_for_review_then_completes_after_approval() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["zhongshu", "menxia", "bingbu"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: format!("{employee_id} 负责团队交付"),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string()],
                enabled: true,
                is_default: employee_id == "zhongshu",
                skill_ids: vec!["builtin-general".to_string()],
            },
        )
        .await
        .expect("seed review employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "审核后执行团队".to_string(),
            coordinator_employee_id: "zhongshu".to_string(),
            member_employee_ids: vec![
                "zhongshu".to_string(),
                "menxia".to_string(),
                "bingbu".to_string(),
            ],
        },
    )
    .await
    .expect("create group");

    sqlx::query(
        "UPDATE employee_groups
         SET review_mode = 'hard', entry_employee_id = 'zhongshu'
         WHERE id = ?",
    )
    .bind(&group_id)
    .execute(&pool)
    .await
    .expect("enable hard review");
    sqlx::query(
        "INSERT INTO employee_group_rules (
            id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
         ) VALUES ('rule-review-continue', ?, 'zhongshu', 'menxia', 'review', 'plan', 1, 100, '2026-03-07T00:00:00Z')",
    )
    .bind(&group_id)
    .execute(&pool)
    .await
    .expect("seed review rule");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "输出复杂方案并执行".to_string(),
            execution_window: 2,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start run");

    let waiting_snapshot = continue_employee_group_run_with_pool(&pool, &outcome.run_id)
        .await
        .expect("continue run before review");
    assert_eq!(waiting_snapshot.state, "waiting_review");
    assert_eq!(waiting_snapshot.current_phase, "review");

    review_group_run_step_with_pool(&pool, &outcome.run_id, "approve", "方案通过")
        .await
        .expect("approve review");

    let done_snapshot = continue_employee_group_run_with_pool(&pool, &outcome.run_id)
        .await
        .expect("continue run after review");
    assert_eq!(done_snapshot.state, "done");
    assert_eq!(done_snapshot.current_phase, "finalize");
    assert!(done_snapshot.final_report.contains("团队协作已完成"));
}

#[tokio::test]
async fn group_run_snapshot_exposes_phase_review_and_events() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'https://example.com', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["zhongshu", "menxia"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: "".to_string(),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string()],
                enabled: true,
                is_default: employee_id == "zhongshu",
                skill_ids: vec!["builtin-general".to_string()],
            },
        )
        .await
        .expect("seed reviewable employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "审议团队".to_string(),
            coordinator_employee_id: "zhongshu".to_string(),
            member_employee_ids: vec!["zhongshu".to_string(), "menxia".to_string()],
        },
    )
    .await
    .expect("create reviewable group");

    sqlx::query(
        "UPDATE employee_groups
         SET review_mode = 'hard', entry_employee_id = 'zhongshu'
         WHERE id = ?",
    )
    .bind(&group_id)
    .execute(&pool)
    .await
    .expect("enable hard review");
    sqlx::query(
        "INSERT INTO employee_group_rules (
            id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
         ) VALUES ('rule-review-snapshot', ?, 'zhongshu', 'menxia', 'review', 'plan', 1, 100, '2026-03-07T00:00:00Z')",
    )
    .bind(&group_id)
    .execute(&pool)
    .await
    .expect("seed review rule");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "输出复杂方案".to_string(),
            execution_window: 2,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start reviewable run");

    let snapshot = get_employee_group_run_snapshot_with_pool(&pool, &outcome.session_id)
        .await
        .expect("get snapshot")
        .expect("snapshot exists");
    assert_eq!(snapshot.current_phase, "review");
    assert_eq!(snapshot.review_round, 0);
    assert!(snapshot
        .events
        .iter()
        .any(|event| event.event_type == "run_created"));
    assert!(snapshot
        .events
        .iter()
        .any(|event| event.event_type == "phase_started"));
}

#[tokio::test]
async fn pause_and_resume_group_run_updates_state() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["project_manager", "dev_team"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: "".to_string(),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["feishu".to_string(), "app".to_string()],
                enabled: true,
                is_default: employee_id == "project_manager",
                skill_ids: vec![],
            },
        )
        .await
        .expect("seed team employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "交付战队".to_string(),
            coordinator_employee_id: "project_manager".to_string(),
            member_employee_ids: vec!["project_manager".to_string(), "dev_team".to_string()],
        },
    )
    .await
    .expect("create group");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "完成版本发布方案".to_string(),
            execution_window: 2,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start run");

    sqlx::query(
        "UPDATE group_runs SET state = 'executing', current_phase = 'execute' WHERE id = ?",
    )
    .bind(&outcome.run_id)
    .execute(&pool)
    .await
    .expect("force run executing");

    pause_employee_group_run_with_pool(&pool, &outcome.run_id, "人工介入")
        .await
        .expect("pause run");
    let paused: (String, String) =
        sqlx::query_as("SELECT state, status_reason FROM group_runs WHERE id = ?")
            .bind(&outcome.run_id)
            .fetch_one(&pool)
            .await
            .expect("load paused run");
    assert_eq!(paused.0, "paused");
    assert_eq!(paused.1, "人工介入");

    resume_employee_group_run_with_pool(&pool, &outcome.run_id)
        .await
        .expect("resume run");
    let resumed: (String, String) =
        sqlx::query_as("SELECT state, current_phase FROM group_runs WHERE id = ?")
            .bind(&outcome.run_id)
            .fetch_one(&pool)
            .await
            .expect("load resumed run");
    assert_eq!(resumed.0, "executing");
    assert_eq!(resumed.1, "execute");
}

#[tokio::test]
async fn reassign_failed_group_step_updates_assignee_and_resets_status() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["shangshu", "bingbu", "gongbu"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: "".to_string(),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string()],
                enabled: true,
                is_default: employee_id == "shangshu",
                skill_ids: vec!["builtin-general".to_string()],
            },
        )
        .await
        .expect("seed reassign employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "改派团队".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec![
                "shangshu".to_string(),
                "bingbu".to_string(),
                "gongbu".to_string(),
            ],
        },
    )
    .await
    .expect("create group");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "推进执行".to_string(),
            execution_window: 2,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start run");

    let (step_id,): (String,) = sqlx::query_as(
        "SELECT id FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute' AND assignee_employee_id = 'bingbu'
         LIMIT 1",
    )
    .bind(&outcome.run_id)
    .fetch_one(&pool)
    .await
    .expect("load failed step");

    sqlx::query(
        "UPDATE group_run_steps
         SET status = 'failed',
             output = '原负责人失败',
             output_summary = '原负责人失败',
             session_id = 'session-old'
         WHERE id = ?",
    )
    .bind(&step_id)
    .execute(&pool)
    .await
    .expect("mark step failed");

    reassign_group_run_step_with_pool(&pool, &step_id, "gongbu")
        .await
        .expect("reassign step");

    let (assignee_employee_id, status, session_id, output): (String, String, String, String) =
        sqlx::query_as(
            "SELECT assignee_employee_id, status, session_id, output
             FROM group_run_steps WHERE id = ?",
        )
        .bind(&step_id)
        .fetch_one(&pool)
        .await
        .expect("reload reassigned step");
    assert_eq!(assignee_employee_id, "gongbu");
    assert_eq!(status, "pending");
    assert_eq!(session_id, "");
    assert_eq!(output, "");

    let (event_step_id, payload_json): (String, String) = sqlx::query_as(
        "SELECT step_id, payload_json
         FROM group_run_events
         WHERE run_id = ? AND event_type = 'step_reassigned'
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
    )
    .bind(&outcome.run_id)
    .fetch_one(&pool)
    .await
    .expect("load reassign event");
    assert_eq!(event_step_id, step_id);
    assert!(payload_json.contains("\"assignee_employee_id\":\"gongbu\""));
    assert!(payload_json.contains("\"previous_assignee_employee_id\":\"bingbu\""));
    assert!(payload_json.contains("\"previous_output_summary\":\"原负责人失败\""));
}

#[tokio::test]
async fn reassign_specific_failed_step_keeps_other_failed_steps_blocking_run() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["shangshu", "bingbu", "libu", "gongbu"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: "".to_string(),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string()],
                enabled: true,
                is_default: employee_id == "shangshu",
                skill_ids: vec!["builtin-general".to_string()],
            },
        )
        .await
        .expect("seed employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "多失败改派团队".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec![
                "shangshu".to_string(),
                "bingbu".to_string(),
                "libu".to_string(),
                "gongbu".to_string(),
            ],
        },
    )
    .await
    .expect("create group");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "推进执行".to_string(),
            execution_window: 3,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start run");

    let step_rows = sqlx::query_as::<_, (String, String)>(
        "SELECT id, assignee_employee_id
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute'",
    )
    .bind(&outcome.run_id)
    .fetch_all(&pool)
    .await
    .expect("load execute steps");
    let bingbu_step_id = step_rows
        .iter()
        .find(|(_, assignee)| assignee == "bingbu")
        .map(|(id, _)| id.clone())
        .expect("bingbu step");
    let libu_step_id = step_rows
        .iter()
        .find(|(_, assignee)| assignee == "libu")
        .map(|(id, _)| id.clone())
        .expect("libu step");

    sqlx::query(
        "UPDATE group_run_steps
         SET status = 'failed',
             output = CASE id
            WHEN ? THEN '兵部失败'
            WHEN ? THEN '礼部失败'
            ELSE output
         END,
             output_summary = CASE id
            WHEN ? THEN '兵部失败'
            WHEN ? THEN '礼部失败'
            ELSE output_summary
         END
         WHERE id IN (?, ?)",
    )
    .bind(&bingbu_step_id)
    .bind(&libu_step_id)
    .bind(&bingbu_step_id)
    .bind(&libu_step_id)
    .bind(&bingbu_step_id)
    .bind(&libu_step_id)
    .execute(&pool)
    .await
    .expect("mark failed steps");

    sqlx::query(
        "UPDATE group_runs
         SET state = 'failed', current_phase = 'execute', waiting_for_employee_id = 'bingbu'
         WHERE id = ?",
    )
    .bind(&outcome.run_id)
    .execute(&pool)
    .await
    .expect("mark run failed");

    reassign_group_run_step_with_pool(&pool, &libu_step_id, "gongbu")
        .await
        .expect("reassign specific failed step");

    let (bingbu_status,): (String,) =
        sqlx::query_as("SELECT status FROM group_run_steps WHERE id = ?")
            .bind(&bingbu_step_id)
            .fetch_one(&pool)
            .await
            .expect("reload bingbu step");
    assert_eq!(bingbu_status, "failed");

    let (libu_assignee, libu_status, libu_output): (String, String, String) = sqlx::query_as(
        "SELECT assignee_employee_id, status, output
         FROM group_run_steps WHERE id = ?",
    )
    .bind(&libu_step_id)
    .fetch_one(&pool)
    .await
    .expect("reload libu step");
    assert_eq!(libu_assignee, "gongbu");
    assert_eq!(libu_status, "pending");
    assert_eq!(libu_output, "");

    let (run_state, waiting_for_employee_id): (String, String) = sqlx::query_as(
        "SELECT state, waiting_for_employee_id
         FROM group_runs WHERE id = ?",
    )
    .bind(&outcome.run_id)
    .fetch_one(&pool)
    .await
    .expect("reload run");
    assert_eq!(run_state, "failed");
    assert_eq!(waiting_for_employee_id, "bingbu");

    let (event_step_id, payload_json): (String, String) = sqlx::query_as(
        "SELECT step_id, payload_json
         FROM group_run_events
         WHERE run_id = ? AND event_type = 'step_reassigned'
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
    )
    .bind(&outcome.run_id)
    .fetch_one(&pool)
    .await
    .expect("load reassign event");
    assert_eq!(event_step_id, libu_step_id);
    assert!(payload_json.contains("\"assignee_employee_id\":\"gongbu\""));
    assert!(payload_json.contains("\"dispatch_source_employee_id\":\"shangshu\""));
    assert!(payload_json.contains("\"previous_assignee_employee_id\":\"libu\""));
    assert!(payload_json.contains("\"previous_output_summary\":\"礼部失败\""));
}

#[tokio::test]
async fn start_group_run_uses_execute_rules_instead_of_all_members() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["shangshu", "bingbu", "gongbu", "hubu"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: format!("{employee_id} 负责团队协作"),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string()],
                enabled: true,
                is_default: employee_id == "shangshu",
                skill_ids: vec!["builtin-general".to_string()],
            },
        )
        .await
        .expect("seed employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
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
    .expect("create group");

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
        .execute(&pool)
        .await
        .expect("insert execute rule");
    }

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "推进复杂执行".to_string(),
            execution_window: 3,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start run");

    let execute_steps = sqlx::query_as::<_, (String, String)>(
        "SELECT assignee_employee_id, COALESCE(dispatch_source_employee_id, '')
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute'
         ORDER BY assignee_employee_id ASC",
    )
    .bind(&outcome.run_id)
    .fetch_all(&pool)
    .await
    .expect("load execute steps");

    assert_eq!(
        execute_steps,
        vec![
            ("gongbu".to_string(), "shangshu".to_string()),
            ("hubu".to_string(), "shangshu".to_string()),
        ]
    );
}

#[tokio::test]
async fn review_reject_routes_revision_back_to_planner_instead_of_coordinator() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'https://example.com', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["taizi", "zhongshu", "menxia", "shangshu", "bingbu"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: format!("{employee_id} 负责复杂任务协作"),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string()],
                enabled: true,
                is_default: employee_id == "taizi",
                skill_ids: vec!["builtin-general".to_string()],
            },
        )
        .await
        .expect("seed employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "三省审议团队".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec![
                "taizi".to_string(),
                "zhongshu".to_string(),
                "menxia".to_string(),
                "shangshu".to_string(),
                "bingbu".to_string(),
            ],
        },
    )
    .await
    .expect("create group");

    sqlx::query(
        "UPDATE employee_groups
         SET entry_employee_id = 'taizi',
             review_mode = 'hard'
         WHERE id = ?",
    )
    .bind(&group_id)
    .execute(&pool)
    .await
    .expect("update group runtime config");

    for (from_employee_id, to_employee_id, relation_type, phase_scope, priority) in [
        ("taizi", "zhongshu", "delegate", "intake", 100_i64),
        ("zhongshu", "menxia", "review", "plan", 110_i64),
        ("shangshu", "bingbu", "delegate", "execute", 120_i64),
    ] {
        sqlx::query(
            "INSERT INTO employee_group_rules (
                id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
             ) VALUES (?, ?, ?, ?, ?, ?, 1, ?, datetime('now'))",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&group_id)
        .bind(from_employee_id)
        .bind(to_employee_id)
        .bind(relation_type)
        .bind(phase_scope)
        .bind(priority)
        .execute(&pool)
        .await
        .expect("insert group rule");
    }

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "输出复杂执行方案".to_string(),
            execution_window: 2,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start run");

    let latest_plan_assignee: String = sqlx::query_as::<_, (String,)>(
        "SELECT assignee_employee_id
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'plan'
         ORDER BY round_no DESC, id DESC
         LIMIT 1",
    )
    .bind(&outcome.run_id)
    .fetch_one(&pool)
    .await
    .expect("load initial plan step")
    .0;
    assert_eq!(latest_plan_assignee, "zhongshu");

    review_group_run_step_with_pool(&pool, &outcome.run_id, "reject", "缺少风险缓冲")
        .await
        .expect("reject review");

    let revised_plan_assignee: String = sqlx::query_as::<_, (String,)>(
        "SELECT assignee_employee_id
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'plan'
         ORDER BY round_no DESC, id DESC
         LIMIT 1",
    )
    .bind(&outcome.run_id)
    .fetch_one(&pool)
    .await
    .expect("load revised plan step")
    .0;
    assert_eq!(revised_plan_assignee, "zhongshu");
}

#[tokio::test]
async fn reassign_group_step_rejects_targets_not_allowed_by_execute_rules() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["shangshu", "bingbu", "gongbu", "hubu"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: "".to_string(),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string()],
                enabled: true,
                is_default: employee_id == "shangshu",
                skill_ids: vec!["builtin-general".to_string()],
            },
        )
        .await
        .expect("seed employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
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
    .expect("create group");

    sqlx::query(
        "INSERT INTO employee_group_rules (
            id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
         ) VALUES (?, ?, 'shangshu', 'gongbu', 'delegate', 'execute', 0, 10, datetime('now'))",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&group_id)
    .execute(&pool)
    .await
    .expect("insert execute rule");
    sqlx::query(
        "INSERT INTO employee_group_rules (
            id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
         ) VALUES (?, ?, 'menxia', 'hubu', 'delegate', 'execute', 0, 20, datetime('now'))",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&group_id)
    .execute(&pool)
    .await
    .expect("insert non-coordinator execute rule");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "推进执行".to_string(),
            execution_window: 3,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start run");

    let step_id = sqlx::query_as::<_, (String,)>(
        "SELECT id
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute' AND assignee_employee_id = 'gongbu'
         LIMIT 1",
    )
    .bind(&outcome.run_id)
    .fetch_one(&pool)
    .await
    .expect("load gongbu step")
    .0;

    sqlx::query(
        "UPDATE group_run_steps
         SET status = 'failed', output = '兵部失败'
         WHERE id = ?",
    )
    .bind(&step_id)
    .execute(&pool)
    .await
    .expect("mark step failed");

    sqlx::query(
        "UPDATE group_runs
         SET state = 'failed', current_phase = 'execute', waiting_for_employee_id = 'gongbu'
         WHERE id = ?",
    )
    .bind(&outcome.run_id)
    .execute(&pool)
    .await
    .expect("mark run failed");

    let err = reassign_group_run_step_with_pool(&pool, &step_id, "hubu")
        .await
        .expect_err("reject disallowed execute target");
    assert!(
        err.contains("not eligible for execute reassignment"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn reassign_group_step_uses_step_dispatch_source_when_present() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["shangshu", "bingbu", "gongbu", "hubu", "libu"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: "".to_string(),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string()],
                enabled: true,
                is_default: employee_id == "shangshu",
                skill_ids: vec!["builtin-general".to_string()],
            },
        )
        .await
        .expect("seed employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
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
    .expect("create group");

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
        .execute(&pool)
        .await
        .expect("insert execute rule");
    }

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "推进执行".to_string(),
            execution_window: 3,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start run");

    let step_id = sqlx::query_as::<_, (String,)>(
        "SELECT id
         FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute' AND assignee_employee_id = 'gongbu'
         LIMIT 1",
    )
    .bind(&outcome.run_id)
    .fetch_one(&pool)
    .await
    .expect("load execute step")
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
    .execute(&pool)
    .await
    .expect("mark failed step with source");

    sqlx::query(
        "UPDATE group_runs
         SET state = 'failed', current_phase = 'execute', waiting_for_employee_id = 'gongbu'
         WHERE id = ?",
    )
    .bind(&outcome.run_id)
    .execute(&pool)
    .await
    .expect("mark run failed");

    reassign_group_run_step_with_pool(&pool, &step_id, "libu")
        .await
        .expect("reassign by step dispatch source");

    let (assignee_employee_id, status): (String, String) = sqlx::query_as(
        "SELECT assignee_employee_id, status
         FROM group_run_steps
         WHERE id = ?",
    )
    .bind(&step_id)
    .fetch_one(&pool)
    .await
    .expect("reload step after reassign");
    assert_eq!(assignee_employee_id, "libu");
    assert_eq!(status, "pending");
}

#[tokio::test]
async fn start_group_run_auto_continues_and_returns_done_snapshot_when_review_not_required() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["shangshu", "bingbu"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: format!("{employee_id} 负责团队交付"),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string()],
                enabled: true,
                is_default: employee_id == "shangshu",
                skill_ids: vec!["builtin-general".to_string()],
            },
        )
        .await
        .expect("seed employee");
    }

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "自动推进团队".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec!["shangshu".to_string(), "bingbu".to_string()],
        },
    )
    .await
    .expect("create group");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "输出并执行复杂交付方案".to_string(),
            execution_window: 2,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start run");

    assert_eq!(outcome.state, "done");
    assert!(outcome.final_report.contains("团队协作已完成"));

    let (state, current_phase): (String, String) =
        sqlx::query_as("SELECT state, current_phase FROM group_runs WHERE id = ?")
            .bind(&outcome.run_id)
            .fetch_one(&pool)
            .await
            .expect("reload run state");
    assert_eq!(state, "done");
    assert_eq!(current_phase, "finalize");

    let execute_statuses: Vec<(String,)> = sqlx::query_as(
        "SELECT status FROM group_run_steps
         WHERE run_id = ? AND step_type = 'execute'
         ORDER BY id ASC",
    )
    .bind(&outcome.run_id)
    .fetch_all(&pool)
    .await
    .expect("load execute statuses");
    assert!(
        execute_statuses
            .iter()
            .all(|(status,)| status == "completed"),
        "all execute steps should be completed after start auto-continue"
    );
}

#[tokio::test]
async fn continue_group_run_completes_even_when_execute_step_hits_max_iterations() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock-tool-loop', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["taizi", "zhongshu", "menxia", "shangshu", "bingbu"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: format!("{employee_id} 负责团队交付"),
                feishu_open_id: "".to_string(),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string()],
                enabled: true,
                is_default: employee_id == "taizi",
                skill_ids: vec!["builtin-general".to_string()],
            },
        )
        .await
        .expect("seed employee");
    }

    let group_id = create_employee_team_with_pool(
        &pool,
        CreateEmployeeTeamInput {
            name: "迭代兜底团队".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec![
                "taizi".to_string(),
                "zhongshu".to_string(),
                "menxia".to_string(),
                "shangshu".to_string(),
                "bingbu".to_string(),
            ],
            entry_employee_id: "taizi".to_string(),
            planner_employee_id: "zhongshu".to_string(),
            reviewer_employee_id: "menxia".to_string(),
            review_mode: "hard".to_string(),
            execution_mode: "parallel".to_string(),
            visibility_mode: "shared".to_string(),
            rules: vec![],
        },
    )
    .await
    .expect("create employee team");

    let outcome = start_employee_group_run_with_pool(
        &pool,
        StartEmployeeGroupRunInput {
            group_id,
            user_goal: "你们能做什么".to_string(),
            execution_window: 2,
            max_retry_per_step: 1,
            timeout_employee_ids: vec![],
        },
    )
    .await
    .expect("start reviewable run");

    review_group_run_step_with_pool(&pool, &outcome.run_id, "approve", "方案通过")
        .await
        .expect("approve review");

    let snapshot = continue_employee_group_run_with_pool(&pool, &outcome.run_id)
        .await
        .expect("continue run after approve");

    assert_eq!(snapshot.state, "done");
    assert_eq!(snapshot.current_phase, "finalize");
    assert!(
        snapshot.final_report.contains("团队协作已完成"),
        "final report should still be produced when an execute step hits max iterations"
    );
    assert!(
        snapshot
            .steps
            .iter()
            .filter(|step| step.step_type == "execute")
            .all(|step| step.status == "completed"),
        "execute steps should be downgraded into completed outputs instead of leaving the run failed"
    );
    assert!(
        snapshot
            .steps
            .iter()
            .any(|step| step.assignee_employee_id == "bingbu" && step.output.contains("bingbu")),
        "the looping step should still contribute a visible fallback output"
    );
}

