use crate::helpers;
use runtime_lib::commands::employee_agents::test_support::create_employee_team_with_pool;
use runtime_lib::commands::employee_agents::{
    bridge_inbound_event_to_employee_sessions_with_pool, ensure_agent_sessions_for_event_with_pool,
    ensure_employee_sessions_for_event_with_pool, link_inbound_event_to_agent_session_with_pool,
    list_agent_employees_with_pool, resolve_target_employees_for_event,
    save_feishu_employee_association_with_pool, upsert_agent_employee_with_pool,
    CreateEmployeeTeamInput, SaveFeishuEmployeeAssociationInput, UpsertAgentEmployeeInput,
};
use runtime_lib::commands::im_routing::{
    list_im_routing_bindings_with_pool, upsert_im_routing_binding_with_pool,
    UpsertImRoutingBindingInput,
};
use runtime_lib::im::types::{ImEvent, ImEventType};

fn feishu_peer_event(
    thread_id: &str,
    event_id: &str,
    message_id: &str,
    text: &str,
    tenant_id: Option<&str>,
    role_id: Option<&str>,
) -> ImEvent {
    let tenant_id = tenant_id.map(str::to_string);
    let conversation_id = tenant_id
        .as_deref()
        .map(|tenant| format!("feishu:{tenant}:group:{thread_id}"));

    ImEvent {
        channel: "feishu".to_string(),
        event_type: ImEventType::MessageCreated,
        thread_id: thread_id.to_string(),
        event_id: Some(event_id.to_string()),
        message_id: Some(message_id.to_string()),
        text: Some(text.to_string()),
        role_id: role_id.map(str::to_string),
        account_id: None,
        tenant_id,
        sender_id: None,
        chat_type: Some("group".to_string()),
        conversation_id: conversation_id.clone(),
        base_conversation_id: conversation_id,
        parent_conversation_candidates: Vec::new(),
        conversation_scope: Some("peer".to_string()),
    }
}

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

    let employee_db_id = upsert_agent_employee_with_pool(
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

    let event = feishu_peer_event(
        "chat_001",
        "evt_001",
        "msg_001",
        "请评估这个商机",
        Some("tenant-a"),
        None,
    );

    let sessions = ensure_agent_sessions_for_event_with_pool(&pool, &event)
        .await
        .expect("ensure agent sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(employees[0].id, employee_db_id);
    assert_eq!(sessions[0].agent_id, "project_manager");

    let (skill_id, work_dir, permission_mode): (String, String, String) =
        sqlx::query_as("SELECT skill_id, work_dir, permission_mode FROM sessions WHERE id = ?")
            .bind(&sessions[0].session_id)
            .fetch_one(&pool)
            .await
            .expect("query created session");
    assert_eq!(skill_id, "builtin-general");
    assert_eq!(work_dir, "E:/workspace/pm");
    assert_eq!(permission_mode, "standard");

    link_inbound_event_to_agent_session_with_pool(
        &pool,
        &event,
        &sessions[0].agent_id,
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
        &feishu_peer_event(
            "chat_group_1",
            "evt_001",
            "msg_001",
            "大家讨论一下这个商机",
            None,
            None,
        ),
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
        &feishu_peer_event(
            "chat_group_mention",
            "evt_mention_001",
            "msg_mention_001",
            "@开发团队 请开始处理",
            None,
            Some("ou_dev_team"),
        ),
    )
    .await
    .expect("resolve target employees");

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].id, dev_id);
}

#[tokio::test]
async fn save_feishu_employee_association_replaces_default_binding_and_updates_scope() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let pm_id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "pm".to_string(),
            name: "项目经理".to_string(),
            role_id: "pm".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "pm".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert pm");

    let tech_id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "tech".to_string(),
            name: "技术负责人".to_string(),
            role_id: "tech".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "tech".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["app".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert tech");

    upsert_im_routing_binding_with_pool(
        &pool,
        UpsertImRoutingBindingInput {
            id: Some("binding-default-pm".to_string()),
            agent_id: "pm".to_string(),
            channel: "feishu".to_string(),
            account_id: "*".to_string(),
            peer_kind: "group".to_string(),
            peer_id: "".to_string(),
            guild_id: "".to_string(),
            team_id: "".to_string(),
            role_ids: vec![],
            connector_meta: serde_json::json!({ "connector_id": "feishu" }),
            priority: 100,
            enabled: true,
        },
    )
    .await
    .expect("seed default binding");

    save_feishu_employee_association_with_pool(
        &pool,
        SaveFeishuEmployeeAssociationInput {
            employee_db_id: tech_id.clone(),
            enabled: true,
            mode: "default".to_string(),
            peer_kind: "group".to_string(),
            peer_id: "".to_string(),
            priority: 66,
        },
    )
    .await
    .expect("save association");

    let employees = list_agent_employees_with_pool(&pool)
        .await
        .expect("list employees");
    let tech = employees
        .iter()
        .find(|employee| employee.id == tech_id)
        .expect("find tech");
    assert!(tech.enabled_scopes.iter().any(|scope| scope == "feishu"));

    let bindings = list_im_routing_bindings_with_pool(&pool)
        .await
        .expect("list bindings");
    let feishu_defaults: Vec<_> = bindings
        .iter()
        .filter(|binding| binding.channel == "feishu" && binding.peer_id.is_empty())
        .collect();
    assert_eq!(feishu_defaults.len(), 1);
    assert_eq!(feishu_defaults[0].agent_id, "tech");
    assert_eq!(feishu_defaults[0].priority, 66);
    assert!(!bindings
        .iter()
        .any(|binding| binding.id == "binding-default-pm"));

    let pm = employees
        .iter()
        .find(|employee| employee.id == pm_id)
        .expect("find pm");
    assert!(!pm.enabled_scopes.iter().any(|scope| scope == "feishu"));

    let targets = resolve_target_employees_for_event(
        &pool,
        &feishu_peer_event(
            "chat-default",
            "evt_default_001",
            "msg_default_001",
            "请处理今天的项目进展",
            None,
            None,
        ),
    )
    .await
    .expect("resolve targets after default replacement");
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].id, tech_id);
}

#[tokio::test]
async fn save_feishu_employee_association_rolls_back_scope_update_when_binding_insert_fails() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let tech_id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "tech".to_string(),
            name: "技术负责人".to_string(),
            role_id: "tech".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "tech".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["app".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert tech");

    sqlx::query(
        "CREATE TRIGGER fail_feishu_binding_insert
         BEFORE INSERT ON im_routing_bindings
         BEGIN
           SELECT RAISE(ABORT, 'blocked by test trigger');
         END;",
    )
    .execute(&pool)
    .await
    .expect("create failing trigger");

    let error = save_feishu_employee_association_with_pool(
        &pool,
        SaveFeishuEmployeeAssociationInput {
            employee_db_id: tech_id.clone(),
            enabled: true,
            mode: "default".to_string(),
            peer_kind: "group".to_string(),
            peer_id: "".to_string(),
            priority: 100,
        },
    )
    .await
    .expect_err("save should fail");
    assert!(error.contains("blocked by test trigger"));

    let employees = list_agent_employees_with_pool(&pool)
        .await
        .expect("list employees after failure");
    let tech = employees
        .iter()
        .find(|employee| employee.id == tech_id)
        .expect("find tech");
    assert_eq!(tech.enabled_scopes, vec!["app".to_string()]);

    let bindings = list_im_routing_bindings_with_pool(&pool)
        .await
        .expect("list bindings after failure");
    assert!(bindings.is_empty());
}

#[tokio::test]
async fn wecom_event_prefers_wecom_scoped_employee_and_creates_session() {
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
    .expect("upsert feishu default");

    let wecom_employee_id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "wecom_sales".to_string(),
            name: "企业微信销售".to_string(),
            role_id: "wecom_sales".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "E:/workspace/wecom-sales".to_string(),
            openclaw_agent_id: "wecom_sales".to_string(),
            routing_priority: 90,
            enabled_scopes: vec!["wecom".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec!["builtin-general".to_string()],
        },
    )
    .await
    .expect("upsert wecom employee");

    let event = ImEvent {
        channel: "wecom".to_string(),
        event_type: ImEventType::MessageCreated,
        thread_id: "wecom_chat_001".to_string(),
        event_id: Some("evt_wecom_001".to_string()),
        message_id: Some("msg_wecom_001".to_string()),
        text: Some("请跟进企业微信线索".to_string()),
        role_id: None,
        account_id: Some("corp-123".to_string()),
        tenant_id: Some("corp-123".to_string()),
        sender_id: None,
        chat_type: Some("group".to_string()),
        conversation_id: Some("wecom:corp-123:group:wecom_chat_001".to_string()),
        base_conversation_id: Some("wecom:corp-123:group:wecom_chat_001".to_string()),
        parent_conversation_candidates: Vec::new(),
        conversation_scope: Some("peer".to_string()),
    };

    let targets = resolve_target_employees_for_event(&pool, &event)
        .await
        .expect("resolve wecom target employees");
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].id, wecom_employee_id);

    let sessions = ensure_employee_sessions_for_event_with_pool(&pool, &event)
        .await
        .expect("ensure wecom sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].employee_id, "wecom_sales");

    let (work_dir,): (String,) = sqlx::query_as("SELECT work_dir FROM sessions WHERE id = ?")
        .bind(&sessions[0].session_id)
        .fetch_one(&pool)
        .await
        .expect("query wecom session");
    assert_eq!(work_dir, "E:/workspace/wecom-sales");
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
        &feishu_peer_event(
            "chat_group_text_mention",
            "evt_text_mention_001",
            "msg_text_mention_001",
            "@开发团队 请开始处理",
            None,
            None,
        ),
    )
    .await
    .expect("resolve target employees");

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].id, dev_id);
}

#[tokio::test]
async fn ensure_employee_sessions_for_event_prefers_team_entry_employee_when_binding_team_id_matches(
) {
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
            default_work_dir: "E:/workspace/main".to_string(),
            openclaw_agent_id: "main".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec!["builtin-general".to_string()],
        },
    )
    .await
    .expect("seed main employee");

    let taizi_id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "taizi".to_string(),
            name: "太子".to_string(),
            role_id: "taizi".to_string(),
            persona: "".to_string(),
            feishu_open_id: "ou_taizi".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "E:/workspace/taizi".to_string(),
            openclaw_agent_id: "taizi".to_string(),
            routing_priority: 90,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec!["builtin-general".to_string()],
        },
    )
    .await
    .expect("seed team entry employee");

    let group_id = create_employee_team_with_pool(
        &pool,
        CreateEmployeeTeamInput {
            name: "绑定团队入口".to_string(),
            coordinator_employee_id: "main".to_string(),
            member_employee_ids: vec!["main".to_string(), "taizi".to_string()],
            entry_employee_id: "taizi".to_string(),
            planner_employee_id: "".to_string(),
            reviewer_employee_id: "".to_string(),
            review_mode: "none".to_string(),
            execution_mode: "sequential".to_string(),
            visibility_mode: "shared".to_string(),
            rules: vec![],
        },
    )
    .await
    .expect("create bound team");

    upsert_im_routing_binding_with_pool(
        &pool,
        UpsertImRoutingBindingInput {
            id: None,
            agent_id: "main".to_string(),
            channel: "feishu".to_string(),
            account_id: "tenant-a".to_string(),
            peer_kind: "group".to_string(),
            peer_id: "chat_team_001".to_string(),
            guild_id: "".to_string(),
            team_id: group_id,
            role_ids: vec![],
            connector_meta: serde_json::json!({}),
            priority: 1,
            enabled: true,
        },
    )
    .await
    .expect("seed routing binding");

    let event = feishu_peer_event(
        "chat_team_001",
        "evt_team_001",
        "msg_team_001",
        "请团队开始处理",
        Some("tenant-a"),
        None,
    );

    let sessions = ensure_agent_sessions_for_event_with_pool(&pool, &event)
        .await
        .expect("ensure agent sessions");

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].agent_id, "taizi");
    assert_ne!(sessions[0].agent_id, "main");

    let (session_employee_id,): (String,) =
        sqlx::query_as("SELECT employee_id FROM sessions WHERE id = ?")
            .bind(&sessions[0].session_id)
            .fetch_one(&pool)
            .await
            .expect("load ensured session");
    assert_eq!(session_employee_id, "taizi");
}

#[tokio::test]
async fn employee_wrapper_bridge_follows_agent_first_dispatch_authority() {
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
            openclaw_agent_id: "agent-main".to_string(),
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
            employee_id: "dev_team".to_string(),
            name: "开发团队".to_string(),
            role_id: "dev_team".to_string(),
            persona: "".to_string(),
            feishu_open_id: "ou_dev_team".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "agent-dev".to_string(),
            routing_priority: 90,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert dev");

    let event = feishu_peer_event(
        "chat_wrapper_authority",
        "evt_wrapper_authority",
        "msg_wrapper_authority",
        "@开发团队 请先看一下",
        Some("tenant-a"),
        Some("ou_dev_team"),
    );

    let dispatches = bridge_inbound_event_to_employee_sessions_with_pool(
        &pool,
        &event,
        Some(&serde_json::json!({
            "agentId": "agent-main",
            "sessionKey": "feishu:tenant-a:agent-main",
            "matchedBy": "openclaw",
        })),
    )
    .await
    .expect("bridge employee dispatches");

    assert_eq!(dispatches.len(), 1);
    assert_eq!(dispatches[0].employee_id, "agent-main");
    assert_eq!(dispatches[0].route_agent_id, "agent-main");
    assert_eq!(dispatches[0].matched_by, "openclaw");

    let ensured = ensure_employee_sessions_for_event_with_pool(&pool, &event)
        .await
        .expect("ensure employee sessions without route override");
    assert_eq!(ensured.len(), 1);
    assert_eq!(ensured[0].employee_id, "agent-dev");
}
