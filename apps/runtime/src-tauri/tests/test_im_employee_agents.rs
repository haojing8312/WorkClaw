mod helpers;

use runtime_lib::commands::employee_agents::{
    cancel_employee_group_run_with_pool, clone_employee_group_template_with_pool,
    continue_employee_group_run_with_pool, create_employee_group_with_pool,
    create_employee_team_with_pool, delete_employee_group_with_pool,
    ensure_employee_sessions_for_event_with_pool, get_employee_group_run_snapshot_with_pool,
    link_inbound_event_to_session_with_pool, list_agent_employees_with_pool,
    list_employee_group_rules_with_pool, list_employee_groups_with_pool,
    maybe_handle_team_entry_session_message_with_pool, pause_employee_group_run_with_pool,
    reassign_group_run_step_with_pool, resolve_target_employees_for_event,
    resume_employee_group_run_with_pool, retry_employee_group_run_failed_steps_with_pool,
    review_group_run_step_with_pool, run_group_step_with_pool, start_employee_group_run_with_pool,
    upsert_agent_employee_with_pool, CloneEmployeeGroupTemplateInput, CreateEmployeeGroupInput,
    CreateEmployeeTeamInput, StartEmployeeGroupRunInput, UpsertAgentEmployeeInput,
};
use runtime_lib::commands::im_routing::{
    upsert_im_routing_binding_with_pool, UpsertImRoutingBindingInput,
};
use runtime_lib::im::types::{ImEvent, ImEventType};
use uuid::Uuid;

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
        channel: "feishu".to_string(),
        event_type: ImEventType::MessageCreated,
        thread_id: "chat_001".to_string(),
        event_id: Some("evt_001".to_string()),
        message_id: Some("msg_001".to_string()),
        text: Some("请评估这个商机".to_string()),
        role_id: None,
        account_id: None,
        tenant_id: Some("tenant-a".to_string()),
    };

    let sessions = ensure_employee_sessions_for_event_with_pool(&pool, &event)
        .await
        .expect("ensure employee sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].employee_id, employee_id);

    let (skill_id, work_dir, permission_mode): (String, String, String) =
        sqlx::query_as("SELECT skill_id, work_dir, permission_mode FROM sessions WHERE id = ?")
            .bind(&sessions[0].session_id)
            .fetch_one(&pool)
            .await
            .expect("query created session");
    assert_eq!(skill_id, "builtin-general");
    assert_eq!(work_dir, "E:/workspace/pm");
    assert_eq!(permission_mode, "standard");

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
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat_group_1".to_string(),
            event_id: Some("evt_001".to_string()),
            message_id: Some("msg_001".to_string()),
            text: Some("大家讨论一下这个商机".to_string()),
            role_id: None,
            account_id: None,
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
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat_group_mention".to_string(),
            event_id: Some("evt_mention_001".to_string()),
            message_id: Some("msg_mention_001".to_string()),
            text: Some("@开发团队 请开始处理".to_string()),
            role_id: Some("ou_dev_team".to_string()),
            account_id: None,
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
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat_group_text_mention".to_string(),
            event_id: Some("evt_text_mention_001".to_string()),
            message_id: Some("msg_text_mention_001".to_string()),
            text: Some("@开发团队 请开始处理".to_string()),
            role_id: None,
            account_id: None,
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

    let groups = list_employee_groups_with_pool(&pool)
        .await
        .expect("list groups");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].id, group_id);
    assert_eq!(groups[0].coordinator_employee_id, "project_manager");
    assert_eq!(groups[0].member_count, 3);

    delete_employee_group_with_pool(&pool, &group_id)
        .await
        .expect("delete group");

    let groups_after_delete = list_employee_groups_with_pool(&pool)
        .await
        .expect("list groups");
    assert!(groups_after_delete.is_empty());
}

#[tokio::test]
async fn list_employee_group_rules_returns_review_relationships() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "审议团队".to_string(),
            coordinator_employee_id: "zhongshu".to_string(),
            member_employee_ids: vec!["zhongshu".to_string(), "menxia".to_string()],
        },
    )
    .await
    .expect("create group");

    sqlx::query(
        "INSERT INTO employee_group_rules (
            id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
         ) VALUES ('rule-review-list', ?, 'zhongshu', 'menxia', 'review', 'plan', 1, 100, '2026-03-07T00:00:00Z')",
    )
    .bind(&group_id)
    .execute(&pool)
    .await
    .expect("insert review rule");

    let rules = list_employee_group_rules_with_pool(&pool, &group_id)
        .await
        .expect("list group rules");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].relation_type, "review");
    assert_eq!(rules[0].from_employee_id, "zhongshu");
    assert_eq!(rules[0].to_employee_id, "menxia");
}

#[tokio::test]
async fn create_employee_team_persists_runtime_config_and_default_rules() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let group_id = create_employee_team_with_pool(
        &pool,
        CreateEmployeeTeamInput {
            name: "自定义复杂任务团队".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec![
                "taizi".to_string(),
                "zhongshu".to_string(),
                "menxia".to_string(),
                "shangshu".to_string(),
                "bingbu".to_string(),
                "gongbu".to_string(),
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

    let groups = list_employee_groups_with_pool(&pool)
        .await
        .expect("list groups");
    let group = groups
        .into_iter()
        .find(|item| item.id == group_id)
        .expect("created group exists");
    assert_eq!(group.entry_employee_id, "taizi");
    assert_eq!(group.review_mode, "hard");
    assert_eq!(group.execution_mode, "parallel");
    assert_eq!(group.visibility_mode, "shared");
    assert!(group.config_json.contains("\"role_type\":\"planner\""));
    assert!(group.config_json.contains("\"employee_id\":\"zhongshu\""));

    let rules = list_employee_group_rules_with_pool(&pool, &group_id)
        .await
        .expect("list rules");
    assert!(rules.iter().any(|rule| {
        rule.from_employee_id == "taizi"
            && rule.to_employee_id == "zhongshu"
            && rule.relation_type == "delegate"
            && rule.phase_scope == "intake"
    }));
    assert!(rules.iter().any(|rule| {
        rule.from_employee_id == "zhongshu"
            && rule.to_employee_id == "menxia"
            && rule.relation_type == "review"
            && rule.phase_scope == "plan"
    }));
    assert!(rules.iter().any(|rule| {
        rule.from_employee_id == "shangshu"
            && rule.to_employee_id == "bingbu"
            && rule.relation_type == "delegate"
            && rule.phase_scope == "execute"
    }));
    assert!(rules.iter().any(|rule| {
        rule.from_employee_id == "shangshu"
            && rule.to_employee_id == "gongbu"
            && rule.relation_type == "delegate"
            && rule.phase_scope == "execute"
    }));
    assert!(rules.iter().any(|rule| {
        rule.from_employee_id == "shangshu"
            && rule.to_employee_id == "taizi"
            && rule.relation_type == "report"
            && rule.phase_scope == "finalize"
    }));
}

#[tokio::test]
async fn clone_employee_group_template_preserves_rules_and_template_metadata() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    let source_group_id = create_employee_group_with_pool(
        &pool,
        CreateEmployeeGroupInput {
            name: "默认复杂任务团队".to_string(),
            coordinator_employee_id: "shangshu".to_string(),
            member_employee_ids: vec![
                "taizi".to_string(),
                "zhongshu".to_string(),
                "menxia".to_string(),
                "shangshu".to_string(),
            ],
        },
    )
    .await
    .expect("create source group");

    sqlx::query(
        "UPDATE employee_groups
         SET template_id = 'sansheng-liubu',
             entry_employee_id = 'taizi',
             review_mode = 'hard',
             execution_mode = 'parallel',
             visibility_mode = 'team_only',
             is_bootstrap_seeded = 1,
             config_json = '{\"roles\":[{\"role_type\":\"entry\",\"employee_key\":\"taizi\"}]}'
         WHERE id = ?",
    )
    .bind(&source_group_id)
    .execute(&pool)
    .await
    .expect("update source group metadata");

    sqlx::query(
        "INSERT INTO employee_group_rules (
            id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
         ) VALUES ('rule-clone-1', ?, 'zhongshu', 'menxia', 'review', 'plan', 1, 100, '2026-03-07T00:00:00Z')",
    )
    .bind(&source_group_id)
    .execute(&pool)
    .await
    .expect("insert source rule");

    let cloned_group_id = clone_employee_group_template_with_pool(
        &pool,
        CloneEmployeeGroupTemplateInput {
            source_group_id: source_group_id.clone(),
            name: "默认复杂任务团队（副本）".to_string(),
        },
    )
    .await
    .expect("clone team template");

    assert_ne!(cloned_group_id, source_group_id);

    let cloned_groups = list_employee_groups_with_pool(&pool)
        .await
        .expect("list groups after clone");
    let cloned = cloned_groups
        .iter()
        .find(|group| group.id == cloned_group_id)
        .expect("cloned group should exist");
    assert_eq!(cloned.name, "默认复杂任务团队（副本）");
    assert_eq!(cloned.template_id, "sansheng-liubu");
    assert_eq!(cloned.entry_employee_id, "taizi");
    assert_eq!(cloned.review_mode, "hard");
    assert_eq!(cloned.execution_mode, "parallel");
    assert_eq!(cloned.visibility_mode, "team_only");
    assert!(!cloned.is_bootstrap_seeded);

    let rules = list_employee_group_rules_with_pool(&pool, &cloned_group_id)
        .await
        .expect("list cloned rules");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].group_id, cloned_group_id);
    assert_eq!(rules[0].relation_type, "review");
    assert_eq!(rules[0].from_employee_id, "zhongshu");
    assert_eq!(rules[0].to_employee_id, "menxia");
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

    let event = ImEvent {
        channel: "feishu".to_string(),
        event_type: ImEventType::MessageCreated,
        thread_id: "chat_team_001".to_string(),
        event_id: Some("evt_team_001".to_string()),
        message_id: Some("msg_team_001".to_string()),
        text: Some("请团队开始处理".to_string()),
        role_id: None,
        account_id: None,
        tenant_id: Some("tenant-a".to_string()),
    };

    let sessions = ensure_employee_sessions_for_event_with_pool(&pool, &event)
        .await
        .expect("ensure employee sessions");

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].employee_id, taizi_id);
    assert_ne!(sessions[0].employee_id, main_id);

    let (session_employee_id,): (String,) =
        sqlx::query_as("SELECT employee_id FROM sessions WHERE id = ?")
            .bind(&sessions[0].session_id)
            .fetch_one(&pool)
            .await
            .expect("load ensured session");
    assert_eq!(session_employee_id, "taizi");
}
