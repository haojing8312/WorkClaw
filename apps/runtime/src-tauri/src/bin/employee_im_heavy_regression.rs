#![recursion_limit = "256"]

use anyhow::{anyhow, Context, Result};
#[path = "../../tests/helpers/mod.rs"]
mod test_helpers;

use runtime_lib::commands::employee_agents::test_support::{
    clone_employee_group_template_with_pool, create_employee_group_with_pool,
    create_employee_team_with_pool, list_employee_group_rules_with_pool, list_employee_groups_with_pool,
    maybe_handle_team_entry_session_message_with_pool,
};
use runtime_lib::commands::employee_agents::{
    ensure_employee_sessions_for_event_with_pool, link_inbound_event_to_session_with_pool,
    list_agent_employees_with_pool, resolve_target_employees_for_event,
    upsert_agent_employee_with_pool, CloneEmployeeGroupTemplateInput, CreateEmployeeGroupInput,
    CreateEmployeeTeamInput, UpsertAgentEmployeeInput,
};
use runtime_lib::commands::im_routing::{
    upsert_im_routing_binding_with_pool, UpsertImRoutingBindingInput,
};
use runtime_lib::im::types::{ImEvent, ImEventType};
use uuid::Uuid;

#[derive(Debug, serde::Serialize)]
struct RegressionCaseSummary {
    scenario: &'static str,
    status: &'static str,
}

fn main() -> Result<()> {
    let handle = std::thread::Builder::new()
        .name("employee-im-heavy-regression".to_string())
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
    let results = vec![
        scenario_create_group_constraints().await?,
        scenario_create_team_persists_runtime_config().await?,
        scenario_clone_group_template_preserves_metadata().await?,
        scenario_employee_config_and_im_session_mapping().await?,
        scenario_group_message_text_mention_routes_target().await?,
        scenario_team_entry_binding_prefers_entry_employee().await?,
        scenario_team_entry_ignores_non_entry_sessions().await?,
        scenario_team_entry_reuses_existing_chat_session().await?,
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

async fn seed_model(pool: &sqlx::SqlitePool, base_url: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
         VALUES ('m1', 'default', 'openai', ?, 'gpt-4o-mini', 1, 'k')",
    )
    .bind(base_url)
    .execute(pool)
    .await
    .context("seed model config")?;
    Ok(())
}

async fn seed_employee(
    pool: &sqlx::SqlitePool,
    employee_id: &str,
    name: &str,
    persona: &str,
    work_dir: &str,
    scopes: Vec<String>,
    is_default: bool,
    feishu_open_id: &str,
) -> Result<String> {
    upsert_agent_employee_with_pool(
        pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: employee_id.to_string(),
            name: name.to_string(),
            role_id: employee_id.to_string(),
            persona: persona.to_string(),
            feishu_open_id: feishu_open_id.to_string(),
            feishu_app_id: String::new(),
            feishu_app_secret: String::new(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: work_dir.to_string(),
            openclaw_agent_id: employee_id.to_string(),
            routing_priority: 100,
            enabled_scopes: scopes,
            enabled: true,
            is_default,
            skill_ids: vec!["builtin-general".to_string()],
        },
    )
    .await
    .map_err(|error| anyhow!("seed employee {employee_id}: {error}"))
}

async fn scenario_create_group_constraints() -> Result<RegressionCaseSummary> {
    let (pool, _tmp) = test_helpers::setup_test_db().await;

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
    if !too_many.contains("cannot exceed 10") {
        return Err(anyhow!("unexpected too-many-members error: {too_many}"));
    }

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
    if !missing_coordinator.contains("must be included in members") {
        return Err(anyhow!(
            "unexpected missing-coordinator error: {missing_coordinator}"
        ));
    }

    Ok(RegressionCaseSummary {
        scenario: "create_employee_group_rejects_more_than_ten_members_and_missing_coordinator",
        status: "pass",
    })
}

async fn scenario_create_team_persists_runtime_config() -> Result<RegressionCaseSummary> {
    let (pool, _tmp) = test_helpers::setup_test_db().await;

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
    .map_err(|error| anyhow!("create employee team: {error}"))?;

    let groups = list_employee_groups_with_pool(&pool)
        .await
        .map_err(|error| anyhow!("list groups: {error}"))?;
    let group = groups
        .into_iter()
        .find(|item| item.id == group_id)
        .ok_or_else(|| anyhow!("created group missing"))?;
    if group.entry_employee_id != "taizi"
        || group.review_mode != "hard"
        || group.execution_mode != "parallel"
        || group.visibility_mode != "shared"
    {
        return Err(anyhow!("unexpected runtime config persisted on created team"));
    }

    let rules = list_employee_group_rules_with_pool(&pool, &group_id)
        .await
        .map_err(|error| anyhow!("list rules: {error}"))?;
    let has_planner = rules.iter().any(|rule| {
        rule.from_employee_id == "taizi"
            && rule.to_employee_id == "zhongshu"
            && rule.relation_type == "delegate"
            && rule.phase_scope == "intake"
    });
    let has_review = rules.iter().any(|rule| {
        rule.from_employee_id == "zhongshu"
            && rule.to_employee_id == "menxia"
            && rule.relation_type == "review"
            && rule.phase_scope == "plan"
    });
    if !(has_planner && has_review) {
        return Err(anyhow!("default runtime rules missing from created team"));
    }

    Ok(RegressionCaseSummary {
        scenario: "create_employee_team_persists_runtime_config_and_default_rules",
        status: "pass",
    })
}

async fn scenario_clone_group_template_preserves_metadata() -> Result<RegressionCaseSummary> {
    let (pool, _tmp) = test_helpers::setup_test_db().await;

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
    .map_err(|error| anyhow!("create source group: {error}"))?;

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
    .context("update source group metadata")?;

    sqlx::query(
        "INSERT INTO employee_group_rules (
            id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at
         ) VALUES ('rule-clone-1', ?, 'zhongshu', 'menxia', 'review', 'plan', 1, 100, '2026-03-07T00:00:00Z')",
    )
    .bind(&source_group_id)
    .execute(&pool)
    .await
    .context("insert source rule")?;

    let cloned_group_id = clone_employee_group_template_with_pool(
        &pool,
        CloneEmployeeGroupTemplateInput {
            source_group_id: source_group_id.clone(),
            name: "默认复杂任务团队（副本）".to_string(),
        },
    )
    .await
    .map_err(|error| anyhow!("clone team template: {error}"))?;

    let cloned_groups = list_employee_groups_with_pool(&pool)
        .await
        .map_err(|error| anyhow!("list groups after clone: {error}"))?;
    let cloned = cloned_groups
        .iter()
        .find(|group| group.id == cloned_group_id)
        .ok_or_else(|| anyhow!("cloned group missing"))?;
    if cloned.template_id != "sansheng-liubu"
        || cloned.entry_employee_id != "taizi"
        || cloned.review_mode != "hard"
        || cloned.execution_mode != "parallel"
        || cloned.visibility_mode != "team_only"
        || cloned.is_bootstrap_seeded
    {
        return Err(anyhow!("cloned group metadata mismatch"));
    }

    let rules = list_employee_group_rules_with_pool(&pool, &cloned_group_id)
        .await
        .map_err(|error| anyhow!("list cloned rules: {error}"))?;
    if rules.len() != 1
        || rules[0].relation_type != "review"
        || rules[0].from_employee_id != "zhongshu"
        || rules[0].to_employee_id != "menxia"
    {
        return Err(anyhow!("cloned rules mismatch"));
    }

    Ok(RegressionCaseSummary {
        scenario: "clone_employee_group_template_preserves_rules_and_template_metadata",
        status: "pass",
    })
}

async fn scenario_employee_config_and_im_session_mapping() -> Result<RegressionCaseSummary> {
    let (pool, _tmp) = test_helpers::setup_test_db().await;
    seed_model(&pool, "https://example.com").await?;

    let employee_id = seed_employee(
        &pool,
        "project_manager",
        "项目经理智能体",
        "负责拆解需求",
        "E:/workspace/pm",
        vec!["feishu".to_string()],
        true,
        "ou_pm_1",
    )
    .await?;

    let employees = list_agent_employees_with_pool(&pool)
        .await
        .map_err(|error| anyhow!("list employees: {error}"))?;
    if employees.len() != 1 || employees[0].skill_ids != vec!["builtin-general".to_string()] {
        return Err(anyhow!("employee config not persisted as expected"));
    }

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
        sender_id: None,
        chat_type: None,
    };

    let sessions = ensure_employee_sessions_for_event_with_pool(&pool, &event)
        .await
        .map_err(|error| anyhow!("ensure employee sessions: {error}"))?;
    if sessions.len() != 1 || sessions[0].employee_id != employee_id {
        return Err(anyhow!("unexpected ensured employee sessions"));
    }

    let (skill_id, work_dir, permission_mode): (String, String, String) =
        sqlx::query_as("SELECT skill_id, work_dir, permission_mode FROM sessions WHERE id = ?")
            .bind(&sessions[0].session_id)
            .fetch_one(&pool)
            .await
            .context("query created session")?;
    if skill_id != "builtin-general"
        || work_dir != "E:/workspace/pm"
        || permission_mode != "standard"
    {
        return Err(anyhow!("ensured session config mismatch"));
    }

    link_inbound_event_to_session_with_pool(
        &pool,
        &event,
        &sessions[0].employee_id,
        &sessions[0].session_id,
    )
    .await
    .map_err(|error| anyhow!("link inbound event: {error}"))?;

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM im_message_links WHERE thread_id = 'chat_001' AND session_id = ?",
    )
    .bind(&sessions[0].session_id)
    .fetch_one(&pool)
    .await
    .context("count message links")?;
    if count != 1 {
        return Err(anyhow!("expected exactly one im_message_link, got {count}"));
    }

    Ok(RegressionCaseSummary {
        scenario: "employee_config_and_im_session_mapping_work",
        status: "pass",
    })
}

async fn scenario_group_message_text_mention_routes_target() -> Result<RegressionCaseSummary> {
    let (pool, _tmp) = test_helpers::setup_test_db().await;
    seed_model(&pool, "https://example.com").await?;

    let _ = seed_employee(
        &pool,
        "main",
        "主员工",
        "",
        "",
        vec!["feishu".to_string()],
        true,
        "ou_main",
    )
    .await?;
    let dev_id = seed_employee(
        &pool,
        "dev_team",
        "开发团队",
        "",
        "",
        vec!["feishu".to_string()],
        false,
        "ou_dev_team",
    )
    .await?;

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
            sender_id: None,
            chat_type: None,
        },
    )
    .await
    .map_err(|error| anyhow!("resolve target employees: {error}"))?;

    if targets.len() != 1 || targets[0].id != dev_id {
        return Err(anyhow!("text mention did not route to target employee"));
    }

    Ok(RegressionCaseSummary {
        scenario: "group_message_with_text_mention_routes_to_target_employee_when_role_id_missing",
        status: "pass",
    })
}

async fn scenario_team_entry_binding_prefers_entry_employee() -> Result<RegressionCaseSummary> {
    let (pool, _tmp) = test_helpers::setup_test_db().await;
    seed_model(&pool, "https://example.com").await?;

    let main_id = seed_employee(
        &pool,
        "main",
        "主员工",
        "",
        "E:/workspace/main",
        vec!["feishu".to_string()],
        true,
        "ou_main",
    )
    .await?;
    let taizi_id = seed_employee(
        &pool,
        "taizi",
        "太子",
        "",
        "E:/workspace/taizi",
        vec!["feishu".to_string()],
        false,
        "ou_taizi",
    )
    .await?;

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
    .map_err(|error| anyhow!("create bound team: {error}"))?;

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
    .map_err(|error| anyhow!("seed routing binding: {error}"))?;

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
        sender_id: None,
        chat_type: None,
    };

    let sessions = ensure_employee_sessions_for_event_with_pool(&pool, &event)
        .await
        .map_err(|error| anyhow!("ensure employee sessions: {error}"))?;

    if sessions.len() != 1 || sessions[0].employee_id != taizi_id || sessions[0].employee_id == main_id
    {
        return Err(anyhow!("team entry binding did not prefer entry employee"));
    }

    let (session_employee_id,): (String,) =
        sqlx::query_as("SELECT employee_id FROM sessions WHERE id = ?")
            .bind(&sessions[0].session_id)
            .fetch_one(&pool)
            .await
            .context("load ensured session")?;
    if session_employee_id != "taizi" {
        return Err(anyhow!("ensured session employee_id mismatch"));
    }

    Ok(RegressionCaseSummary {
        scenario: "ensure_employee_sessions_for_event_prefers_team_entry_employee_when_binding_team_id_matches",
        status: "pass",
    })
}

async fn scenario_team_entry_ignores_non_entry_sessions() -> Result<RegressionCaseSummary> {
    let (pool, _tmp) = test_helpers::setup_test_db().await;
    seed_model(&pool, "http://mock").await?;

    for employee_id in ["taizi", "zhongshu", "shangshu", "bingbu"] {
        let _ = seed_employee(
            &pool,
            employee_id,
            employee_id,
            &format!("{employee_id} 负责复杂任务协作"),
            &format!("E:/workspace/{employee_id}"),
            vec!["app".to_string(), "feishu".to_string()],
            employee_id == "taizi",
            &format!("ou_{employee_id}"),
        )
        .await?;
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
    .map_err(|error| anyhow!("create employee team: {error}"))?;

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
        .context("seed non-team-entry session")?;

        let handled = maybe_handle_team_entry_session_message_with_pool(
            &pool,
            session_id,
            "请制定并执行交付方案",
        )
        .await
        .map_err(|error| anyhow!("handle team entry session: {error}"))?;

        if handled.is_some() {
            return Err(anyhow!("{session_mode} session should not trigger team orchestration"));
        }
    }

    Ok(RegressionCaseSummary {
        scenario: "maybe_handle_team_entry_message_ignores_non_team_entry_sessions",
        status: "pass",
    })
}

async fn scenario_team_entry_reuses_existing_chat_session() -> Result<RegressionCaseSummary> {
    let (pool, _tmp) = test_helpers::setup_test_db().await;
    seed_model(&pool, "http://mock").await?;

    for employee_id in ["taizi", "zhongshu", "shangshu", "bingbu"] {
        let _ = seed_employee(
            &pool,
            employee_id,
            employee_id,
            &format!("{employee_id} 负责复杂任务协作"),
            &format!("E:/workspace/{employee_id}"),
            vec!["app".to_string(), "feishu".to_string()],
            employee_id == "taizi",
            &format!("ou_{employee_id}"),
        )
        .await?;
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
    .map_err(|error| anyhow!("create employee team: {error}"))?;

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
    .map_err(|error| anyhow!("create distractor employee team: {error}"))?;

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
    .context("seed entry session")?;

    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at)
         VALUES (?, ?, 'user', '请制定并执行交付方案', '2026-03-07T00:00:01Z')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&session_id)
    .execute(&pool)
    .await
    .context("seed user message")?;

    let handled = maybe_handle_team_entry_session_message_with_pool(
        &pool,
        &session_id,
        "请制定并执行交付方案",
    )
    .await
    .map_err(|error| anyhow!("handle team entry session: {error}"))?
    .ok_or_else(|| anyhow!("team entry should be handled"))?;

    if handled.group_id != group_id || handled.session_id != session_id || handled.state != "done" {
        return Err(anyhow!("team entry handler did not reuse existing session correctly"));
    }

    let (run_session_id, entry_session_id, state): (String, String, String) = sqlx::query_as(
        "SELECT session_id, entry_session_id, state
         FROM group_runs
         WHERE id = ?",
    )
    .bind(&handled.run_id)
    .fetch_one(&pool)
    .await
    .context("load created run")?;
    if run_session_id != session_id || entry_session_id != session_id || state != "done" {
        return Err(anyhow!("created group run did not reuse entry session"));
    }

    let messages: Vec<(String, String)> = sqlx::query_as(
        "SELECT role, content FROM messages WHERE session_id = ? ORDER BY created_at ASC, id ASC",
    )
    .bind(&session_id)
    .fetch_all(&pool)
    .await
    .context("load entry session messages")?;

    let user_count = messages.iter().filter(|(role, _)| role == "user").count();
    let has_summary = messages
        .iter()
        .any(|(role, content)| role == "assistant" && content.contains("团队协作已完成"));
    if user_count != 1 || !has_summary {
        return Err(anyhow!("team entry session messages were not reused correctly"));
    }

    Ok(RegressionCaseSummary {
        scenario: "maybe_handle_team_entry_message_reuses_existing_chat_session_for_group_run",
        status: "pass",
    })
}
