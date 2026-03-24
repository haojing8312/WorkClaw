use crate::helpers;
use runtime_lib::commands::employee_agents::{
    review_group_run_step_with_pool, upsert_agent_employee_with_pool, CreateEmployeeTeamInput,
    StartEmployeeGroupRunInput, UpsertAgentEmployeeInput,
};
use runtime_lib::commands::employee_agents::test_support::{
    continue_employee_group_run_with_pool, create_employee_team_with_pool,
    maybe_handle_team_entry_session_message_with_pool, start_employee_group_run_with_pool,
};
use uuid::Uuid;

#[tokio::test]
async fn maybe_handle_team_entry_message_ignores_non_team_entry_sessions() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["taizi", "zhongshu", "shangshu", "bingbu"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: format!("{employee_id} 负责复杂任务协作"),
                feishu_open_id: format!("ou_{employee_id}"),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string(), "feishu".to_string()],
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
            name: "团队入口协作".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec![
                "taizi".to_string(),
                "zhongshu".to_string(),
                "shangshu".to_string(),
                "bingbu".to_string(),
            ],
            entry_employee_id: "taizi".to_string(),
            planner_employee_id: "zhongshu".to_string(),
            reviewer_employee_id: "".to_string(),
            review_mode: "none".to_string(),
            execution_mode: "parallel".to_string(),
            visibility_mode: "shared".to_string(),
            rules: vec![],
        },
    )
    .await
    .expect("create employee team");

    for (session_id, session_mode) in [
        ("session-general", "general"),
        ("session-employee-direct", "employee_direct"),
    ] {
        sqlx::query(
            "INSERT INTO sessions (
                id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id
             ) VALUES (?, 'builtin-general', '普通会话', '2026-03-07T00:00:00Z', 'm1', 'standard', 'E:/workspace/taizi', 'taizi', ?, ?)",
        )
        .bind(session_id)
        .bind(session_mode)
        .bind(&group_id)
        .execute(&pool)
        .await
        .expect("seed non-team-entry session");

        let handled = maybe_handle_team_entry_session_message_with_pool(
            &pool,
            session_id,
            "请制定并执行交付方案",
        )
        .await
        .expect("handle team entry session");

        assert!(
            handled.is_none(),
            "{session_mode} session should not trigger team orchestration"
        );
    }
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

#[tokio::test]
async fn maybe_handle_team_entry_message_reuses_existing_chat_session_for_group_run() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'http://mock', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    for employee_id in ["taizi", "zhongshu", "shangshu", "bingbu"] {
        upsert_agent_employee_with_pool(
            &pool,
            UpsertAgentEmployeeInput {
                id: None,
                employee_id: employee_id.to_string(),
                name: employee_id.to_string(),
                role_id: employee_id.to_string(),
                persona: format!("{employee_id} 负责复杂任务协作"),
                feishu_open_id: format!("ou_{employee_id}"),
                feishu_app_id: "".to_string(),
                feishu_app_secret: "".to_string(),
                primary_skill_id: "builtin-general".to_string(),
                default_work_dir: format!("E:/workspace/{employee_id}"),
                openclaw_agent_id: employee_id.to_string(),
                routing_priority: 100,
                enabled_scopes: vec!["app".to_string(), "feishu".to_string()],
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
            name: "团队入口协作".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec![
                "taizi".to_string(),
                "zhongshu".to_string(),
                "shangshu".to_string(),
                "bingbu".to_string(),
            ],
            entry_employee_id: "taizi".to_string(),
            planner_employee_id: "zhongshu".to_string(),
            reviewer_employee_id: "".to_string(),
            review_mode: "none".to_string(),
            execution_mode: "parallel".to_string(),
            visibility_mode: "shared".to_string(),
            rules: vec![],
        },
    )
    .await
    .expect("create employee team");

    create_employee_team_with_pool(
        &pool,
        CreateEmployeeTeamInput {
            name: "另一个团队入口协作".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec![
                "taizi".to_string(),
                "zhongshu".to_string(),
                "shangshu".to_string(),
                "bingbu".to_string(),
            ],
            entry_employee_id: "taizi".to_string(),
            planner_employee_id: "zhongshu".to_string(),
            reviewer_employee_id: "".to_string(),
            review_mode: "none".to_string(),
            execution_mode: "parallel".to_string(),
            visibility_mode: "shared".to_string(),
            rules: vec![],
        },
    )
    .await
    .expect("create distractor employee team");

    let session_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO sessions (
            id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id
         ) VALUES (?, 'builtin-general', '团队入口会话', '2026-03-07T00:00:00Z', 'm1', 'standard', 'E:/workspace/taizi', 'taizi', 'team_entry', ?)",
    )
    .bind(&session_id)
    .bind(&group_id)
    .execute(&pool)
    .await
    .expect("seed entry session");

    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at)
         VALUES (?, ?, 'user', '请制定并执行交付方案', '2026-03-07T00:00:01Z')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&session_id)
    .execute(&pool)
    .await
    .expect("seed user message");

    let handled = maybe_handle_team_entry_session_message_with_pool(
        &pool,
        &session_id,
        "请制定并执行交付方案",
    )
    .await
    .expect("handle team entry session")
    .expect("team entry should be handled");

    assert_eq!(handled.group_id, group_id);
    assert_eq!(handled.session_id, session_id);
    assert_eq!(handled.state, "done");
    assert!(handled.final_report.contains("团队协作已完成"));

    let (run_session_id, entry_session_id, state): (String, String, String) = sqlx::query_as(
        "SELECT session_id, entry_session_id, state
         FROM group_runs
         WHERE id = ?",
    )
    .bind(&handled.run_id)
    .fetch_one(&pool)
    .await
    .expect("load created run");
    assert_eq!(run_session_id, session_id);
    assert_eq!(entry_session_id, session_id);
    assert_eq!(state, "done");

    let messages: Vec<(String, String)> = sqlx::query_as(
        "SELECT role, content FROM messages WHERE session_id = ? ORDER BY created_at ASC, id ASC",
    )
    .bind(&session_id)
    .fetch_all(&pool)
    .await
    .expect("load entry session messages");
    assert_eq!(
        messages.iter().filter(|(role, _)| role == "user").count(),
        1,
        "team entry handler should reuse the existing user message instead of duplicating it"
    );
    assert!(
        messages
            .iter()
            .any(|(role, content)| role == "assistant" && content.contains("团队协作已完成")),
        "team entry handler should append the team result into the current chat session"
    );
}
