mod helpers;

use runtime_lib::commands::employee_agents::{
    upsert_agent_employee_with_pool, UpsertAgentEmployeeInput,
};
use runtime_lib::commands::feishu_gateway::test_support::
    list_enabled_employee_feishu_connections_with_pool;

#[tokio::test]
async fn list_enabled_employee_feishu_connections_returns_all_bound_employees() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "project_manager".to_string(),
            name: "项目经理".to_string(),
            role_id: "project_manager".to_string(),
            persona: "".to_string(),
            feishu_open_id: "ou_pm".to_string(),
            feishu_app_id: "cli_pm".to_string(),
            feishu_app_secret: "sec_pm".to_string(),
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
    .expect("seed project manager");

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "tech_lead".to_string(),
            name: "技术负责人".to_string(),
            role_id: "tech_lead".to_string(),
            persona: "".to_string(),
            feishu_open_id: "ou_tech".to_string(),
            feishu_app_id: "cli_tech".to_string(),
            feishu_app_secret: "sec_tech".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "tech_lead".to_string(),
            routing_priority: 90,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec![],
        },
    )
    .await
    .expect("seed tech lead");

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "disabled_employee".to_string(),
            name: "禁用员工".to_string(),
            role_id: "disabled_employee".to_string(),
            persona: "".to_string(),
            feishu_open_id: "ou_disabled".to_string(),
            feishu_app_id: "cli_disabled".to_string(),
            feishu_app_secret: "sec_disabled".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "disabled_employee".to_string(),
            routing_priority: 120,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: false,
            is_default: false,
            skill_ids: vec![],
        },
    )
    .await
    .expect("seed disabled employee");

    let rows = list_enabled_employee_feishu_connections_with_pool(&pool)
        .await
        .expect("list employee feishu connections");

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].employee_id, "project_manager");
    assert_eq!(rows[0].app_id, "cli_pm");
    assert_eq!(rows[1].employee_id, "tech_lead");
    assert_eq!(rows[1].app_secret, "sec_tech");
}
