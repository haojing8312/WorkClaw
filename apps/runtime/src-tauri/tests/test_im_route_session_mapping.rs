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
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-1".to_string(),
            event_id: Some("evt-1".to_string()),
            message_id: Some("msg-1".to_string()),
            text: Some("hello".to_string()),
            role_id: None,
            tenant_id: Some("tenant-a".to_string()),
        },
    )
    .await
    .expect("first ensure");
    assert_eq!(first.len(), 1);
    assert!(first[0].created);

    let second = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-2".to_string(),
            event_id: Some("evt-2".to_string()),
            message_id: Some("msg-2".to_string()),
            text: Some("hello 2".to_string()),
            role_id: None,
            tenant_id: Some("tenant-a".to_string()),
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
