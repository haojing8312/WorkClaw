mod helpers;

use runtime_lib::commands::agent_profile::{
    apply_agent_profile_with_pool, generate_agent_profile_draft_with_pool, AgentProfileAnswerInput,
    AgentProfilePayload,
};
use runtime_lib::commands::employee_agents::{
    upsert_agent_employee_with_pool, UpsertAgentEmployeeInput,
};

#[tokio::test]
async fn apply_agent_profile_writes_agents_soul_user_files() {
    let (pool, tmp) = helpers::setup_test_db().await;
    let work_dir = tmp
        .path()
        .join("employee-workspace")
        .to_string_lossy()
        .to_string();

    let employee_db_id = upsert_agent_employee_with_pool(
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
            default_work_dir: work_dir.clone(),
            openclaw_agent_id: "project_manager".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert employee");

    let payload = AgentProfilePayload {
        employee_db_id: employee_db_id.clone(),
        answers: vec![
            AgentProfileAnswerInput {
                key: "mission".to_string(),
                question: "该员工的核心使命是什么？".to_string(),
                answer: "推进需求到上线的高质量交付。".to_string(),
            },
            AgentProfileAnswerInput {
                key: "tone".to_string(),
                question: "沟通风格是什么？".to_string(),
                answer: "专业、直接、可执行。".to_string(),
            },
        ],
    };

    let draft = generate_agent_profile_draft_with_pool(&pool, payload.clone())
        .await
        .expect("generate draft");
    assert!(draft.agents_md.contains("# AGENTS"));
    assert!(draft.soul_md.contains("# SOUL"));
    assert!(draft.user_md.contains("# USER"));

    let result = apply_agent_profile_with_pool(&pool, payload)
        .await
        .expect("apply profile");
    assert_eq!(result.files.len(), 3);
    assert!(result.files.iter().all(|file| file.ok));

    let profile_root = std::path::PathBuf::from(work_dir)
        .join("openclaw")
        .join("project_manager");
    let agents_path = profile_root.join("AGENTS.md");
    let soul_path = profile_root.join("SOUL.md");
    let user_path = profile_root.join("USER.md");

    assert!(agents_path.exists(), "AGENTS.md should exist");
    assert!(soul_path.exists(), "SOUL.md should exist");
    assert!(user_path.exists(), "USER.md should exist");

    let agents_text = std::fs::read_to_string(&agents_path).expect("read AGENTS.md");
    let soul_text = std::fs::read_to_string(&soul_path).expect("read SOUL.md");
    let user_text = std::fs::read_to_string(&user_path).expect("read USER.md");

    assert!(agents_text.contains("项目经理"));
    assert!(agents_text.contains("推进需求到上线的高质量交付"));
    assert!(soul_text.contains("专业、直接、可执行"));
    assert!(user_text.contains("# USER"));
}

#[tokio::test]
async fn agent_profile_draft_uses_employee_enabled_scopes_in_agents_doc() {
    let (pool, tmp) = helpers::setup_test_db().await;
    let work_dir = tmp
        .path()
        .join("employee-workspace-wecom")
        .to_string_lossy()
        .to_string();

    let employee_db_id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "wecom_operator".to_string(),
            name: "企业微信运营".to_string(),
            role_id: "wecom_operator".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: work_dir,
            openclaw_agent_id: "wecom_operator".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["app".to_string(), "wecom".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert employee");

    let draft = generate_agent_profile_draft_with_pool(
        &pool,
        AgentProfilePayload {
            employee_db_id,
            answers: vec![],
        },
    )
    .await
    .expect("generate draft");

    assert!(draft.agents_md.contains("适用范围: app, wecom"));
    assert!(!draft.agents_md.contains("飞书范围: feishu"));
}
