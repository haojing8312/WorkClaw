mod helpers;

use runtime_lib::commands::employee_agents::{
    ensure_employee_sessions_for_event_with_pool, upsert_agent_employee_with_pool,
    UpsertAgentEmployeeInput,
};
use runtime_lib::im::types::{ImEvent, ImEventType};

#[tokio::test]
async fn same_route_session_key_reuses_existing_session() {
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
            feishu_open_id: "".to_string(),
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
    .expect("upsert employee");

    let first = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-1".to_string(),
            event_id: Some("evt-1".to_string()),
            message_id: Some("msg-1".to_string()),
            text: Some("hello".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: None,
            chat_type: None,
        },
    )
    .await
    .expect("first ensure");
    assert_eq!(first.len(), 1);
    assert!(first[0].created);

    let second = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-2".to_string(),
            event_id: Some("evt-2".to_string()),
            message_id: Some("msg-2".to_string()),
            text: Some("hello 2".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: None,
            chat_type: None,
        },
    )
    .await
    .expect("second ensure");
    assert_eq!(second.len(), 1);
    assert!(!second[0].created);
    assert_eq!(second[0].session_id, first[0].session_id);

    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM im_thread_sessions WHERE session_id = ?")
            .bind(&first[0].session_id)
            .fetch_one(&pool)
            .await
            .expect("count mappings");
    assert_eq!(count, 2);
}

#[tokio::test]
async fn same_thread_reuses_session_when_mention_switches_employee() {
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
            openclaw_agent_id: "dev_team".to_string(),
            routing_priority: 90,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert dev");

    let first = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-1".to_string(),
            event_id: Some("evt-1".to_string()),
            message_id: Some("msg-1".to_string()),
            text: Some("先给一个初步方案".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: None,
            chat_type: None,
        },
    )
    .await
    .expect("first ensure");
    assert_eq!(first.len(), 1);
    assert!(first[0].created);

    let second = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-1".to_string(),
            event_id: Some("evt-2".to_string()),
            message_id: Some("msg-2".to_string()),
            text: Some("@开发团队 细化技术方案".to_string()),
            role_id: Some("ou_dev_team".to_string()),
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: None,
            chat_type: None,
        },
    )
    .await
    .expect("second ensure");

    assert_eq!(second.len(), 1);
    assert!(!second[0].created);
    assert_eq!(second[0].session_id, first[0].session_id);

    let (mapping_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM im_thread_sessions WHERE thread_id = 'chat-1'")
            .fetch_one(&pool)
            .await
            .expect("count thread mappings");
    assert_eq!(mapping_count, 2);

    let (distinct_session_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT session_id) FROM im_thread_sessions WHERE thread_id = 'chat-1'",
    )
    .fetch_one(&pool)
    .await
    .expect("count distinct session ids");
    assert_eq!(distinct_session_count, 1);
}

#[tokio::test]
async fn recreates_session_when_thread_mapping_points_to_deleted_session() {
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
            feishu_open_id: "".to_string(),
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
    .expect("upsert employee");

    let first = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-1".to_string(),
            event_id: Some("evt-1".to_string()),
            message_id: Some("msg-1".to_string()),
            text: Some("hello".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: None,
            chat_type: None,
        },
    )
    .await
    .expect("first ensure");
    assert_eq!(first.len(), 1);
    let stale_session_id = first[0].session_id.clone();
    let employee_row_id = first[0].employee_id.clone();

    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(&stale_session_id)
        .execute(&pool)
        .await
        .expect("delete stale session");

    let second = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-1".to_string(),
            event_id: Some("evt-2".to_string()),
            message_id: Some("msg-2".to_string()),
            text: Some("hello again".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: None,
            chat_type: None,
        },
    )
    .await
    .expect("second ensure");

    assert_eq!(second.len(), 1);
    assert!(second[0].created);
    assert_ne!(second[0].session_id, stale_session_id);

    let (session_exists,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE id = ?")
        .bind(&second[0].session_id)
        .fetch_one(&pool)
        .await
        .expect("query recreated session");
    assert_eq!(session_exists, 1);

    let (mapped_session_id,): (String,) = sqlx::query_as(
        "SELECT session_id FROM im_thread_sessions WHERE thread_id = ? AND employee_id = ?",
    )
    .bind("chat-1")
    .bind(&employee_row_id)
    .fetch_one(&pool)
    .await
    .expect("query thread mapping");
    assert_eq!(mapped_session_id, second[0].session_id);
}
