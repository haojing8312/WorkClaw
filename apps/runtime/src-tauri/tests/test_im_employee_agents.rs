mod helpers;

use runtime_lib::commands::employee_agents::{
    bind_thread_employees_with_pool, ensure_employee_sessions_for_event_with_pool,
    get_thread_employee_bindings_with_pool, link_inbound_event_to_session_with_pool,
    list_agent_employees_with_pool, resolve_target_employees_for_event,
    upsert_agent_employee_with_pool, UpsertAgentEmployeeInput,
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

    bind_thread_employees_with_pool(&pool, "chat_001", std::slice::from_ref(&employee_id))
        .await
        .expect("bind thread employees");
    let binding = get_thread_employee_bindings_with_pool(&pool, "chat_001")
        .await
        .expect("get thread bindings");
    assert_eq!(binding.employee_ids, vec![employee_id.clone()]);

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

    let other_id = upsert_agent_employee_with_pool(
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

    bind_thread_employees_with_pool(&pool, "chat_group_1", &[other_id.clone(), main_id.clone()])
        .await
        .expect("bind thread employees");

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
