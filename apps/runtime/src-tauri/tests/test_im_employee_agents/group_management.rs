use crate::helpers;
use runtime_lib::commands::employee_agents::{
    upsert_agent_employee_with_pool, CloneEmployeeGroupTemplateInput, CreateEmployeeGroupInput,
    CreateEmployeeTeamInput, UpsertAgentEmployeeInput,
};
use runtime_lib::commands::employee_agents::test_support::{
    clone_employee_group_template_with_pool, create_employee_group_with_pool,
    create_employee_team_with_pool, delete_employee_group_with_pool,
    list_employee_group_rules_with_pool, list_employee_groups_with_pool,
};

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
