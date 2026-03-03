mod helpers;

use runtime_lib::commands::employee_agents::{
    cancel_employee_group_run_with_pool, create_employee_group_with_pool,
    delete_employee_group_with_pool, ensure_employee_sessions_for_event_with_pool,
    get_employee_group_run_snapshot_with_pool, link_inbound_event_to_session_with_pool,
    list_agent_employees_with_pool, list_employee_groups_with_pool, resolve_target_employees_for_event,
    retry_employee_group_run_failed_steps_with_pool, start_employee_group_run_with_pool,
    upsert_agent_employee_with_pool, CreateEmployeeGroupInput, StartEmployeeGroupRunInput,
    UpsertAgentEmployeeInput,
};
use runtime_lib::im::types::{ImEvent, ImEventType};

#[tokio::test]
async fn employee_config_and_im_session_mapping_work() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'https://example.com', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    let employee_id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "project_manager".to_string(),
            name: "项目经理智能体".to_string(),
            role_id: "project_manager".to_string(),
            persona: "负责拆解需求".to_string(),
            feishu_open_id: "ou_pm_1".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "E:/workspace/pm".to_string(),
            openclaw_agent_id: "project_manager".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec!["builtin-general".to_string()],
        },
    )
    .await
    .expect("upsert employee");

    let employees = list_agent_employees_with_pool(&pool)
        .await
        .expect("list employees");
    assert_eq!(employees.len(), 1);
    assert_eq!(employees[0].skill_ids, vec!["builtin-general".to_string()]);

    let event = ImEvent {
        event_type: ImEventType::MessageCreated,
        thread_id: "chat_001".to_string(),
        event_id: Some("evt_001".to_string()),
        message_id: Some("msg_001".to_string()),
        text: Some("请评估这个商机".to_string()),
        role_id: None,
        tenant_id: Some("tenant-a".to_string()),
    };

    let sessions = ensure_employee_sessions_for_event_with_pool(&pool, &event)
        .await
        .expect("ensure employee sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].employee_id, employee_id);

    let (skill_id, work_dir): (String, String) =
        sqlx::query_as("SELECT skill_id, work_dir FROM sessions WHERE id = ?")
            .bind(&sessions[0].session_id)
            .fetch_one(&pool)
            .await
            .expect("query created session");
    assert_eq!(skill_id, "builtin-general");
    assert_eq!(work_dir, "E:/workspace/pm");

    link_inbound_event_to_session_with_pool(
        &pool,
        &event,
        &sessions[0].employee_id,
        &sessions[0].session_id,
    )
    .await
    .expect("link inbound event");

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM im_message_links WHERE thread_id = 'chat_001' AND session_id = ?",
    )
    .bind(&sessions[0].session_id)
    .fetch_one(&pool)
    .await
    .expect("count message links");
    assert_eq!(count, 1);
}

#[tokio::test]
async fn group_message_without_mention_routes_to_main_employee() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'https://example.com', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    let main_id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "main".to_string(),
            name: "主员工".to_string(),
            role_id: "main".to_string(),
            persona: "".to_string(),
            feishu_open_id: "ou_main".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "main".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert main");

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "presales".to_string(),
            name: "售前".to_string(),
            role_id: "presales".to_string(),
            persona: "".to_string(),
            feishu_open_id: "ou_presales".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "presales".to_string(),
            routing_priority: 90,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec!["builtin-general".to_string()],
        },
    )
    .await
    .expect("upsert other");

    let targets = resolve_target_employees_for_event(
        &pool,
        &ImEvent {
            event_type: ImEventType::MessageCreated,
            thread_id: "chat_group_1".to_string(),
            event_id: Some("evt_001".to_string()),
            message_id: Some("msg_001".to_string()),
            text: Some("大家讨论一下这个商机".to_string()),
            role_id: None,
            tenant_id: None,
        },
    )
    .await
    .expect("resolve target employees");

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].id, main_id);
}

#[tokio::test]
async fn group_message_with_mention_routes_to_target_employee() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'https://example.com', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "main".to_string(),
            name: "主员工".to_string(),
            role_id: "main".to_string(),
            persona: "".to_string(),
            feishu_open_id: "ou_main".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "main".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert main");

    let dev_id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "dev_team".to_string(),
            name: "开发团队".to_string(),
            role_id: "dev_team".to_string(),
            persona: "".to_string(),
            feishu_open_id: "ou_dev_team".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "dev_team".to_string(),
            routing_priority: 90,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec!["builtin-general".to_string()],
        },
    )
    .await
    .expect("upsert dev team");

    let targets = resolve_target_employees_for_event(
        &pool,
        &ImEvent {
            event_type: ImEventType::MessageCreated,
            thread_id: "chat_group_mention".to_string(),
            event_id: Some("evt_mention_001".to_string()),
            message_id: Some("msg_mention_001".to_string()),
            text: Some("@开发团队 请开始处理".to_string()),
            role_id: Some("ou_dev_team".to_string()),
            tenant_id: None,
        },
    )
    .await
    .expect("resolve target employees");

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].id, dev_id);
}

#[tokio::test]
async fn group_message_with_text_mention_routes_to_target_employee_when_role_id_missing() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'https://example.com', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "main".to_string(),
            name: "主员工".to_string(),
            role_id: "main".to_string(),
            persona: "".to_string(),
            feishu_open_id: "ou_main".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "main".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert main");

    let dev_id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "dev_team".to_string(),
            name: "开发团队".to_string(),
            role_id: "dev_team".to_string(),
            persona: "".to_string(),
            feishu_open_id: "ou_dev_team".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "dev_team".to_string(),
            routing_priority: 90,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec!["builtin-general".to_string()],
        },
    )
    .await
    .expect("upsert dev team");

    let targets = resolve_target_employees_for_event(
        &pool,
        &ImEvent {
            event_type: ImEventType::MessageCreated,
            thread_id: "chat_group_text_mention".to_string(),
            event_id: Some("evt_text_mention_001".to_string()),
            message_id: Some("msg_text_mention_001".to_string()),
            text: Some("@开发团队 请开始处理".to_string()),
            role_id: None,
            tenant_id: None,
        },
    )
    .await
    .expect("resolve target employees");

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].id, dev_id);
}

#[tokio::test]
async fn upsert_employee_rejects_duplicate_employee_id() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "duplicate_role".to_string(),
            name: "角色A".to_string(),
            role_id: "duplicate_role".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "duplicate_role".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec![],
        },
    )
    .await
    .expect("insert first employee");

    let err = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "duplicate_role".to_string(),
            name: "角色B".to_string(),
            role_id: "duplicate_role".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "duplicate_role".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec![],
        },
    )
    .await
    .expect_err("duplicate employee_id should fail");

    assert!(err.contains("employee_id"));
}

#[tokio::test]
async fn employee_persists_openclaw_agent_mapping() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "main".to_string(),
            name: "主员工".to_string(),
            role_id: "main".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "main".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert");

    let list = list_agent_employees_with_pool(&pool).await.expect("list");
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].openclaw_agent_id, "main");
}

#[tokio::test]
async fn create_list_delete_employee_group_with_constraints() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "project_manager".to_string(),
            name: "项目经理".to_string(),
            role_id: "project_manager".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "project_manager".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("seed coordinator");

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

    let groups = list_employee_groups_with_pool(&pool).await.expect("list groups");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].id, group_id);
    assert_eq!(groups[0].coordinator_employee_id, "project_manager");
    assert_eq!(groups[0].member_count, 3);

    delete_employee_group_with_pool(&pool, &group_id)
        .await
        .expect("delete group");

    let groups_after_delete = list_employee_groups_with_pool(&pool).await.expect("list groups");
    assert!(groups_after_delete.is_empty());
}

#[tokio::test]
async fn create_employee_group_rejects_more_than_ten_members_and_missing_coordinator() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let too_many = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "超限群".to_string(),
            coordinator_employee_id: "pm".to_string(),
            member_employee_ids: vec![
                "pm".to_string(),
                "e1".to_string(),
                "e2".to_string(),
                "e3".to_string(),
                "e4".to_string(),
                "e5".to_string(),
                "e6".to_string(),
                "e7".to_string(),
                "e8".to_string(),
                "e9".to_string(),
                "e10".to_string(),
            ],
        },
    )
    .await
    .expect_err("should reject > 10 members");
    assert!(too_many.contains("cannot exceed 10"));

    let missing_coordinator = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "缺协调员".to_string(),
            coordinator_employee_id: "pm".to_string(),
            member_employee_ids: vec!["e1".to_string(), "e2".to_string()],
        },
    )
    .await
    .expect_err("coordinator must be in members");
    assert!(missing_coordinator.contains("must be included in members"));
}

#[tokio::test]
async fn start_employee_group_run_persists_run_and_steps() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'https://example.com', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "project_manager".to_string(),
            name: "项目经理".to_string(),
            role_id: "project_manager".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "project_manager".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("seed coordinator employee");

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
    assert_eq!(outcome.steps.len(), 3);

    let (run_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM group_runs WHERE id = ?")
        .bind(&outcome.run_id)
        .fetch_one(&pool)
        .await
        .expect("count group run");
    assert_eq!(run_count, 1);

    let (step_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM group_run_steps WHERE run_id = ?")
        .bind(&outcome.run_id)
        .fetch_one(&pool)
        .await
        .expect("count run steps");
    assert_eq!(step_count, 3);

    let snapshot = get_employee_group_run_snapshot_with_pool(&pool, &outcome.session_id)
        .await
        .expect("get snapshot")
        .expect("snapshot should exist");
    assert_eq!(snapshot.run_id, outcome.run_id);
    assert_eq!(snapshot.group_id, outcome.group_id);
    assert_eq!(snapshot.steps.len(), 3);
}

#[tokio::test]
async fn cancel_and_retry_failed_group_run_steps_work() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', 'https://example.com', 'gpt-4o-mini', 1, 'k')",
    )
    .execute(&pool)
    .await
    .expect("seed model config");

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "project_manager".to_string(),
            name: "项目经理".to_string(),
            role_id: "project_manager".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "project_manager".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("seed coordinator employee");

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "交付战队".to_string(),
            coordinator_employee_id: "project_manager".to_string(),
            member_employee_ids: vec![
                "project_manager".to_string(),
                "dev_team".to_string(),
            ],
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
async fn employee_persists_openclaw_agent_mapping() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            name: "主员工".to_string(),
            role_id: "main".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "main".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert");

    let list = list_agent_employees_with_pool(&pool).await.expect("list");
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].openclaw_agent_id, "main");
}
